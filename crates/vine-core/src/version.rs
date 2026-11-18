use crate::api::{
    ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiSymbol,
};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::pe32::ResourceIdentifier;
use crate::runtime::Runtime;

pub(crate) fn version_argument_words(endpoint: &ApiEndpointIdentity) -> Option<u8> {
    match &endpoint.symbol {
        ApiSymbol::Name { name } => match name.as_str() {
            "GetFileVersionInfoSizeW" => Some(2),
            "GetFileVersionInfoW" | "VerQueryValueW" => Some(4),
            _ => None,
        },
        _ => None,
    }
}

fn arg(frame: &ApiCallFrame, index: usize) -> Result<u32, ApiFault> {
    frame
        .argument_u32(index)
        .ok_or_else(|| ApiFault::new("version API argument is missing"))
}

fn file_version<'a>(
    frame: &ApiCallFrame,
    runtime: &'a Runtime,
) -> Result<Result<&'a [u8], u32>, ApiFault> {
    let path = crate::win32::read_nul_terminated_source_path(
        &frame.endpoint,
        &runtime.memory,
        GuestAddress(arg(frame, 0)?),
        32768,
    )?;
    if !runtime
        .is_process_image_path_a(&path)
        .map_err(|error| ApiFault::from_runtime(error, &frame.endpoint))?
    {
        return Err(ApiFault::new(
            "file-version queries for files other than the process image are not implemented",
        ));
    }
    let image = crate::resources::image(runtime, 0)?
        .ok_or_else(|| ApiFault::new("process image is not live"))?;
    let entry = match crate::resources::select(
        image,
        &ResourceIdentifier::Id { id: 16 },
        &ResourceIdentifier::Id { id: 1 },
    ) {
        Ok(entry) => entry,
        Err(error) => return Ok(Err(error)),
    };
    let bytes = entry
        .version_bytes
        .as_deref()
        .ok_or_else(|| ApiFault::new("version resource exceeds its WORD length bound"))?;
    let root = block(bytes, 0, bytes.len())?;
    if root.key != "VS_VERSION_INFO"
        || root.kind != 0
        || root.value_length != 52
        || word(bytes, root.value_start)? != 0x04bd
        || word(bytes, root.value_start + 2)? != 0xfeef
    {
        return Err(ApiFault::new(
            "version resource has no valid VS_FIXEDFILEINFO",
        ));
    }
    Ok(Ok(bytes))
}

pub(crate) fn dispatch_version_api(frame: &ApiCallFrame, runtime: &Runtime) -> ApiHandlerResult {
    let result = (|| {
        let ApiSymbol::Name { name } = &frame.endpoint.symbol else {
            return Err(ApiFault::new("unsupported version ordinal"));
        };
        if name == "VerQueryValueW" {
            return query(frame, runtime);
        }
        if !matches!(
            name.as_str(),
            "GetFileVersionInfoSizeW" | "GetFileVersionInfoW"
        ) {
            return Err(ApiFault::new("unsupported version import"));
        }
        let bytes = file_version(frame, runtime)?;
        if name == "GetFileVersionInfoSizeW" {
            let output = GuestAddress(arg(frame, 1)?);
            let mut result = match bytes {
                Ok(bytes) => ApiImmediate::new(Some(bytes.len() as u32 * 2 + 4)).with_last_error(0),
                Err(error) => ApiImmediate::new(Some(0)).with_last_error(error),
            };
            if output.0 != 0 {
                result = result.with_guest_write(output, vec![0; 4]);
            }
            return Ok(result);
        }
        let bytes = match bytes {
            Ok(bytes) => bytes,
            Err(error) => return Ok(ApiImmediate::new(Some(0)).with_last_error(error)),
        };
        let capacity = arg(frame, 2)? as usize;
        let output = GuestAddress(arg(frame, 3)?);
        if capacity < bytes.len() * 2 + 4 {
            return Err(ApiFault::new(
                "truncated file-version buffers are not implemented",
            ));
        }
        if capacity > 1024 * 1024 {
            return Err(ApiFault::new(
                "file-version buffer exceeds its bounded capacity",
            ));
        }
        runtime
            .memory
            .validate_access(output, capacity, MemoryAccess::Write)
            .map_err(|error| ApiFault::from_memory(error, &frame.endpoint))?;
        let mut data = bytes.to_vec();
        data.extend_from_slice(b"FE2X");
        Ok(ApiImmediate::new(Some(1))
            .with_last_error(0)
            .with_guest_write(output, data))
    })();
    match result {
        Ok(result) => ApiHandlerResult::Immediate(result),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

struct Block {
    kind: u16,
    end: usize,
    key: String,
    value_start: usize,
    value_length: u16,
    children: usize,
}

fn word(bytes: &[u8], offset: usize) -> Result<u16, ApiFault> {
    let bytes = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| ApiFault::new("version record extends beyond its buffer"))?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn aligned(value: usize) -> usize {
    (value + 3) & !3
}

fn block(bytes: &[u8], start: usize, limit: usize) -> Result<Block, ApiFault> {
    let length = usize::from(word(bytes, start)?);
    let end = start + length;
    if length < 6 || end > limit || end > bytes.len() {
        return Err(ApiFault::new("version record length is invalid"));
    }
    let value_length = word(bytes, start + 2)?;
    let kind = word(bytes, start + 4)?;
    if kind > 1 {
        return Err(ApiFault::new("version record value type is invalid"));
    }
    let mut position = start + 6;
    let mut units = Vec::new();
    loop {
        if position + 2 > end {
            return Err(ApiFault::new("version record key is unterminated"));
        }
        let unit = word(bytes, position)?;
        position += 2;
        if unit == 0 {
            break;
        }
        units.push(unit);
    }
    let value_start = aligned(position);
    let value_end = value_start + usize::from(value_length) * if kind == 1 { 2 } else { 1 };
    if value_end > end {
        return Err(ApiFault::new("version value exceeds its record"));
    }
    let key =
        String::from_utf16(&units).map_err(|_| ApiFault::new("version key is not valid UTF-16"))?;
    Ok(Block {
        kind,
        end,
        key,
        value_start,
        value_length,
        children: aligned(value_end),
    })
}

fn query(frame: &ApiCallFrame, runtime: &Runtime) -> Result<ApiImmediate, ApiFault> {
    let base = GuestAddress(arg(frame, 0)?);
    let subblock = crate::win32::read_utf16_nul(
        &frame.endpoint,
        &runtime.memory,
        GuestAddress(arg(frame, 1)?),
        32768,
    )?;
    let pointer = GuestAddress(arg(frame, 2)?);
    let length = GuestAddress(arg(frame, 3)?);
    for address in [pointer, length] {
        runtime
            .memory
            .validate_access(address, 4, MemoryAccess::Write)
            .map_err(|error| ApiFault::from_memory(error, &frame.endpoint))?;
    }
    let header = runtime
        .memory
        .read(base, 2)
        .map_err(|error| ApiFault::from_memory(error, &frame.endpoint))?;
    let size = usize::from(u16::from_le_bytes([header[0], header[1]]));
    let bytes = runtime
        .memory
        .read(base, size)
        .map_err(|error| ApiFault::from_memory(error, &frame.endpoint))?;
    let mut current = block(&bytes, 0, size)?;
    if current.key != "VS_VERSION_INFO" {
        return Err(ApiFault::new("version query root is invalid"));
    }
    for key in subblock.split('\\').filter(|key| !key.is_empty()) {
        let mut position = current.children;
        let mut found = None;
        while position < current.end {
            let child = block(&bytes, position, current.end)?;
            position = aligned(child.end);
            if child.key.eq_ignore_ascii_case(key) {
                found = Some(child);
                break;
            }
        }
        let Some(child) = found else {
            return Ok(ApiImmediate::new(Some(0))
                .with_last_error(1813)
                .with_guest_write(length, vec![0; 4]));
        };
        current = child;
    }
    let address = base
        .0
        .checked_add(current.value_start as u32)
        .ok_or_else(|| ApiFault::new("version value pointer overflowed"))?;
    Ok(ApiImmediate::new(Some(1))
        .with_last_error(0)
        .with_guest_write(pointer, address.to_le_bytes().to_vec())
        .with_guest_write(
            length,
            u32::from(current.value_length).to_le_bytes().to_vec(),
        ))
}
