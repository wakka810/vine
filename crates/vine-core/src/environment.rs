use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestRange, MemoryInvalidation,
    MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection,
};
use crate::runtime::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationLifetime,
    ProcessAddressAllocationPurpose, ProcessAddressAllocationRecord,
    ProcessAddressAllocationReleasePolicy, ProcessAddressAllocationRequest,
    ProcessAddressMappingRequirements, RuntimeAddressRange, RuntimeState, RuntimeStateError,
    validate_process_address_mapping_allocation_for_memory,
};

pub const CONTROLLED_TEB_BASE: GuestAddress = GuestAddress(0x7ffd_e000);
pub const CONTROLLED_PEB_BASE: GuestAddress = GuestAddress(0x7ffd_f000);
pub const CONTROLLED_PROCESS_PARAMETERS_BASE: GuestAddress = GuestAddress(0x7ffd_c000);
pub const CONTROLLED_COMMAND_LINE_BASE: GuestAddress = GuestAddress(0x7ffd_d000);
pub const CONTROLLED_ENVIRONMENT_PAGE_SIZE: u64 = GUEST_PAGE_SIZE;
pub const CONTROLLED_COMMAND_LINE: &str = "bootstrap-params.exe";
pub const TEB_SELF_POINTER_OFFSET: u32 = 0x18;
pub const TEB_PEB_POINTER_OFFSET: u32 = 0x30;
pub const PEB_PROCESS_PARAMETERS_POINTER_OFFSET: u32 = 0x10;
pub const PROCESS_PARAMETERS_COMMAND_LINE_OFFSET: u32 = 0x40;
pub const PROCESS_PARAMETERS_ENVIRONMENT_POINTER_OFFSET: u32 = 0x48;
pub const UNICODE_STRING_BUFFER_OFFSET: u32 = 0x04;
pub const CONTROLLED_ANSI_COMMAND_LINE_ENCODING: &str = "ascii-v1";
pub const CONTROLLED_WIDE_ENVIRONMENT_ENCODING: &str = "utf16le-double-nul-v1";
const CONTROLLED_EMPTY_WIDE_ENVIRONMENT: [u8; 4] = [0, 0, 0, 0];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialAnsiCommandLine {
    pub buffer_address: GuestAddress,
    pub byte_length_with_nul: u32,
    pub encoding: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialWideEnvironmentStrings {
    pub authority_buffer_address: GuestAddress,
    pub copy_buffer_address: GuestAddress,
    pub byte_length_with_double_nul: u32,
    pub encoding: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialProcessParameters {
    pub process_parameters_owner: MemoryOwnerId,
    pub command_line_owner: MemoryOwnerId,
    pub process_parameters_region_kind: MemoryRegionKind,
    pub command_line_region_kind: MemoryRegionKind,
    pub process_parameters_range: GuestRange,
    pub command_line_range: GuestRange,
    pub committed_pages: usize,
    pub process_parameters_address: GuestAddress,
    pub command_line_buffer_address: GuestAddress,
    pub peb_process_parameters_pointer_offset: u32,
    pub process_parameters_command_line_offset: u32,
    pub unicode_string_buffer_offset: u32,
    pub command_line: String,
    pub command_line_utf16_units: usize,
    pub command_line_length_bytes: u16,
    pub command_line_maximum_length_bytes: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ansi: Option<InitialAnsiCommandLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_strings: Option<InitialWideEnvironmentStrings>,
    pub process_parameters_content_sha256: String,
    pub command_line_content_sha256: String,
    pub owners_sealed: bool,
    pub invalidation: MemoryInvalidation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialProcessEnvironment {
    pub address_allocation_id: crate::runtime::ProcessAddressAllocationId,
    pub teb_owner: MemoryOwnerId,
    pub peb_owner: MemoryOwnerId,
    pub teb_region_kind: MemoryRegionKind,
    pub peb_region_kind: MemoryRegionKind,
    pub teb_range: GuestRange,
    pub peb_range: GuestRange,
    pub committed_pages: usize,
    pub teb_address: GuestAddress,
    pub peb_address: GuestAddress,
    pub teb_self_pointer_offset: u32,
    pub teb_peb_pointer_offset: u32,
    pub teb_content_sha256: String,
    pub peb_content_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_parameters: Option<InitialProcessParameters>,
    pub owners_sealed: bool,
    pub invalidation: MemoryInvalidation,
}

#[derive(Debug)]
struct ProcessParameterPages {
    process_parameters_owner: MemoryOwnerId,
    command_line_owner: MemoryOwnerId,
    process_parameters_range: GuestRange,
    command_line_range: GuestRange,
    command_line_utf16_units: usize,
    command_line_length_bytes: u16,
    command_line_maximum_length_bytes: u16,
    ansi: Option<InitialAnsiCommandLine>,
    environment_strings: Option<InitialWideEnvironmentStrings>,
    process_parameters_content_sha256: String,
    command_line_content_sha256: String,
}

#[derive(Debug, Clone, Copy)]
struct ProcessParameterRegions {
    process_parameters_owner: MemoryOwnerId,
    process_parameters_range: GuestRange,
    command_line_owner: MemoryOwnerId,
    command_line_range: GuestRange,
}

#[derive(Debug)]
struct ValidatedCommandLine {
    text: String,
    utf16_units: usize,
    bytes_with_nul: Vec<u8>,
    length_bytes: u16,
    maximum_length_bytes: u16,
    ansi: Option<ValidatedAnsiCommandLine>,
    environment_strings: Option<ValidatedWideEnvironmentStrings>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvironmentMutationStage {
    TebReserved,
    PebReserved,
    ParameterRegionsReserved,
    TebCommitted,
    PebCommitted,
    ParameterPagesCommitted,
    TebZeroed,
    PebZeroed,
    ParameterPagesZeroed,
    TebSelfPointerWritten,
    TebPebPointerWritten,
    PebParametersPointerWritten,
    CommandLineLengthWritten,
    CommandLineMaximumLengthWritten,
    CommandLineBufferPointerWritten,
    CommandLineBytesWritten,
    CommandLineAnsiBytesWritten,
    ProcessParametersEnvironmentPointerWritten,
    EnvironmentAuthorityBytesWritten,
    EnvironmentCopyBytesWritten,
    CommandLineOwnerSealed,
    ProcessParametersOwnerSealed,
    PebOwnerSealed,
    TebOwnerSealed,
}

pub fn install_controlled_process_environment(
    memory: &mut GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
) -> Result<InitialProcessEnvironment, GuestMemoryError> {
    install_controlled_process_environment_internal(memory, allocation, None, false, false)
}

pub fn install_controlled_process_environment_with_parameters(
    memory: &mut GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
) -> Result<InitialProcessEnvironment, GuestMemoryError> {
    install_controlled_process_environment_with_command_line(
        memory,
        allocation,
        CONTROLLED_COMMAND_LINE,
    )
}

pub fn install_controlled_process_environment_with_command_line(
    memory: &mut GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
    command_line: &str,
) -> Result<InitialProcessEnvironment, GuestMemoryError> {
    install_controlled_process_environment_internal(
        memory,
        allocation,
        Some(command_line),
        false,
        false,
    )
}

pub fn install_controlled_process_environment_with_command_line_and_ansi(
    memory: &mut GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
    command_line: &str,
) -> Result<InitialProcessEnvironment, GuestMemoryError> {
    install_controlled_process_environment_internal(
        memory,
        allocation,
        Some(command_line),
        true,
        false,
    )
}

pub fn install_controlled_process_environment_with_command_line_ansi_and_environment_strings(
    memory: &mut GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
    command_line: &str,
) -> Result<InitialProcessEnvironment, GuestMemoryError> {
    install_controlled_process_environment_internal(
        memory,
        allocation,
        Some(command_line),
        true,
        true,
    )
}

fn install_controlled_process_environment_internal(
    memory: &mut GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
    command_line: Option<&str>,
    include_ansi: bool,
    include_environment_strings: bool,
) -> Result<InitialProcessEnvironment, GuestMemoryError> {
    validate_process_environment_mapping_allocation(allocation, command_line.is_some())?;
    let command_line = preflight_process_environment(
        memory,
        command_line,
        include_ansi,
        include_environment_strings,
    )?;
    let mut transaction = memory.begin_transaction();
    let mut checkpoint = |_| Ok(());
    let environment = install_controlled_process_environment_transaction(
        &mut transaction,
        allocation,
        command_line.as_ref(),
        &mut checkpoint,
    )?;
    transaction.commit_transaction();
    Ok(environment)
}

pub(crate) fn install_controlled_process_environment_for_runtime(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    command_line: Option<&str>,
    include_ansi: bool,
    include_environment_strings: bool,
) -> Result<InitialProcessEnvironment, RuntimeStateError> {
    let command_line = preflight_process_environment(
        memory,
        command_line,
        include_ansi,
        include_environment_strings,
    )
    .map_err(RuntimeStateError::from_memory)?;
    let reservation = runtime.plan_initial_process_environment_reservation(
        controlled_process_environment_allocation_request(command_line.is_some()),
    )?;
    let allocation = reservation.allocation.clone();
    validate_process_environment_mapping_allocation(&allocation, command_line.is_some())
        .map_err(RuntimeStateError::from_memory)?;

    let mut transaction = memory.begin_transaction();
    let mut checkpoint = |_| Ok(());
    let environment = install_controlled_process_environment_transaction(
        &mut transaction,
        &allocation,
        command_line.as_ref(),
        &mut checkpoint,
    )
    .map_err(RuntimeStateError::from_memory)?;
    runtime.commit_initial_process_environment_reservation(reservation, &environment)?;
    transaction.commit_transaction();
    Ok(environment)
}

fn install_controlled_process_environment_transaction<F>(
    memory: &mut GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
    command_line: Option<&ValidatedCommandLine>,
    checkpoint: &mut F,
) -> Result<InitialProcessEnvironment, GuestMemoryError>
where
    F: FnMut(EnvironmentMutationStage) -> Result<(), GuestMemoryError>,
{
    let (teb_owner, teb_range) = reserve_environment_page(
        memory,
        allocation.id.0,
        MemoryRegionKind::ThreadEnvironmentBlock,
        CONTROLLED_TEB_BASE,
    )?;
    checkpoint(EnvironmentMutationStage::TebReserved)?;
    let (peb_owner, peb_range) = reserve_environment_page(
        memory,
        allocation.id.0,
        MemoryRegionKind::ProcessEnvironmentBlock,
        CONTROLLED_PEB_BASE,
    )?;
    checkpoint(EnvironmentMutationStage::PebReserved)?;
    let parameter_regions = command_line
        .is_some()
        .then(|| {
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
            Ok::<_, GuestMemoryError>(ProcessParameterRegions {
                process_parameters_owner,
                process_parameters_range,
                command_line_owner,
                command_line_range,
            })
        })
        .transpose()?;
    if parameter_regions.is_some() {
        checkpoint(EnvironmentMutationStage::ParameterRegionsReserved)?;
    }
    let protection = PageProtection {
        read: true,
        write: true,
        execute: false,
    };
    memory.commit(teb_owner, teb_range, protection)?;
    checkpoint(EnvironmentMutationStage::TebCommitted)?;
    memory.commit(peb_owner, peb_range, protection)?;
    checkpoint(EnvironmentMutationStage::PebCommitted)?;
    if let Some(regions) = parameter_regions {
        memory.commit(
            regions.process_parameters_owner,
            regions.process_parameters_range,
            protection,
        )?;
        memory.commit(
            regions.command_line_owner,
            regions.command_line_range,
            protection,
        )?;
        checkpoint(EnvironmentMutationStage::ParameterPagesCommitted)?;
    }
    zero_environment_page(memory, teb_owner, CONTROLLED_TEB_BASE)?;
    checkpoint(EnvironmentMutationStage::TebZeroed)?;
    zero_environment_page(memory, peb_owner, CONTROLLED_PEB_BASE)?;
    checkpoint(EnvironmentMutationStage::PebZeroed)?;
    if let Some(regions) = parameter_regions {
        zero_environment_page(
            memory,
            regions.process_parameters_owner,
            CONTROLLED_PROCESS_PARAMETERS_BASE,
        )?;
        zero_environment_page(
            memory,
            regions.command_line_owner,
            CONTROLLED_COMMAND_LINE_BASE,
        )?;
        checkpoint(EnvironmentMutationStage::ParameterPagesZeroed)?;
    }

    let teb_self_address = add_offset(CONTROLLED_TEB_BASE, TEB_SELF_POINTER_OFFSET)?;
    let teb_peb_address = add_offset(CONTROLLED_TEB_BASE, TEB_PEB_POINTER_OFFSET)?;
    memory.write_initialization(
        teb_owner,
        teb_self_address,
        &CONTROLLED_TEB_BASE.0.to_le_bytes(),
    )?;
    checkpoint(EnvironmentMutationStage::TebSelfPointerWritten)?;
    memory.write_initialization(
        teb_owner,
        teb_peb_address,
        &CONTROLLED_PEB_BASE.0.to_le_bytes(),
    )?;
    checkpoint(EnvironmentMutationStage::TebPebPointerWritten)?;

    let process_parameter_pages = parameter_regions
        .map(|regions| {
            initialize_process_parameters(
                memory,
                peb_owner,
                regions,
                command_line.ok_or_else(|| {
                    GuestMemoryError::runtime_defect(
                        "process-bootstrap",
                        "validated-command-line-missing",
                        "parameter regions were created without validated command-line bytes",
                    )
                })?,
                checkpoint,
            )
        })
        .transpose()?;

    let teb_content_sha256 = hash_environment_page(memory, teb_owner, CONTROLLED_TEB_BASE)?;
    let peb_content_sha256 = hash_environment_page(memory, peb_owner, CONTROLLED_PEB_BASE)?;
    if let Some(parameters) = &process_parameter_pages {
        memory.seal_owner(parameters.command_line_owner)?;
        checkpoint(EnvironmentMutationStage::CommandLineOwnerSealed)?;
        memory.seal_owner(parameters.process_parameters_owner)?;
        checkpoint(EnvironmentMutationStage::ProcessParametersOwnerSealed)?;
    }
    memory.seal_owner(peb_owner)?;
    checkpoint(EnvironmentMutationStage::PebOwnerSealed)?;
    let invalidation = memory.seal_owner(teb_owner)?;
    checkpoint(EnvironmentMutationStage::TebOwnerSealed)?;
    let process_parameters = process_parameter_pages
        .map(|parameters| {
            let command_line = command_line.ok_or_else(|| {
                GuestMemoryError::runtime_defect(
                    "process-bootstrap",
                    "validated-command-line-missing",
                    "initialized process parameters lost their validated command line",
                )
            })?;
            Ok::<_, GuestMemoryError>(InitialProcessParameters {
                process_parameters_owner: parameters.process_parameters_owner,
                command_line_owner: parameters.command_line_owner,
                process_parameters_region_kind: MemoryRegionKind::ProcessParameters,
                command_line_region_kind: MemoryRegionKind::CommandLine,
                process_parameters_range: parameters.process_parameters_range,
                command_line_range: parameters.command_line_range,
                committed_pages: 2,
                process_parameters_address: CONTROLLED_PROCESS_PARAMETERS_BASE,
                command_line_buffer_address: CONTROLLED_COMMAND_LINE_BASE,
                peb_process_parameters_pointer_offset: PEB_PROCESS_PARAMETERS_POINTER_OFFSET,
                process_parameters_command_line_offset: PROCESS_PARAMETERS_COMMAND_LINE_OFFSET,
                unicode_string_buffer_offset: UNICODE_STRING_BUFFER_OFFSET,
                command_line: command_line.text.clone(),
                command_line_utf16_units: parameters.command_line_utf16_units,
                command_line_length_bytes: parameters.command_line_length_bytes,
                command_line_maximum_length_bytes: parameters.command_line_maximum_length_bytes,
                ansi: parameters.ansi,
                environment_strings: parameters.environment_strings,
                process_parameters_content_sha256: parameters.process_parameters_content_sha256,
                command_line_content_sha256: parameters.command_line_content_sha256,
                owners_sealed: true,
                invalidation,
            })
        })
        .transpose()?;

    Ok(InitialProcessEnvironment {
        address_allocation_id: allocation.id,
        teb_owner,
        peb_owner,
        teb_region_kind: MemoryRegionKind::ThreadEnvironmentBlock,
        peb_region_kind: MemoryRegionKind::ProcessEnvironmentBlock,
        teb_range,
        peb_range,
        committed_pages: 2,
        teb_address: CONTROLLED_TEB_BASE,
        peb_address: CONTROLLED_PEB_BASE,
        teb_self_pointer_offset: TEB_SELF_POINTER_OFFSET,
        teb_peb_pointer_offset: TEB_PEB_POINTER_OFFSET,
        teb_content_sha256,
        peb_content_sha256,
        process_parameters,
        owners_sealed: true,
        invalidation,
    })
}

fn initialize_process_parameters<F>(
    memory: &mut GuestMemory,
    peb_owner: MemoryOwnerId,
    regions: ProcessParameterRegions,
    command_line: &ValidatedCommandLine,
    checkpoint: &mut F,
) -> Result<ProcessParameterPages, GuestMemoryError>
where
    F: FnMut(EnvironmentMutationStage) -> Result<(), GuestMemoryError>,
{
    let peb_process_parameters_address =
        add_offset(CONTROLLED_PEB_BASE, PEB_PROCESS_PARAMETERS_POINTER_OFFSET)?;
    memory.write_initialization(
        peb_owner,
        peb_process_parameters_address,
        &CONTROLLED_PROCESS_PARAMETERS_BASE.0.to_le_bytes(),
    )?;
    checkpoint(EnvironmentMutationStage::PebParametersPointerWritten)?;
    let command_line_address = add_offset(
        CONTROLLED_PROCESS_PARAMETERS_BASE,
        PROCESS_PARAMETERS_COMMAND_LINE_OFFSET,
    )?;
    memory.write_initialization(
        regions.process_parameters_owner,
        command_line_address,
        &command_line.length_bytes.to_le_bytes(),
    )?;
    checkpoint(EnvironmentMutationStage::CommandLineLengthWritten)?;
    memory.write_initialization(
        regions.process_parameters_owner,
        add_offset(command_line_address, 2)?,
        &command_line.maximum_length_bytes.to_le_bytes(),
    )?;
    checkpoint(EnvironmentMutationStage::CommandLineMaximumLengthWritten)?;
    memory.write_initialization(
        regions.process_parameters_owner,
        add_offset(command_line_address, UNICODE_STRING_BUFFER_OFFSET)?,
        &CONTROLLED_COMMAND_LINE_BASE.0.to_le_bytes(),
    )?;
    checkpoint(EnvironmentMutationStage::CommandLineBufferPointerWritten)?;
    write_environment_bytes(
        memory,
        regions.command_line_owner,
        CONTROLLED_COMMAND_LINE_BASE,
        &command_line.bytes_with_nul,
    )?;
    checkpoint(EnvironmentMutationStage::CommandLineBytesWritten)?;
    let ansi = command_line
        .ansi
        .as_ref()
        .map(|ansi| {
            let buffer_address = add_offset(CONTROLLED_COMMAND_LINE_BASE, ansi.buffer_offset)?;
            write_environment_bytes(
                memory,
                regions.command_line_owner,
                buffer_address,
                &ansi.bytes_with_nul,
            )?;
            checkpoint(EnvironmentMutationStage::CommandLineAnsiBytesWritten)?;
            Ok::<_, GuestMemoryError>(InitialAnsiCommandLine {
                buffer_address,
                byte_length_with_nul: u32::try_from(ansi.bytes_with_nul.len()).map_err(|_| {
                    GuestMemoryError::runtime_defect(
                        "process-bootstrap",
                        "ansi-command-line-length-not-representable",
                        "validated ANSI command-line length does not fit x86 accounting",
                    )
                })?,
                encoding: CONTROLLED_ANSI_COMMAND_LINE_ENCODING.to_owned(),
            })
        })
        .transpose()?;
    let environment_strings = command_line
        .environment_strings
        .as_ref()
        .map(|environment| {
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
            checkpoint(EnvironmentMutationStage::ProcessParametersEnvironmentPointerWritten)?;
            write_environment_bytes(
                memory,
                regions.command_line_owner,
                authority_buffer_address,
                &environment.bytes_with_double_nul,
            )?;
            checkpoint(EnvironmentMutationStage::EnvironmentAuthorityBytesWritten)?;
            write_environment_bytes(
                memory,
                regions.command_line_owner,
                copy_buffer_address,
                &environment.bytes_with_double_nul,
            )?;
            checkpoint(EnvironmentMutationStage::EnvironmentCopyBytesWritten)?;
            Ok::<_, GuestMemoryError>(InitialWideEnvironmentStrings {
                authority_buffer_address,
                copy_buffer_address,
                byte_length_with_double_nul: u32::try_from(environment.bytes_with_double_nul.len())
                    .map_err(|_| {
                        GuestMemoryError::runtime_defect(
                            "process-bootstrap",
                            "wide-environment-length-not-representable",
                            "validated wide environment length does not fit x86 accounting",
                        )
                    })?,
                encoding: CONTROLLED_WIDE_ENVIRONMENT_ENCODING.to_owned(),
            })
        })
        .transpose()?;

    Ok(ProcessParameterPages {
        process_parameters_owner: regions.process_parameters_owner,
        command_line_owner: regions.command_line_owner,
        process_parameters_range: regions.process_parameters_range,
        command_line_range: regions.command_line_range,
        command_line_utf16_units: command_line.utf16_units,
        command_line_length_bytes: command_line.length_bytes,
        command_line_maximum_length_bytes: command_line.maximum_length_bytes,
        ansi,
        environment_strings,
        process_parameters_content_sha256: hash_environment_page(
            memory,
            regions.process_parameters_owner,
            CONTROLLED_PROCESS_PARAMETERS_BASE,
        )?,
        command_line_content_sha256: hash_environment_page(
            memory,
            regions.command_line_owner,
            CONTROLLED_COMMAND_LINE_BASE,
        )?,
    })
}

fn preflight_process_environment(
    memory: &GuestMemory,
    command_line: Option<&str>,
    include_ansi: bool,
    include_environment_strings: bool,
) -> Result<Option<ValidatedCommandLine>, GuestMemoryError> {
    if memory.limits().max_access_bytes < 4 {
        return Err(GuestMemoryError::resource_limit(
            "process-bootstrap",
            "environment-field-access-limit-too-small",
            "controlled process bootstrap requires a four-byte atomic guest pointer access",
        ));
    }

    let command_line = command_line
        .map(|command_line| {
            validate_command_line(command_line, include_ansi, include_environment_strings)
        })
        .transpose()?;
    let page_count = if command_line.is_some() { 4 } else { 2 };
    memory.preflight_growth(page_count, page_count, page_count, "process-bootstrap")?;

    let mut requested_ranges = Vec::with_capacity(page_count);
    let mut add_page = |address: GuestAddress| -> Result<(), GuestMemoryError> {
        let range = memory.preflight_fixed_reservation(
            address,
            CONTROLLED_ENVIRONMENT_PAGE_SIZE,
            "process-bootstrap",
        )?;
        if requested_ranges.iter().any(|existing: &GuestRange| {
            u64::from(existing.start().0) < range.end()
                && u64::from(range.start().0) < existing.end()
        }) {
            return Err(GuestMemoryError::runtime_defect(
                "process-bootstrap",
                "controlled-environment-range-overlap",
                "controlled process-environment fixed ranges overlap each other",
            ));
        }
        requested_ranges.push(range);
        Ok(())
    };
    add_page(CONTROLLED_TEB_BASE)?;
    add_page(CONTROLLED_PEB_BASE)?;
    if command_line.is_some() {
        add_page(CONTROLLED_PROCESS_PARAMETERS_BASE)?;
        add_page(CONTROLLED_COMMAND_LINE_BASE)?;
    }

    add_offset(CONTROLLED_TEB_BASE, TEB_SELF_POINTER_OFFSET)?;
    add_offset(CONTROLLED_TEB_BASE, TEB_PEB_POINTER_OFFSET)?;
    if command_line.is_some() {
        add_offset(CONTROLLED_PEB_BASE, PEB_PROCESS_PARAMETERS_POINTER_OFFSET)?;
        let command_line_field = add_offset(
            CONTROLLED_PROCESS_PARAMETERS_BASE,
            PROCESS_PARAMETERS_COMMAND_LINE_OFFSET,
        )?;
        add_offset(command_line_field, UNICODE_STRING_BUFFER_OFFSET)?;
        if include_environment_strings {
            add_offset(
                CONTROLLED_PROCESS_PARAMETERS_BASE,
                PROCESS_PARAMETERS_ENVIRONMENT_POINTER_OFFSET,
            )?;
        }
    }
    Ok(command_line)
}

fn validate_command_line(
    command_line: &str,
    include_ansi: bool,
    include_environment_strings: bool,
) -> Result<ValidatedCommandLine, GuestMemoryError> {
    if include_environment_strings && !include_ansi {
        return Err(GuestMemoryError::runtime_defect(
            "process-bootstrap",
            "wide-environment-requires-ansi-layout",
            "the layered environment-strings profile requires the preceding ANSI command-line view",
        ));
    }
    let utf16 = command_line.encode_utf16().collect::<Vec<_>>();
    if utf16.contains(&0) {
        return Err(GuestMemoryError::invalid_request(
            "process-bootstrap",
            "controlled-command-line-contains-nul",
            "controlled command line contains an embedded NUL",
        ));
    }
    let byte_length = utf16.len().checked_mul(2).ok_or_else(|| {
        GuestMemoryError::resource_limit(
            "process-bootstrap",
            "controlled-command-line-byte-count-overflow",
            "controlled command-line byte length overflowed host accounting",
        )
    })?;
    let length_bytes = u16::try_from(byte_length).map_err(|_| {
        GuestMemoryError::invalid_request(
            "process-bootstrap",
            "controlled-command-line-length-not-representable",
            "controlled command line length does not fit UNICODE_STRING.Length",
        )
    })?;
    let maximum_byte_length = byte_length.checked_add(2).ok_or_else(|| {
        GuestMemoryError::resource_limit(
            "process-bootstrap",
            "controlled-command-line-byte-count-overflow",
            "controlled command-line terminator overflowed host accounting",
        )
    })?;
    let maximum_length_bytes = u16::try_from(maximum_byte_length).map_err(|_| {
        GuestMemoryError::invalid_request(
            "process-bootstrap",
            "controlled-command-line-maximum-length-not-representable",
            "controlled command line allocation does not fit UNICODE_STRING.MaximumLength",
        )
    })?;
    let required_pages = maximum_byte_length.div_ceil(GUEST_PAGE_SIZE as usize);
    if required_pages != 1 {
        return Err(GuestMemoryError::invalid_request(
            "process-bootstrap",
            "multi-page-command-line-unimplemented",
            format!(
                "controlled command line needs {required_pages} pages; the measured bootstrap layout currently owns exactly one command-line page"
            ),
        ));
    }
    let mut bytes_with_nul = Vec::with_capacity(maximum_byte_length);
    for unit in &utf16 {
        bytes_with_nul.extend_from_slice(&unit.to_le_bytes());
    }
    bytes_with_nul.extend_from_slice(&0u16.to_le_bytes());
    let mut next_buffer_offset = maximum_byte_length;
    let ansi = if include_ansi {
        if !command_line.is_ascii() {
            return Err(GuestMemoryError::invalid_request(
                "process-bootstrap",
                "ansi-command-line-non-ascii-unsupported",
                "the controlled ANSI command-line view currently supports ASCII input only",
            ));
        }
        let buffer_offset = align_up_usize(maximum_byte_length, 16).ok_or_else(|| {
            GuestMemoryError::resource_limit(
                "process-bootstrap",
                "ansi-command-line-offset-overflow",
                "ANSI command-line alignment overflowed host accounting",
            )
        })?;
        let mut ansi_bytes_with_nul = command_line.as_bytes().to_vec();
        ansi_bytes_with_nul.push(0);
        let end = buffer_offset
            .checked_add(ansi_bytes_with_nul.len())
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "process-bootstrap",
                    "ansi-command-line-length-overflow",
                    "ANSI command-line view overflowed host accounting",
                )
            })?;
        if end > GUEST_PAGE_SIZE as usize {
            return Err(GuestMemoryError::invalid_request(
                "process-bootstrap",
                "ansi-command-line-view-does-not-fit",
                "the controlled UTF-16 and ANSI command-line views do not fit one command-line page",
            ));
        }
        next_buffer_offset = end;
        Some(ValidatedAnsiCommandLine {
            buffer_offset: u32::try_from(buffer_offset).map_err(|_| {
                GuestMemoryError::runtime_defect(
                    "process-bootstrap",
                    "ansi-command-line-offset-not-representable",
                    "ANSI command-line offset does not fit x86 accounting",
                )
            })?,
            bytes_with_nul: ansi_bytes_with_nul,
        })
    } else {
        None
    };
    let environment_strings = if include_environment_strings {
        let authority_buffer_offset = align_up_usize(next_buffer_offset, 16).ok_or_else(|| {
            GuestMemoryError::resource_limit(
                "process-bootstrap",
                "wide-environment-authority-offset-overflow",
                "wide environment authority alignment overflowed host accounting",
            )
        })?;
        let copy_buffer_offset = align_up_usize(
            authority_buffer_offset
                .checked_add(CONTROLLED_EMPTY_WIDE_ENVIRONMENT.len())
                .ok_or_else(|| {
                    GuestMemoryError::resource_limit(
                        "process-bootstrap",
                        "wide-environment-copy-offset-overflow",
                        "wide environment copy offset overflowed host accounting",
                    )
                })?,
            16,
        )
        .ok_or_else(|| {
            GuestMemoryError::resource_limit(
                "process-bootstrap",
                "wide-environment-copy-offset-overflow",
                "wide environment copy alignment overflowed host accounting",
            )
        })?;
        let end = copy_buffer_offset
            .checked_add(CONTROLLED_EMPTY_WIDE_ENVIRONMENT.len())
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "process-bootstrap",
                    "wide-environment-length-overflow",
                    "wide environment views overflowed host accounting",
                )
            })?;
        if end > GUEST_PAGE_SIZE as usize {
            return Err(GuestMemoryError::invalid_request(
                "process-bootstrap",
                "wide-environment-views-do-not-fit",
                "the controlled command-line and environment views do not fit one process page",
            ));
        }
        Some(ValidatedWideEnvironmentStrings {
            authority_buffer_offset: u32::try_from(authority_buffer_offset).map_err(|_| {
                GuestMemoryError::runtime_defect(
                    "process-bootstrap",
                    "wide-environment-authority-offset-not-representable",
                    "wide environment authority offset does not fit x86 accounting",
                )
            })?,
            copy_buffer_offset: u32::try_from(copy_buffer_offset).map_err(|_| {
                GuestMemoryError::runtime_defect(
                    "process-bootstrap",
                    "wide-environment-copy-offset-not-representable",
                    "wide environment copy offset does not fit x86 accounting",
                )
            })?,
            bytes_with_double_nul: CONTROLLED_EMPTY_WIDE_ENVIRONMENT.to_vec(),
        })
    } else {
        None
    };
    Ok(ValidatedCommandLine {
        text: command_line.to_owned(),
        utf16_units: utf16.len(),
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

pub fn controlled_process_environment_allocation_request(
    includes_process_parameters: bool,
) -> ProcessAddressAllocationRequest {
    let range = process_environment_address_range(includes_process_parameters);
    ProcessAddressAllocationRequest {
        purpose: ProcessAddressAllocationPurpose::ProcessEnvironment,
        owner: "runtime-initial-process-environment".to_owned(),
        preferred_address: Some(range.base),
        search_range: range,
        reserve_size: range.byte_length,
        alignment: GUEST_PAGE_SIZE as u32,
        guard_pages: 0,
        classification: ProcessAddressAllocationClassification::ProcessEnvironment,
        lifetime: ProcessAddressAllocationLifetime::Process,
        release_policy: ProcessAddressAllocationReleasePolicy::Permanent,
        executable: false,
    }
}

fn process_environment_address_range(includes_process_parameters: bool) -> RuntimeAddressRange {
    RuntimeAddressRange {
        base: if includes_process_parameters {
            CONTROLLED_PROCESS_PARAMETERS_BASE
        } else {
            CONTROLLED_TEB_BASE
        },
        byte_length: if includes_process_parameters {
            4 * GUEST_PAGE_SIZE
        } else {
            2 * GUEST_PAGE_SIZE
        },
    }
}

fn validate_process_environment_mapping_allocation(
    allocation: &ProcessAddressAllocationRecord,
    includes_process_parameters: bool,
) -> Result<(), GuestMemoryError> {
    let request = controlled_process_environment_allocation_request(includes_process_parameters);
    validate_process_address_mapping_allocation_for_memory(
        allocation,
        &ProcessAddressMappingRequirements {
            purpose: request.purpose,
            owner: request.owner,
            range: request.search_range,
            classification: request.classification,
            lifetime: request.lifetime,
            release_policy: request.release_policy,
            executable: request.executable,
        },
    )
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

fn hash_environment_page(
    memory: &GuestMemory,
    owner: MemoryOwnerId,
    base: GuestAddress,
) -> Result<String, GuestMemoryError> {
    let page_size = GUEST_PAGE_SIZE as usize;
    let chunk_length = memory.limits().max_access_bytes.min(page_size);
    let mut hasher = Sha256::new();
    let mut offset = 0usize;
    while offset < page_size {
        let byte_length = (page_size - offset).min(chunk_length);
        let address = add_offset_usize(base, offset)?;
        hasher.update(memory.read_initialization(owner, address, byte_length)?);
        offset += byte_length;
    }
    Ok(hex_digest(&hasher.finalize()))
}

fn add_offset(base: GuestAddress, offset: u32) -> Result<GuestAddress, GuestMemoryError> {
    let address = base.0.checked_add(offset).ok_or_else(|| {
        GuestMemoryError::runtime_defect(
            "process-bootstrap",
            "environment-field-address-overflow",
            "controlled process environment field address overflows 32 bits",
        )
    })?;
    Ok(GuestAddress(address))
}

fn add_offset_usize(base: GuestAddress, offset: usize) -> Result<GuestAddress, GuestMemoryError> {
    let offset = u32::try_from(offset).map_err(|_| {
        GuestMemoryError::runtime_defect(
            "process-bootstrap",
            "environment-page-offset-not-representable",
            "controlled process environment page offset does not fit 32 bits",
        )
    })?;
    add_offset(base, offset)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{GuestMemoryErrorClass, MemoryLimits};
    use crate::runtime::{ProcessAddressSpaceAllocator, ProcessAddressSpaceLayout};

    #[test]
    fn rolls_back_every_process_environment_mutation_checkpoint() {
        let stages = [
            EnvironmentMutationStage::TebReserved,
            EnvironmentMutationStage::PebReserved,
            EnvironmentMutationStage::ParameterRegionsReserved,
            EnvironmentMutationStage::TebCommitted,
            EnvironmentMutationStage::PebCommitted,
            EnvironmentMutationStage::ParameterPagesCommitted,
            EnvironmentMutationStage::TebZeroed,
            EnvironmentMutationStage::PebZeroed,
            EnvironmentMutationStage::ParameterPagesZeroed,
            EnvironmentMutationStage::TebSelfPointerWritten,
            EnvironmentMutationStage::TebPebPointerWritten,
            EnvironmentMutationStage::PebParametersPointerWritten,
            EnvironmentMutationStage::CommandLineLengthWritten,
            EnvironmentMutationStage::CommandLineMaximumLengthWritten,
            EnvironmentMutationStage::CommandLineBufferPointerWritten,
            EnvironmentMutationStage::CommandLineBytesWritten,
            EnvironmentMutationStage::CommandLineAnsiBytesWritten,
            EnvironmentMutationStage::ProcessParametersEnvironmentPointerWritten,
            EnvironmentMutationStage::EnvironmentAuthorityBytesWritten,
            EnvironmentMutationStage::EnvironmentCopyBytesWritten,
            EnvironmentMutationStage::CommandLineOwnerSealed,
            EnvironmentMutationStage::ProcessParametersOwnerSealed,
            EnvironmentMutationStage::PebOwnerSealed,
            EnvironmentMutationStage::TebOwnerSealed,
        ];

        for fail_at in stages {
            let mut memory = GuestMemory::new(MemoryLimits {
                max_physical_pages: 4,
                max_regions: 4,
                max_owners: 4,
                max_access_bytes: GUEST_PAGE_SIZE as usize,
            })
            .expect("memory");
            let before = memory.stats();
            let command_line =
                preflight_process_environment(&memory, Some("failpoint.exe"), true, true)
                    .expect("preflight")
                    .expect("validated command line");
            let allocation = environment_allocation(true);
            {
                let mut transaction = memory.begin_transaction();
                let mut checkpoint = |stage| {
                    if stage == fail_at {
                        Err(GuestMemoryError::invalid_request(
                            "process-bootstrap-test",
                            "injected-process-environment-failure",
                            format!("injected failure after {stage:?}"),
                        ))
                    } else {
                        Ok(())
                    }
                };
                let error = install_controlled_process_environment_transaction(
                    &mut transaction,
                    &allocation,
                    Some(&command_line),
                    &mut checkpoint,
                )
                .expect_err("injected checkpoint must fail");
                assert_eq!(error.class, GuestMemoryErrorClass::InvalidRequest);
                assert_eq!(error.code, "injected-process-environment-failure");
            }
            assert_eq!(memory.stats(), before, "rollback after {fail_at:?}");
            install_controlled_process_environment_with_command_line_ansi_and_environment_strings(
                &mut memory,
                &allocation,
                "failpoint.exe",
            )
            .unwrap_or_else(|error| panic!("retry after {fail_at:?} failed: {error}"));
        }
    }

    fn environment_allocation(includes_process_parameters: bool) -> ProcessAddressAllocationRecord {
        let mut allocator =
            ProcessAddressSpaceAllocator::vine_v1(ProcessAddressSpaceLayout::vine_v1())
                .expect("address allocator");
        allocator
            .reserve(controlled_process_environment_allocation_request(
                includes_process_parameters,
            ))
            .expect("environment allocation")
    }
}
