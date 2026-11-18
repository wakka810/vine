use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;

use goblin::pe::characteristic::{
    IMAGE_FILE_32BIT_MACHINE, IMAGE_FILE_DLL, IMAGE_FILE_EXECUTABLE_IMAGE,
    IMAGE_FILE_RELOCS_STRIPPED,
};
use goblin::pe::header::{COFF_MACHINE_X86, Header, PE_MAGIC, SIZEOF_COFF_HEADER, SIZEOF_PE_MAGIC};
use goblin::pe::optional_header::IMAGE_NT_OPTIONAL_HDR32_MAGIC;
use goblin::pe::section_table::{
    IMAGE_SCN_MEM_EXECUTE, IMAGE_SCN_MEM_READ, IMAGE_SCN_MEM_WRITE, SectionTable,
};

use crate::snapshot::{ModuleIdentity, ModuleSnapshot};

const IMAGE_DIRECTORY_ENTRY_EXPORT: usize = 0;
const IMAGE_DIRECTORY_ENTRY_IMPORT: usize = 1;
const IMAGE_DIRECTORY_ENTRY_RESOURCE: usize = 2;
const IMAGE_DIRECTORY_ENTRY_SECURITY: usize = 4;
const IMAGE_DIRECTORY_ENTRY_BASERELOC: usize = 5;
const IMAGE_DIRECTORY_ENTRY_GLOBALPTR: usize = 8;
const IMAGE_DIRECTORY_ENTRY_TLS: usize = 9;
const IMAGE_DIRECTORY_ENTRY_IAT: usize = 12;
const IMAGE_DIRECTORY_ENTRY_DELAY_IMPORT: usize = 13;
const IMAGE_DIRECTORY_ENTRY_COM_DESCRIPTOR: usize = 14;
const IMAGE_DIRECTORY_ENTRY_RESERVED: usize = 15;

const IMPORT_DESCRIPTOR_SIZE: u32 = 20;
const EXPORT_DIRECTORY_SIZE: u32 = 40;
const RESOURCE_DIRECTORY_SIZE: u32 = 16;
const RESOURCE_DIRECTORY_ENTRY_SIZE: u32 = 8;
const RESOURCE_DATA_ENTRY_SIZE: u32 = 16;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rva(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pe32Limits {
    pub max_file_bytes: u64,
    pub max_image_bytes: u32,
    pub max_sections: usize,
    pub max_import_modules: usize,
    pub max_imports: usize,
    pub max_import_name_bytes: usize,
    pub max_exports: usize,
    pub max_export_names: usize,
    pub max_export_name_bytes: usize,
    pub max_forwarder_bytes: usize,
    pub max_resource_directories: usize,
    pub max_resource_entries: usize,
    pub max_resource_data_entries: usize,
    pub max_resource_name_code_units: usize,
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
            max_exports: 65_536,
            max_export_names: 65_536,
            max_export_name_bytes: 1_024,
            max_forwarder_bytes: 1_024,
            max_resource_directories: 4_096,
            max_resource_entries: 65_536,
            max_resource_data_entries: 65_536,
            max_resource_name_code_units: 1_024,
            max_relocation_blocks: 65_536,
            max_relocations: 1_000_000,
        }
    }
}

#[derive(Debug)]
pub struct LoaderError {
    pub message: String,
}

impl LoaderError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for LoaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for LoaderError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pe32LoadPlan {
    pub(crate) source: ModuleIdentity,
    pub image: ImagePlan,
    pub(crate) header_mapping: HeaderMappingPlan,
    pub sections: Vec<SectionMappingPlan>,
    pub(crate) exports: Option<ExportDirectoryPlan>,
    pub(crate) resources: Option<ResourceDirectoryPlan>,
    pub(crate) tls: Option<TlsDirectoryPlan>,
    pub imports: Vec<ImportModulePlan>,
    pub(crate) relocations: Vec<Rva>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TlsDirectoryPlan {
    pub template_rva: Rva,
    pub template_size: u32,
    pub zero_fill: u32,
    pub index_rva: Rva,
    pub alignment: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Executable,
    DynamicLibrary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImagePlan {
    pub kind: ImageKind,
    pub(crate) coff_characteristics: u16,
    pub preferred_image_base: u32,
    pub(crate) image_end: u64,
    pub size_of_image: u32,
    pub(crate) size_of_headers: u32,
    pub(crate) entry_point_rva: Rva,
    pub entry_point_address: u32,
    pub stack_reserve: u64,
    pub stack_commit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderMappingPlan {
    pub source_offset: u32,
    pub destination_rva: Rva,
    pub copy_size: u32,
    pub mapped_size: u32,
    pub final_protection: MappingProtection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MappingProtection {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionMappingPlan {
    pub name: String,
    pub destination_rva: Rva,
    pub virtual_size: u32,
    pub(crate) mapped_size: u32,
    pub(crate) source_offset: u32,
    pub(crate) source_size: u32,
    pub(crate) copy_size: u32,
    pub(crate) final_protection: MappingProtection,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResourceIdentifier {
    Id { id: u32 },
    Name { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceDataEntryPlan {
    pub descriptor_rva: Rva,
    pub resource_type: ResourceIdentifier,
    pub name: ResourceIdentifier,
    pub language: ResourceIdentifier,
    pub data_rva: Rva,
    pub size: u32,
    pub code_page: u32,
    // File-version queries read the immutable file, not a guest-modified image.
    pub(crate) version_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceDirectoryPlan {
    pub directory_rva: Rva,
    pub directory_size: u32,
    pub directory_count: usize,
    pub entry_count: usize,
    pub entries: Vec<ResourceDataEntryPlan>,
}

impl ResourceDirectoryPlan {
    pub fn integer_entries(
        &self,
        type_id: u32,
        name_id: u32,
    ) -> impl Iterator<Item = &ResourceDataEntryPlan> {
        self.entries.iter().filter(move |entry| {
            matches!(
                &entry.resource_type,
                ResourceIdentifier::Id { id } if *id == type_id
            ) && matches!(&entry.name, ResourceIdentifier::Id { id } if *id == name_id)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDirectoryPlan {
    pub module_name: String,
    pub symbols: Vec<ExportSymbolPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSymbolPlan {
    pub ordinal: u32,
    pub names: Vec<String>,
    pub target: ExportTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExportLookup {
    Name { name: String },
    Ordinal { ordinal: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportTarget {
    Address {
        rva: Rva,
    },
    Forwarder {
        raw: String,
        module: String,
        symbol: ExportLookup,
    },
}

impl ExportDirectoryPlan {
    pub fn symbol_by_name(&self, name: &str) -> Option<&ExportSymbolPlan> {
        self.symbols
            .iter()
            .find(|symbol| symbol.names.iter().any(|candidate| candidate == name))
    }

    pub fn symbol_by_ordinal(&self, ordinal: u32) -> Option<&ExportSymbolPlan> {
        self.symbols.iter().find(|symbol| symbol.ordinal == ordinal)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportModulePlan {
    pub name: String,
    pub symbols: Vec<ImportSymbolPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSymbolPlan {
    pub(crate) iat_rva: Rva,
    pub(crate) original_iat_value: u32,
    pub target: ImportTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportTarget {
    Name {
        name: String,
        hint: u16,
        name_rva: Rva,
    },
    Ordinal {
        ordinal: u16,
    },
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
) -> Result<Pe32LoadPlan, LoaderError> {
    if snapshot.bytes().len() as u64 > limits.max_file_bytes {
        return Err(LoaderError::new(
            "owned source exceeds the configured PE32 file limit",
        ));
    }

    let bytes = snapshot.bytes();
    let header = Header::parse(bytes).map_err(|error| {
        LoaderError::new(format!(
            "Goblin rejected the DOS/PE/COFF/optional header: {error}"
        ))
    })?;
    if header.signature != PE_MAGIC {
        return Err(LoaderError::new(format!(
            "PE signature is {:#010x}, expected {PE_MAGIC:#010x}",
            header.signature
        )));
    }

    let coff = &header.coff_header;
    if coff.machine != COFF_MACHINE_X86 {
        return Err(LoaderError::new(format!(
            "COFF machine {:#06x} is outside the PE32/i386 scope",
            coff.machine
        )));
    }
    if coff.characteristics & IMAGE_FILE_EXECUTABLE_IMAGE == 0 {
        return Err(LoaderError::new(
            "COFF characteristics do not mark this file as an executable image",
        ));
    }
    if coff.characteristics & IMAGE_FILE_32BIT_MACHINE == 0 {
        return Err(LoaderError::new(
            "i386 image does not carry IMAGE_FILE_32BIT_MACHINE",
        ));
    }
    if usize::from(coff.number_of_sections) > limits.max_sections {
        return Err(LoaderError::new(format!(
            "image declares {} sections; limit is {}",
            coff.number_of_sections, limits.max_sections
        )));
    }

    let optional = header
        .optional_header
        .as_ref()
        .ok_or_else(|| LoaderError::new("executable image has no PE optional header"))?;
    if optional.standard_fields.magic != IMAGE_NT_OPTIONAL_HDR32_MAGIC {
        return Err(LoaderError::new(format!(
            "optional-header magic is {:#06x}; PE32 requires {IMAGE_NT_OPTIONAL_HDR32_MAGIC:#06x}",
            optional.standard_fields.magic
        )));
    }
    if coff.size_of_optional_header != 224 {
        return Err(LoaderError::new(format!(
            "PE32 optional header has {} bytes; this loader requires 224",
            coff.size_of_optional_header
        )));
    }

    let windows = &optional.windows_fields;
    validate_alignment(windows.file_alignment, windows.section_alignment)?;
    if windows.size_of_image == 0 || windows.size_of_image > limits.max_image_bytes {
        return Err(LoaderError::new(format!(
            "SizeOfImage is {} bytes; accepted range is 1..={} bytes",
            windows.size_of_image, limits.max_image_bytes
        )));
    }
    if windows.size_of_image % windows.section_alignment != 0 {
        return Err(LoaderError::new(
            "SizeOfImage is not aligned to SectionAlignment",
        ));
    }
    if windows.size_of_headers == 0
        || windows.size_of_headers % windows.file_alignment != 0
        || u64::from(windows.size_of_headers) > bytes.len() as u64
    {
        return Err(LoaderError::new(
            "SizeOfHeaders is zero, misaligned, or extends beyond the immutable source",
        ));
    }
    if windows.image_base > u64::from(u32::MAX) {
        return Err(LoaderError::new(
            "PE32 ImageBase does not fit a 32-bit guest address",
        ));
    }
    let image_base = windows.image_base as u32;
    if !image_base.is_multiple_of(0x1_0000) {
        return Err(LoaderError::new(
            "ImageBase is not aligned to the Windows 64 KiB allocation boundary",
        ));
    }
    let image_end = u64::from(image_base) + u64::from(windows.size_of_image);
    if image_end > u64::from(u32::MAX) + 1 {
        return Err(LoaderError::new(
            "preferred image range extends beyond the 32-bit guest address space",
        ));
    }
    if windows.size_of_stack_commit > windows.size_of_stack_reserve
        || windows.size_of_heap_commit > windows.size_of_heap_reserve
    {
        return Err(LoaderError::new(
            "stack or heap commit size exceeds its corresponding reserve size",
        ));
    }

    let section_table_offset = u64::from(header.dos_header.pe_pointer)
        + SIZEOF_PE_MAGIC as u64
        + SIZEOF_COFF_HEADER as u64
        + u64::from(coff.size_of_optional_header);
    let section_table_size = u64::from(coff.number_of_sections) * 40;
    if section_table_offset + section_table_size > u64::from(windows.size_of_headers) {
        return Err(LoaderError::new(
            "section table extends beyond SizeOfHeaders",
        ));
    }
    let mut section_offset = usize::try_from(section_table_offset)
        .map_err(|_| LoaderError::new("section table offset does not fit the host index type"))?;
    let section_headers = coff
        .sections(bytes, &mut section_offset)
        .map_err(|error| LoaderError::new(format!("Goblin rejected the section table: {error}")))?;

    let layout = validate_sections(
        bytes,
        &section_headers,
        windows.size_of_headers,
        windows.size_of_image,
        windows.file_alignment,
        windows.section_alignment,
    )?;
    validate_directories(optional, &layout)?;

    if optional.data_directories.data_directories[IMAGE_DIRECTORY_ENTRY_COM_DESCRIPTOR].is_some() {
        return Err(LoaderError::new(
            "CLR runtime header is present; managed images are outside the active scope",
        ));
    }
    if optional.data_directories.data_directories[IMAGE_DIRECTORY_ENTRY_DELAY_IMPORT].is_some() {
        return Err(LoaderError::new("delay imports are not implemented"));
    }

    let tls = parse_tls(optional, &layout, image_base)?;
    let exports = parse_exports(optional, &layout, &limits)?;
    let resources = parse_resources(optional, &layout, &limits)?;
    let imports = parse_imports(optional, &layout, &limits)?;
    let relocations = parse_relocations(optional, &layout, &limits, coff.characteristics)?;

    let entry_rva = optional.standard_fields.address_of_entry_point;
    let kind = if coff.characteristics & IMAGE_FILE_DLL != 0 {
        ImageKind::DynamicLibrary
    } else {
        ImageKind::Executable
    };
    if entry_rva == 0 && kind == ImageKind::Executable {
        return Err(LoaderError::new(
            "executable image has a zero entry point RVA",
        ));
    }
    if entry_rva != 0 {
        let section = layout
            .section_containing_mapped(entry_rva, 1)
            .ok_or_else(|| {
                LoaderError::new(format!(
                    "entry point RVA {entry_rva:#010x} is outside mapped sections"
                ))
            })?;
        if !section.plan.final_protection.execute {
            return Err(LoaderError::new(format!(
                "entry point RVA {entry_rva:#010x} lands in non-executable section {}",
                section.plan.name
            )));
        }
        layout.file_span(entry_rva, 1)?;
    }
    let entry_point_address = image_base.checked_add(entry_rva).ok_or_else(|| {
        LoaderError::new("ImageBase plus AddressOfEntryPoint overflows a 32-bit guest address")
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
        source: snapshot.identity(),
        image: ImagePlan {
            kind,
            coff_characteristics: coff.characteristics,
            preferred_image_base: image_base,
            image_end,
            size_of_image: windows.size_of_image,
            size_of_headers: windows.size_of_headers,
            entry_point_rva: Rva(entry_rva),
            entry_point_address,
            stack_reserve: windows.size_of_stack_reserve,
            stack_commit: windows.size_of_stack_commit,
        },
        header_mapping,
        sections,
        exports,
        resources,
        tls,
        imports,
        relocations,
    };

    Ok(load_plan)
}

fn validate_alignment(file_alignment: u32, section_alignment: u32) -> Result<(), LoaderError> {
    if file_alignment == 0
        || section_alignment == 0
        || !file_alignment.is_power_of_two()
        || !section_alignment.is_power_of_two()
    {
        return Err(LoaderError::new(
            "FileAlignment and SectionAlignment must be non-zero powers of two",
        ));
    }
    if section_alignment < file_alignment {
        return Err(LoaderError::new(
            "SectionAlignment must be greater than or equal to FileAlignment",
        ));
    }

    if section_alignment < 0x1000 {
        if file_alignment != section_alignment {
            return Err(LoaderError::new(
                "when SectionAlignment is below 4096, FileAlignment must match it",
            ));
        }
        return Err(LoaderError::new(format!(
            "valid low-alignment PE uses FileAlignment {file_alignment:#x} and SectionAlignment {section_alignment:#x}; Vine's 4 KiB GuestMemory backing does not yet materialize sub-page section mappings"
        )));
    } else if !(0x200..=0x1_0000).contains(&file_alignment) {
        return Err(LoaderError::new(
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
        return Err(LoaderError::new(
            "section-aligned headers extend beyond SizeOfImage",
        ));
    }
    let mut validated: Vec<ValidatedSection<'a>> = Vec::with_capacity(sections.len());
    let mut previous_virtual_address = None;

    for (index, section) in sections.iter().enumerate() {
        if let Some(previous) = previous_virtual_address
            && section.virtual_address <= previous
        {
            return Err(LoaderError::new(
                "section RVAs must be strictly increasing in table order",
            ));
        }
        previous_virtual_address = Some(section.virtual_address);

        if section.virtual_address % section_alignment != 0 {
            return Err(LoaderError::new(format!(
                "section {} RVA is not SectionAlignment-aligned",
                index + 1
            )));
        }
        if section.size_of_raw_data != 0
            && (section.pointer_to_raw_data == 0
                || section.pointer_to_raw_data % file_alignment != 0
                || section.size_of_raw_data % file_alignment != 0)
        {
            return Err(LoaderError::new(format!(
                "section {} has a zero or FileAlignment-misaligned raw range",
                index + 1
            )));
        }

        let logical_size = section.virtual_size.max(section.size_of_raw_data);
        let mapped_size = if logical_size == 0 {
            0
        } else {
            align_up(logical_size, section_alignment)?
        };
        let virtual_end = u64::from(section.virtual_address) + u64::from(mapped_size);
        if section.virtual_address < header_mapped_size || virtual_end > u64::from(size_of_image) {
            return Err(LoaderError::new(format!(
                "section {} overlaps mapped headers or extends beyond SizeOfImage",
                index + 1
            )));
        }

        let raw_range = if section.size_of_raw_data == 0 {
            None
        } else {
            let start = u64::from(section.pointer_to_raw_data);
            let end = start + u64::from(section.size_of_raw_data);
            if start < u64::from(size_of_headers) || end > bytes.len() as u64 {
                return Err(LoaderError::new(format!(
                    "section {} overlaps headers or extends beyond the immutable source",
                    index + 1
                )));
            }
            Some(start..end)
        };

        let name = section.name().map_err(|error| {
            LoaderError::new(format!(
                "section {} has an invalid name: {error}",
                index + 1
            ))
        })?;
        let protection = MappingProtection {
            read: section.characteristics & IMAGE_SCN_MEM_READ != 0,
            write: section.characteristics & IMAGE_SCN_MEM_WRITE != 0,
            execute: section.characteristics & IMAGE_SCN_MEM_EXECUTE != 0,
        };
        validated.push(ValidatedSection {
            raw: section,
            plan: SectionMappingPlan {
                name: name.to_owned(),
                destination_rva: Rva(section.virtual_address),
                virtual_size: section.virtual_size,
                mapped_size,
                source_offset: section.pointer_to_raw_data,
                source_size: section.size_of_raw_data,
                copy_size: section.size_of_raw_data,
                final_protection: protection,
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
                return Err(LoaderError::new(format!(
                    "sections {} and {} have overlapping mapped ranges",
                    left + 1,
                    right + 1
                )));
            }
            if let (Some(left_raw), Some(right_raw)) = (
                validated[left].raw_range.as_ref(),
                validated[right].raw_range.as_ref(),
            ) && ranges_overlap(left_raw, right_raw)
            {
                return Err(LoaderError::new(format!(
                    "sections {} and {} have overlapping source ranges",
                    left + 1,
                    right + 1
                )));
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
) -> Result<(), LoaderError> {
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
            return Err(LoaderError::new(format!(
                "{} directory has a zero address or size, but not both",
                DIRECTORY_NAMES[index]
            )));
        }

        if index == IMAGE_DIRECTORY_ENTRY_SECURITY {
            let end = u64::from(directory.virtual_address) + u64::from(directory.size);
            if directory.virtual_address % 8 != 0 || end > layout.bytes.len() as u64 {
                return Err(LoaderError::new(
                    "certificate table is misaligned or extends beyond the immutable source",
                ));
            }
        } else {
            let mapped_size = directory.size.max(1);
            if !layout.mapped_contains(directory.virtual_address, mapped_size) {
                return Err(LoaderError::new(format!(
                    "{} directory RVA range is outside mapped headers/sections",
                    DIRECTORY_NAMES[index]
                )));
            }
            layout.file_span(directory.virtual_address, mapped_size)?;
        }

        if index == IMAGE_DIRECTORY_ENTRY_RESERVED {
            return Err(LoaderError::new(
                "reserved PE data directory entry 15 is non-empty",
            ));
        }
    }
    Ok(())
}

#[derive(Debug)]
struct RawResourceDirectoryEntry {
    identifier: ResourceIdentifier,
    target_offset: u32,
    target_is_directory: bool,
}

#[derive(Debug, Default)]
struct ResourceParseState {
    directory_count: usize,
    entry_count: usize,
    data_entry_count: usize,
    visited_directories: BTreeSet<u32>,
}

// PE TLS uses preferred virtual addresses, unlike most PE directories. Keep
// RVAs in the plan so relocation cannot turn metadata into an unchecked pointer.
fn parse_tls(
    optional: &goblin::pe::optional_header::OptionalHeader,
    layout: &ValidatedLayout<'_>,
    image_base: u32,
) -> Result<Option<TlsDirectoryPlan>, LoaderError> {
    let Some((_, directory)) =
        optional.data_directories.data_directories[IMAGE_DIRECTORY_ENTRY_TLS]
    else {
        return Ok(None);
    };
    if directory.size < 24 {
        return Err(LoaderError::new("PE32 TLS directory is truncated"));
    }
    let bytes = layout.file_span(directory.virtual_address, 24)?;
    let word = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    let rva = |va: u32| {
        va.checked_sub(image_base)
            .ok_or_else(|| LoaderError::new("TLS virtual address precedes its image"))
    };
    let start = word(0);
    let end = word(4);
    let template_size = end
        .checked_sub(start)
        .ok_or_else(|| LoaderError::new("TLS template has a reversed range"))?;
    let template_rva = if start == 0 && end == 0 {
        0
    } else {
        rva(start)?
    };
    if template_rva > layout.size_of_image {
        return Err(LoaderError::new(
            "TLS template address is outside its image",
        ));
    }
    if template_size != 0 {
        if !layout.mapped_contains(template_rva, template_size) {
            return Err(LoaderError::new(
                "TLS template is outside mapped image bytes",
            ));
        }
        layout.file_span(template_rva, template_size)?;
    }
    let zero_fill = word(16);
    if template_size
        .checked_add(zero_fill)
        .is_none_or(|size| size > 1024 * 1024)
    {
        return Err(LoaderError::new(
            "TLS template exceeds the one MiB per-module limit",
        ));
    }
    let index_rva = rva(word(8))?;
    if !layout
        .section_containing_mapped(index_rva, 4)
        .is_some_and(|section| section.plan.final_protection.write)
    {
        return Err(LoaderError::new(
            "TLS index must lie in writable image data",
        ));
    }
    let characteristics = word(20);
    let alignment_code = (characteristics >> 20) & 15;
    if characteristics & !0x00f0_0000 != 0 || alignment_code == 15 {
        return Err(LoaderError::new(
            "TLS alignment characteristics are invalid",
        ));
    }
    let alignment = if alignment_code == 0 {
        4
    } else {
        1 << (alignment_code - 1)
    };
    if word(12) != 0 {
        let callbacks = rva(word(12))?;
        if !layout.mapped_contains(callbacks, 4) {
            return Err(LoaderError::new("TLS callback array is outside the image"));
        }
        if layout.file_span(callbacks, 4)? != [0, 0, 0, 0] {
            return Err(LoaderError::new(
                "nonempty PE TLS callback arrays are not implemented",
            ));
        }
    }
    Ok(Some(TlsDirectoryPlan {
        template_rva: Rva(template_rva),
        template_size,
        zero_fill,
        index_rva: Rva(index_rva),
        alignment,
    }))
}

fn parse_resources(
    optional: &goblin::pe::optional_header::OptionalHeader,
    layout: &ValidatedLayout<'_>,
    limits: &Pe32Limits,
) -> Result<Option<ResourceDirectoryPlan>, LoaderError> {
    let Some((_, directory)) =
        optional.data_directories.data_directories[IMAGE_DIRECTORY_ENTRY_RESOURCE]
    else {
        return Ok(None);
    };
    if directory.size < RESOURCE_DIRECTORY_SIZE {
        return Err(LoaderError::new(
            "resource directory is smaller than IMAGE_RESOURCE_DIRECTORY",
        ));
    }
    let directory_end = directory
        .virtual_address
        .checked_add(directory.size)
        .ok_or_else(|| {
            LoaderError::new("resource directory RVA range overflowed x86 accounting")
        })?;
    let mut state = ResourceParseState::default();
    let root = parse_resource_directory(
        layout,
        directory.virtual_address,
        directory_end,
        0,
        limits,
        &mut state,
    )?;
    let mut entries = Vec::new();
    let mut identities = BTreeSet::new();

    for type_entry in root {
        if !type_entry.target_is_directory {
            return Err(LoaderError::new(
                "resource type entry must reference a name directory",
            ));
        }
        let names = parse_resource_directory(
            layout,
            directory.virtual_address,
            directory_end,
            type_entry.target_offset,
            limits,
            &mut state,
        )?;
        for name_entry in names {
            if !name_entry.target_is_directory {
                return Err(LoaderError::new(
                    "resource name entry must reference a language directory",
                ));
            }
            let languages = parse_resource_directory(
                layout,
                directory.virtual_address,
                directory_end,
                name_entry.target_offset,
                limits,
                &mut state,
            )?;
            for language_entry in languages {
                if language_entry.target_is_directory {
                    return Err(LoaderError::new(
                        "resource language entry must reference a data entry",
                    ));
                }
                state.data_entry_count =
                    state.data_entry_count.checked_add(1).ok_or_else(|| {
                        LoaderError::new("resource data-entry count overflowed host accounting")
                    })?;
                if state.data_entry_count > limits.max_resource_data_entries {
                    return Err(LoaderError::new(format!(
                        "image declares more than {} resource data entries",
                        limits.max_resource_data_entries
                    )));
                }
                let data = resource_relative_span(
                    layout,
                    directory.virtual_address,
                    directory_end,
                    language_entry.target_offset,
                    RESOURCE_DATA_ENTRY_SIZE,
                )?;
                let data_rva = read_u32(data, 0)?;
                let size = read_u32(data, 4)?;
                let code_page = read_u32(data, 8)?;
                let reserved = read_u32(data, 12)?;
                if data_rva == 0 || size == 0 {
                    return Err(LoaderError::new(
                        "resource data entry requires a nonzero RVA and byte length",
                    ));
                }
                if reserved != 0 {
                    return Err(LoaderError::new(
                        "resource data entry reserved field is nonzero",
                    ));
                }
                let resource_bytes = layout.file_span(data_rva, size)?;
                let identity = (
                    type_entry.identifier.clone(),
                    name_entry.identifier.clone(),
                    language_entry.identifier.clone(),
                );
                if !identities.insert(identity.clone()) {
                    return Err(LoaderError::new(
                        "resource type, name, and language identity is duplicated",
                    ));
                }
                entries.push(ResourceDataEntryPlan {
                    version_bytes: (matches!(identity.0, ResourceIdentifier::Id { id: 16 })
                        && size <= u32::from(u16::MAX))
                    .then(|| resource_bytes.to_vec()),
                    descriptor_rva: Rva(directory
                        .virtual_address
                        .checked_add(language_entry.target_offset)
                        .ok_or_else(|| LoaderError::new("resource descriptor RVA overflowed"))?),
                    resource_type: identity.0,
                    name: identity.1,
                    language: identity.2,
                    data_rva: Rva(data_rva),
                    size,
                    code_page,
                });
            }
        }
    }

    Ok(Some(ResourceDirectoryPlan {
        directory_rva: Rva(directory.virtual_address),
        directory_size: directory.size,
        directory_count: state.directory_count,
        entry_count: state.entry_count,
        entries,
    }))
}

fn parse_resource_directory(
    layout: &ValidatedLayout<'_>,
    root_rva: u32,
    directory_end: u32,
    directory_offset: u32,
    limits: &Pe32Limits,
    state: &mut ResourceParseState,
) -> Result<Vec<RawResourceDirectoryEntry>, LoaderError> {
    if !state.visited_directories.insert(directory_offset) {
        return Err(LoaderError::new(
            "resource directory offset is referenced more than once",
        ));
    }
    state.directory_count = state
        .directory_count
        .checked_add(1)
        .ok_or_else(|| LoaderError::new("resource directory count overflowed host accounting"))?;
    if state.directory_count > limits.max_resource_directories {
        return Err(LoaderError::new(format!(
            "image declares more than {} resource directories",
            limits.max_resource_directories
        )));
    }
    let header = resource_relative_span(
        layout,
        root_rva,
        directory_end,
        directory_offset,
        RESOURCE_DIRECTORY_SIZE,
    )?;
    if read_u32(header, 0)? != 0 {
        return Err(LoaderError::new(
            "reserved resource directory Characteristics field is nonzero",
        ));
    }
    let named_count = u32::from(read_u16(header, 12)?);
    let id_count = u32::from(read_u16(header, 14)?);
    let count = named_count.checked_add(id_count).ok_or_else(|| {
        LoaderError::new("resource directory entry count overflowed x86 accounting")
    })?;
    let count_usize = usize::try_from(count).map_err(|_| {
        LoaderError::new("resource directory entry count does not fit host accounting")
    })?;
    state.entry_count = state
        .entry_count
        .checked_add(count_usize)
        .ok_or_else(|| LoaderError::new("resource entry count overflowed host accounting"))?;
    if state.entry_count > limits.max_resource_entries {
        return Err(LoaderError::new(format!(
            "image declares more than {} resource directory entries",
            limits.max_resource_entries
        )));
    }
    let table_offset = directory_offset
        .checked_add(RESOURCE_DIRECTORY_SIZE)
        .ok_or_else(|| LoaderError::new("resource entry table offset overflowed x86 accounting"))?;
    let table_size = count
        .checked_mul(RESOURCE_DIRECTORY_ENTRY_SIZE)
        .ok_or_else(|| LoaderError::new("resource entry table size overflowed x86 accounting"))?;
    let table = resource_relative_span(layout, root_rva, directory_end, table_offset, table_size)?;
    let mut identifiers = BTreeSet::new();
    let mut entries = Vec::with_capacity(count_usize);
    for index in 0..count {
        let offset = usize::try_from(index * RESOURCE_DIRECTORY_ENTRY_SIZE)
            .map_err(|_| LoaderError::new("resource entry offset does not fit host accounting"))?;
        let name_raw = read_u32(table, offset)?;
        let target_raw = read_u32(table, offset + 4)?;
        let is_named = name_raw & 0x8000_0000 != 0;
        if (index < named_count) != is_named {
            return Err(LoaderError::new(
                "resource directory named and integer entry bands disagree with their counts",
            ));
        }
        let identifier =
            parse_resource_identifier(layout, root_rva, directory_end, name_raw, limits)?;
        if !identifiers.insert(identifier.clone()) {
            return Err(LoaderError::new(
                "resource directory contains a duplicate child identifier",
            ));
        }
        entries.push(RawResourceDirectoryEntry {
            identifier,
            target_offset: target_raw & 0x7fff_ffff,
            target_is_directory: target_raw & 0x8000_0000 != 0,
        });
    }
    Ok(entries)
}

fn parse_resource_identifier(
    layout: &ValidatedLayout<'_>,
    root_rva: u32,
    directory_end: u32,
    raw: u32,
    limits: &Pe32Limits,
) -> Result<ResourceIdentifier, LoaderError> {
    if raw & 0x8000_0000 == 0 {
        if raw & 0x7fff_0000 != 0 {
            return Err(LoaderError::new(
                "integer resource identifier has nonzero reserved bits",
            ));
        }
        return Ok(ResourceIdentifier::Id { id: raw & 0xffff });
    }
    let offset = raw & 0x7fff_ffff;
    let length_bytes = resource_relative_span(layout, root_rva, directory_end, offset, 2)?;
    let length = usize::from(read_u16(length_bytes, 0)?);
    if length == 0 || length > limits.max_resource_name_code_units {
        return Err(LoaderError::new(format!(
            "resource name has {length} UTF-16 code units; accepted range is 1..={} ",
            limits.max_resource_name_code_units
        )));
    }
    let byte_length = u32::try_from(length)
        .ok()
        .and_then(|units| units.checked_mul(2))
        .ok_or_else(|| LoaderError::new("resource name byte length overflowed x86 accounting"))?;
    let string_offset = offset
        .checked_add(2)
        .ok_or_else(|| LoaderError::new("resource name offset overflowed x86 accounting"))?;
    let bytes =
        resource_relative_span(layout, root_rva, directory_end, string_offset, byte_length)?;
    let units = bytes
        .chunks_exact(2)
        .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
        .collect::<Vec<_>>();
    let name = String::from_utf16(&units)
        .map_err(|_| LoaderError::new("resource name is not valid UTF-16"))?;
    if name.contains('\0') {
        return Err(LoaderError::new(
            "resource name contains an embedded NUL code point",
        ));
    }
    Ok(ResourceIdentifier::Name { name })
}

fn resource_relative_span<'a>(
    layout: &'a ValidatedLayout<'_>,
    root_rva: u32,
    directory_end: u32,
    relative_offset: u32,
    size: u32,
) -> Result<&'a [u8], LoaderError> {
    let rva = root_rva
        .checked_add(relative_offset)
        .ok_or_else(|| LoaderError::new("resource-relative RVA overflowed x86 accounting"))?;
    let end = rva
        .checked_add(size)
        .ok_or_else(|| LoaderError::new("resource-relative span overflowed x86 accounting"))?;
    if end > directory_end {
        return Err(LoaderError::new(
            "resource-relative span extends beyond the resource directory",
        ));
    }
    layout.file_span(rva, size)
}

fn parse_exports(
    optional: &goblin::pe::optional_header::OptionalHeader,
    layout: &ValidatedLayout<'_>,
    limits: &Pe32Limits,
) -> Result<Option<ExportDirectoryPlan>, LoaderError> {
    let Some((_, directory)) =
        optional.data_directories.data_directories[IMAGE_DIRECTORY_ENTRY_EXPORT]
    else {
        return Ok(None);
    };
    if directory.size < EXPORT_DIRECTORY_SIZE {
        return Err(LoaderError::new(
            "export directory is smaller than IMAGE_EXPORT_DIRECTORY",
        ));
    }
    let directory_end = directory
        .virtual_address
        .checked_add(directory.size)
        .ok_or_else(|| LoaderError::new("export directory RVA range overflowed x86 accounting"))?;
    let header = layout.file_span(directory.virtual_address, EXPORT_DIRECTORY_SIZE)?;
    let characteristics = read_u32(header, 0)?;
    if characteristics != 0 {
        return Err(LoaderError::new(
            "reserved export directory Characteristics field is nonzero",
        ));
    }
    let module_name_rva = read_u32(header, 12)?;
    let ordinal_base = read_u32(header, 16)?;
    let function_count = read_u32(header, 20)?;
    let name_count = read_u32(header, 24)?;
    let address_table_rva = read_u32(header, 28)?;
    let name_pointer_table_rva = read_u32(header, 32)?;
    let ordinal_table_rva = read_u32(header, 36)?;

    let function_count_usize = usize::try_from(function_count)
        .map_err(|_| LoaderError::new("export function count does not fit host accounting"))?;
    let name_count_usize = usize::try_from(name_count)
        .map_err(|_| LoaderError::new("export name count does not fit host accounting"))?;
    if function_count_usize > limits.max_exports {
        return Err(LoaderError::new(format!(
            "image declares {function_count} export address slots; limit is {}",
            limits.max_exports
        )));
    }
    if name_count_usize > limits.max_export_names {
        return Err(LoaderError::new(format!(
            "image declares {name_count} export names; limit is {}",
            limits.max_export_names
        )));
    }
    if name_count != 0 && function_count == 0 {
        return Err(LoaderError::new(
            "export name table is nonempty while the address table is empty",
        ));
    }
    if function_count != 0 {
        ordinal_base
            .checked_add(function_count - 1)
            .ok_or_else(|| {
                LoaderError::new("export ordinal base plus declared function range overflowed")
            })?;
    }
    if module_name_rva == 0
        || (function_count != 0 && address_table_rva == 0)
        || (name_count != 0 && (name_pointer_table_rva == 0 || ordinal_table_rva == 0))
    {
        return Err(LoaderError::new(
            "export directory has a zero module or required table RVA",
        ));
    }
    if function_count != 0 {
        let bytes = function_count.checked_mul(4).ok_or_else(|| {
            LoaderError::new("export address table size overflowed x86 accounting")
        })?;
        layout.file_span(address_table_rva, bytes)?;
    }
    if name_count != 0 {
        let pointer_bytes = name_count.checked_mul(4).ok_or_else(|| {
            LoaderError::new("export name pointer table size overflowed x86 accounting")
        })?;
        let ordinal_bytes = name_count.checked_mul(2).ok_or_else(|| {
            LoaderError::new("export ordinal table size overflowed x86 accounting")
        })?;
        layout.file_span(name_pointer_table_rva, pointer_bytes)?;
        layout.file_span(ordinal_table_rva, ordinal_bytes)?;
    }

    let module_name =
        layout.read_export_ascii_z(module_name_rva, limits.max_export_name_bytes, None)?;
    let mut function_rvas = Vec::with_capacity(function_count_usize);
    for index in 0..function_count {
        let rva = checked_add_mul(address_table_rva, index, 4)?;
        function_rvas.push(layout.read_u32_rva(rva)?);
    }

    let mut names_by_function = vec![Vec::new(); function_count_usize];
    let mut unique_names = BTreeSet::new();
    for index in 0..name_count {
        let pointer_rva = checked_add_mul(name_pointer_table_rva, index, 4)?;
        let name_rva = layout.read_u32_rva(pointer_rva)?;
        let ordinal_rva = checked_add_mul(ordinal_table_rva, index, 2)?;
        let function_index = u32::from(layout.read_u16_rva(ordinal_rva)?);
        if function_index >= function_count {
            return Err(LoaderError::new(format!(
                "export name {index} refers to function index {function_index}, but function count is {function_count}"
            )));
        }
        let name = layout.read_export_ascii_z(name_rva, limits.max_export_name_bytes, None)?;
        if !unique_names.insert(name.clone()) {
            return Err(LoaderError::new(format!(
                "export name {name} appears more than once"
            )));
        }
        let function_index = usize::try_from(function_index)
            .map_err(|_| LoaderError::new("export function index does not fit host accounting"))?;
        names_by_function[function_index].push(name);
    }

    let mut symbols = Vec::with_capacity(function_count_usize);
    for (index, function_rva) in function_rvas.into_iter().enumerate() {
        let names = std::mem::take(&mut names_by_function[index]);
        if function_rva == 0 {
            if !names.is_empty() {
                return Err(LoaderError::new(
                    "named export refers to a zero address-table slot",
                ));
            }
            continue;
        }
        let index_u32 = u32::try_from(index)
            .map_err(|_| LoaderError::new("export function index does not fit x86 accounting"))?;
        let ordinal = ordinal_base.checked_add(index_u32).ok_or_else(|| {
            LoaderError::new("export ordinal base plus function index overflowed")
        })?;
        let target = if (directory.virtual_address..directory_end).contains(&function_rva) {
            let raw = layout.read_export_ascii_z(
                function_rva,
                limits.max_forwarder_bytes,
                Some(directory_end),
            )?;
            parse_export_forwarder(raw)?
        } else {
            if !layout.mapped_contains(function_rva, 1) {
                return Err(LoaderError::new(format!(
                    "export ordinal {ordinal} targets unmapped RVA {function_rva:#010x}"
                )));
            }
            ExportTarget::Address {
                rva: Rva(function_rva),
            }
        };
        symbols.push(ExportSymbolPlan {
            ordinal,
            names,
            target,
        });
    }

    Ok(Some(ExportDirectoryPlan {
        module_name,
        symbols,
    }))
}

fn parse_export_forwarder(raw: String) -> Result<ExportTarget, LoaderError> {
    let Some((module, target)) = raw.rsplit_once('.') else {
        return Err(LoaderError::new(
            "export forwarder must contain module and target separated by a period",
        ));
    };
    if module.is_empty()
        || target.is_empty()
        || module
            .bytes()
            .any(|byte| byte.is_ascii_control() || matches!(byte, b'/' | b'\\' | b':'))
    {
        return Err(LoaderError::new(
            "export forwarder module and target must be bounded ASCII identities",
        ));
    }
    let module = module.to_owned();
    let symbol = if let Some(ordinal) = target.strip_prefix('#') {
        let ordinal = ordinal
            .parse::<u32>()
            .map_err(|_| LoaderError::new("export forwarder ordinal is not a decimal integer"))?;
        if ordinal > u32::from(u16::MAX) {
            return Err(LoaderError::new(
                "export forwarder ordinal exceeds the PE32 import ordinal range",
            ));
        }
        ExportLookup::Ordinal { ordinal }
    } else {
        ExportLookup::Name {
            name: target.to_owned(),
        }
    };
    Ok(ExportTarget::Forwarder {
        raw,
        module,
        symbol,
    })
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
        return Err(LoaderError::new(
            "import directory size is not a whole number of 20-byte descriptors",
        ));
    }
    let descriptor_capacity = import_directory.size / IMPORT_DESCRIPTOR_SIZE;
    if usize::try_from(descriptor_capacity).unwrap_or(usize::MAX)
        > limits.max_import_modules.saturating_add(1)
    {
        return Err(LoaderError::new(format!(
            "import directory can contain {descriptor_capacity} descriptors; module limit is {}",
            limits.max_import_modules
        )));
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
        )?;
        let descriptor = layout.file_span(descriptor_rva, IMPORT_DESCRIPTOR_SIZE)?;
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
            return Err(LoaderError::new(
                "non-terminal import descriptor has a zero name or IAT RVA",
            ));
        }
        if modules.len() >= limits.max_import_modules {
            return Err(LoaderError::new(format!(
                "import module limit {} was reached",
                limits.max_import_modules
            )));
        }

        let module_name = layout.read_ascii_z(name_rva, limits.max_import_name_bytes)?;
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
                LoaderError::new("import thunk index does not fit a PE32 RVA calculation")
            })?;
            let lookup_rva =
                checked_add_mul(effective_lookup_rva, thunk_index_u32, PE32_THUNK_SIZE)?;
            let lookup = layout.read_u32_rva(lookup_rva)?;
            let iat_rva =
                checked_add_mul(import_address_table_rva, thunk_index_u32, PE32_THUNK_SIZE)?;
            if let Some(iat) = iat_directory
                && !range_contains(iat.virtual_address, iat.size, iat_rva, PE32_THUNK_SIZE)
            {
                return Err(LoaderError::new(format!(
                    "module {module_name} has an IAT slot outside the IAT directory"
                )));
            }
            let original_iat_value = layout.read_u32_rva(iat_rva)?;

            if lookup == 0 {
                if original_iat_value != 0 {
                    return Err(LoaderError::new(format!(
                        "module {module_name} lookup table terminates before its IAT"
                    )));
                }
                thunk_terminated = true;
                break;
            }
            if thunk_index == max_module_imports {
                break;
            }
            let target = if lookup & 0x8000_0000 != 0 {
                if lookup & 0x7fff_0000 != 0 {
                    return Err(LoaderError::new(format!(
                        "module {module_name} has malformed ordinal thunk {lookup:#010x}"
                    )));
                }
                ImportTarget::Ordinal {
                    ordinal: lookup as u16,
                }
            } else {
                let name_rva = lookup & 0x7fff_ffff;
                let hint = layout.read_u16_rva(name_rva)?;
                let string_rva = name_rva
                    .checked_add(2)
                    .ok_or_else(|| LoaderError::new("import hint/name RVA overflows 32 bits"))?;
                let name = layout.read_ascii_z(string_rva, limits.max_import_name_bytes)?;
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
            return Err(LoaderError::new(format!(
                "module {module_name} did not terminate before the global import limit {}",
                limits.max_imports
            )));
        }
        total_imports += symbols.len();
        modules.push(ImportModulePlan {
            name: module_name,
            symbols,
        });
    }

    if !terminated {
        return Err(LoaderError::new(
            "import directory has no all-zero descriptor within its declared size",
        ));
    }
    Ok(modules)
}

fn parse_relocations(
    optional: &goblin::pe::optional_header::OptionalHeader,
    layout: &ValidatedLayout<'_>,
    limits: &Pe32Limits,
    coff_characteristics: u16,
) -> Result<Vec<Rva>, LoaderError> {
    let Some((_, directory)) =
        optional.data_directories.data_directories[IMAGE_DIRECTORY_ENTRY_BASERELOC]
    else {
        return Ok(Vec::new());
    };
    let bytes = layout.file_span(directory.virtual_address, directory.size)?;
    let mut offset = 0usize;
    let mut block_count = 0usize;
    let mut operations = Vec::new();
    let mut entry_count = 0usize;

    while offset < bytes.len() {
        if block_count >= limits.max_relocation_blocks {
            return Err(LoaderError::new(format!(
                "base relocation block limit {} was reached",
                limits.max_relocation_blocks
            )));
        }
        if bytes.len() - offset < RELOCATION_BLOCK_HEADER_SIZE {
            return Err(LoaderError::new(
                "base relocation directory ends inside an 8-byte block header",
            ));
        }
        let page_rva = read_u32(bytes, offset)?;
        let encoded_size = read_u32(bytes, offset + 4)?;
        let block_size = usize::try_from(encoded_size).map_err(|_| {
            LoaderError::new("base relocation block size does not fit the host index type")
        })?;
        if block_size < RELOCATION_BLOCK_HEADER_SIZE
            || block_size % 2 != 0
            || block_size > bytes.len() - offset
        {
            return Err(LoaderError::new(
                "base relocation block size is too small, odd, or exceeds the directory",
            ));
        }
        if !page_rva.is_multiple_of(0x1000) {
            return Err(LoaderError::new(format!(
                "base relocation page RVA {page_rva:#010x} is not 4 KiB aligned"
            )));
        }

        let word_count = (block_size - RELOCATION_BLOCK_HEADER_SIZE) / 2;
        if entry_count.saturating_add(word_count) > limits.max_relocations {
            return Err(LoaderError::new(format!(
                "base relocation entry limit {} would be exceeded",
                limits.max_relocations
            )));
        }
        for word_index in 0..word_count {
            let word_offset = offset + RELOCATION_BLOCK_HEADER_SIZE + word_index * 2;
            let word = read_u16(bytes, word_offset)?;
            let relocation_type = (word >> 12) as u8;
            let page_offset = u32::from(word & 0x0fff);
            match relocation_type {
                IMAGE_REL_BASED_ABSOLUTE => {}
                IMAGE_REL_BASED_HIGHLOW => {
                    let target_rva = page_rva.checked_add(page_offset).ok_or_else(|| {
                        LoaderError::new("HIGHLOW relocation target RVA overflows 32 bits")
                    })?;
                    if !layout.mapped_contains(target_rva, 4) {
                        return Err(LoaderError::new(format!(
                            "HIGHLOW relocation target {target_rva:#010x} is not fully mapped"
                        )));
                    }
                    operations.push(Rva(target_rva));
                }
                unsupported => {
                    return Err(LoaderError::new(format!(
                        "PE32 base relocation type {unsupported} is not ABSOLUTE or HIGHLOW"
                    )));
                }
            }
        }
        entry_count += word_count;
        block_count += 1;
        offset += block_size;
    }

    if coff_characteristics & IMAGE_FILE_RELOCS_STRIPPED != 0 && entry_count != 0 {
        return Err(LoaderError::new(
            "COFF flags mark relocations stripped while base relocation entries are present",
        ));
    }
    Ok(operations)
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

    fn file_span(&self, rva: u32, size: u32) -> Result<&[u8], LoaderError> {
        let range = checked_u64_range(rva, size)
            .ok_or_else(|| LoaderError::new("RVA range overflows 32 bits"))?;
        if range.end <= u64::from(self.size_of_headers) {
            let start = usize::try_from(range.start).map_err(|_| {
                LoaderError::new("header source offset does not fit the host index type")
            })?;
            let end = usize::try_from(range.end).map_err(|_| {
                LoaderError::new("header source end does not fit the host index type")
            })?;
            return self.bytes.get(start..end).ok_or_else(|| {
                LoaderError::new("header RVA range extends beyond the immutable source")
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
                    LoaderError::new("resolved source offset does not fit the host index type")
                })?;
                let end = usize::try_from(source_end).map_err(|_| {
                    LoaderError::new("resolved source end does not fit the host index type")
                })?;
                return self.bytes.get(start..end).ok_or_else(|| {
                    LoaderError::new("resolved RVA range extends beyond the immutable source")
                });
            }
        }
        Err(LoaderError::new(format!(
            "RVA range {rva:#010x}..+{size:#x} is not file-backed"
        )))
    }

    fn file_tail(&self, rva: u32) -> Result<&[u8], LoaderError> {
        if rva < self.size_of_headers {
            let start = usize::try_from(rva).map_err(|_| {
                LoaderError::new("header string offset does not fit the host index type")
            })?;
            let end = usize::try_from(self.size_of_headers).map_err(|_| {
                LoaderError::new("header string end does not fit the host index type")
            })?;
            return self.bytes.get(start..end).ok_or_else(|| {
                LoaderError::new("header string RVA extends beyond the immutable source")
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
                    LoaderError::new("string source offset does not fit the host index type")
                })?;
                let end = usize::try_from(source_end).map_err(|_| {
                    LoaderError::new("string source end does not fit the host index type")
                })?;
                return self.bytes.get(start..end).ok_or_else(|| {
                    LoaderError::new("string RVA resolves beyond the immutable source")
                });
            }
        }
        Err(LoaderError::new(format!(
            "string RVA {rva:#010x} is not file-backed"
        )))
    }

    fn read_ascii_z(&self, rva: u32, max_bytes: usize) -> Result<String, LoaderError> {
        let tail = self.file_tail(rva)?;
        let bounded = &tail[..tail.len().min(max_bytes.saturating_add(1))];
        let Some(length) = bounded.iter().position(|byte| *byte == 0) else {
            return Err(LoaderError::new(format!(
                "string at RVA {rva:#010x} has no terminator within {max_bytes} bytes"
            )));
        };
        let bytes = &bounded[..length];
        if bytes.is_empty() || !bytes.iter().all(|byte| (0x20..=0x7e).contains(byte)) {
            return Err(LoaderError::new(format!(
                "string at RVA {rva:#010x} is empty or not printable ASCII"
            )));
        }
        Ok(bytes.iter().copied().map(char::from).collect())
    }

    fn read_export_ascii_z(
        &self,
        rva: u32,
        max_bytes: usize,
        exclusive_rva_end: Option<u32>,
    ) -> Result<String, LoaderError> {
        let tail = self.file_tail(rva)?;
        let boundary_bytes = match exclusive_rva_end {
            Some(end) => {
                let length = end.checked_sub(rva).ok_or_else(|| {
                    LoaderError::new("export string starts outside its containing directory")
                })?;
                Some(usize::try_from(length).map_err(|_| {
                    LoaderError::new("export string boundary does not fit host accounting")
                })?)
            }
            None => None,
        };
        let bounded_length = tail
            .len()
            .min(max_bytes.saturating_add(1))
            .min(boundary_bytes.unwrap_or(usize::MAX));
        let bounded = &tail[..bounded_length];
        let Some(length) = bounded.iter().position(|byte| *byte == 0) else {
            return Err(LoaderError::new(format!(
                "string at RVA {rva:#010x} has no terminator within its bounded range"
            )));
        };
        let bytes = &bounded[..length];
        if bytes.is_empty() || !bytes.iter().all(|byte| (0x20..=0x7e).contains(byte)) {
            return Err(LoaderError::new(format!(
                "string at RVA {rva:#010x} is empty or not printable ASCII"
            )));
        }
        Ok(bytes.iter().copied().map(char::from).collect())
    }

    fn read_u16_rva(&self, rva: u32) -> Result<u16, LoaderError> {
        read_u16(self.file_span(rva, 2)?, 0)
    }

    fn read_u32_rva(&self, rva: u32) -> Result<u32, LoaderError> {
        read_u32(self.file_span(rva, 4)?, 0)
    }
}

fn checked_add_mul(base: u32, index: u32, stride: u32) -> Result<u32, LoaderError> {
    let offset = index
        .checked_mul(stride)
        .ok_or_else(|| LoaderError::new("RVA stride multiplication overflowed"))?;
    base.checked_add(offset)
        .ok_or_else(|| LoaderError::new("RVA addition overflowed"))
}

fn align_up(value: u32, alignment: u32) -> Result<u32, LoaderError> {
    let mask = alignment - 1;
    value
        .checked_add(mask)
        .map(|sum| sum & !mask)
        .ok_or_else(|| LoaderError::new("32-bit alignment operation overflowed"))
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
    LoaderError::new("integer field extends beyond its validated source span")
}
