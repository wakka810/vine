use std::ops::Range;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryErrorClass,
    ImageMemoryLayout, MemoryOwnerId,
};
use crate::pe32::{
    ImportTarget, LoaderError, LoaderErrorClass, LoaderIngressReport, Pe32LoadPlan, Rva,
    UnsupportedPoint,
};
use crate::snapshot::{ModuleIdentity, ModuleSnapshot};

const GUEST_ADDRESS_SPACE_SIZE: u64 = 1_u64 << 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnboundImport {
    pub module: String,
    pub iat_rva: Rva,
    pub iat_address: GuestAddress,
    pub original_iat_value: u32,
    #[serde(flatten)]
    pub target: ImportTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMaterialization {
    pub source: ModuleIdentity,
    pub layout: ImageMemoryLayout,
    pub preferred_image_base: GuestAddress,
    pub load_base: GuestAddress,
    pub relocation_delta: u32,
    pub entry_point_address: GuestAddress,
    pub copied_ranges: usize,
    pub copied_bytes: u64,
    pub zero_filled_ranges: usize,
    pub zero_filled_bytes: u64,
    pub zero_fill_verified_bytes: u64,
    pub relocations_applied: usize,
    pub iat_slots_verified: usize,
    pub mapped_content_ranges: usize,
    pub mapped_content_bytes: u64,
    pub mapped_content_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_unbound_import: Option<UnboundImport>,
    pub stop: UnsupportedPoint,
}

pub fn materialize_pe32_image(
    memory: &mut GuestMemory,
    snapshot: &ModuleSnapshot,
    loader: &LoaderIngressReport,
    load_base: GuestAddress,
) -> Result<ImageMaterialization, LoaderError> {
    validate_materialization_input(snapshot, loader, load_base)?;

    let plan = &loader.load_plan;
    let relocation_delta = load_base.0.wrapping_sub(plan.image.preferred_image_base);
    let layout = memory
        .prepare_image_layout_at(&loader.source, plan, load_base)
        .map_err(memory_error)?;
    let owner = layout.owner;
    let max_access_bytes = memory.limits().max_access_bytes;

    let mut copied_ranges = 0usize;
    let mut copied_bytes = 0u64;
    let header = &plan.header_mapping;
    let header_source = source_span(
        snapshot.bytes(),
        header.source_offset,
        header.copy_size,
        "header-source-range-invalid",
    )?;
    if !header_source.is_empty() {
        copy_source_range(
            memory,
            owner,
            load_base,
            header.destination_rva,
            header_source,
            max_access_bytes,
        )?;
        copied_ranges += 1;
        copied_bytes = copied_bytes
            .checked_add(header_source.len() as u64)
            .ok_or_else(materialization_accounting_overflow)?;
    }
    for section in &plan.sections {
        let source = source_span(
            snapshot.bytes(),
            section.source_offset,
            section.copy_size,
            "section-source-range-invalid",
        )?;
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
        copied_ranges += 1;
        copied_bytes = copied_bytes
            .checked_add(source.len() as u64)
            .ok_or_else(materialization_accounting_overflow)?;
    }

    let mut zero_filled_ranges = 0usize;
    let mut zero_filled_bytes = 0u64;
    let header_zero_rva = header
        .destination_rva
        .0
        .checked_add(header.copy_size)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "image-materialization",
                "header-zero-fill-rva-overflow",
                "header destination RVA plus copy size overflows 32 bits",
            )
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
    if header_zero_size != 0 {
        zero_filled_ranges += 1;
        zero_filled_bytes = u64::from(header_zero_size);
    }
    for section in &plan.sections {
        zero_range(
            memory,
            owner,
            load_base,
            section.zero_fill_rva,
            section.zero_fill_size,
            max_access_bytes,
        )?;
        if section.zero_fill_size != 0 {
            zero_filled_ranges += 1;
            zero_filled_bytes = zero_filled_bytes
                .checked_add(u64::from(section.zero_fill_size))
                .ok_or_else(materialization_accounting_overflow)?;
        }
    }

    let mut zero_fill_verified_bytes = 0u64;
    verify_zero_range(
        memory,
        owner,
        load_base,
        Rva(header_zero_rva),
        header_zero_size,
        max_access_bytes,
    )?;
    zero_fill_verified_bytes = zero_fill_verified_bytes
        .checked_add(u64::from(header_zero_size))
        .ok_or_else(materialization_accounting_overflow)?;
    for section in &plan.sections {
        verify_zero_range(
            memory,
            owner,
            load_base,
            section.zero_fill_rva,
            section.zero_fill_size,
            max_access_bytes,
        )?;
        zero_fill_verified_bytes = zero_fill_verified_bytes
            .checked_add(u64::from(section.zero_fill_size))
            .ok_or_else(materialization_accounting_overflow)?;
    }

    let relocations_applied =
        apply_highlow_relocations(memory, owner, plan, load_base, relocation_delta)?;
    let (iat_slots_verified, first_unbound_import) =
        verify_original_iat(memory, owner, plan, load_base)?;
    let (mapped_content_ranges, mapped_content_bytes, mapped_content_sha256) =
        hash_mapped_content(memory, owner, plan, load_base, max_access_bytes)?;
    let entry_point_address = add_rva(
        load_base,
        plan.image.entry_point_rva,
        "entry-point-address-overflow",
    )?;
    let stop = if first_unbound_import.is_some() {
        UnsupportedPoint {
            stage: "import-binding",
            code: "iat-binding-unimplemented",
            message: "the PE image bytes and HIGHLOW relocations are materialized; IAT thunk resolution, owner sealing, process bootstrap, and guest execution were not attempted",
        }
    } else {
        UnsupportedPoint {
            stage: "process-bootstrap",
            code: "peb-teb-process-parameters-unimplemented",
            message: "the import-free PE image is materialized; owner sealing, PEB/TEB/process parameters, initial stack, and guest execution were not attempted",
        }
    };

    Ok(ImageMaterialization {
        source: loader.source.clone(),
        layout,
        preferred_image_base: GuestAddress(plan.image.preferred_image_base),
        load_base,
        relocation_delta,
        entry_point_address,
        copied_ranges,
        copied_bytes,
        zero_filled_ranges,
        zero_filled_bytes,
        zero_fill_verified_bytes,
        relocations_applied,
        iat_slots_verified,
        mapped_content_ranges,
        mapped_content_bytes,
        mapped_content_sha256,
        first_unbound_import,
        stop,
    })
}

fn validate_materialization_input(
    snapshot: &ModuleSnapshot,
    loader: &LoaderIngressReport,
    load_base: GuestAddress,
) -> Result<(), LoaderError> {
    snapshot.verify_integrity()?;
    if snapshot.identity() != loader.source {
        return Err(LoaderError::invalid_input(
            "image-materialization",
            "source-plan-identity-mismatch",
            "owned source identity does not match the source recorded by the load plan",
        ));
    }
    if loader.schema != "vine.pe32-loader-ingress" || loader.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "image-materialization",
            "loader-schema-unsupported",
            "materialization requires vine.pe32-loader-ingress schema version 1",
        ));
    }
    if !load_base.0.is_multiple_of(0x1_0000) {
        return Err(LoaderError::invalid_input(
            "image-materialization",
            "image-load-base-not-allocation-aligned",
            "image load base must be aligned to the 64 KiB Windows allocation granularity",
        ));
    }

    let plan = &loader.load_plan;
    let preferred_end = u64::from(plan.image.preferred_image_base)
        .checked_add(u64::from(plan.image.size_of_image))
        .ok_or_else(materialization_accounting_overflow)?;
    if preferred_end != plan.image.image_end {
        return Err(LoaderError::invalid_input(
            "image-materialization",
            "preferred-image-end-mismatch",
            "preferred image base plus SizeOfImage does not match the load plan image end",
        ));
    }
    let selected_end = u64::from(load_base.0)
        .checked_add(u64::from(plan.image.size_of_image))
        .ok_or_else(materialization_accounting_overflow)?;
    if selected_end > GUEST_ADDRESS_SPACE_SIZE {
        return Err(LoaderError::invalid_input(
            "image-materialization",
            "selected-image-range-out-of-bounds",
            "selected load base plus SizeOfImage exceeds the 4 GiB guest address space",
        ));
    }
    let expected_entry = plan
        .image
        .preferred_image_base
        .checked_add(plan.image.entry_point_rva.0)
        .ok_or_else(materialization_accounting_overflow)?;
    if expected_entry != plan.image.entry_point_address {
        return Err(LoaderError::invalid_input(
            "image-materialization",
            "preferred-entry-point-mismatch",
            "preferred image base plus entry RVA does not match the load plan entry address",
        ));
    }

    let header = &plan.header_mapping;
    if header.destination_rva != Rva(0)
        || header.copy_size > header.mapped_size
        || !header.mapped_size.is_multiple_of(GUEST_PAGE_SIZE as u32)
    {
        return Err(LoaderError::invalid_input(
            "image-materialization",
            "header-mapping-contract-invalid",
            "header mapping must start at RVA zero, fit its mapped range, and use whole pages",
        ));
    }
    source_span(
        snapshot.bytes(),
        header.source_offset,
        header.copy_size,
        "header-source-range-invalid",
    )?;

    for section in &plan.sections {
        if section.copy_size > section.source_size
            || section.copy_size > section.mapped_size
            || !section.mapped_size.is_multiple_of(GUEST_PAGE_SIZE as u32)
        {
            return Err(LoaderError::invalid_input(
                "image-materialization",
                "section-mapping-contract-invalid",
                format!(
                    "section {} copy and mapped sizes do not form a valid page mapping",
                    section.name
                ),
            ));
        }
        let expected_zero_rva = section
            .destination_rva
            .0
            .checked_add(section.copy_size)
            .ok_or_else(materialization_accounting_overflow)?;
        let expected_zero_size = section.mapped_size - section.copy_size;
        if section.zero_fill_rva != Rva(expected_zero_rva)
            || section.zero_fill_size != expected_zero_size
        {
            return Err(LoaderError::invalid_input(
                "image-materialization",
                "section-zero-fill-contract-invalid",
                format!(
                    "section {} zero-fill does not cover the mapped tail after source copy",
                    section.name
                ),
            ));
        }
        source_span(
            snapshot.bytes(),
            section.source_offset,
            section.copy_size,
            "section-source-range-invalid",
        )?;
    }

    let relocations = &plan.relocations;
    if relocations.entry_count != relocations.absolute_count + relocations.highlow_count
        || relocations.operations.len() != relocations.highlow_count
    {
        return Err(LoaderError::invalid_input(
            "image-materialization",
            "relocation-plan-count-mismatch",
            "relocation entry counts do not match the planned HIGHLOW operations",
        ));
    }
    let relocation_delta = load_base.0.wrapping_sub(plan.image.preferred_image_base);
    if relocation_delta != 0
        && (plan.features.relocations_stripped || relocations.operations.is_empty())
    {
        return Err(LoaderError::unsupported(
            "image-materialization",
            "image-relocation-required-but-unavailable",
            "selected load base differs from ImageBase but usable HIGHLOW relocations are unavailable",
        ));
    }
    for operation in &relocations.operations {
        if operation.width != 4 || operation.kind != "highlow" {
            return Err(LoaderError::invalid_input(
                "image-materialization",
                "relocation-operation-contract-invalid",
                "version 1 materialization accepts only 4-byte HIGHLOW operations",
            ));
        }
        if !mapped_range_contains(plan, operation.target_rva.0, 4) {
            return Err(LoaderError::invalid_input(
                "image-materialization",
                "relocation-target-not-mapped",
                format!(
                    "HIGHLOW target RVA {:#010x} is outside the planned mapped ranges",
                    operation.target_rva.0
                ),
            ));
        }
    }
    for module in &plan.imports {
        for symbol in &module.symbols {
            if !mapped_range_contains(plan, symbol.iat_rva.0, 4) {
                return Err(LoaderError::invalid_input(
                    "image-materialization",
                    "iat-slot-not-mapped",
                    format!(
                        "IAT slot RVA {:#010x} is outside the planned mapped ranges",
                        symbol.iat_rva.0
                    ),
                ));
            }
            let source = source_span_for_rva(
                snapshot.bytes(),
                plan,
                symbol.iat_rva.0,
                4,
                "iat-slot-not-file-backed",
            )?;
            let actual = u32::from_le_bytes(source.try_into().map_err(|_| {
                LoaderError::runtime_defect(
                    "image-materialization",
                    "iat-source-width-invalid",
                    "preflighted IAT source span is not four bytes",
                )
            })?);
            if actual != symbol.original_iat_value {
                return Err(LoaderError::invalid_input(
                    "image-materialization",
                    "iat-source-plan-mismatch",
                    format!(
                        "IAT source value at RVA {:#010x} does not match the load plan",
                        symbol.iat_rva.0
                    ),
                ));
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
        let address = add_rva_offset(
            load_base,
            destination_rva,
            offset,
            "copy-destination-address-overflow",
        )?;
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
        let address = add_rva_offset(
            load_base,
            destination_rva,
            offset,
            "zero-fill-destination-address-overflow",
        )?;
        memory
            .zero_initialization(owner, address, chunk_length)
            .map_err(memory_error)?;
        Ok(())
    })
}

fn verify_zero_range(
    memory: &GuestMemory,
    owner: MemoryOwnerId,
    load_base: GuestAddress,
    destination_rva: Rva,
    byte_length: u32,
    max_access_bytes: usize,
) -> Result<(), LoaderError> {
    for_each_chunk(byte_length, max_access_bytes, |offset, chunk_length| {
        let address = add_rva_offset(
            load_base,
            destination_rva,
            offset,
            "zero-fill-verification-address-overflow",
        )?;
        let bytes = memory
            .read_initialization(owner, address, chunk_length)
            .map_err(memory_error)?;
        if bytes.iter().any(|byte| *byte != 0) {
            return Err(LoaderError::runtime_defect(
                "image-materialization",
                "zero-fill-verification-failed",
                format!(
                    "materialized zero-fill contains nonzero data at {:#010x}",
                    address.0
                ),
            ));
        }
        Ok(())
    })
}

fn apply_highlow_relocations(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    plan: &Pe32LoadPlan,
    load_base: GuestAddress,
    relocation_delta: u32,
) -> Result<usize, LoaderError> {
    if relocation_delta == 0 {
        return Ok(0);
    }
    for operation in &plan.relocations.operations {
        let address = add_rva(
            load_base,
            operation.target_rva,
            "relocation-target-address-overflow",
        )?;
        let original = memory
            .read_initialization(owner, address, 4)
            .map_err(memory_error)?;
        let original = u32::from_le_bytes(original.as_slice().try_into().map_err(|_| {
            LoaderError::runtime_defect(
                "image-materialization",
                "relocation-read-width-invalid",
                "preflighted HIGHLOW target read is not four bytes",
            )
        })?);
        let relocated = original.wrapping_add(relocation_delta);
        memory
            .write_initialization(owner, address, &relocated.to_le_bytes())
            .map_err(memory_error)?;
    }
    Ok(plan.relocations.operations.len())
}

fn verify_original_iat(
    memory: &GuestMemory,
    owner: MemoryOwnerId,
    plan: &Pe32LoadPlan,
    load_base: GuestAddress,
) -> Result<(usize, Option<UnboundImport>), LoaderError> {
    let mut verified = 0usize;
    let mut first = None;
    for module in &plan.imports {
        for symbol in &module.symbols {
            let address = add_rva(load_base, symbol.iat_rva, "iat-slot-address-overflow")?;
            let bytes = memory
                .read_initialization(owner, address, 4)
                .map_err(memory_error)?;
            let actual = u32::from_le_bytes(bytes.as_slice().try_into().map_err(|_| {
                LoaderError::runtime_defect(
                    "image-materialization",
                    "iat-read-width-invalid",
                    "preflighted materialized IAT read is not four bytes",
                )
            })?);
            if actual != symbol.original_iat_value {
                return Err(LoaderError::runtime_defect(
                    "image-materialization",
                    "materialized-iat-value-mismatch",
                    format!(
                        "materialized IAT value at RVA {:#010x} changed before binding",
                        symbol.iat_rva.0
                    ),
                ));
            }
            if first.is_none() {
                first = Some(UnboundImport {
                    module: module.name.clone(),
                    iat_rva: symbol.iat_rva,
                    iat_address: address,
                    original_iat_value: symbol.original_iat_value,
                    target: symbol.target.clone(),
                });
            }
            verified = verified
                .checked_add(1)
                .ok_or_else(materialization_accounting_overflow)?;
        }
    }
    Ok((verified, first))
}

fn hash_mapped_content(
    memory: &GuestMemory,
    owner: MemoryOwnerId,
    plan: &Pe32LoadPlan,
    load_base: GuestAddress,
    max_access_bytes: usize,
) -> Result<(usize, u64, String), LoaderError> {
    let mut hasher = Sha256::new();
    let mut range_count = 0usize;
    let mut byte_count = 0u64;
    hash_range(
        memory,
        owner,
        load_base,
        plan.header_mapping.destination_rva,
        plan.header_mapping.mapped_size,
        max_access_bytes,
        &mut hasher,
    )?;
    if plan.header_mapping.mapped_size != 0 {
        range_count += 1;
        byte_count = u64::from(plan.header_mapping.mapped_size);
    }
    for section in &plan.sections {
        hash_range(
            memory,
            owner,
            load_base,
            section.destination_rva,
            section.mapped_size,
            max_access_bytes,
            &mut hasher,
        )?;
        if section.mapped_size != 0 {
            range_count += 1;
            byte_count = byte_count
                .checked_add(u64::from(section.mapped_size))
                .ok_or_else(materialization_accounting_overflow)?;
        }
    }
    Ok((range_count, byte_count, hex_digest(&hasher.finalize())))
}

#[allow(clippy::too_many_arguments)]
fn hash_range(
    memory: &GuestMemory,
    owner: MemoryOwnerId,
    load_base: GuestAddress,
    destination_rva: Rva,
    byte_length: u32,
    max_access_bytes: usize,
    hasher: &mut Sha256,
) -> Result<(), LoaderError> {
    for_each_chunk(byte_length, max_access_bytes, |offset, chunk_length| {
        let address = add_rva_offset(
            load_base,
            destination_rva,
            offset,
            "mapped-content-address-overflow",
        )?;
        let bytes = memory
            .read_initialization(owner, address, chunk_length)
            .map_err(memory_error)?;
        hasher.update(bytes);
        Ok(())
    })
}

fn for_each_chunk(
    byte_length: u32,
    max_access_bytes: usize,
    mut action: impl FnMut(usize, usize) -> Result<(), LoaderError>,
) -> Result<(), LoaderError> {
    let total = usize::try_from(byte_length).map_err(|_| {
        LoaderError::resource_limit(
            "image-materialization",
            "mapping-length-not-representable",
            "mapping length does not fit the host index type",
        )
    })?;
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

fn source_span<'a>(
    bytes: &'a [u8],
    source_offset: u32,
    byte_length: u32,
    code: &'static str,
) -> Result<&'a [u8], LoaderError> {
    let start = usize::try_from(source_offset).map_err(|_| {
        LoaderError::resource_limit(
            "image-materialization",
            "source-offset-not-representable",
            "source offset does not fit the host index type",
        )
    })?;
    let length = usize::try_from(byte_length).map_err(|_| {
        LoaderError::resource_limit(
            "image-materialization",
            "source-length-not-representable",
            "source length does not fit the host index type",
        )
    })?;
    let end = start
        .checked_add(length)
        .ok_or_else(materialization_accounting_overflow)?;
    bytes.get(start..end).ok_or_else(|| {
        LoaderError::invalid_input(
            "image-materialization",
            code,
            "planned source span extends beyond the immutable module snapshot",
        )
    })
}

fn source_span_for_rva<'a>(
    bytes: &'a [u8],
    plan: &Pe32LoadPlan,
    rva: u32,
    byte_length: u32,
    code: &'static str,
) -> Result<&'a [u8], LoaderError> {
    let requested = checked_range(rva, byte_length).ok_or_else(|| {
        LoaderError::invalid_input(
            "image-materialization",
            code,
            "requested RVA span overflows 32 bits",
        )
    })?;
    let header = &plan.header_mapping;
    let header_range = checked_range(header.destination_rva.0, header.copy_size)
        .ok_or_else(materialization_accounting_overflow)?;
    if contains_range(&header_range, &requested) {
        let delta = requested.start - header_range.start;
        let source_offset = u64::from(header.source_offset) + delta;
        return source_span_u64(bytes, source_offset, u64::from(byte_length), code);
    }
    for section in &plan.sections {
        let copy_range = checked_range(section.destination_rva.0, section.copy_size)
            .ok_or_else(materialization_accounting_overflow)?;
        if contains_range(&copy_range, &requested) {
            let delta = requested.start - copy_range.start;
            let source_offset = u64::from(section.source_offset) + delta;
            return source_span_u64(bytes, source_offset, u64::from(byte_length), code);
        }
    }
    Err(LoaderError::invalid_input(
        "image-materialization",
        code,
        format!("RVA span {rva:#010x}..+{byte_length:#x} is not file-backed"),
    ))
}

fn source_span_u64<'a>(
    bytes: &'a [u8],
    source_offset: u64,
    byte_length: u64,
    code: &'static str,
) -> Result<&'a [u8], LoaderError> {
    let start = usize::try_from(source_offset).map_err(|_| {
        LoaderError::resource_limit(
            "image-materialization",
            "source-offset-not-representable",
            "resolved source offset does not fit the host index type",
        )
    })?;
    let length = usize::try_from(byte_length).map_err(|_| {
        LoaderError::resource_limit(
            "image-materialization",
            "source-length-not-representable",
            "resolved source length does not fit the host index type",
        )
    })?;
    let end = start
        .checked_add(length)
        .ok_or_else(materialization_accounting_overflow)?;
    bytes.get(start..end).ok_or_else(|| {
        LoaderError::invalid_input(
            "image-materialization",
            code,
            "resolved source span extends beyond the immutable module snapshot",
        )
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

fn add_rva(
    load_base: GuestAddress,
    rva: Rva,
    code: &'static str,
) -> Result<GuestAddress, LoaderError> {
    add_rva_offset(load_base, rva, 0, code)
}

fn add_rva_offset(
    load_base: GuestAddress,
    rva: Rva,
    offset: usize,
    code: &'static str,
) -> Result<GuestAddress, LoaderError> {
    let value = u64::from(load_base.0)
        .checked_add(u64::from(rva.0))
        .and_then(|value| value.checked_add(offset as u64))
        .ok_or_else(materialization_accounting_overflow)?;
    let value = u32::try_from(value).map_err(|_| {
        LoaderError::invalid_input(
            "image-materialization",
            code,
            "selected load base plus RVA extends beyond the 32-bit guest address space",
        )
    })?;
    Ok(GuestAddress(value))
}

fn memory_error(error: GuestMemoryError) -> LoaderError {
    let class = match error.class {
        GuestMemoryErrorClass::ResourceLimit => LoaderErrorClass::ResourceLimit,
        GuestMemoryErrorClass::RuntimeDefect => LoaderErrorClass::RuntimeDefect,
        GuestMemoryErrorClass::InvalidRequest
        | GuestMemoryErrorClass::Conflict
        | GuestMemoryErrorClass::SynchronizationRequired
        | GuestMemoryErrorClass::Fault => LoaderErrorClass::InvalidInput,
    };
    LoaderError {
        class,
        stage: error.stage,
        code: error.code,
        message: error.message,
    }
}

fn materialization_accounting_overflow() -> LoaderError {
    LoaderError::resource_limit(
        "image-materialization",
        "materialization-accounting-overflow",
        "image materialization arithmetic exceeded the host accounting range",
    )
}

fn hex_digest(digest: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(digest.len() * 2);
    for byte in digest {
        value.push(char::from(HEX[usize::from(byte >> 4)]));
        value.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    value
}
