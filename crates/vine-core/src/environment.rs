use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestRange, MemoryInvalidation,
    MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection,
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
pub const UNICODE_STRING_BUFFER_OFFSET: u32 = 0x04;

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
    pub process_parameters_content_sha256: String,
    pub command_line_content_sha256: String,
    pub owners_sealed: bool,
    pub invalidation: MemoryInvalidation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialProcessEnvironment {
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
    process_parameters_content_sha256: String,
    command_line_content_sha256: String,
}

pub fn install_controlled_process_environment(
    memory: &mut GuestMemory,
) -> Result<InitialProcessEnvironment, GuestMemoryError> {
    install_controlled_process_environment_internal(memory, None)
}

pub fn install_controlled_process_environment_with_parameters(
    memory: &mut GuestMemory,
) -> Result<InitialProcessEnvironment, GuestMemoryError> {
    install_controlled_process_environment_with_command_line(memory, CONTROLLED_COMMAND_LINE)
}

pub fn install_controlled_process_environment_with_command_line(
    memory: &mut GuestMemory,
    command_line: &str,
) -> Result<InitialProcessEnvironment, GuestMemoryError> {
    install_controlled_process_environment_internal(memory, Some(command_line))
}

fn install_controlled_process_environment_internal(
    memory: &mut GuestMemory,
    command_line: Option<&str>,
) -> Result<InitialProcessEnvironment, GuestMemoryError> {
    let (teb_owner, teb_range) = reserve_environment_page(
        memory,
        MemoryRegionKind::ThreadEnvironmentBlock,
        CONTROLLED_TEB_BASE,
    )?;
    let (peb_owner, peb_range) = reserve_environment_page(
        memory,
        MemoryRegionKind::ProcessEnvironmentBlock,
        CONTROLLED_PEB_BASE,
    )?;
    let parameter_regions = command_line
        .is_some()
        .then(|| {
            let (process_parameters_owner, process_parameters_range) = reserve_environment_page(
                memory,
                MemoryRegionKind::ProcessParameters,
                CONTROLLED_PROCESS_PARAMETERS_BASE,
            )?;
            let (command_line_owner, command_line_range) = reserve_environment_page(
                memory,
                MemoryRegionKind::CommandLine,
                CONTROLLED_COMMAND_LINE_BASE,
            )?;
            Ok::<_, GuestMemoryError>((
                process_parameters_owner,
                process_parameters_range,
                command_line_owner,
                command_line_range,
            ))
        })
        .transpose()?;
    let protection = PageProtection {
        read: true,
        write: true,
        execute: false,
    };
    memory.commit(teb_owner, teb_range, protection)?;
    memory.commit(peb_owner, peb_range, protection)?;
    if let Some((
        process_parameters_owner,
        process_parameters_range,
        command_line_owner,
        command_line_range,
    )) = parameter_regions
    {
        memory.commit(
            process_parameters_owner,
            process_parameters_range,
            protection,
        )?;
        memory.commit(command_line_owner, command_line_range, protection)?;
    }
    zero_environment_page(memory, teb_owner, CONTROLLED_TEB_BASE)?;
    zero_environment_page(memory, peb_owner, CONTROLLED_PEB_BASE)?;
    if let Some((process_parameters_owner, _, command_line_owner, _)) = parameter_regions {
        zero_environment_page(
            memory,
            process_parameters_owner,
            CONTROLLED_PROCESS_PARAMETERS_BASE,
        )?;
        zero_environment_page(memory, command_line_owner, CONTROLLED_COMMAND_LINE_BASE)?;
    }

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

    let process_parameter_pages = parameter_regions
        .map(
            |(
                process_parameters_owner,
                process_parameters_range,
                command_line_owner,
                command_line_range,
            )| {
                initialize_process_parameters(
                    memory,
                    peb_owner,
                    process_parameters_owner,
                    command_line_owner,
                    process_parameters_range,
                    command_line_range,
                    command_line.expect("parameter regions require a command line"),
                )
            },
        )
        .transpose()?;

    let teb_content_sha256 = hash_environment_page(memory, teb_owner, CONTROLLED_TEB_BASE)?;
    let peb_content_sha256 = hash_environment_page(memory, peb_owner, CONTROLLED_PEB_BASE)?;
    if let Some(parameters) = &process_parameter_pages {
        memory.seal_owner(parameters.command_line_owner)?;
        memory.seal_owner(parameters.process_parameters_owner)?;
    }
    memory.seal_owner(peb_owner)?;
    let invalidation = memory.seal_owner(teb_owner)?;
    let process_parameters = process_parameter_pages.map(|parameters| InitialProcessParameters {
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
        command_line: command_line
            .expect("initialized process parameters require a command line")
            .to_owned(),
        command_line_utf16_units: parameters.command_line_utf16_units,
        command_line_length_bytes: parameters.command_line_length_bytes,
        command_line_maximum_length_bytes: parameters.command_line_maximum_length_bytes,
        process_parameters_content_sha256: parameters.process_parameters_content_sha256,
        command_line_content_sha256: parameters.command_line_content_sha256,
        owners_sealed: true,
        invalidation,
    });

    Ok(InitialProcessEnvironment {
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

fn initialize_process_parameters(
    memory: &mut GuestMemory,
    peb_owner: MemoryOwnerId,
    process_parameters_owner: MemoryOwnerId,
    command_line_owner: MemoryOwnerId,
    process_parameters_range: GuestRange,
    command_line_range: GuestRange,
    command_line: &str,
) -> Result<ProcessParameterPages, GuestMemoryError> {
    let mut command_line_bytes = Vec::new();
    let command_line_utf16: Vec<u16> = command_line.encode_utf16().collect();
    if command_line_utf16.contains(&0) {
        return Err(GuestMemoryError::runtime_defect(
            "process-bootstrap",
            "controlled-command-line-contains-nul",
            "controlled command line contains an embedded NUL",
        ));
    }
    for unit in &command_line_utf16 {
        command_line_bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let command_line_length_bytes = u16::try_from(command_line_bytes.len()).map_err(|_| {
        GuestMemoryError::runtime_defect(
            "process-bootstrap",
            "controlled-command-line-length-not-representable",
            "controlled command line length does not fit UNICODE_STRING.Length",
        )
    })?;
    command_line_bytes.extend_from_slice(&0u16.to_le_bytes());
    let command_line_maximum_length_bytes =
        u16::try_from(command_line_bytes.len()).map_err(|_| {
            GuestMemoryError::runtime_defect(
                "process-bootstrap",
                "controlled-command-line-maximum-length-not-representable",
                "controlled command line allocation does not fit UNICODE_STRING.MaximumLength",
            )
        })?;

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
        process_parameters_owner,
        command_line_address,
        &command_line_length_bytes.to_le_bytes(),
    )?;
    memory.write_initialization(
        process_parameters_owner,
        add_offset(command_line_address, 2)?,
        &command_line_maximum_length_bytes.to_le_bytes(),
    )?;
    memory.write_initialization(
        process_parameters_owner,
        add_offset(command_line_address, UNICODE_STRING_BUFFER_OFFSET)?,
        &CONTROLLED_COMMAND_LINE_BASE.0.to_le_bytes(),
    )?;
    memory.write_initialization(
        command_line_owner,
        CONTROLLED_COMMAND_LINE_BASE,
        &command_line_bytes,
    )?;

    Ok(ProcessParameterPages {
        process_parameters_owner,
        command_line_owner,
        process_parameters_range,
        command_line_range,
        command_line_utf16_units: command_line_utf16.len(),
        command_line_length_bytes,
        command_line_maximum_length_bytes,
        process_parameters_content_sha256: hash_environment_page(
            memory,
            process_parameters_owner,
            CONTROLLED_PROCESS_PARAMETERS_BASE,
        )?,
        command_line_content_sha256: hash_environment_page(
            memory,
            command_line_owner,
            CONTROLLED_COMMAND_LINE_BASE,
        )?,
    })
}

fn reserve_environment_page(
    memory: &mut GuestMemory,
    kind: MemoryRegionKind,
    address: GuestAddress,
) -> Result<(MemoryOwnerId, GuestRange), GuestMemoryError> {
    let owner = memory.create_owner(MemoryOwnerKind::Anonymous)?;
    match memory.reserve_fixed(owner, kind, address, CONTROLLED_ENVIRONMENT_PAGE_SIZE) {
        Ok(range) => Ok((owner, range)),
        Err(error) => {
            memory.destroy_owner(owner)?;
            Err(error)
        }
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
