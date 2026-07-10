#![forbid(unsafe_code)]

mod pe32;
mod snapshot;

pub use pe32::{
    DataDirectoryAddressKind, DataDirectoryPlan, FeatureInventory, HeaderMappingPlan, ImageKind,
    ImagePlan, ImportModulePlan, ImportSymbolPlan, ImportTarget, LoaderError, LoaderErrorClass,
    LoaderIngressReport, MappingProtection, Pe32Limits, Pe32LoadPlan, RelocationBlockPlan,
    RelocationOperation, RelocationPlan, Rva, SectionMappingPlan, UnsupportedPoint,
    build_pe32_load_plan,
};
pub use snapshot::{ModuleIdentity, ModuleSnapshot};
