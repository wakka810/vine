use crate::api::{ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiImmediate};
use crate::memory::GuestAddress;
use crate::module_images::RuntimeModuleImage;
use crate::pe32::{ResourceDataEntryPlan, ResourceIdentifier};
use crate::runtime::Runtime;

pub(crate) fn image(
    runtime: &Runtime,
    instance: u32,
) -> Result<Option<&RuntimeModuleImage>, ApiFault> {
    let image = if instance == 0 {
        let module = runtime
            .main_module()
            .map_err(|e| ApiFault::new(e.to_string()))?;
        Some(
            runtime
                .module_images()
                .image_by_name(&module.canonical_name)
                .map_err(|e| ApiFault::new(e.to_string()))?,
        )
    } else {
        runtime
            .module_image_by_load_base(GuestAddress(instance))
            .map_err(|e| ApiFault::new(e.to_string()))?
    };
    if let Some(image) = image
        && runtime
            .module_option(image.handle)
            .map_err(|e| ApiFault::new(e.to_string()))?
            .is_none()
    {
        return Ok(None);
    }
    Ok(image)
}

pub(crate) fn select<'a>(
    image: &'a RuntimeModuleImage,
    kind: &ResourceIdentifier,
    name: &ResourceIdentifier,
) -> Result<&'a ResourceDataEntryPlan, u32> {
    let resources = image.resources.as_ref().ok_or(1813_u32)?;
    let equal = |a: &ResourceIdentifier, b: &ResourceIdentifier| match (a, b) {
        (ResourceIdentifier::Name { name: a }, ResourceIdentifier::Name { name: b }) => {
            a.to_uppercase() == b.to_uppercase()
        }
        _ => a == b,
    };
    if !resources
        .entries
        .iter()
        .any(|e| equal(&e.resource_type, kind))
    {
        return Err(1813);
    }
    let mut candidates = resources
        .entries
        .iter()
        .filter(|e| equal(&e.resource_type, kind) && equal(&e.name, name))
        .peekable();
    if candidates.peek().is_none() {
        return Err(1814);
    }
    candidates
        .filter_map(|entry| match entry.language {
            ResourceIdentifier::Id { id } => Some((entry, id)),
            _ => None,
        })
        .min_by_key(|&(_, language)| match language {
            0 => (0, 0),
            0x0411 => (1, 0),
            0x0011 => (2, 0),
            0x0409 => (3, 0),
            other => (4, other),
        })
        .map(|(entry, _)| entry)
        .ok_or(1815)
}

pub(crate) fn identifier(
    endpoint: &ApiEndpointIdentity,
    runtime: &Runtime,
    pointer: u32,
) -> Result<ResourceIdentifier, ApiFault> {
    if pointer <= u32::from(u16::MAX) {
        return Ok(ResourceIdentifier::Id { id: pointer });
    }
    let name =
        crate::win32::read_utf16_nul(endpoint, &runtime.memory, GuestAddress(pointer), 1024)?;
    if let Some(digits) = name.strip_prefix('#')
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && let Ok(id) = digits.parse::<u16>()
    {
        return Ok(ResourceIdentifier::Id { id: u32::from(id) });
    }
    Ok(ResourceIdentifier::Name { name })
}

pub(crate) fn address(image: &RuntimeModuleImage, rva: u32) -> Result<GuestAddress, ApiFault> {
    image
        .image_range
        .start()
        .0
        .checked_add(rva)
        .map(GuestAddress)
        .ok_or_else(|| ApiFault::new("resource address exceeds the guest address space"))
}

pub(crate) fn failure(error: u32) -> ApiImmediate {
    ApiImmediate::new(Some(0)).with_last_error(error)
}

pub(crate) fn handle(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &Runtime,
    name: &str,
) -> Result<ApiImmediate, ApiFault> {
    let argument = |index| {
        frame
            .argument_u32(index)
            .ok_or_else(|| ApiFault::new("resource call is missing an argument"))
    };
    if name == "LockResource" {
        let data = argument(0)?;
        for image in runtime.module_images().images().values() {
            if let Some(resources) = &image.resources {
                for entry in &resources.entries {
                    if address(image, entry.data_rva.0)?.0 == data {
                        if self::image(runtime, image.image_range.start().0)?.is_none() {
                            continue;
                        }
                        runtime
                            .memory
                            .read_owned(GuestAddress(data), entry.size as usize, image.memory_owner)
                            .map_err(|e| ApiFault::from_memory(e, endpoint))?;
                        return Ok(ApiImmediate::new(Some(data)));
                    }
                }
            }
        }
        return Ok(failure(6));
    }
    let Some(image) = image(runtime, argument(0)?)? else {
        return Ok(failure(6));
    };
    if name == "FindResourceW" || name == "LoadAcceleratorsW" {
        let resource_name = identifier(endpoint, runtime, argument(1)?)?;
        let kind = if name == "LoadAcceleratorsW" {
            ResourceIdentifier::Id { id: 9 }
        } else {
            identifier(endpoint, runtime, argument(2)?)?
        };
        let entry = match select(image, &kind, &resource_name) {
            Ok(entry) => entry,
            Err(error) => return Ok(failure(error)),
        };
        if name == "LoadAcceleratorsW" {
            return Err(ApiFault::new(
                "loading an existing accelerator table is unsupported",
            ));
        }
        let descriptor = address(image, entry.descriptor_rva.0)?;
        runtime
            .memory
            .read_owned(descriptor, 16, image.memory_owner)
            .map_err(|e| ApiFault::from_memory(e, endpoint))?;
        return Ok(ApiImmediate::new(Some(descriptor.0)));
    }
    let descriptor = argument(1)?;
    let Some(entry) = image.resources.as_ref().and_then(|r| {
        r.entries
            .iter()
            .find(|e| address(image, e.descriptor_rva.0).is_ok_and(|a| a.0 == descriptor))
    }) else {
        return Ok(failure(6));
    };
    runtime
        .memory
        .read_owned(GuestAddress(descriptor), 16, image.memory_owner)
        .map_err(|e| ApiFault::from_memory(e, endpoint))?;
    if name == "SizeofResource" {
        return Ok(ApiImmediate::new(Some(entry.size)));
    }
    let data = address(image, entry.data_rva.0)?;
    runtime
        .memory
        .read_owned(data, entry.size as usize, image.memory_owner)
        .map_err(|e| ApiFault::from_memory(e, endpoint))?;
    Ok(ApiImmediate::new(Some(data.0)))
}
