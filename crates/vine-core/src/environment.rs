use encoding_rs::SHIFT_JIS;

use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestRange, MemoryOwnerId,
    MemoryOwnerKind, MemoryRegionKind, PageProtection,
};
use crate::runtime::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationRecord, ProcessAddressAllocationReleasePolicy,
    ProcessAddressAllocationRequest, RuntimeAddressRange,
};

pub const CONTROLLED_TEB_BASE: GuestAddress = GuestAddress(0x7ffd_e000);
pub const CONTROLLED_PEB_BASE: GuestAddress = GuestAddress(0x7ffd_f000);
pub const CONTROLLED_PROCESS_PARAMETERS_BASE: GuestAddress = GuestAddress(0x7ffd_c000);
pub const CONTROLLED_COMMAND_LINE_BASE: GuestAddress = GuestAddress(0x7ffd_d000);
pub const CONTROLLED_ENVIRONMENT_PAGE_SIZE: u64 = GUEST_PAGE_SIZE;
pub const TEB_SELF_POINTER_OFFSET: u32 = 0x18;
pub const TEB_PEB_POINTER_OFFSET: u32 = 0x30;
pub const PEB_PROCESS_PARAMETERS_POINTER_OFFSET: u32 = 0x10;
pub const PROCESS_PARAMETERS_COMMAND_LINE_OFFSET: u32 = 0x40;
pub const PROCESS_PARAMETERS_ENVIRONMENT_POINTER_OFFSET: u32 = 0x48;
pub const UNICODE_STRING_BUFFER_OFFSET: u32 = 0x04;
const CONTROLLED_EMPTY_WIDE_ENVIRONMENT: [u8; 4] = [0, 0, 0, 0];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialAnsiCommandLine {
    pub buffer_address: GuestAddress,
    pub byte_length_with_nul: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialWideEnvironmentStrings {
    pub authority_buffer_address: GuestAddress,
    pub copy_buffer_address: GuestAddress,
    pub byte_length_with_double_nul: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialProcessParameters {
    pub process_parameters_range: GuestRange,
    pub command_line_range: GuestRange,
    pub process_parameters_address: GuestAddress,
    pub command_line_buffer_address: GuestAddress,
    pub command_line: String,
    pub command_line_length_bytes: u16,
    pub command_line_maximum_length_bytes: u16,
    pub ansi: InitialAnsiCommandLine,
    pub environment_strings: InitialWideEnvironmentStrings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialProcessEnvironment {
    pub address_allocation_id: crate::runtime::ProcessAddressAllocationId,
    pub teb_range: GuestRange,
    pub peb_range: GuestRange,
    pub teb_address: GuestAddress,
    pub peb_address: GuestAddress,
    pub process_parameters: InitialProcessParameters,
}

#[derive(Debug)]
struct ProcessParameterPages {
    process_parameters_owner: MemoryOwnerId,
    command_line_owner: MemoryOwnerId,
    process_parameters_range: GuestRange,
    command_line_range: GuestRange,
    command_line_length_bytes: u16,
    command_line_maximum_length_bytes: u16,
    ansi: InitialAnsiCommandLine,
    environment_strings: InitialWideEnvironmentStrings,
}

#[derive(Debug, Clone, Copy)]
struct ProcessParameterRegions {
    process_parameters_owner: MemoryOwnerId,
    process_parameters_range: GuestRange,
    command_line_owner: MemoryOwnerId,
    command_line_range: GuestRange,
}

#[derive(Debug)]
pub(crate) struct ValidatedCommandLine {
    text: String,
    bytes_with_nul: Vec<u8>,
    length_bytes: u16,
    maximum_length_bytes: u16,
    ansi: ValidatedAnsiCommandLine,
    environment_strings: ValidatedWideEnvironmentStrings,
}

#[derive(Debug)]
struct ValidatedAnsiCommandLine {
    buffer_offset: u32,
    bytes_with_nul: Vec<u8>,
}

#[derive(Debug)]
struct ValidatedWideEnvironmentStrings {
    authority_buffer_offset: u32,
    copy_buffer_offset: u32,
    bytes_with_double_nul: Vec<u8>,
}

#[cfg(test)]
pub(crate) fn install_process_environment(
    memory: &mut GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
    command_line: &str,
) -> Result<InitialProcessEnvironment, GuestMemoryError> {
    let command_line = preflight_process_environment(memory, command_line)?;
    let mut transaction = memory.begin_transaction();
    let environment = install_controlled_process_environment_transaction(
        &mut transaction,
        allocation,
        &command_line,
    )?;
    transaction.commit_transaction();
    Ok(environment)
}

pub(crate) fn install_controlled_process_environment_transaction(
    memory: &mut GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
    command_line: &ValidatedCommandLine,
) -> Result<InitialProcessEnvironment, GuestMemoryError> {
    let (teb_owner, teb_range) = reserve_environment_page(
        memory,
        allocation.id.0,
        MemoryRegionKind::ThreadEnvironmentBlock,
        CONTROLLED_TEB_BASE,
    )?;
    let (peb_owner, peb_range) = reserve_environment_page(
        memory,
        allocation.id.0,
        MemoryRegionKind::ProcessEnvironmentBlock,
        CONTROLLED_PEB_BASE,
    )?;
    let (process_parameters_owner, process_parameters_range) = reserve_environment_page(
        memory,
        allocation.id.0,
        MemoryRegionKind::ProcessParameters,
        CONTROLLED_PROCESS_PARAMETERS_BASE,
    )?;
    let (command_line_owner, command_line_range) = reserve_environment_page(
        memory,
        allocation.id.0,
        MemoryRegionKind::CommandLine,
        CONTROLLED_COMMAND_LINE_BASE,
    )?;
    let parameter_regions = ProcessParameterRegions {
        process_parameters_owner,
        process_parameters_range,
        command_line_owner,
        command_line_range,
    };
    let protection = PageProtection {
        read: true,
        write: true,
        execute: false,
    };
    memory.commit(teb_owner, teb_range, protection)?;
    memory.commit(peb_owner, peb_range, protection)?;
    memory.commit(
        parameter_regions.process_parameters_owner,
        parameter_regions.process_parameters_range,
        protection,
    )?;
    memory.commit(
        parameter_regions.command_line_owner,
        parameter_regions.command_line_range,
        protection,
    )?;
    zero_environment_page(memory, teb_owner, CONTROLLED_TEB_BASE)?;
    zero_environment_page(memory, peb_owner, CONTROLLED_PEB_BASE)?;
    zero_environment_page(
        memory,
        parameter_regions.process_parameters_owner,
        CONTROLLED_PROCESS_PARAMETERS_BASE,
    )?;
    zero_environment_page(
        memory,
        parameter_regions.command_line_owner,
        CONTROLLED_COMMAND_LINE_BASE,
    )?;

    let teb_self_address = add_offset(CONTROLLED_TEB_BASE, TEB_SELF_POINTER_OFFSET)?;
    let teb_peb_address = add_offset(CONTROLLED_TEB_BASE, TEB_PEB_POINTER_OFFSET)?;
    memory.write_initialization(
        teb_owner,
        teb_self_address,
        &CONTROLLED_TEB_BASE.0.to_le_bytes(),
    )?;
    memory.write_initialization(
        teb_owner,
        teb_peb_address,
        &CONTROLLED_PEB_BASE.0.to_le_bytes(),
    )?;

    let parameters =
        initialize_process_parameters(memory, peb_owner, parameter_regions, command_line)?;
    memory.seal_owner(parameters.command_line_owner)?;
    memory.seal_owner(parameters.process_parameters_owner)?;
    memory.seal_owner(peb_owner)?;
    memory.seal_owner(teb_owner)?;
    let process_parameters = InitialProcessParameters {
        process_parameters_range: parameters.process_parameters_range,
        command_line_range: parameters.command_line_range,
        process_parameters_address: CONTROLLED_PROCESS_PARAMETERS_BASE,
        command_line_buffer_address: CONTROLLED_COMMAND_LINE_BASE,
        command_line: command_line.text.clone(),
        command_line_length_bytes: parameters.command_line_length_bytes,
        command_line_maximum_length_bytes: parameters.command_line_maximum_length_bytes,
        ansi: parameters.ansi,
        environment_strings: parameters.environment_strings,
    };

    Ok(InitialProcessEnvironment {
        address_allocation_id: allocation.id,
        teb_range,
        peb_range,
        teb_address: CONTROLLED_TEB_BASE,
        peb_address: CONTROLLED_PEB_BASE,
        process_parameters,
    })
}

fn initialize_process_parameters(
    memory: &mut GuestMemory,
    peb_owner: MemoryOwnerId,
    regions: ProcessParameterRegions,
    command_line: &ValidatedCommandLine,
) -> Result<ProcessParameterPages, GuestMemoryError> {
    let peb_process_parameters_address =
        add_offset(CONTROLLED_PEB_BASE, PEB_PROCESS_PARAMETERS_POINTER_OFFSET)?;
    memory.write_initialization(
        peb_owner,
        peb_process_parameters_address,
        &CONTROLLED_PROCESS_PARAMETERS_BASE.0.to_le_bytes(),
    )?;
    let command_line_address = add_offset(
        CONTROLLED_PROCESS_PARAMETERS_BASE,
        PROCESS_PARAMETERS_COMMAND_LINE_OFFSET,
    )?;
    memory.write_initialization(
        regions.process_parameters_owner,
        command_line_address,
        &command_line.length_bytes.to_le_bytes(),
    )?;
    memory.write_initialization(
        regions.process_parameters_owner,
        add_offset(command_line_address, 2)?,
        &command_line.maximum_length_bytes.to_le_bytes(),
    )?;
    memory.write_initialization(
        regions.process_parameters_owner,
        add_offset(command_line_address, UNICODE_STRING_BUFFER_OFFSET)?,
        &CONTROLLED_COMMAND_LINE_BASE.0.to_le_bytes(),
    )?;
    write_environment_bytes(
        memory,
        regions.command_line_owner,
        CONTROLLED_COMMAND_LINE_BASE,
        &command_line.bytes_with_nul,
    )?;
    let ansi = {
        let ansi = &command_line.ansi;
        let buffer_address = add_offset(CONTROLLED_COMMAND_LINE_BASE, ansi.buffer_offset)?;
        write_environment_bytes(
            memory,
            regions.command_line_owner,
            buffer_address,
            &ansi.bytes_with_nul,
        )?;
        InitialAnsiCommandLine {
            buffer_address,
            byte_length_with_nul: u32::try_from(ansi.bytes_with_nul.len()).map_err(|_| {
                GuestMemoryError::runtime_defect(
                    "validated ANSI command-line length does not fit x86 accounting",
                )
            })?,
        }
    };
    let environment_strings = {
        let environment = &command_line.environment_strings;
        let authority_buffer_address = add_offset(
            CONTROLLED_COMMAND_LINE_BASE,
            environment.authority_buffer_offset,
        )?;
        let copy_buffer_address =
            add_offset(CONTROLLED_COMMAND_LINE_BASE, environment.copy_buffer_offset)?;
        memory.write_initialization(
            regions.process_parameters_owner,
            add_offset(
                CONTROLLED_PROCESS_PARAMETERS_BASE,
                PROCESS_PARAMETERS_ENVIRONMENT_POINTER_OFFSET,
            )?,
            &authority_buffer_address.0.to_le_bytes(),
        )?;
        write_environment_bytes(
            memory,
            regions.command_line_owner,
            authority_buffer_address,
            &environment.bytes_with_double_nul,
        )?;
        write_environment_bytes(
            memory,
            regions.command_line_owner,
            copy_buffer_address,
            &environment.bytes_with_double_nul,
        )?;
        InitialWideEnvironmentStrings {
            authority_buffer_address,
            copy_buffer_address,
            byte_length_with_double_nul: u32::try_from(environment.bytes_with_double_nul.len())
                .map_err(|_| {
                    GuestMemoryError::runtime_defect(
                        "validated wide environment length does not fit x86 accounting",
                    )
                })?,
        }
    };

    Ok(ProcessParameterPages {
        process_parameters_owner: regions.process_parameters_owner,
        command_line_owner: regions.command_line_owner,
        process_parameters_range: regions.process_parameters_range,
        command_line_range: regions.command_line_range,
        command_line_length_bytes: command_line.length_bytes,
        command_line_maximum_length_bytes: command_line.maximum_length_bytes,
        ansi,
        environment_strings,
    })
}

pub(crate) fn preflight_process_environment(
    memory: &GuestMemory,
    command_line: &str,
) -> Result<ValidatedCommandLine, GuestMemoryError> {
    if memory.limits().max_access_bytes < 4 {
        return Err(GuestMemoryError::resource_limit(
            "controlled process bootstrap requires a four-byte atomic guest pointer access",
        ));
    }

    let command_line = validate_command_line(command_line)?;
    memory.preflight_growth(4, 4, 4)?;

    let mut requested_ranges = Vec::with_capacity(4);
    let mut add_page = |address: GuestAddress| -> Result<(), GuestMemoryError> {
        let range =
            memory.preflight_fixed_reservation(address, CONTROLLED_ENVIRONMENT_PAGE_SIZE)?;
        if requested_ranges.iter().any(|existing: &GuestRange| {
            u64::from(existing.start().0) < range.end()
                && u64::from(range.start().0) < existing.end()
        }) {
            return Err(GuestMemoryError::runtime_defect(
                "controlled process-environment fixed ranges overlap each other",
            ));
        }
        requested_ranges.push(range);
        Ok(())
    };
    add_page(CONTROLLED_TEB_BASE)?;
    add_page(CONTROLLED_PEB_BASE)?;
    add_page(CONTROLLED_PROCESS_PARAMETERS_BASE)?;
    add_page(CONTROLLED_COMMAND_LINE_BASE)?;

    add_offset(CONTROLLED_TEB_BASE, TEB_SELF_POINTER_OFFSET)?;
    add_offset(CONTROLLED_TEB_BASE, TEB_PEB_POINTER_OFFSET)?;
    add_offset(CONTROLLED_PEB_BASE, PEB_PROCESS_PARAMETERS_POINTER_OFFSET)?;
    let command_line_field = add_offset(
        CONTROLLED_PROCESS_PARAMETERS_BASE,
        PROCESS_PARAMETERS_COMMAND_LINE_OFFSET,
    )?;
    add_offset(command_line_field, UNICODE_STRING_BUFFER_OFFSET)?;
    add_offset(
        CONTROLLED_PROCESS_PARAMETERS_BASE,
        PROCESS_PARAMETERS_ENVIRONMENT_POINTER_OFFSET,
    )?;
    Ok(command_line)
}

fn validate_command_line(command_line: &str) -> Result<ValidatedCommandLine, GuestMemoryError> {
    let utf16 = command_line.encode_utf16().collect::<Vec<_>>();
    if utf16.contains(&0) {
        return Err(GuestMemoryError::invalid_request(
            "controlled command line contains an embedded NUL",
        ));
    }
    let byte_length = utf16.len().checked_mul(2).ok_or_else(|| {
        GuestMemoryError::resource_limit(
            "controlled command-line byte length overflowed host accounting",
        )
    })?;
    let length_bytes = u16::try_from(byte_length).map_err(|_| {
        GuestMemoryError::invalid_request(
            "controlled command line length does not fit UNICODE_STRING.Length",
        )
    })?;
    let maximum_byte_length = byte_length.checked_add(2).ok_or_else(|| {
        GuestMemoryError::resource_limit(
            "controlled command-line terminator overflowed host accounting",
        )
    })?;
    let maximum_length_bytes = u16::try_from(maximum_byte_length).map_err(|_| {
        GuestMemoryError::invalid_request(
            "controlled command line allocation does not fit UNICODE_STRING.MaximumLength",
        )
    })?;
    let required_pages = maximum_byte_length.div_ceil(GUEST_PAGE_SIZE as usize);
    if required_pages != 1 {
        return Err(GuestMemoryError::invalid_request(format!(
            "controlled command line needs {required_pages} pages; the measured bootstrap layout currently owns exactly one command-line page"
        )));
    }
    let mut bytes_with_nul = Vec::with_capacity(maximum_byte_length);
    for unit in &utf16 {
        bytes_with_nul.extend_from_slice(&unit.to_le_bytes());
    }
    bytes_with_nul.extend_from_slice(&0u16.to_le_bytes());
    let (encoded, _, had_errors) = SHIFT_JIS.encode(command_line);
    let ansi = {
        if had_errors {
            return Err(GuestMemoryError::invalid_request(
                "command line cannot be represented in Windows-932 without replacement",
            ));
        }
        let buffer_offset = align_up_usize(maximum_byte_length, 16).ok_or_else(|| {
            GuestMemoryError::resource_limit(
                "ANSI command-line alignment overflowed host accounting",
            )
        })?;
        let mut ansi_bytes_with_nul = encoded.into_owned();
        ansi_bytes_with_nul.push(0);
        let end = buffer_offset
            .checked_add(ansi_bytes_with_nul.len())
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "ANSI command-line view overflowed host accounting",
                )
            })?;
        if end > GUEST_PAGE_SIZE as usize {
            return Err(GuestMemoryError::invalid_request(
                "the controlled UTF-16 and ANSI command-line views do not fit one command-line page",
            ));
        }
        ValidatedAnsiCommandLine {
            buffer_offset: u32::try_from(buffer_offset).map_err(|_| {
                GuestMemoryError::runtime_defect(
                    "ANSI command-line offset does not fit x86 accounting",
                )
            })?,
            bytes_with_nul: ansi_bytes_with_nul,
        }
    };
    let environment_strings = {
        let ansi_end = usize::try_from(ansi.buffer_offset)
            .unwrap_or(usize::MAX)
            .checked_add(ansi.bytes_with_nul.len())
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "ANSI command-line view offset overflowed host accounting",
                )
            })?;
        let authority_buffer_offset = align_up_usize(ansi_end, 16).ok_or_else(|| {
            GuestMemoryError::resource_limit(
                "wide environment authority alignment overflowed host accounting",
            )
        })?;
        let copy_buffer_offset = align_up_usize(
            authority_buffer_offset
                .checked_add(CONTROLLED_EMPTY_WIDE_ENVIRONMENT.len())
                .ok_or_else(|| {
                    GuestMemoryError::resource_limit(
                        "wide environment copy offset overflowed host accounting",
                    )
                })?,
            16,
        )
        .ok_or_else(|| {
            GuestMemoryError::resource_limit(
                "wide environment copy alignment overflowed host accounting",
            )
        })?;
        let end = copy_buffer_offset
            .checked_add(CONTROLLED_EMPTY_WIDE_ENVIRONMENT.len())
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "wide environment views overflowed host accounting",
                )
            })?;
        if end > GUEST_PAGE_SIZE as usize {
            return Err(GuestMemoryError::invalid_request(
                "the controlled command-line and environment views do not fit one process page",
            ));
        }
        ValidatedWideEnvironmentStrings {
            authority_buffer_offset: u32::try_from(authority_buffer_offset).map_err(|_| {
                GuestMemoryError::runtime_defect(
                    "wide environment authority offset does not fit x86 accounting",
                )
            })?,
            copy_buffer_offset: u32::try_from(copy_buffer_offset).map_err(|_| {
                GuestMemoryError::runtime_defect(
                    "wide environment copy offset does not fit x86 accounting",
                )
            })?,
            bytes_with_double_nul: CONTROLLED_EMPTY_WIDE_ENVIRONMENT.to_vec(),
        }
    };
    Ok(ValidatedCommandLine {
        text: command_line.to_owned(),
        bytes_with_nul,
        length_bytes,
        maximum_length_bytes,
        ansi,
        environment_strings,
    })
}

fn align_up_usize(value: usize, alignment: usize) -> Option<usize> {
    value
        .checked_add(alignment.checked_sub(1)?)
        .map(|rounded| rounded & !(alignment - 1))
}

fn reserve_environment_page(
    memory: &mut GuestMemory,
    address_allocation_id: u64,
    kind: MemoryRegionKind,
    address: GuestAddress,
) -> Result<(MemoryOwnerId, GuestRange), GuestMemoryError> {
    let owner = memory.create_owner(MemoryOwnerKind::Anonymous)?;
    match memory.reserve_fixed_for_address_allocation(
        address_allocation_id,
        owner,
        kind,
        address,
        CONTROLLED_ENVIRONMENT_PAGE_SIZE,
    ) {
        Ok(range) => Ok((owner, range)),
        Err(error) => {
            memory.destroy_owner(owner)?;
            Err(error)
        }
    }
}

pub(crate) fn controlled_process_environment_allocation_request() -> ProcessAddressAllocationRequest
{
    let range = RuntimeAddressRange {
        base: CONTROLLED_PROCESS_PARAMETERS_BASE,
        byte_length: 4 * GUEST_PAGE_SIZE,
    };
    ProcessAddressAllocationRequest {
        purpose: ProcessAddressAllocationPurpose::ProcessEnvironment,

        preferred_address: Some(range.base),
        search_range: range,
        reserve_size: range.byte_length,
        alignment: GUEST_PAGE_SIZE as u32,
        guard_pages: 0,
        classification: ProcessAddressAllocationClassification::ProcessEnvironment,

        release_policy: ProcessAddressAllocationReleasePolicy::Permanent,
        executable: false,
    }
}

fn zero_environment_page(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    base: GuestAddress,
) -> Result<(), GuestMemoryError> {
    let page_size = GUEST_PAGE_SIZE as usize;
    let chunk_length = memory.limits().max_access_bytes.min(page_size);
    let mut offset = 0usize;
    while offset < page_size {
        let byte_length = (page_size - offset).min(chunk_length);
        let address = add_offset_usize(base, offset)?;
        memory.zero_initialization(owner, address, byte_length)?;
        offset += byte_length;
    }
    Ok(())
}

fn write_environment_bytes(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    base: GuestAddress,
    bytes: &[u8],
) -> Result<(), GuestMemoryError> {
    let chunk_length = memory.limits().max_access_bytes.min(bytes.len());
    let mut offset = 0usize;
    while offset < bytes.len() {
        let byte_length = (bytes.len() - offset).min(chunk_length);
        let address = add_offset_usize(base, offset)?;
        memory.write_initialization(owner, address, &bytes[offset..offset + byte_length])?;
        offset += byte_length;
    }
    Ok(())
}

fn add_offset(base: GuestAddress, offset: u32) -> Result<GuestAddress, GuestMemoryError> {
    let address = base.0.checked_add(offset).ok_or_else(|| {
        GuestMemoryError::runtime_defect(
            "controlled process environment field address overflows 32 bits",
        )
    })?;
    Ok(GuestAddress(address))
}

fn add_offset_usize(base: GuestAddress, offset: usize) -> Result<GuestAddress, GuestMemoryError> {
    let offset = u32::try_from(offset).map_err(|_| {
        GuestMemoryError::runtime_defect(
            "controlled process environment page offset does not fit 32 bits",
        )
    })?;
    add_offset(base, offset)
}
