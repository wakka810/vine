use std::ops::Range;

use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, ImageMemoryLayout, MemoryOwnerId,
};
use crate::pe32::{LoaderError, Pe32LoadPlan, Rva};
use crate::snapshot::{ModuleIdentity, ModuleSnapshot};

const GUEST_ADDRESS_SPACE_SIZE: u64 = 1_u64 << 32;

#[derive(Debug)]
pub struct ImageMaterialization {
    pub source: ModuleIdentity,
    pub layout: ImageMemoryLayout,
    pub entry_point_address: GuestAddress,
}

pub fn materialize_pe32_image(
    memory: &mut GuestMemory,
    snapshot: &ModuleSnapshot,
    loader: &Pe32LoadPlan,
    load_base: GuestAddress,
) -> Result<ImageMaterialization, LoaderError> {
    validate_materialization_input(snapshot, loader, load_base)?;
    let mut transaction = memory.begin_transaction();
    let materialization =
        materialize_pe32_image_transaction(&mut transaction, snapshot, loader, load_base)?;
    transaction.commit_transaction();
    Ok(materialization)
}

pub(crate) fn materialize_pe32_image_transaction(
    memory: &mut GuestMemory,
    snapshot: &ModuleSnapshot,
    loader: &Pe32LoadPlan,
    load_base: GuestAddress,
) -> Result<ImageMaterialization, LoaderError> {
    validate_materialization_input(snapshot, loader, load_base)?;
    let plan = loader;
    let relocation_delta = load_base.0.wrapping_sub(plan.image.preferred_image_base);
    let layout = memory
        .prepare_image_layout_at_transaction(plan, load_base)
        .map_err(memory_error)?;
    let owner = layout.owner;
    let max_access_bytes = memory.limits().max_access_bytes;

    let header = &plan.header_mapping;
    let header_source = source_span(snapshot.bytes(), header.source_offset, header.copy_size)?;
    if !header_source.is_empty() {
        copy_source_range(
            memory,
            owner,
            load_base,
            header.destination_rva,
            header_source,
            max_access_bytes,
        )?;
    }
    for section in &plan.sections {
        let source = source_span(snapshot.bytes(), section.source_offset, section.copy_size)?;
        if source.is_empty() {
            continue;
        }
        copy_source_range(
            memory,
            owner,
            load_base,
            section.destination_rva,
            source,
            max_access_bytes,
        )?;
    }

    let header_zero_rva = header
        .destination_rva
        .0
        .checked_add(header.copy_size)
        .ok_or_else(|| {
            LoaderError::new("header destination RVA plus copy size overflows 32 bits")
        })?;
    let header_zero_size = header.mapped_size - header.copy_size;
    zero_range(
        memory,
        owner,
        load_base,
        Rva(header_zero_rva),
        header_zero_size,
        max_access_bytes,
    )?;
    for section in &plan.sections {
        let tail = section
            .destination_rva
            .0
            .checked_add(section.copy_size)
            .ok_or_else(materialization_accounting_overflow)?;
        zero_range(
            memory,
            owner,
            load_base,
            Rva(tail),
            section.mapped_size - section.copy_size,
            max_access_bytes,
        )?;
    }

    apply_highlow_relocations(memory, owner, plan, load_base, relocation_delta)?;
    verify_original_iat(memory, owner, plan, load_base)?;
    let entry_point_address = add_rva(load_base, plan.image.entry_point_rva)?;
    Ok(ImageMaterialization {
        source: loader.source.clone(),
        layout,
        entry_point_address,
    })
}

fn validate_materialization_input(
    snapshot: &ModuleSnapshot,
    loader: &Pe32LoadPlan,
    load_base: GuestAddress,
) -> Result<(), LoaderError> {
    if snapshot.identity() != loader.source {
        return Err(LoaderError::new(
            "owned source identity does not match the source recorded by the load plan",
        ));
    }
    if !load_base.0.is_multiple_of(0x1_0000) {
        return Err(LoaderError::new(
            "image load base must be aligned to the 64 KiB Windows allocation granularity",
        ));
    }

    let plan = loader;
    let preferred_end = u64::from(plan.image.preferred_image_base)
        .checked_add(u64::from(plan.image.size_of_image))
        .ok_or_else(materialization_accounting_overflow)?;
    if preferred_end != plan.image.image_end {
        return Err(LoaderError::new(
            "preferred image base plus SizeOfImage does not match the load plan image end",
        ));
    }
    let selected_end = u64::from(load_base.0)
        .checked_add(u64::from(plan.image.size_of_image))
        .ok_or_else(materialization_accounting_overflow)?;
    if selected_end > GUEST_ADDRESS_SPACE_SIZE {
        return Err(LoaderError::new(
            "selected load base plus SizeOfImage exceeds the 4 GiB guest address space",
        ));
    }
    let expected_entry = plan
        .image
        .preferred_image_base
        .checked_add(plan.image.entry_point_rva.0)
        .ok_or_else(materialization_accounting_overflow)?;
    if expected_entry != plan.image.entry_point_address {
        return Err(LoaderError::new(
            "preferred image base plus entry RVA does not match the load plan entry address",
        ));
    }

    let header = &plan.header_mapping;
    if header.destination_rva != Rva(0)
        || header.copy_size > header.mapped_size
        || !header.mapped_size.is_multiple_of(GUEST_PAGE_SIZE as u32)
    {
        return Err(LoaderError::new(
            "header mapping must start at RVA zero, fit its mapped range, and use whole pages",
        ));
    }
    source_span(snapshot.bytes(), header.source_offset, header.copy_size)?;

    for section in &plan.sections {
        if section.copy_size > section.source_size
            || section.copy_size > section.mapped_size
            || !section.mapped_size.is_multiple_of(GUEST_PAGE_SIZE as u32)
        {
            return Err(LoaderError::new(format!(
                "section {} copy and mapped sizes do not form a valid page mapping",
                section.name
            )));
        }
        section
            .destination_rva
            .0
            .checked_add(section.copy_size)
            .ok_or_else(materialization_accounting_overflow)?;
        source_span(snapshot.bytes(), section.source_offset, section.copy_size)?;
    }

    let relocations = &plan.relocations;
    let relocation_delta = load_base.0.wrapping_sub(plan.image.preferred_image_base);
    if relocation_delta != 0
        && (plan.image.coff_characteristics
            & goblin::pe::characteristic::IMAGE_FILE_RELOCS_STRIPPED
            != 0
            || relocations.is_empty())
    {
        return Err(LoaderError::new(
            "selected load base differs from ImageBase but usable HIGHLOW relocations are unavailable",
        ));
    }
    for target in relocations {
        if !mapped_range_contains(plan, target.0, 4) {
            return Err(LoaderError::new(format!(
                "HIGHLOW target RVA {:#010x} is outside the planned mapped ranges",
                target.0
            )));
        }
    }
    for module in &plan.imports {
        for symbol in &module.symbols {
            if !mapped_range_contains(plan, symbol.iat_rva.0, 4) {
                return Err(LoaderError::new(format!(
                    "IAT slot RVA {:#010x} is outside the planned mapped ranges",
                    symbol.iat_rva.0
                )));
            }
            let source = source_span_for_rva(snapshot.bytes(), plan, symbol.iat_rva.0, 4)?;
            let actual =
                u32::from_le_bytes(source.try_into().map_err(|_| {
                    LoaderError::new("preflighted IAT source span is not four bytes")
                })?);
            if actual != symbol.original_iat_value {
                return Err(LoaderError::new(format!(
                    "IAT source value at RVA {:#010x} does not match the load plan",
                    symbol.iat_rva.0
                )));
            }
        }
    }
    Ok(())
}

fn copy_source_range(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    load_base: GuestAddress,
    destination_rva: Rva,
    source: &[u8],
    max_access_bytes: usize,
) -> Result<(), LoaderError> {
    for (chunk_index, chunk) in source.chunks(max_access_bytes).enumerate() {
        let offset = chunk_index
            .checked_mul(max_access_bytes)
            .ok_or_else(materialization_accounting_overflow)?;
        let address = add_rva_offset(load_base, destination_rva, offset)?;
        memory
            .write_initialization(owner, address, chunk)
            .map_err(memory_error)?;
    }
    Ok(())
}

fn zero_range(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    load_base: GuestAddress,
    destination_rva: Rva,
    byte_length: u32,
    max_access_bytes: usize,
) -> Result<(), LoaderError> {
    for_each_chunk(byte_length, max_access_bytes, |offset, chunk_length| {
        let address = add_rva_offset(load_base, destination_rva, offset)?;
        memory
            .zero_initialization(owner, address, chunk_length)
            .map_err(memory_error)?;
        Ok(())
    })
}

fn apply_highlow_relocations(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    plan: &Pe32LoadPlan,
    load_base: GuestAddress,
    relocation_delta: u32,
) -> Result<(), LoaderError> {
    if relocation_delta == 0 {
        return Ok(());
    }
    for &target in &plan.relocations {
        let address = add_rva(load_base, target)?;
        let original = memory
            .read_initialization(owner, address, 4)
            .map_err(memory_error)?;
        let original =
            u32::from_le_bytes(original.as_slice().try_into().map_err(|_| {
                LoaderError::new("preflighted HIGHLOW target read is not four bytes")
            })?);
        let relocated = original.wrapping_add(relocation_delta);
        memory
            .write_initialization(owner, address, &relocated.to_le_bytes())
            .map_err(memory_error)?;
    }
    Ok(())
}

fn verify_original_iat(
    memory: &GuestMemory,
    owner: MemoryOwnerId,
    plan: &Pe32LoadPlan,
    load_base: GuestAddress,
) -> Result<(), LoaderError> {
    for module in &plan.imports {
        for symbol in &module.symbols {
            let address = add_rva(load_base, symbol.iat_rva)?;
            let bytes = memory
                .read_initialization(owner, address, 4)
                .map_err(memory_error)?;
            let actual = u32::from_le_bytes(bytes.as_slice().try_into().map_err(|_| {
                LoaderError::new("preflighted materialized IAT read is not four bytes")
            })?);
            if actual != symbol.original_iat_value {
                return Err(LoaderError::new(format!(
                    "materialized IAT value at RVA {:#010x} changed before binding",
                    symbol.iat_rva.0
                )));
            }
        }
    }
    Ok(())
}

fn for_each_chunk(
    byte_length: u32,
    max_access_bytes: usize,
    mut action: impl FnMut(usize, usize) -> Result<(), LoaderError>,
) -> Result<(), LoaderError> {
    let total = usize::try_from(byte_length)
        .map_err(|_| LoaderError::new("mapping length does not fit the host index type"))?;
    let mut offset = 0usize;
    while offset < total {
        let chunk_length = (total - offset).min(max_access_bytes);
        action(offset, chunk_length)?;
        offset = offset
            .checked_add(chunk_length)
            .ok_or_else(materialization_accounting_overflow)?;
    }
    Ok(())
}

fn source_span(bytes: &[u8], source_offset: u32, byte_length: u32) -> Result<&[u8], LoaderError> {
    let start = usize::try_from(source_offset)
        .map_err(|_| LoaderError::new("source offset does not fit the host index type"))?;
    let length = usize::try_from(byte_length)
        .map_err(|_| LoaderError::new("source length does not fit the host index type"))?;
    let end = start
        .checked_add(length)
        .ok_or_else(materialization_accounting_overflow)?;
    bytes.get(start..end).ok_or_else(|| {
        LoaderError::new("planned source span extends beyond the immutable module snapshot")
    })
}

fn source_span_for_rva<'a>(
    bytes: &'a [u8],
    plan: &Pe32LoadPlan,
    rva: u32,
    byte_length: u32,
) -> Result<&'a [u8], LoaderError> {
    let requested = checked_range(rva, byte_length)
        .ok_or_else(|| LoaderError::new("requested RVA span overflows 32 bits"))?;
    let header = &plan.header_mapping;
    let header_range = checked_range(header.destination_rva.0, header.copy_size)
        .ok_or_else(materialization_accounting_overflow)?;
    if contains_range(&header_range, &requested) {
        let delta = requested.start - header_range.start;
        let source_offset = u64::from(header.source_offset) + delta;
        return source_span_u64(bytes, source_offset, u64::from(byte_length));
    }
    for section in &plan.sections {
        let copy_range = checked_range(section.destination_rva.0, section.copy_size)
            .ok_or_else(materialization_accounting_overflow)?;
        if contains_range(&copy_range, &requested) {
            let delta = requested.start - copy_range.start;
            let source_offset = u64::from(section.source_offset) + delta;
            return source_span_u64(bytes, source_offset, u64::from(byte_length));
        }
    }
    Err(LoaderError::new(format!(
        "RVA span {rva:#010x}..+{byte_length:#x} is not file-backed"
    )))
}

fn source_span_u64(
    bytes: &[u8],
    source_offset: u64,
    byte_length: u64,
) -> Result<&[u8], LoaderError> {
    let start = usize::try_from(source_offset)
        .map_err(|_| LoaderError::new("resolved source offset does not fit the host index type"))?;
    let length = usize::try_from(byte_length)
        .map_err(|_| LoaderError::new("resolved source length does not fit the host index type"))?;
    let end = start
        .checked_add(length)
        .ok_or_else(materialization_accounting_overflow)?;
    bytes.get(start..end).ok_or_else(|| {
        LoaderError::new("resolved source span extends beyond the immutable module snapshot")
    })
}

fn mapped_range_contains(plan: &Pe32LoadPlan, rva: u32, byte_length: u32) -> bool {
    let Some(requested) = checked_range(rva, byte_length) else {
        return false;
    };
    let header = checked_range(
        plan.header_mapping.destination_rva.0,
        plan.header_mapping.mapped_size,
    );
    header.is_some_and(|range| contains_range(&range, &requested))
        || plan.sections.iter().any(|section| {
            checked_range(section.destination_rva.0, section.mapped_size)
                .is_some_and(|range| contains_range(&range, &requested))
        })
}

fn checked_range(start: u32, byte_length: u32) -> Option<Range<u64>> {
    let start = u64::from(start);
    let end = start.checked_add(u64::from(byte_length))?;
    (end <= GUEST_ADDRESS_SPACE_SIZE).then_some(start..end)
}

fn contains_range(container: &Range<u64>, value: &Range<u64>) -> bool {
    value.start >= container.start && value.end <= container.end
}

fn add_rva(load_base: GuestAddress, rva: Rva) -> Result<GuestAddress, LoaderError> {
    add_rva_offset(load_base, rva, 0)
}

fn add_rva_offset(
    load_base: GuestAddress,
    rva: Rva,
    offset: usize,
) -> Result<GuestAddress, LoaderError> {
    let value = u64::from(load_base.0)
        .checked_add(u64::from(rva.0))
        .and_then(|value| value.checked_add(offset as u64))
        .ok_or_else(materialization_accounting_overflow)?;
    let value = u32::try_from(value).map_err(|_| {
        LoaderError::new(
            "selected load base plus RVA extends beyond the 32-bit guest address space",
        )
    })?;
    Ok(GuestAddress(value))
}

fn memory_error(error: GuestMemoryError) -> LoaderError {
    LoaderError::new(error.message)
}

fn materialization_accounting_overflow() -> LoaderError {
    LoaderError::new("image materialization arithmetic exceeded the host accounting range")
}
