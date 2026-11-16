//! Loader-derived AOT roots. These are discovery hints, not control-flow
//! assumptions: execution still reads indirect targets and checks live memory.
use std::collections::BTreeSet;

use crate::materialize::ImageMaterialization;
use crate::memory::{GuestAddress, GuestMemory, GuestMemoryErrorClass};
use crate::pe32::{ExportTarget, LoaderError, Pe32LoadPlan};

#[derive(Debug, PartialEq, Eq)]
pub struct CodePreparationEntries {
    pub entries: Vec<u32>,
    pub limited: bool,
}

/// Inspect sealed, relocated images before executing the guest. Exported code
/// and address-taken code in HIGHLOW relocations can be absent from the entry
/// point's direct-call graph. Never scan arbitrary words or grant permissions
/// on the strength of a pointer. Unreadable fixups provide no optional hint.
pub fn code_preparation_entries(
    memory: &GuestMemory,
    images: &[(&Pe32LoadPlan, &ImageMaterialization)],
    limit: usize,
) -> Result<CodePreparationEntries, LoaderError> {
    if !(1..=4096).contains(&limit) || images.is_empty() || images.len() > 4096 {
        return Err(LoaderError::new(
            "invalid code preparation image/root bounds",
        ));
    }
    for &(plan, image) in images {
        if plan.source != image.source
            || image.layout.image_range.byte_length() != u64::from(plan.image.size_of_image)
            || image
                .layout
                .image_range
                .start()
                .0
                .checked_add(plan.image.entry_point_rva.0)
                != Some(image.entry_point_address.0)
        {
            return Err(LoaderError::new("code preparation image identity mismatch"));
        }
        memory
            .validate_sealed_image_owner(
                image.layout.owner,
                &image.source,
                image.layout.image_range,
            )
            .map_err(|e| LoaderError::new(e.to_string()))?;
    }
    let mut result = CodePreparationEntries {
        entries: Vec::new(),
        limited: false,
    };
    let mut seen = BTreeSet::new();
    let mut insert =
        |image: &ImageMaterialization, address: GuestAddress| -> Result<bool, LoaderError> {
            if seen.contains(&address.0) {
                return Ok(true);
            }
            if let Err(error) = memory.validate_owned_host_execute(image.layout.owner, address, 1) {
                return match error.class {
                    GuestMemoryErrorClass::Fault
                    | GuestMemoryErrorClass::InvalidRequest
                    | GuestMemoryErrorClass::Conflict => Ok(true),
                    _ => Err(LoaderError::new(error.to_string())),
                };
            }
            if result.entries.len() == limit {
                result.limited = true;
                return Ok(false);
            }
            seen.insert(address.0);
            result.entries.push(address.0);
            Ok(true)
        };
    // Give every actual entry priority over optional exported/address-taken code.
    for &(plan, image) in images {
        if plan.image.entry_point_rva.0 != 0 && !insert(image, image.entry_point_address)? {
            return Ok(result);
        }
    }
    for &(plan, image) in images {
        let base = image.layout.image_range.start().0;
        for export in plan.exports.iter().flat_map(|exports| &exports.symbols) {
            if let ExportTarget::Address { rva } = export.target {
                let address = base
                    .checked_add(rva.0)
                    .ok_or_else(|| LoaderError::new("code export address overflow"))?;
                if !insert(image, GuestAddress(address))? {
                    return Ok(result);
                }
            }
        }
        for rva in &plan.relocations {
            let address = base
                .checked_add(rva.0)
                .ok_or_else(|| LoaderError::new("code relocation address overflow"))?;
            match memory.read_owned_host(GuestAddress(address), 4, image.layout.owner) {
                Ok(bytes) => {
                    let target =
                        u32::from_le_bytes(bytes.try_into().expect("four-byte relocation"));
                    if !insert(image, GuestAddress(target))? {
                        return Ok(result);
                    }
                }
                Err(error) if error.class == GuestMemoryErrorClass::Fault => {}
                Err(error) => return Err(LoaderError::new(error.to_string())),
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
