use std::fmt;
use std::ops::Range;

use goblin::pe::characteristic::{
    IMAGE_FILE_32BIT_MACHINE, IMAGE_FILE_DLL, IMAGE_FILE_EXECUTABLE_IMAGE,
    IMAGE_FILE_LARGE_ADDRESS_AWARE, IMAGE_FILE_RELOCS_STRIPPED,
};
use goblin::pe::header::{COFF_MACHINE_X86, Header, PE_MAGIC, SIZEOF_COFF_HEADER, SIZEOF_PE_MAGIC};
use goblin::pe::optional_header::IMAGE_NT_OPTIONAL_HDR32_MAGIC;
use goblin::pe::section_table::{
    IMAGE_SCN_MEM_DISCARDABLE, IMAGE_SCN_MEM_EXECUTE, IMAGE_SCN_MEM_READ, IMAGE_SCN_MEM_WRITE,
    SectionTable,
};
use serde::Serialize;

use crate::snapshot::{ModuleIdentity, ModuleSnapshot};

const IMAGE_DIRECTORY_ENTRY_EXPORT: usize = 0;
const IMAGE_DIRECTORY_ENTRY_IMPORT: usize = 1;
const IMAGE_DIRECTORY_ENTRY_RESOURCE: usize = 2;
const IMAGE_DIRECTORY_ENTRY_EXCEPTION: usize = 3;
const IMAGE_DIRECTORY_ENTRY_SECURITY: usize = 4;
const IMAGE_DIRECTORY_ENTRY_BASERELOC: usize = 5;
const IMAGE_DIRECTORY_ENTRY_DEBUG: usize = 6;
const IMAGE_DIRECTORY_ENTRY_ARCHITECTURE: usize = 7;
const IMAGE_DIRECTORY_ENTRY_GLOBALPTR: usize = 8;
const IMAGE_DIRECTORY_ENTRY_TLS: usize = 9;
const IMAGE_DIRECTORY_ENTRY_LOAD_CONFIG: usize = 10;
const IMAGE_DIRECTORY_ENTRY_BOUND_IMPORT: usize = 11;
const IMAGE_DIRECTORY_ENTRY_IAT: usize = 12;
const IMAGE_DIRECTORY_ENTRY_DELAY_IMPORT: usize = 13;
const IMAGE_DIRECTORY_ENTRY_COM_DESCRIPTOR: usize = 14;
const IMAGE_DIRECTORY_ENTRY_RESERVED: usize = 15;

const IMAGE_DLLCHARACTERISTICS_DYNAMIC_BASE: u16 = 0x0040;
const IMAGE_DLLCHARACTERISTICS_NX_COMPAT: u16 = 0x0100;
const IMPORT_DESCRIPTOR_SIZE: u32 = 20;
const PE32_THUNK_SIZE: u32 = 4;
const RELOCATION_BLOCK_HEADER_SIZE: usize = 8;
const IMAGE_REL_BASED_ABSOLUTE: u8 = 0;
const IMAGE_REL_BASED_HIGHLOW: u8 = 3;

const DIRECTORY_NAMES: [&str; 16] = [
    "export",
    "import",
    "resource",
    "exception",
    "certificate",
    "base-relocation",
    "debug",
    "architecture",
    "global-pointer",
    "tls",
    "load-config",
    "bound-import",
    "iat",
    "delay-import",
    "clr-runtime-header",
    "reserved",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Rva(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pe32Limits {
    pub max_file_bytes: u64,
    pub max_image_bytes: u32,
    pub max_sections: usize,
    pub max_import_modules: usize,
    pub max_imports: usize,
    pub max_import_name_bytes: usize,
    pub max_relocation_blocks: usize,
    pub max_relocations: usize,
}

impl Default for Pe32Limits {
    fn default() -> Self {
        Self {
            max_file_bytes: 256 * 1024 * 1024,
            max_image_bytes: 512 * 1024 * 1024,
            max_sections: 96,
            max_import_modules: 512,
            max_imports: 65_536,
            max_import_name_bytes: 1_024,
            max_relocation_blocks: 65_536,
            max_relocations: 1_000_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LoaderErrorClass {
    InvalidInput,
    Unsupported,
    ResourceLimit,
    RuntimeDefect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderError {
    pub class: LoaderErrorClass,
    pub stage: &'static str,
    pub code: &'static str,
    pub message: String,
}

impl LoaderError {
    pub fn invalid_input(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(LoaderErrorClass::InvalidInput, stage, code, message)
    }

    pub fn unsupported(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(LoaderErrorClass::Unsupported, stage, code, message)
    }

    pub fn resource_limit(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(LoaderErrorClass::ResourceLimit, stage, code, message)
    }

    pub fn runtime_defect(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(LoaderErrorClass::RuntimeDefect, stage, code, message)
    }

    fn new(
        class: LoaderErrorClass,
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            class,
            stage,
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for LoaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} / {}: {}",
            self.stage, self.code, self.message
        )
    }
}

impl std::error::Error for LoaderError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderIngressReport {
    pub schema: &'static str,
    pub schema_version: u32,
    pub source: ModuleIdentity,
    pub limits: Pe32Limits,
    pub load_plan: Pe32LoadPlan,
    pub stop: UnsupportedPoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsupportedPoint {
    pub stage: &'static str,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pe32LoadPlan {
    pub image: ImagePlan,
    pub header_mapping: HeaderMappingPlan,
    pub sections: Vec<SectionMappingPlan>,
    pub data_directories: Vec<DataDirectoryPlan>,
    pub imports: Vec<ImportModulePlan>,
    pub relocations: RelocationPlan,
    pub features: FeatureInventory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImageKind {
    Executable,
    DynamicLibrary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePlan {
    pub format: &'static str,
    pub machine: &'static str,
    pub kind: ImageKind,
    pub subsystem: u16,
    pub coff_characteristics: u16,
    pub dll_characteristics: u16,
    pub preferred_image_base: u32,
    pub image_end: u64,
    pub size_of_image: u32,
    pub size_of_headers: u32,
    pub section_alignment: u32,
    pub file_alignment: u32,
    pub entry_point_rva: Rva,
    pub entry_point_address: u32,
    pub stack_reserve: u64,
    pub stack_commit: u64,
    pub heap_reserve: u64,
    pub heap_commit: u64,
    pub large_address_aware: bool,
    pub dynamic_base: bool,
    pub nx_compatible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderMappingPlan {
    pub source_offset: u32,
    pub destination_rva: Rva,
    pub copy_size: u32,
    pub mapped_size: u32,
    pub final_protection: MappingProtection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingProtection {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionMappingPlan {
    pub index: u16,
    pub name: String,
    pub destination_rva: Rva,
    pub virtual_size: u32,
    pub mapped_size: u32,
    pub source_offset: u32,
    pub source_size: u32,
    pub copy_size: u32,
    pub zero_fill_rva: Rva,
    pub zero_fill_size: u32,
    pub characteristics: u32,
    pub final_protection: MappingProtection,
    pub discardable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DataDirectoryAddressKind {
    Rva,
    FileOffset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataDirectoryPlan {
    pub index: u8,
    pub name: &'static str,
    pub address_kind: DataDirectoryAddressKind,
    pub address: u32,
    pub size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportModulePlan {
    pub name: String,
    pub descriptor_rva: Rva,
    pub lookup_table_rva: Rva,
    pub import_address_table_rva: Rva,
    pub time_date_stamp: u32,
    pub forwarder_chain: u32,
    pub symbols: Vec<ImportSymbolPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSymbolPlan {
    pub iat_rva: Rva,
    pub original_iat_value: u32,
    #[serde(flatten)]
    pub target: ImportTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ImportTarget {
    Name {
        name: String,
        hint: u16,
        #[serde(rename = "nameRva")]
        name_rva: Rva,
    },
    Ordinal {
        ordinal: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelocationPlan {
    pub directory_rva: Option<Rva>,
    pub directory_size: u32,
    pub block_count: usize,
    pub entry_count: usize,
    pub absolute_count: usize,
    pub highlow_count: usize,
    pub blocks: Vec<RelocationBlockPlan>,
    pub operations: Vec<RelocationOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelocationBlockPlan {
    pub page_rva: Rva,
    pub encoded_size: u32,
    pub entry_count: usize,
    pub absolute_count: usize,
    pub highlow_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelocationOperation {
    pub target_rva: Rva,
    pub width: u8,
    pub kind: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureInventory {
    pub exports: bool,
    pub resources: bool,
    pub exceptions: bool,
    pub certificates: bool,
    pub debug: bool,
    pub architecture: bool,
    pub global_pointer: bool,
    pub tls: bool,
    pub load_config: bool,
    pub bound_imports: bool,
    pub delay_imports: bool,
    pub clr: bool,
    pub relocations_stripped: bool,
}

#[derive(Debug)]
struct ValidatedSection<'a> {
    raw: &'a SectionTable,
    plan: SectionMappingPlan,
    virtual_range: Range<u64>,
    raw_range: Option<Range<u64>>,
}

#[derive(Debug)]
struct ValidatedLayout<'a> {
    bytes: &'a [u8],
    size_of_headers: u32,
    size_of_image: u32,
    sections: Vec<ValidatedSection<'a>>,
}

pub fn build_pe32_load_plan(
    snapshot: &ModuleSnapshot,
    limits: Pe32Limits,
) -> Result<LoaderIngressReport, LoaderError> {
    snapshot.verify_integrity()?;
    if snapshot.bytes().len() as u64 > limits.max_file_bytes {
        return Err(LoaderError::resource_limit(
            "pe-parse",
            "source-file-limit-exceeded",
            "owned source exceeds the configured PE32 file limit",
        ));
    }

    let bytes = snapshot.bytes();
    let header = Header::parse(bytes).map_err(|error| {
        LoaderError::invalid_input(
            "pe-parse",
            "pe-header-invalid",
            format!("Goblin rejected the DOS/PE/COFF/optional header: {error}"),
        )
    })?;
    if header.signature != PE_MAGIC {
        return Err(LoaderError::invalid_input(
            "pe-parse",
            "pe-signature-invalid",
            format!(
                "PE signature is {:#010x}, expected {PE_MAGIC:#010x}",
                header.signature
            ),
        ));
    }

    let coff = &header.coff_header;
    if coff.machine != COFF_MACHINE_X86 {
        return Err(LoaderError::unsupported(
            "pe-parse",
            "machine-not-i386",
            format!(
                "COFF machine {:#06x} is outside the PE32/i386 scope",
                coff.machine
            ),
        ));
    }
    if coff.characteristics & IMAGE_FILE_EXECUTABLE_IMAGE == 0 {
        return Err(LoaderError::invalid_input(
            "pe-parse",
            "not-an-executable-image",
            "COFF characteristics do not mark this file as an executable image",
        ));
    }
    if coff.characteristics & IMAGE_FILE_32BIT_MACHINE == 0 {
        return Err(LoaderError::invalid_input(
            "pe-parse",
            "missing-32bit-machine-flag",
            "i386 image does not carry IMAGE_FILE_32BIT_MACHINE",
        ));
    }
    if usize::from(coff.number_of_sections) > limits.max_sections {
        return Err(LoaderError::resource_limit(
            "pe-parse",
            "section-count-limit-exceeded",
            format!(
                "image declares {} sections; limit is {}",
                coff.number_of_sections, limits.max_sections
            ),
        ));
    }

    let optional = header.optional_header.as_ref().ok_or_else(|| {
        LoaderError::invalid_input(
            "pe-parse",
            "optional-header-missing",
            "executable image has no PE optional header",
        )
    })?;
    if optional.standard_fields.magic != IMAGE_NT_OPTIONAL_HDR32_MAGIC {
        return Err(LoaderError::unsupported(
            "pe-parse",
            "pe32-plus-unsupported",
            format!(
                "optional-header magic is {:#06x}; PE32 requires {IMAGE_NT_OPTIONAL_HDR32_MAGIC:#06x}",
                optional.standard_fields.magic
            ),
        ));
    }
    if coff.size_of_optional_header != 224 {
        return Err(LoaderError::invalid_input(
            "pe-parse",
            "pe32-optional-header-size-invalid",
            format!(
                "PE32 optional header has {} bytes; this loader requires 224",
                coff.size_of_optional_header
            ),
        ));
    }

    let windows = &optional.windows_fields;
    validate_alignment(windows.file_alignment, windows.section_alignment)?;
    if windows.size_of_image == 0 || windows.size_of_image > limits.max_image_bytes {
        return Err(LoaderError::resource_limit(
            "image-layout",
            "image-size-limit-exceeded",
            format!(
                "SizeOfImage is {} bytes; accepted range is 1..={} bytes",
                windows.size_of_image, limits.max_image_bytes
            ),
        ));
    }
    if windows.size_of_image % windows.section_alignment != 0 {
        return Err(LoaderError::invalid_input(
            "image-layout",
            "image-size-not-section-aligned",
            "SizeOfImage is not aligned to SectionAlignment",
        ));
    }
    if windows.size_of_headers == 0
        || windows.size_of_headers % windows.file_alignment != 0
        || u64::from(windows.size_of_headers) > bytes.len() as u64
    {
        return Err(LoaderError::invalid_input(
            "image-layout",
            "header-size-invalid",
            "SizeOfHeaders is zero, misaligned, or extends beyond the immutable source",
        ));
    }
    if windows.image_base > u64::from(u32::MAX) {
        return Err(LoaderError::unsupported(
            "image-layout",
            "pe32-image-base-out-of-range",
            "PE32 ImageBase does not fit a 32-bit guest address",
        ));
    }
    let image_base = windows.image_base as u32;
    if !image_base.is_multiple_of(0x1_0000) {
        return Err(LoaderError::invalid_input(
            "image-layout",
            "image-base-not-64k-aligned",
            "ImageBase is not aligned to the Windows 64 KiB allocation boundary",
        ));
    }
    let image_end = u64::from(image_base) + u64::from(windows.size_of_image);
    if image_end > u64::from(u32::MAX) + 1 {
        return Err(LoaderError::invalid_input(
            "image-layout",
            "image-address-range-overflow",
            "preferred image range extends beyond the 32-bit guest address space",
        ));
    }
    if windows.size_of_stack_commit > windows.size_of_stack_reserve
        || windows.size_of_heap_commit > windows.size_of_heap_reserve
    {
        return Err(LoaderError::invalid_input(
            "image-layout",
            "commit-exceeds-reserve",
            "stack or heap commit size exceeds its corresponding reserve size",
        ));
    }

    let section_table_offset = u64::from(header.dos_header.pe_pointer)
        + SIZEOF_PE_MAGIC as u64
        + SIZEOF_COFF_HEADER as u64
        + u64::from(coff.size_of_optional_header);
    let section_table_size = u64::from(coff.number_of_sections) * 40;
    if section_table_offset + section_table_size > u64::from(windows.size_of_headers) {
        return Err(LoaderError::invalid_input(
            "image-layout",
            "section-table-outside-headers",
            "section table extends beyond SizeOfHeaders",
        ));
    }
    let mut section_offset = usize::try_from(section_table_offset).map_err(|_| {
        LoaderError::invalid_input(
            "pe-parse",
            "section-table-offset-overflow",
            "section table offset does not fit the host index type",
        )
    })?;
    let section_headers = coff.sections(bytes, &mut section_offset).map_err(|error| {
        LoaderError::invalid_input(
            "pe-parse",
            "section-table-invalid",
            format!("Goblin rejected the section table: {error}"),
        )
    })?;

    let layout = validate_sections(
        bytes,
        &section_headers,
        windows.size_of_headers,
        windows.size_of_image,
        windows.file_alignment,
        windows.section_alignment,
    )?;
    let directories = validate_directories(optional, &layout)?;
    let features = feature_inventory(optional, coff.characteristics);

    if features.clr {
        return Err(LoaderError::unsupported(
            "feature-preflight",
            "clr-image-unsupported",
            "CLR runtime header is present; managed images are outside the active scope",
        ));
    }
    if features.tls {
        return Err(LoaderError::unsupported(
            "feature-preflight",
            "tls-bootstrap-unimplemented",
            "TLS template and callback planning are not implemented at this checkpoint",
        ));
    }
    if features.delay_imports {
        return Err(LoaderError::unsupported(
            "feature-preflight",
            "delay-import-unimplemented",
            "delay-import descriptor planning is not implemented at this checkpoint",
        ));
    }

    let imports = parse_imports(optional, &layout, &limits)?;
    let relocations = parse_relocations(optional, &layout, &limits)?;
    if features.relocations_stripped && relocations.entry_count != 0 {
        return Err(LoaderError::invalid_input(
            "base-relocations",
            "relocations-present-but-stripped",
            "COFF flags mark relocations stripped while a base relocation plan is present",
        ));
    }

    let entry_rva = optional.standard_fields.address_of_entry_point;
    let kind = if coff.characteristics & IMAGE_FILE_DLL != 0 {
        ImageKind::DynamicLibrary
    } else {
        ImageKind::Executable
    };
    if entry_rva == 0 && kind == ImageKind::Executable {
        return Err(LoaderError::invalid_input(
            "image-layout",
            "executable-entry-point-missing",
            "executable image has a zero entry point RVA",
        ));
    }
    if entry_rva != 0 {
        let section = layout
            .section_containing_mapped(entry_rva, 1)
            .ok_or_else(|| {
                LoaderError::invalid_input(
                    "image-layout",
                    "entry-point-not-mapped",
                    format!("entry point RVA {entry_rva:#010x} is outside mapped sections"),
                )
            })?;
        if !section.plan.final_protection.execute {
            return Err(LoaderError::invalid_input(
                "image-layout",
                "entry-point-not-executable",
                format!(
                    "entry point RVA {entry_rva:#010x} lands in non-executable section {}",
                    section.plan.name
                ),
            ));
        }
        layout.file_span(entry_rva, 1, "entry-point-not-file-backed")?;
    }
    let entry_point_address = image_base.checked_add(entry_rva).ok_or_else(|| {
        LoaderError::invalid_input(
            "image-layout",
            "entry-point-address-overflow",
            "ImageBase plus AddressOfEntryPoint overflows a 32-bit guest address",
        )
    })?;

    let header_mapping = HeaderMappingPlan {
        source_offset: 0,
        destination_rva: Rva(0),
        copy_size: windows.size_of_headers,
        mapped_size: align_up(windows.size_of_headers, windows.section_alignment)?,
        final_protection: MappingProtection {
            read: true,
            write: false,
            execute: false,
        },
    };
    let sections = layout
        .sections
        .iter()
        .map(|section| section.plan.clone())
        .collect();

    let load_plan = Pe32LoadPlan {
        image: ImagePlan {
            format: "pe32",
            machine: "i386",
            kind,
            subsystem: windows.subsystem,
            coff_characteristics: coff.characteristics,
            dll_characteristics: windows.dll_characteristics,
            preferred_image_base: image_base,
            image_end,
            size_of_image: windows.size_of_image,
            size_of_headers: windows.size_of_headers,
            section_alignment: windows.section_alignment,
            file_alignment: windows.file_alignment,
            entry_point_rva: Rva(entry_rva),
            entry_point_address,
            stack_reserve: windows.size_of_stack_reserve,
            stack_commit: windows.size_of_stack_commit,
            heap_reserve: windows.size_of_heap_reserve,
            heap_commit: windows.size_of_heap_commit,
            large_address_aware: coff.characteristics & IMAGE_FILE_LARGE_ADDRESS_AWARE != 0,
            dynamic_base: windows.dll_characteristics & IMAGE_DLLCHARACTERISTICS_DYNAMIC_BASE != 0,
            nx_compatible: windows.dll_characteristics & IMAGE_DLLCHARACTERISTICS_NX_COMPAT != 0,
        },
        header_mapping,
        sections,
        data_directories: directories,
        imports,
        relocations,
        features,
    };

    Ok(LoaderIngressReport {
        schema: "vine.pe32-loader-ingress",
        schema_version: 1,
        source: snapshot.identity(),
        limits,
        load_plan,
        stop: UnsupportedPoint {
            stage: "process-bootstrap",
            code: "peb-teb-process-parameters-unimplemented",
            message: "the immutable image plan is valid; guest memory materialization, import binding, PEB/TEB/process parameters, initial stack, and entry execution were not attempted",
        },
    })
}

fn validate_alignment(file_alignment: u32, section_alignment: u32) -> Result<(), LoaderError> {
    if file_alignment == 0
        || section_alignment == 0
        || !file_alignment.is_power_of_two()
        || !section_alignment.is_power_of_two()
    {
        return Err(LoaderError::invalid_input(
            "image-layout",
            "invalid-pe-alignment",
            "FileAlignment and SectionAlignment must be non-zero powers of two",
        ));
    }
    if section_alignment < file_alignment {
        return Err(LoaderError::invalid_input(
            "image-layout",
            "section-alignment-below-file-alignment",
            "SectionAlignment must be greater than or equal to FileAlignment",
        ));
    }

    if section_alignment < 0x1000 {
        if file_alignment != section_alignment {
            return Err(LoaderError::invalid_input(
                "image-layout",
                "low-section-alignment-mismatch",
                "when SectionAlignment is below 4096, FileAlignment must match it",
            ));
        }
        return Err(LoaderError::unsupported(
            "image-layout",
            "low-section-alignment-materialization-unimplemented",
            format!(
                "valid low-alignment PE uses FileAlignment {file_alignment:#x} and SectionAlignment {section_alignment:#x}; Vine's 4 KiB GuestMemory backing does not yet materialize sub-page section mappings"
            ),
        ));
    } else if !(0x200..=0x1_0000).contains(&file_alignment) {
        return Err(LoaderError::invalid_input(
            "image-layout",
            "file-alignment-out-of-range",
            "FileAlignment must be between 512 and 65536 for page-aligned images",
        ));
    }
    Ok(())
}

fn validate_sections<'a>(
    bytes: &'a [u8],
    sections: &'a [SectionTable],
    size_of_headers: u32,
    size_of_image: u32,
    file_alignment: u32,
    section_alignment: u32,
) -> Result<ValidatedLayout<'a>, LoaderError> {
    let header_mapped_size = align_up(size_of_headers, section_alignment)?;
    if header_mapped_size > size_of_image {
        return Err(LoaderError::invalid_input(
            "section-layout",
            "mapped-headers-exceed-image",
            "section-aligned headers extend beyond SizeOfImage",
        ));
    }
    let mut validated: Vec<ValidatedSection<'a>> = Vec::with_capacity(sections.len());
    let mut previous_virtual_address = None;

    for (index, section) in sections.iter().enumerate() {
        if let Some(previous) = previous_virtual_address
            && section.virtual_address <= previous
        {
            return Err(LoaderError::invalid_input(
                "section-layout",
                "sections-not-in-rva-order",
                "section RVAs must be strictly increasing in table order",
            ));
        }
        previous_virtual_address = Some(section.virtual_address);

        if section.virtual_address % section_alignment != 0 {
            return Err(LoaderError::invalid_input(
                "section-layout",
                "section-rva-misaligned",
                format!("section {} RVA is not SectionAlignment-aligned", index + 1),
            ));
        }
        if section.size_of_raw_data != 0
            && (section.pointer_to_raw_data == 0
                || section.pointer_to_raw_data % file_alignment != 0
                || section.size_of_raw_data % file_alignment != 0)
        {
            return Err(LoaderError::invalid_input(
                "section-layout",
                "section-raw-range-misaligned",
                format!(
                    "section {} has a zero or FileAlignment-misaligned raw range",
                    index + 1
                ),
            ));
        }

        let logical_size = section.virtual_size.max(section.size_of_raw_data);
        let mapped_size = if logical_size == 0 {
            0
        } else {
            align_up(logical_size, section_alignment)?
        };
        let virtual_end = u64::from(section.virtual_address) + u64::from(mapped_size);
        if section.virtual_address < header_mapped_size || virtual_end > u64::from(size_of_image) {
            return Err(LoaderError::invalid_input(
                "section-layout",
                "section-virtual-range-invalid",
                format!(
                    "section {} overlaps mapped headers or extends beyond SizeOfImage",
                    index + 1
                ),
            ));
        }

        let raw_range = if section.size_of_raw_data == 0 {
            None
        } else {
            let start = u64::from(section.pointer_to_raw_data);
            let end = start + u64::from(section.size_of_raw_data);
            if start < u64::from(size_of_headers) || end > bytes.len() as u64 {
                return Err(LoaderError::invalid_input(
                    "section-layout",
                    "section-raw-range-invalid",
                    format!(
                        "section {} overlaps headers or extends beyond the immutable source",
                        index + 1
                    ),
                ));
            }
            Some(start..end)
        };

        let name = section.name().map_err(|error| {
            LoaderError::invalid_input(
                "section-layout",
                "section-name-invalid",
                format!("section {} has an invalid name: {error}", index + 1),
            )
        })?;
        let zero_fill_rva = section
            .virtual_address
            .checked_add(section.size_of_raw_data)
            .ok_or_else(|| {
                LoaderError::invalid_input(
                    "section-layout",
                    "section-zero-fill-overflow",
                    "section zero-fill RVA overflows 32 bits",
                )
            })?;
        let protection = MappingProtection {
            read: section.characteristics & IMAGE_SCN_MEM_READ != 0,
            write: section.characteristics & IMAGE_SCN_MEM_WRITE != 0,
            execute: section.characteristics & IMAGE_SCN_MEM_EXECUTE != 0,
        };
        let plan_index = u16::try_from(index + 1).map_err(|_| {
            LoaderError::resource_limit(
                "section-layout",
                "section-index-not-representable",
                "section index does not fit the version 1 load-plan schema",
            )
        })?;
        validated.push(ValidatedSection {
            raw: section,
            plan: SectionMappingPlan {
                index: plan_index,
                name: name.to_owned(),
                destination_rva: Rva(section.virtual_address),
                virtual_size: section.virtual_size,
                mapped_size,
                source_offset: section.pointer_to_raw_data,
                source_size: section.size_of_raw_data,
                copy_size: section.size_of_raw_data,
                zero_fill_rva: Rva(zero_fill_rva),
                zero_fill_size: mapped_size - section.size_of_raw_data,
                characteristics: section.characteristics,
                final_protection: protection,
                discardable: section.characteristics & IMAGE_SCN_MEM_DISCARDABLE != 0,
            },
            virtual_range: u64::from(section.virtual_address)..virtual_end,
            raw_range,
        });
    }

    for left in 0..validated.len() {
        for right in left + 1..validated.len() {
            if ranges_overlap(
                &validated[left].virtual_range,
                &validated[right].virtual_range,
            ) {
                return Err(LoaderError::invalid_input(
                    "section-layout",
                    "section-virtual-ranges-overlap",
                    format!(
                        "sections {} and {} have overlapping mapped ranges",
                        left + 1,
                        right + 1
                    ),
                ));
            }
            if let (Some(left_raw), Some(right_raw)) = (
                validated[left].raw_range.as_ref(),
                validated[right].raw_range.as_ref(),
            ) && ranges_overlap(left_raw, right_raw)
            {
                return Err(LoaderError::invalid_input(
                    "section-layout",
                    "section-raw-ranges-overlap",
                    format!(
                        "sections {} and {} have overlapping source ranges",
                        left + 1,
                        right + 1
                    ),
                ));
            }
        }
    }

    Ok(ValidatedLayout {
        bytes,
        size_of_headers,
        size_of_image,
        sections: validated,
    })
}

fn validate_directories(
    optional: &goblin::pe::optional_header::OptionalHeader,
    layout: &ValidatedLayout<'_>,
) -> Result<Vec<DataDirectoryPlan>, LoaderError> {
    let mut plans = Vec::new();
    for (index, entry) in optional
        .data_directories
        .data_directories
        .iter()
        .enumerate()
    {
        let Some((_, directory)) = entry else {
            continue;
        };
        let zero_sized_global_pointer = index == IMAGE_DIRECTORY_ENTRY_GLOBALPTR
            && directory.virtual_address != 0
            && directory.size == 0;
        if directory.virtual_address == 0 || (directory.size == 0 && !zero_sized_global_pointer) {
            return Err(LoaderError::invalid_input(
                "data-directories",
                "partial-data-directory",
                format!(
                    "{} directory has a zero address or size, but not both",
                    DIRECTORY_NAMES[index]
                ),
            ));
        }

        let address_kind = if index == IMAGE_DIRECTORY_ENTRY_SECURITY {
            let end = u64::from(directory.virtual_address) + u64::from(directory.size);
            if directory.virtual_address % 8 != 0 || end > layout.bytes.len() as u64 {
                return Err(LoaderError::invalid_input(
                    "data-directories",
                    "certificate-directory-range-invalid",
                    "certificate table is misaligned or extends beyond the immutable source",
                ));
            }
            DataDirectoryAddressKind::FileOffset
        } else {
            let mapped_size = directory.size.max(1);
            if !layout.mapped_contains(directory.virtual_address, mapped_size) {
                return Err(LoaderError::invalid_input(
                    "data-directories",
                    "data-directory-not-mapped",
                    format!(
                        "{} directory RVA range is outside mapped headers/sections",
                        DIRECTORY_NAMES[index]
                    ),
                ));
            }
            layout.file_span(
                directory.virtual_address,
                mapped_size,
                "data-directory-not-file-backed",
            )?;
            DataDirectoryAddressKind::Rva
        };

        if index == IMAGE_DIRECTORY_ENTRY_RESERVED {
            return Err(LoaderError::unsupported(
                "data-directories",
                "reserved-data-directory-present",
                "reserved PE data directory entry 15 is non-empty",
            ));
        }
        plans.push(DataDirectoryPlan {
            index: index as u8,
            name: DIRECTORY_NAMES[index],
            address_kind,
            address: directory.virtual_address,
            size: directory.size,
        });
    }
    Ok(plans)
}

fn feature_inventory(
    optional: &goblin::pe::optional_header::OptionalHeader,
    coff_characteristics: u16,
) -> FeatureInventory {
    let present = |index: usize| optional.data_directories.data_directories[index].is_some();
    FeatureInventory {
        exports: present(IMAGE_DIRECTORY_ENTRY_EXPORT),
        resources: present(IMAGE_DIRECTORY_ENTRY_RESOURCE),
        exceptions: present(IMAGE_DIRECTORY_ENTRY_EXCEPTION),
        certificates: present(IMAGE_DIRECTORY_ENTRY_SECURITY),
        debug: present(IMAGE_DIRECTORY_ENTRY_DEBUG),
        architecture: present(IMAGE_DIRECTORY_ENTRY_ARCHITECTURE),
        global_pointer: present(IMAGE_DIRECTORY_ENTRY_GLOBALPTR),
        tls: present(IMAGE_DIRECTORY_ENTRY_TLS),
        load_config: present(IMAGE_DIRECTORY_ENTRY_LOAD_CONFIG),
        bound_imports: present(IMAGE_DIRECTORY_ENTRY_BOUND_IMPORT),
        delay_imports: present(IMAGE_DIRECTORY_ENTRY_DELAY_IMPORT),
        clr: present(IMAGE_DIRECTORY_ENTRY_COM_DESCRIPTOR),
        relocations_stripped: coff_characteristics & IMAGE_FILE_RELOCS_STRIPPED != 0,
    }
}

fn parse_imports(
    optional: &goblin::pe::optional_header::OptionalHeader,
    layout: &ValidatedLayout<'_>,
    limits: &Pe32Limits,
) -> Result<Vec<ImportModulePlan>, LoaderError> {
    let Some((_, import_directory)) =
        optional.data_directories.data_directories[IMAGE_DIRECTORY_ENTRY_IMPORT]
    else {
        return Ok(Vec::new());
    };
    if import_directory.size % IMPORT_DESCRIPTOR_SIZE != 0 {
        return Err(LoaderError::invalid_input(
            "imports",
            "import-directory-size-invalid",
            "import directory size is not a whole number of 20-byte descriptors",
        ));
    }
    let descriptor_capacity = import_directory.size / IMPORT_DESCRIPTOR_SIZE;
    if usize::try_from(descriptor_capacity).unwrap_or(usize::MAX)
        > limits.max_import_modules.saturating_add(1)
    {
        return Err(LoaderError::resource_limit(
            "imports",
            "import-module-limit-exceeded",
            format!(
                "import directory can contain {descriptor_capacity} descriptors; module limit is {}",
                limits.max_import_modules
            ),
        ));
    }

    let iat_directory = optional.data_directories.data_directories[IMAGE_DIRECTORY_ENTRY_IAT]
        .map(|(_, directory)| directory);
    let mut modules = Vec::new();
    let mut total_imports = 0usize;
    let mut terminated = false;

    for descriptor_index in 0..descriptor_capacity {
        let descriptor_rva = checked_add_mul(
            import_directory.virtual_address,
            descriptor_index,
            IMPORT_DESCRIPTOR_SIZE,
            "import-descriptor-rva-overflow",
        )?;
        let descriptor = layout.file_span(
            descriptor_rva,
            IMPORT_DESCRIPTOR_SIZE,
            "import-descriptor-not-file-backed",
        )?;
        let lookup_table_rva = read_u32(descriptor, 0)?;
        let time_date_stamp = read_u32(descriptor, 4)?;
        let forwarder_chain = read_u32(descriptor, 8)?;
        let name_rva = read_u32(descriptor, 12)?;
        let import_address_table_rva = read_u32(descriptor, 16)?;
        if [
            lookup_table_rva,
            time_date_stamp,
            forwarder_chain,
            name_rva,
            import_address_table_rva,
        ]
        .iter()
        .all(|value| *value == 0)
        {
            terminated = true;
            break;
        }
        if name_rva == 0 || import_address_table_rva == 0 {
            return Err(LoaderError::invalid_input(
                "imports",
                "import-descriptor-invalid",
                "non-terminal import descriptor has a zero name or IAT RVA",
            ));
        }
        if modules.len() >= limits.max_import_modules {
            return Err(LoaderError::resource_limit(
                "imports",
                "import-module-limit-exceeded",
                format!(
                    "import module limit {} was reached",
                    limits.max_import_modules
                ),
            ));
        }

        let module_name = layout.read_ascii_z(
            name_rva,
            limits.max_import_name_bytes,
            "import-module-name-invalid",
        )?;
        let effective_lookup_rva = if lookup_table_rva == 0 {
            import_address_table_rva
        } else {
            lookup_table_rva
        };
        let mut symbols = Vec::new();
        let max_module_imports = limits.max_imports.saturating_sub(total_imports);
        let mut thunk_index = 0usize;
        let mut thunk_terminated = false;

        while thunk_index <= max_module_imports {
            let thunk_index_u32 = u32::try_from(thunk_index).map_err(|_| {
                LoaderError::resource_limit(
                    "imports",
                    "import-index-not-representable",
                    "import thunk index does not fit a PE32 RVA calculation",
                )
            })?;
            let lookup_rva = checked_add_mul(
                effective_lookup_rva,
                thunk_index_u32,
                PE32_THUNK_SIZE,
                "import-lookup-rva-overflow",
            )?;
            let lookup = layout.read_u32_rva(lookup_rva, "import-lookup-not-file-backed")?;
            let iat_rva = checked_add_mul(
                import_address_table_rva,
                thunk_index_u32,
                PE32_THUNK_SIZE,
                "import-iat-rva-overflow",
            )?;
            if let Some(iat) = iat_directory
                && !range_contains(iat.virtual_address, iat.size, iat_rva, PE32_THUNK_SIZE)
            {
                return Err(LoaderError::invalid_input(
                    "imports",
                    "import-slot-outside-iat-directory",
                    format!("module {module_name} has an IAT slot outside the IAT directory"),
                ));
            }
            let original_iat_value = layout.read_u32_rva(iat_rva, "import-iat-not-file-backed")?;

            if lookup == 0 {
                if original_iat_value != 0 {
                    return Err(LoaderError::invalid_input(
                        "imports",
                        "import-iat-terminator-mismatch",
                        format!("module {module_name} lookup table terminates before its IAT"),
                    ));
                }
                thunk_terminated = true;
                break;
            }
            if thunk_index == max_module_imports {
                break;
            }
            let target = if lookup & 0x8000_0000 != 0 {
                if lookup & 0x7fff_0000 != 0 {
                    return Err(LoaderError::invalid_input(
                        "imports",
                        "ordinal-import-reserved-bits-set",
                        format!("module {module_name} has malformed ordinal thunk {lookup:#010x}"),
                    ));
                }
                ImportTarget::Ordinal {
                    ordinal: lookup as u16,
                }
            } else {
                let name_rva = lookup & 0x7fff_ffff;
                let hint = layout.read_u16_rva(name_rva, "import-hint-not-file-backed")?;
                let string_rva = name_rva.checked_add(2).ok_or_else(|| {
                    LoaderError::invalid_input(
                        "imports",
                        "import-name-rva-overflow",
                        "import hint/name RVA overflows 32 bits",
                    )
                })?;
                let name = layout.read_ascii_z(
                    string_rva,
                    limits.max_import_name_bytes,
                    "import-symbol-name-invalid",
                )?;
                ImportTarget::Name {
                    name,
                    hint,
                    name_rva: Rva(name_rva),
                }
            };
            symbols.push(ImportSymbolPlan {
                iat_rva: Rva(iat_rva),
                original_iat_value,
                target,
            });
            thunk_index += 1;
        }

        if !thunk_terminated {
            return Err(LoaderError::resource_limit(
                "imports",
                "import-symbol-limit-or-terminator-missing",
                format!(
                    "module {module_name} did not terminate before the global import limit {}",
                    limits.max_imports
                ),
            ));
        }
        total_imports += symbols.len();
        modules.push(ImportModulePlan {
            name: module_name,
            descriptor_rva: Rva(descriptor_rva),
            lookup_table_rva: Rva(effective_lookup_rva),
            import_address_table_rva: Rva(import_address_table_rva),
            time_date_stamp,
            forwarder_chain,
            symbols,
        });
    }

    if !terminated {
        return Err(LoaderError::invalid_input(
            "imports",
            "import-directory-terminator-missing",
            "import directory has no all-zero descriptor within its declared size",
        ));
    }
    Ok(modules)
}

fn parse_relocations(
    optional: &goblin::pe::optional_header::OptionalHeader,
    layout: &ValidatedLayout<'_>,
    limits: &Pe32Limits,
) -> Result<RelocationPlan, LoaderError> {
    let Some((_, directory)) =
        optional.data_directories.data_directories[IMAGE_DIRECTORY_ENTRY_BASERELOC]
    else {
        return Ok(RelocationPlan {
            directory_rva: None,
            directory_size: 0,
            block_count: 0,
            entry_count: 0,
            absolute_count: 0,
            highlow_count: 0,
            blocks: Vec::new(),
            operations: Vec::new(),
        });
    };
    let bytes = layout.file_span(
        directory.virtual_address,
        directory.size,
        "relocation-directory-not-file-backed",
    )?;
    let mut offset = 0usize;
    let mut blocks = Vec::new();
    let mut operations = Vec::new();
    let mut entry_count = 0usize;
    let mut absolute_count = 0usize;

    while offset < bytes.len() {
        if blocks.len() >= limits.max_relocation_blocks {
            return Err(LoaderError::resource_limit(
                "base-relocations",
                "relocation-block-limit-exceeded",
                format!(
                    "base relocation block limit {} was reached",
                    limits.max_relocation_blocks
                ),
            ));
        }
        if bytes.len() - offset < RELOCATION_BLOCK_HEADER_SIZE {
            return Err(LoaderError::invalid_input(
                "base-relocations",
                "relocation-block-header-truncated",
                "base relocation directory ends inside an 8-byte block header",
            ));
        }
        let page_rva = read_u32(bytes, offset)?;
        let encoded_size = read_u32(bytes, offset + 4)?;
        let block_size = usize::try_from(encoded_size).map_err(|_| {
            LoaderError::invalid_input(
                "base-relocations",
                "relocation-block-size-overflow",
                "base relocation block size does not fit the host index type",
            )
        })?;
        if block_size < RELOCATION_BLOCK_HEADER_SIZE
            || block_size % 2 != 0
            || block_size > bytes.len() - offset
        {
            return Err(LoaderError::invalid_input(
                "base-relocations",
                "relocation-block-size-invalid",
                "base relocation block size is too small, odd, or exceeds the directory",
            ));
        }
        if !page_rva.is_multiple_of(0x1000) {
            return Err(LoaderError::invalid_input(
                "base-relocations",
                "relocation-page-rva-misaligned",
                format!("base relocation page RVA {page_rva:#010x} is not 4 KiB aligned"),
            ));
        }

        let word_count = (block_size - RELOCATION_BLOCK_HEADER_SIZE) / 2;
        if entry_count.saturating_add(word_count) > limits.max_relocations {
            return Err(LoaderError::resource_limit(
                "base-relocations",
                "relocation-entry-limit-exceeded",
                format!(
                    "base relocation entry limit {} would be exceeded",
                    limits.max_relocations
                ),
            ));
        }
        let mut block_absolute = 0usize;
        let mut block_highlow = 0usize;
        for word_index in 0..word_count {
            let word_offset = offset + RELOCATION_BLOCK_HEADER_SIZE + word_index * 2;
            let word = read_u16(bytes, word_offset)?;
            let relocation_type = (word >> 12) as u8;
            let page_offset = u32::from(word & 0x0fff);
            match relocation_type {
                IMAGE_REL_BASED_ABSOLUTE => {
                    block_absolute += 1;
                    absolute_count += 1;
                }
                IMAGE_REL_BASED_HIGHLOW => {
                    let target_rva = page_rva.checked_add(page_offset).ok_or_else(|| {
                        LoaderError::invalid_input(
                            "base-relocations",
                            "relocation-target-rva-overflow",
                            "HIGHLOW relocation target RVA overflows 32 bits",
                        )
                    })?;
                    if !layout.mapped_contains(target_rva, 4) {
                        return Err(LoaderError::invalid_input(
                            "base-relocations",
                            "relocation-target-not-mapped",
                            format!(
                                "HIGHLOW relocation target {target_rva:#010x} is not fully mapped"
                            ),
                        ));
                    }
                    operations.push(RelocationOperation {
                        target_rva: Rva(target_rva),
                        width: 4,
                        kind: "highlow",
                    });
                    block_highlow += 1;
                }
                unsupported => {
                    return Err(LoaderError::unsupported(
                        "base-relocations",
                        "relocation-type-unsupported",
                        format!(
                            "PE32 base relocation type {unsupported} is not ABSOLUTE or HIGHLOW"
                        ),
                    ));
                }
            }
        }
        entry_count += word_count;
        blocks.push(RelocationBlockPlan {
            page_rva: Rva(page_rva),
            encoded_size,
            entry_count: word_count,
            absolute_count: block_absolute,
            highlow_count: block_highlow,
        });
        offset += block_size;
    }

    Ok(RelocationPlan {
        directory_rva: Some(Rva(directory.virtual_address)),
        directory_size: directory.size,
        block_count: blocks.len(),
        entry_count,
        absolute_count,
        highlow_count: operations.len(),
        blocks,
        operations,
    })
}

impl ValidatedLayout<'_> {
    fn section_containing_mapped(&self, rva: u32, size: u32) -> Option<&ValidatedSection<'_>> {
        let range = checked_u64_range(rva, size)?;
        self.sections
            .iter()
            .find(|section| contains_range(&section.virtual_range, &range))
    }

    fn mapped_contains(&self, rva: u32, size: u32) -> bool {
        let Some(range) = checked_u64_range(rva, size) else {
            return false;
        };
        if range.end <= u64::from(self.size_of_headers) {
            return true;
        }
        self.sections
            .iter()
            .any(|section| contains_range(&section.virtual_range, &range))
            && range.end <= u64::from(self.size_of_image)
    }

    fn file_span(&self, rva: u32, size: u32, code: &'static str) -> Result<&[u8], LoaderError> {
        let range = checked_u64_range(rva, size).ok_or_else(|| {
            LoaderError::invalid_input("rva-resolution", code, "RVA range overflows 32 bits")
        })?;
        if range.end <= u64::from(self.size_of_headers) {
            let start = usize::try_from(range.start).map_err(|_| {
                LoaderError::resource_limit(
                    "rva-resolution",
                    "header-offset-not-representable",
                    "header source offset does not fit the host index type",
                )
            })?;
            let end = usize::try_from(range.end).map_err(|_| {
                LoaderError::resource_limit(
                    "rva-resolution",
                    "header-end-not-representable",
                    "header source end does not fit the host index type",
                )
            })?;
            return self.bytes.get(start..end).ok_or_else(|| {
                LoaderError::invalid_input(
                    "rva-resolution",
                    code,
                    "header RVA range extends beyond the immutable source",
                )
            });
        }

        for section in &self.sections {
            let section_rva = u64::from(section.raw.virtual_address);
            let raw_end_rva = section_rva + u64::from(section.raw.size_of_raw_data);
            if range.start >= section_rva && range.end <= raw_end_rva {
                let delta = range.start - section_rva;
                let source_start = u64::from(section.raw.pointer_to_raw_data) + delta;
                let source_end = source_start + u64::from(size);
                let start = usize::try_from(source_start).map_err(|_| {
                    LoaderError::invalid_input(
                        "rva-resolution",
                        code,
                        "resolved source offset does not fit the host index type",
                    )
                })?;
                let end = usize::try_from(source_end).map_err(|_| {
                    LoaderError::invalid_input(
                        "rva-resolution",
                        code,
                        "resolved source end does not fit the host index type",
                    )
                })?;
                return self.bytes.get(start..end).ok_or_else(|| {
                    LoaderError::invalid_input(
                        "rva-resolution",
                        code,
                        "resolved RVA range extends beyond the immutable source",
                    )
                });
            }
        }
        Err(LoaderError::invalid_input(
            "rva-resolution",
            code,
            format!("RVA range {rva:#010x}..+{size:#x} is not file-backed"),
        ))
    }

    fn file_tail(&self, rva: u32, code: &'static str) -> Result<&[u8], LoaderError> {
        if rva < self.size_of_headers {
            let start = usize::try_from(rva).map_err(|_| {
                LoaderError::resource_limit(
                    "rva-resolution",
                    "header-string-offset-not-representable",
                    "header string offset does not fit the host index type",
                )
            })?;
            let end = usize::try_from(self.size_of_headers).map_err(|_| {
                LoaderError::resource_limit(
                    "rva-resolution",
                    "header-string-end-not-representable",
                    "header string end does not fit the host index type",
                )
            })?;
            return self.bytes.get(start..end).ok_or_else(|| {
                LoaderError::invalid_input(
                    "rva-resolution",
                    code,
                    "header string RVA extends beyond the immutable source",
                )
            });
        }
        for section in &self.sections {
            let start_rva = section.raw.virtual_address;
            let Some(end_rva) = start_rva.checked_add(section.raw.size_of_raw_data) else {
                continue;
            };
            if (start_rva..end_rva).contains(&rva) {
                let delta = u64::from(rva - start_rva);
                let source_start = u64::from(section.raw.pointer_to_raw_data) + delta;
                let source_end = u64::from(section.raw.pointer_to_raw_data)
                    + u64::from(section.raw.size_of_raw_data);
                let start = usize::try_from(source_start).map_err(|_| {
                    LoaderError::invalid_input(
                        "rva-resolution",
                        code,
                        "string source offset does not fit the host index type",
                    )
                })?;
                let end = usize::try_from(source_end).map_err(|_| {
                    LoaderError::invalid_input(
                        "rva-resolution",
                        code,
                        "string source end does not fit the host index type",
                    )
                })?;
                return self.bytes.get(start..end).ok_or_else(|| {
                    LoaderError::invalid_input(
                        "rva-resolution",
                        code,
                        "string RVA resolves beyond the immutable source",
                    )
                });
            }
        }
        Err(LoaderError::invalid_input(
            "rva-resolution",
            code,
            format!("string RVA {rva:#010x} is not file-backed"),
        ))
    }

    fn read_ascii_z(
        &self,
        rva: u32,
        max_bytes: usize,
        code: &'static str,
    ) -> Result<String, LoaderError> {
        let tail = self.file_tail(rva, code)?;
        let bounded = &tail[..tail.len().min(max_bytes.saturating_add(1))];
        let Some(length) = bounded.iter().position(|byte| *byte == 0) else {
            return Err(LoaderError::resource_limit(
                "imports",
                "import-string-limit-or-terminator-missing",
                format!("string at RVA {rva:#010x} has no terminator within {max_bytes} bytes"),
            ));
        };
        let bytes = &bounded[..length];
        if bytes.is_empty() || !bytes.iter().all(|byte| (0x20..=0x7e).contains(byte)) {
            return Err(LoaderError::invalid_input(
                "imports",
                code,
                format!("string at RVA {rva:#010x} is empty or not printable ASCII"),
            ));
        }
        Ok(bytes.iter().copied().map(char::from).collect())
    }

    fn read_u16_rva(&self, rva: u32, code: &'static str) -> Result<u16, LoaderError> {
        read_u16(self.file_span(rva, 2, code)?, 0)
    }

    fn read_u32_rva(&self, rva: u32, code: &'static str) -> Result<u32, LoaderError> {
        read_u32(self.file_span(rva, 4, code)?, 0)
    }
}

fn checked_add_mul(
    base: u32,
    index: u32,
    stride: u32,
    code: &'static str,
) -> Result<u32, LoaderError> {
    let offset = index.checked_mul(stride).ok_or_else(|| {
        LoaderError::invalid_input(
            "checked-address",
            code,
            "RVA stride multiplication overflowed",
        )
    })?;
    base.checked_add(offset).ok_or_else(|| {
        LoaderError::invalid_input("checked-address", code, "RVA addition overflowed")
    })
}

fn align_up(value: u32, alignment: u32) -> Result<u32, LoaderError> {
    let mask = alignment - 1;
    value
        .checked_add(mask)
        .map(|sum| sum & !mask)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "checked-address",
                "alignment-overflow",
                "32-bit alignment operation overflowed",
            )
        })
}

fn checked_u64_range(rva: u32, size: u32) -> Option<Range<u64>> {
    if size == 0 {
        return None;
    }
    let start = u64::from(rva);
    let end = start.checked_add(u64::from(size))?;
    if end > u64::from(u32::MAX) + 1 {
        return None;
    }
    Some(start..end)
}

fn contains_range(container: &Range<u64>, value: &Range<u64>) -> bool {
    value.start >= container.start && value.end <= container.end
}

fn ranges_overlap(left: &Range<u64>, right: &Range<u64>) -> bool {
    left.start < right.end && right.start < left.end
}

fn range_contains(base: u32, size: u32, value: u32, value_size: u32) -> bool {
    let Some(container) = checked_u64_range(base, size) else {
        return false;
    };
    let Some(value) = checked_u64_range(value, value_size) else {
        return false;
    };
    contains_range(&container, &value)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, LoaderError> {
    let end = offset.checked_add(2).ok_or_else(bounded_read_error)?;
    let Some(&[low, high]) = bytes.get(offset..end) else {
        return Err(bounded_read_error());
    };
    Ok(u16::from_le_bytes([low, high]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, LoaderError> {
    let end = offset.checked_add(4).ok_or_else(bounded_read_error)?;
    let Some(&[first, second, third, fourth]) = bytes.get(offset..end) else {
        return Err(bounded_read_error());
    };
    Ok(u32::from_le_bytes([first, second, third, fourth]))
}

fn bounded_read_error() -> LoaderError {
    LoaderError::invalid_input(
        "pe-decode",
        "bounded-integer-read-failed",
        "integer field extends beyond its validated source span",
    )
}
