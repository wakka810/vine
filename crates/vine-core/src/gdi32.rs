use crate::api::{
    ApiCallFrame, ApiDeferredCall, ApiDispatcher, ApiEndpointIdentity, ApiFault, ApiHandlerResult,
    ApiImmediate, ApiSymbol, DispatchOutcome, GuestCallbackArgument, GuestCallbackCompletionPolicy,
    GuestCallbackRequest, HostRequest,
};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::{
    RUNTIME_GDI_DIB_BI_BITFIELDS, RUNTIME_GDI_DIB_BI_RGB, RUNTIME_GDI_DIB_RGB_COLORS,
    RUNTIME_LOG_FONT_A_BYTE_LENGTH, RUNTIME_LOG_FONT_A_FACE_CAPACITY, RuntimeDeviceContextTarget,
    RuntimeGdiDibSectionDescriptor, RuntimeGdiRgbQuad, RuntimeGdiTextRasterTarget,
    RuntimeHostCompletion, RuntimeLogFontA,
};

pub const RUNTIME_GDI_TEXT_RASTER_SERVICE: &str = "browser.gdi-text-raster";
pub const X86_LOGFONTA_BYTE_LENGTH: u32 = RUNTIME_LOG_FONT_A_BYTE_LENGTH as u32;
pub const X86_BITMAPINFOHEADER_BYTE_LENGTH: u32 = 40;
pub const X86_DIB_RGBQUAD_COUNT: usize = 256;
#[cfg(test)]
pub const X86_DIB_BITMAPINFO_BYTE_LENGTH: usize =
    X86_BITMAPINFOHEADER_BYTE_LENGTH as usize + X86_DIB_RGBQUAD_COUNT * 4;
pub const X86_ENUMLOGFONTEXA_BYTE_LENGTH: usize = 188;
pub const X86_NEWTEXTMETRICEXA_BYTE_LENGTH: usize = 96;
pub const X86_OUTLINE_TEXT_METRIC_A_FIXED_BYTE_LENGTH: usize = 212;
pub const X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH: usize = 269;

const SHIFT_JIS_CHARSET: u8 = 128;
const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_CAN_NOT_COMPLETE: u32 = 1003;
const TRUETYPE_FONTTYPE: u32 = 0x0000_0004;
const MS_GOTHIC_WINDOWS_932: &[u8] = &[
    0x82, 0x6c, 0x82, 0x72, 0x20, 0x83, 0x53, 0x83, 0x56, 0x83, 0x62, 0x83, 0x4e,
];
const REGULAR_WINDOWS_932: &[u8] = &[0x95, 0x57, 0x8f, 0x80];
const JAPANESE_WINDOWS_932: &[u8] = &[0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea];
const MICROSOFT_PREFIX: &[u8] = b"Microsoft:";
const FONT_SIGNATURE: [u8; 24] = [
    0xff, 0x02, 0x00, 0xe0, 0xfb, 0xfd, 0xc7, 0x6a, 0x12, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00,
    0x9f, 0x00, 0x02, 0x40, 0x00, 0x00, 0xd7, 0xdf,
];
#[derive(Debug)]
pub struct Gdi32TextRasterHostResponse {
    pub request_id: u64,
    pub pixels: Vec<u8>,
}

#[derive(Debug)]
struct Gdi32TextRasterHostRequest {
    device_context: u32,
    selection_generation: u64,
    font_id: u64,
    dib_section_id: u64,
    content_generation: u64,
    font_height: i32,
    font_width: i32,
    font_weight: i32,
    dib_width: i32,
    dib_height: i32,
    stride_bytes: u32,
    pixel_byte_length: u32,
    character_count: u32,
    packed_text: u32,
}

impl Gdi32TextRasterHostRequest {
    const WORD_COUNT: usize = 18;

    fn from_target(target: &RuntimeGdiTextRasterTarget, text: &[u8]) -> Self {
        let mut packed_text = 0_u32;
        for (index, byte) in text.iter().copied().enumerate() {
            packed_text |= u32::from(byte) << (index * 8);
        }
        Self {
            device_context: target.device_context_handle,
            selection_generation: target.selection_generation,
            font_id: target.font_id.0,
            dib_section_id: target.dib_section_id.0,
            content_generation: target.backing.content_generation,
            font_height: target.log_font.height,
            font_width: target.log_font.width,
            font_weight: target.log_font.weight,
            dib_width: target.descriptor.width,
            dib_height: target.descriptor.height,
            stride_bytes: target.descriptor.stride_bytes,
            pixel_byte_length: target.descriptor.pixel_byte_length,
            character_count: text.len() as u32,
            packed_text,
        }
    }

    fn encode(&self) -> Vec<u8> {
        let words = [
            self.device_context,
            self.selection_generation as u32,
            (self.selection_generation >> 32) as u32,
            self.font_id as u32,
            (self.font_id >> 32) as u32,
            self.dib_section_id as u32,
            (self.dib_section_id >> 32) as u32,
            self.content_generation as u32,
            (self.content_generation >> 32) as u32,
            self.font_height as u32,
            self.font_width as u32,
            self.font_weight as u32,
            self.dib_width as u32,
            self.dib_height as u32,
            self.stride_bytes,
            self.pixel_byte_length,
            self.character_count,
            self.packed_text,
        ];
        words.into_iter().flat_map(u32::to_le_bytes).collect()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ApiFault> {
        if bytes.len() != Self::WORD_COUNT * 4 {
            return Err(ApiFault::new(
                "browser text raster request has an invalid bounded payload size",
            ));
        }
        let words: [u32; Self::WORD_COUNT] = std::array::from_fn(|index| {
            let offset = index * 4;
            u32::from_le_bytes(
                bytes[offset..offset + 4]
                    .try_into()
                    .expect("four-byte chunk"),
            )
        });
        Ok(Self {
            device_context: words[0],
            selection_generation: u64::from(words[1]) | (u64::from(words[2]) << 32),
            font_id: u64::from(words[3]) | (u64::from(words[4]) << 32),
            dib_section_id: u64::from(words[5]) | (u64::from(words[6]) << 32),
            content_generation: u64::from(words[7]) | (u64::from(words[8]) << 32),
            font_height: words[9] as i32,
            font_width: words[10] as i32,
            font_weight: words[11] as i32,
            dib_width: words[12] as i32,
            dib_height: words[13] as i32,
            stride_bytes: words[14],
            pixel_byte_length: words[15],
            character_count: words[16],
            packed_text: words[17],
        })
    }

    fn matches_target(&self, target: &RuntimeGdiTextRasterTarget) -> bool {
        self.device_context == target.device_context_handle
            && self.selection_generation == target.selection_generation
            && self.font_id == target.font_id.0
            && self.dib_section_id == target.dib_section_id.0
            && self.content_generation == target.backing.content_generation
            && self.font_height == target.log_font.height
            && self.font_width == target.log_font.width
            && self.font_weight == target.log_font.weight
            && self.dib_width == target.descriptor.width
            && self.dib_height == target.descriptor.height
            && self.stride_bytes == target.descriptor.stride_bytes
            && self.pixel_byte_length == target.descriptor.pixel_byte_length
    }
}

pub(crate) fn dispatch_gdi32_api(
    callback_return_sentinel: GuestAddress,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let endpoint = &frame.endpoint;
    let result = match &endpoint.symbol {
        ApiSymbol::Name { name } if endpoint.module.eq_ignore_ascii_case("gdi32.dll") => match name
            .as_str()
        {
            "EnumFontFamiliesExA" => {
                return handle_enum_font_families_ex_a(
                    callback_return_sentinel,
                    endpoint,
                    frame,
                    runtime,
                );
            }
            "EnumFontsW" => {
                return handle_enum_fonts_w(callback_return_sentinel, endpoint, frame, runtime);
            }
            "CreateFontA" => handle_create_font_a(endpoint, frame, runtime),
            "CreateFontIndirectA" | "CreateFontIndirectW" => {
                handle_create_font_indirect_w(endpoint, frame, runtime)
            }
            "SetTextColor" | "SetBkColor" | "SetBkMode" => (|| {
                let dc = required_argument(frame, 0, "device context")?;
                let value = required_argument(frame, 1, "text attribute")?;
                let (attribute, failure) = match name.as_str() {
                    "SetTextColor" => (0, u32::MAX),
                    "SetBkColor" => (1, u32::MAX),
                    _ => (2, 0),
                };
                if attribute == 2 && !matches!(value, 1 | 2) {
                    return Ok(ApiImmediate::new(Some(0)).with_last_error(87));
                }
                if attribute != 2 && value & 0xff00_0000 != 0 {
                    return Err(ApiFault::new(
                        "palette-relative GDI colors are not implemented",
                    ));
                }
                let previous = runtime
                    .set_gdi_text_attribute(dc, attribute, value)
                    .map_err(|e| ApiFault::from_runtime(e, endpoint))?;
                Ok(match previous {
                    Some(old) => ApiImmediate::new(Some(old)),
                    None => ApiImmediate::new(Some(failure)).with_last_error(6),
                })
            })(),
            "AddFontMemResourceEx" => handle_add_font_mem_resource(endpoint, frame, runtime),
            "RemoveFontMemResourceEx" => required_argument(frame, 0, "font resource")
                .and_then(|handle| {
                    runtime
                        .remove_gdi_memory_font(handle)
                        .map_err(|error| ApiFault::from_runtime(error, endpoint))
                })
                .map(|removed| ApiImmediate::new(Some(u32::from(removed)))),
            "CreateCompatibleDC" => handle_create_compatible_dc(endpoint, frame, runtime),
            "CreateCompatibleBitmap" => handle_create_compatible_bitmap(endpoint, frame, runtime),
            "SetPixel" | "GetPixel" => handle_pixel(endpoint, frame, runtime, name == "SetPixel"),
            "CreateDIBSection" => handle_create_dib_section(endpoint, frame, runtime),
            "GetOutlineTextMetricsA" | "GetOutlineTextMetricsW" => {
                handle_get_outline_text_metrics(endpoint, frame, runtime, name.ends_with('W'))
            }
            "TextOutA" => {
                return handle_text_out_a(endpoint, frame, runtime);
            }
            "GetStockObject" => handle_get_stock_object(endpoint, frame, runtime),
            "DeleteDC" => handle_delete_dc(endpoint, frame, runtime),
            "GetObjectA" | "GetObjectW" => handle_get_object_a(endpoint, frame, runtime),
            "SelectObject" => handle_select_object(endpoint, frame, runtime),
            "DeleteObject" => handle_delete_object(endpoint, frame, runtime),
            _ => Err(ApiFault::new(format!(
                "unsupported imported function gdi32.dll!{name}"
            ))),
        },
        _ => Err(ApiFault::new("unsupported imported GDI32 ordinal")),
    };
    match result {
        Ok(immediate) => ApiHandlerResult::Immediate(immediate),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_add_font_mem_resource(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let source = GuestAddress(required_argument(frame, 0, "font bytes")?);
    let size = required_argument(frame, 1, "font byte length")? as usize;
    let reserved = required_argument(frame, 2, "reserved")?;
    let output = GuestAddress(required_argument(frame, 3, "font count output")?);
    if reserved != 0
        || size == 0
        || source.0 == 0
        || output.0 == 0
        || size > crate::runtime::FONT_RESOURCE_MAX_BYTES
    {
        return Ok(ApiImmediate::new(Some(0)));
    }
    runtime
        .memory
        .validate_access(output, 4, MemoryAccess::Write)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let bytes = runtime
        .memory
        .read(source, size)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let handle = runtime
        .add_gdi_memory_font(bytes, output)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(handle.unwrap_or(0))))
}

fn handle_create_compatible_dc(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let source = required_argument(frame, 0, "source device context")?;
    if source != 0 {
        let Some(dc) = runtime
            .gdi_device_context_option(source)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        else {
            return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_HANDLE));
        };
        if dc.owner_thread_id != runtime.current_thread_id() {
            return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_HANDLE));
        }
        if matches!(
            dc.target,
            RuntimeDeviceContextTarget::GraphicsSurface { .. }
        ) {
            return Err(ApiFault::new(
                "CreateCompatibleDC from a graphics surface is not implemented",
            ));
        }
    }
    let handle = runtime
        .create_compatible_memory_dc()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(handle.unwrap_or(0))))
}

fn handle_create_compatible_bitmap(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let dc = required_argument(frame, 0, "device context")?;
    let width = required_argument(frame, 1, "bitmap width")? as i32;
    let height = required_argument(frame, 2, "bitmap height")? as i32;
    let handle = runtime
        .create_gdi_compatible_bitmap(dc, width, height)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(handle.unwrap_or(0))))
}

fn handle_pixel(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
    set: bool,
) -> Result<ApiImmediate, ApiFault> {
    let dc = required_argument(frame, 0, "device context")?;
    let x = required_argument(frame, 1, "pixel x")? as i32;
    let y = required_argument(frame, 2, "pixel y")? as i32;
    let color = if set {
        Some(required_argument(frame, 3, "pixel color")?)
    } else {
        None
    };
    let result = runtime
        .gdi_device_bitmap_pixel(dc, x, y, color)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(result)))
}

fn handle_delete_dc(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let device_context = required_argument(frame, 0, "device context")?;
    let current_thread = runtime.current_thread_id();
    let deletable = runtime
        .gdi_device_context_option(device_context)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        .is_some_and(|dc| {
            matches!(dc.target, RuntimeDeviceContextTarget::Memory { .. })
                && dc.owner_thread_id == current_thread
        });
    if !deletable {
        return Ok(ApiImmediate::new(Some(u32::from(false))));
    }
    runtime
        .delete_compatible_memory_dc(device_context)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(u32::from(true))))
}

fn handle_create_dib_section(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let device_context = required_argument(frame, 0, "device context")?;
    let bitmap_info_address = GuestAddress(required_argument(frame, 1, "BITMAPINFO")?);
    let usage = required_argument(frame, 2, "color usage")?;
    let bits_output = GuestAddress(required_argument(frame, 3, "bits output")?);
    let section_handle = required_argument(frame, 4, "section handle")?;
    let offset = required_argument(frame, 5, "section offset")?;
    let header = runtime
        .memory
        .read(
            bitmap_info_address,
            X86_BITMAPINFOHEADER_BYTE_LENGTH as usize,
        )
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let header_size = read_u32(&header, 0);
    let width = read_i32(&header, 4);
    let height = read_i32(&header, 8);
    let planes = read_u16(&header, 12);
    let bits_per_pixel = read_u16(&header, 14);
    let compression = read_u32(&header, 16);
    let size_image = read_u32(&header, 20);
    let colors_used = read_u32(&header, 32);
    let colors_important = read_u32(&header, 36);
    if !matches!(header_size, X86_BITMAPINFOHEADER_BYTE_LENGTH | 108 | 124)
        || width <= 0
        || height == 0
        || height == i32::MIN
        || planes == 0
        || bits_per_pixel == 0
    {
        return Ok(
            ApiImmediate::new(Some(0)).with_guest_write(bits_output, 0_u32.to_le_bytes().to_vec())
        );
    }
    let full_header = if header_size > X86_BITMAPINFOHEADER_BYTE_LENGTH {
        runtime
            .memory
            .read(bitmap_info_address, header_size as usize)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?
    } else {
        header
    };
    let color_masks = if compression == RUNTIME_GDI_DIB_BI_BITFIELDS {
        let mask_bytes = if full_header.len() >= 52 {
            full_header[40..52].to_vec()
        } else {
            let full = runtime
                .memory
                .read(bitmap_info_address, 52)
                .map_err(|error| ApiFault::from_memory(error, endpoint))?;
            full[40..52].to_vec()
        };
        Some([
            read_u32(&mask_bytes, 0),
            read_u32(&mask_bytes, 4),
            read_u32(&mask_bytes, 8),
        ])
    } else {
        None
    };
    let palette = match bits_per_pixel {
        8 => {
            let palette_offset = header_size as usize;
            let bitmap_info = runtime
                .memory
                .read(
                    bitmap_info_address,
                    palette_offset + X86_DIB_RGBQUAD_COUNT * 4,
                )
                .map_err(|error| ApiFault::from_memory(error, endpoint))?;
            bitmap_info[palette_offset..]
                .chunks_exact(4)
                .map(|quad| RuntimeGdiRgbQuad {
                    blue: quad[0],
                    green: quad[1],
                    red: quad[2],
                    reserved: quad[3],
                })
                .collect::<Vec<_>>()
        }
        // Uncompressed or bitfield true-color/high-color DIBs have no palette.
        // Do not read a color table past the caller's header or bitmasks.
        16 | 24 | 32 if colors_used == 0 => Vec::new(),
        _ => {
            return Err(ApiFault::new(
                "CreateDIBSection bit depth or color table is not implemented",
            ));
        }
    };
    let width_u32 = u32::try_from(width).map_err(|_| {
        ApiFault::new("CreateDIBSection width does not fit unsigned pixel accounting")
    })?;
    let height_u32 = u32::try_from(height.abs()).map_err(|_| {
        ApiFault::new("CreateDIBSection height does not fit unsigned pixel accounting")
    })?;
    let stride_bytes = width_u32
        .checked_mul(u32::from(bits_per_pixel))
        .and_then(|bits| bits.checked_add(31))
        .map(|bits| bits / 32)
        .and_then(|words| words.checked_mul(4))
        .ok_or_else(|| ApiFault::new("CreateDIBSection stride overflowed x86 accounting"))?;
    let pixel_byte_length = stride_bytes.checked_mul(height_u32).ok_or_else(|| {
        ApiFault::new("CreateDIBSection pixel byte length overflowed x86 accounting")
    })?;
    let descriptor = RuntimeGdiDibSectionDescriptor {
        width,
        height,
        planes,
        bits_per_pixel,
        compression,
        stride_bytes,
        pixel_byte_length,
        palette,
        color_masks,
    };
    let valid_compression = match (bits_per_pixel, compression, color_masks) {
        (8 | 16 | 24 | 32, RUNTIME_GDI_DIB_BI_RGB, None) => {
            size_image == 0 || size_image == pixel_byte_length || header_size > 40
        }
        (
            16,
            RUNTIME_GDI_DIB_BI_BITFIELDS,
            Some([0x7c00, 0x03e0, 0x001f] | [0xf800, 0x07e0, 0x001f] | [0x0f00, 0x00f0, 0x000f]),
        ) => true,
        (32, RUNTIME_GDI_DIB_BI_BITFIELDS, Some([0x00ff_0000, 0x0000_ff00, 0x0000_00ff])) => true,
        _ => false,
    };
    let supported = usage == RUNTIME_GDI_DIB_RGB_COLORS
        && section_handle == 0
        && offset == 0
        && planes == 1
        && valid_compression
        && width_u32 <= 4096
        && height_u32 <= 4096
        && pixel_byte_length <= 8 * 1024 * 1024
        && (colors_used == 0 || bits_per_pixel == 8 && colors_used == 256)
        && colors_important <= if bits_per_pixel == 8 { 256 } else { 0 };
    if !supported {
        return Err(ApiFault::new(
            "CreateDIBSection supports bounded 8/16/24/32-bit BI_RGB or BI_BITFIELDS bitmaps",
        ));
    }
    let handle = match runtime.create_gdi_dib_section(device_context, descriptor, bits_output) {
        Ok(Some(result)) => result,
        Ok(None) => {
            return Ok(ApiImmediate::new(Some(0))
                .with_guest_write(bits_output, 0_u32.to_le_bytes().to_vec()));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, endpoint)),
    };
    Ok(ApiImmediate::new(Some(handle)))
}

fn handle_get_object_a(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let object_handle = required_argument(frame, 0, "object")?;
    let requested_byte_length = required_argument(frame, 1, "output byte length")?;
    let output = GuestAddress(required_argument(frame, 2, "object output")?);
    let unicode = matches!(&endpoint.symbol, ApiSymbol::Name { name } if name == "GetObjectW");
    let bytes = if unicode {
        let query_length = if output.0 == 0 {
            92
        } else {
            requested_byte_length
        };
        let bytes = runtime
            .get_gdi_object_bytes_w(object_handle, query_length)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        if output.0 == 0 && bytes.is_none() {
            runtime.get_gdi_object_bytes_w(object_handle, 24)
        } else {
            Ok(bytes)
        }
    } else {
        runtime.get_gdi_object_bytes(object_handle, requested_byte_length)
    };
    let Some(bytes) = bytes.map_err(|error| ApiFault::from_runtime(error, endpoint))? else {
        return Ok(ApiImmediate::new(Some(0)));
    };
    if unicode && output.0 == 0 {
        return Ok(ApiImmediate::new(Some(bytes.len() as u32)));
    }
    if bytes.is_empty() {
        return Ok(ApiImmediate::new(Some(0)));
    }
    Ok(
        ApiImmediate::new(Some(u32::try_from(bytes.len()).map_err(|_| {
            ApiFault::new("GetObjectA output length does not fit x86 accounting")
        })?))
        .with_guest_write(output, bytes),
    )
}

fn handle_get_outline_text_metrics(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
    unicode: bool,
) -> Result<ApiImmediate, ApiFault> {
    let device_context = required_argument(frame, 0, "device context")?;
    let requested_byte_length = required_argument(frame, 1, "output byte length")?;
    let output = GuestAddress(required_argument(frame, 2, "outline text metrics output")?);
    let current_thread = runtime.current_thread_id();
    let valid_dc = runtime
        .gdi_device_context_option(device_context)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        .is_some_and(|dc| {
            matches!(dc.target, RuntimeDeviceContextTarget::Memory { .. })
                && dc.owner_thread_id == current_thread
        });
    if !valid_dc {
        return Ok(ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_HANDLE));
    }
    let selected_font = runtime
        .get_gdi_selected_font_a(device_context)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    let mut realized = selected_font.clone();
    if unicode {
        let face = &realized.face_name_windows_932;
        if face.is_empty()
            || face.eq_ignore_ascii_case(b"MS Gothic")
            || face.eq_ignore_ascii_case(b"MS Shell Dlg")
        {
            // The shell/default logical face maps to the available virtual
            // Japanese font. Keep the caller's logical LOGFONT unchanged.
            realized.face_name_windows_932 = MS_GOTHIC_WINDOWS_932.to_vec();
        }
        if realized.character_set == 1 {
            realized.character_set = SHIFT_JIS_CHARSET;
        }
        if realized.height == 0 {
            realized.height = 16;
        }
        if realized.width == 0 {
            realized.width = realized.height.unsigned_abs().div_ceil(2).max(1) as i32;
        }
    }
    let selected_font = &realized;
    if selected_font.character_set != SHIFT_JIS_CHARSET
        || selected_font.face_name_windows_932 != MS_GOTHIC_WINDOWS_932
    {
        return Err(ApiFault::new(format!(
            "only MS Gothic metrics are available in the fixed Japanese environment: {selected_font:?}",
        )));
    }
    let bytes = outline_text_metrics_for_font(selected_font)?;
    if unicode {
        let mut wide = vec![0; 216];
        wide[..48].copy_from_slice(&bytes[..48]);
        for (index, character) in [0_u16, 0xffee, 0xff65, 2].into_iter().enumerate() {
            wide[48 + index * 2..50 + index * 2].copy_from_slice(&character.to_le_bytes());
        }
        wide[56..216].copy_from_slice(&bytes[52..212]);
        for (index, name) in [
            "ＭＳ ゴシック",
            "ＭＳ ゴシック",
            "標準",
            "Microsoft:ＭＳ ゴシック",
        ]
        .into_iter()
        .enumerate()
        {
            let offset = wide.len() as u32;
            write_u32(&mut wide, 200 + index * 4, offset);
            wide.extend(name.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
        }
        let total = wide.len() as u32;
        write_u32(&mut wide, 0, total);
        if requested_byte_length == 0 || output.0 == 0 {
            return Ok(ApiImmediate::new(Some(total)));
        }
        if requested_byte_length <= 216 {
            wide[200..216].fill(0);
            wide.truncate(requested_byte_length as usize);
        } else if requested_byte_length < total {
            // W does not publish a partial fixed header or set LastError here.
            return Ok(ApiImmediate::new(Some(0)));
        }
        return Ok(ApiImmediate::new(Some(wide.len() as u32)).with_guest_write(output, wide));
    }
    complete_outline_metrics(requested_byte_length, output, &bytes)
}

// A single virtual MS Gothic font: scale its fixed metrics by the LOGFONT
// geometry, rather than selecting measured records for particular game sizes.
fn outline_text_metrics_for_font(font: &RuntimeLogFontA) -> Result<Vec<u8>, ApiFault> {
    let height = font.height.unsigned_abs();
    let width = font.width as u32;
    if height == 0
        || height > 4096
        || width == 0
        || width > 4096
        || font.italic != 0
        || font.underline != 0
        || font.strike_out != 0
        || font.escapement != 0
        || font.orientation != 0
    {
        return Err(ApiFault::new(
            "outline metrics require a bounded upright MS Gothic font",
        ));
    }
    let ascent = ((height * 86 + 50) / 100) as i32;
    let descent = height as i32 - ascent;
    let em_width = width.min(height / 2).max(1) as i32;
    let h = height as i32;
    let w = width as i32;
    Ok(outline_text_metrics_a_bytes(&[
        (0, X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as i32),
        (4, h),
        (8, ascent),
        (12, descent),
        (24, w),
        (28, ((width * 395 + 50) / 100) as i32),
        (32, 400),
        (40, 96),
        (44, 96),
        (80, 1),
        (96, ascent),
        (100, -descent),
        (108, em_width),
        (112, em_width / 2),
        (116, -(((width * 250 + 64) / 128) as i32)),
        (120, ascent),
        (124, w * 2),
        (128, -descent),
        (132, ascent),
        (136, -descent),
        (144, 25),
        (148, w),
        (152, em_width),
        (160, ((height * 36 + 50) / 100) as i32),
        (164, w),
        (168, em_width),
        (176, ascent),
        (180, ((height * 51 + 500) / 1000) as i32),
        (184, ((height * 26 + 50) / 100) as i32),
        (188, ((height * 75 + 500) / 1000) as i32),
        (192, -(((height * 68 + 500) / 1000) as i32)),
    ]))
}

fn complete_outline_metrics(
    requested: u32,
    output: GuestAddress,
    bytes: &[u8],
) -> Result<ApiImmediate, ApiFault> {
    let total = X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH as u32;
    if requested == 0 {
        return Ok(ApiImmediate::new(Some(total)));
    }
    if output.0 == 0 {
        return Err(ApiFault::new(
            "GetOutlineTextMetricsA requires an output buffer when cbData is nonzero",
        ));
    }
    let fixed_length = X86_OUTLINE_TEXT_METRIC_A_FIXED_BYTE_LENGTH as u32;
    let (result, error, written) = if requested <= fixed_length {
        let mut fixed = bytes[..fixed_length as usize].to_vec();
        for offset in [196, 200, 204, 208] {
            write_u32(&mut fixed, offset, 0);
        }
        write_u32(&mut fixed, 0, if requested < 4 { 0x122 } else { requested });
        fixed.truncate(requested as usize);
        (requested, None, fixed)
    } else if requested < total {
        let mut fixed = bytes[..fixed_length as usize].to_vec();
        write_u32(&mut fixed, 0, 0);
        for offset in [196, 200, 204, 208] {
            write_u32(&mut fixed, offset, 0);
        }
        (0, Some(ERROR_CAN_NOT_COMPLETE), fixed)
    } else {
        (total, None, bytes.to_vec())
    };
    let mut completion = ApiImmediate::new(Some(result)).with_guest_write(output, written);
    completion.last_error = error;
    Ok(completion)
}

fn handle_text_out_a(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let result = (|| -> Result<ApiHandlerResult, ApiFault> {
        let dc = required_argument(frame, 0, "device context")?;
        let x = required_argument(frame, 1, "x")? as i32;
        let y = required_argument(frame, 2, "y")? as i32;
        let text_address = GuestAddress(required_argument(frame, 3, "ANSI text")?);
        let count = required_argument(frame, 4, "character count")?;
        let Some(target) = runtime
            .get_gdi_text_raster_target(dc)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        else {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(Some(u32::from(false))).with_last_error(ERROR_INVALID_HANDLE),
            ));
        };
        if count == 0 {
            return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(
                u32::from(true),
            ))));
        }
        if matches!(target.descriptor.bits_per_pixel, 16 | 24 | 32) {
            return crate::text_layout::text_out_a(frame, runtime);
        }
        if x != 0
            || y != 0
            || !(1..=2).contains(&count)
            || target.log_font.character_set != SHIFT_JIS_CHARSET
            || target.log_font.face_name_windows_932 != MS_GOTHIC_WINDOWS_932
            || target.log_font.height == 0
            || target.log_font.height.unsigned_abs() > 4096
            || !(1..=4096).contains(&target.log_font.width)
            || target.log_font.escapement != 0
            || target.log_font.orientation != 0
            || target.log_font.italic != 0
            || target.log_font.underline != 0
            || target.log_font.strike_out != 0
            || target.descriptor.height >= 0
            || target.descriptor.pixel_byte_length > 2 * 1024 * 1024
            || target.descriptor.palette.len() != X86_DIB_RGBQUAD_COUNT
            || !target
                .descriptor
                .palette
                .iter()
                .enumerate()
                .all(|(index, color)| {
                    let gray = 255 - index as u8;
                    color.red == gray && color.green == gray && color.blue == gray
                })
        {
            return Err(ApiFault::new(
                "TextOutA supports a bounded top-down 8bpp inverse-grayscale DIB and a CP932 MS Gothic glyph at the origin",
            ));
        }
        if text_address.0 == 0 {
            return Err(ApiFault::new(
                "TextOutA requires readable text when the character count is nonzero",
            ));
        }
        let text = runtime
            .memory
            .read(text_address, count as usize)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        if !valid_windows_932_glyph(&text) {
            return Err(ApiFault::new("TextOutA requires one valid CP932 glyph"));
        }
        let pixels = target.descriptor.pixel_byte_length as usize;
        runtime
            .memory
            .validate_access(target.backing.range.start(), pixels, MemoryAccess::Write)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        if runtime
            .memory
            .read(target.backing.range.start(), pixels)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?
            .iter()
            .any(|&pixel| pixel != 0)
        {
            return Err(ApiFault::new(
                "TextOutA currently requires a cleared DIB; overlay rasterization is not implemented",
            ));
        }
        let payload = Gdi32TextRasterHostRequest::from_target(&target, &text).encode();
        let request = runtime
            .begin_host_request(RUNTIME_GDI_TEXT_RASTER_SERVICE)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        Ok(ApiHandlerResult::Deferred(HostRequest {
            request_id: request.request_id,
            service: RUNTIME_GDI_TEXT_RASTER_SERVICE.to_owned(),
            payload,
        }))
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn valid_windows_932_glyph(text: &[u8]) -> bool {
    match text {
        [byte] => matches!(*byte, 0x20..=0x7e | 0xa1..=0xdf),
        [lead, trail] => {
            matches!(*lead, 0x81..=0x9f | 0xe0..=0xfc)
                && matches!(*trail, 0x40..=0x7e | 0x80..=0xfc)
        }
        _ => false,
    }
}

pub fn complete_gdi32_text_raster(
    dispatcher: &ApiDispatcher,
    call: &ApiDeferredCall,
    response: Gdi32TextRasterHostResponse,
    runtime: &mut crate::runtime::Runtime,
) -> (DispatchOutcome, Option<RuntimeHostCompletion>) {
    if call
        .frame
        .endpoint
        .module
        .eq_ignore_ascii_case("user32.dll")
        || runtime.text_layouts.contains_key(&response.request_id)
    {
        return crate::text_layout::complete(dispatcher, call, response, runtime);
    }
    if !call.frame.endpoint.module.eq_ignore_ascii_case("gdi32.dll")
        || !matches!(&call.frame.endpoint.symbol, ApiSymbol::Name { name } if name == "TextOutA")
        || call.request.request_id != response.request_id
        || call.request.service != RUNTIME_GDI_TEXT_RASTER_SERVICE
    {
        return (
            DispatchOutcome::Fault(ApiFault::new(
                "browser text raster completion does not match its suspended TextOutA call",
            )),
            None,
        );
    }
    let request = match Gdi32TextRasterHostRequest::decode(&call.request.payload) {
        Ok(request) => request,
        Err(fault) => return (DispatchOutcome::Fault(fault), None),
    };
    let endpoint = call.frame.endpoint.clone();
    let frame_matches = call.frame.argument_u32(0) == Some(request.device_context)
        && call.frame.argument_u32(1) == Some(0)
        && call.frame.argument_u32(2) == Some(0)
        && call.frame.argument_u32(4) == Some(request.character_count);
    let text_bytes = request.packed_text.to_le_bytes();
    let text_length = request.character_count as usize;
    if !frame_matches
        || !(1..=2).contains(&request.character_count)
        || text_bytes[text_length..].iter().any(|byte| *byte != 0)
        || !valid_windows_932_glyph(&text_bytes[..text_length])
    {
        return (
            DispatchOutcome::Fault(ApiFault::new(
                "browser text raster payload differs from the captured TextOutA call",
            )),
            None,
        );
    }
    let target = match runtime.pending_gdi_text_raster_target(
        response.request_id,
        RUNTIME_GDI_TEXT_RASTER_SERVICE,
        request.device_context,
    ) {
        Ok(target) => target,
        Err(error) => {
            return (
                DispatchOutcome::Fault(ApiFault::from_runtime(error, &endpoint)),
                None,
            );
        }
    };
    if !request.matches_target(&target)
        || response.pixels.len() != request.pixel_byte_length as usize
        || response.pixels.is_empty()
    {
        return (
            DispatchOutcome::Fault(ApiFault::new(
                "browser text raster response length or Rust-owned GDI identity is stale",
            )),
            None,
        );
    }
    let argument_words =
        match dispatcher.deferred_argument_words(call, response.request_id, &runtime.memory) {
            Ok(endpoint) => endpoint,
            Err(fault) => return (DispatchOutcome::Fault(fault), None),
        };
    let expected_bytes = vec![0; response.pixels.len()];
    let mut pixels = response.pixels;
    let background = if target.background_mode == 1 {
        0x00ff_ffff
    } else {
        target.background_color
    };
    for row in 0..target.descriptor.height.unsigned_abs() as usize {
        for column in 0..target.descriptor.width as usize {
            let index = row * target.descriptor.stride_bytes as usize + column;
            pixels[index] = grayscale_text_pixel(pixels[index], target.text_color, background);
        }
    }
    let runtime_completion = match runtime.commit_pending_gdi_text_raster(
        response.request_id,
        RUNTIME_GDI_TEXT_RASTER_SERVICE,
        target,
        0,
        &expected_bytes,
        &pixels,
    ) {
        Ok(result) => result,
        Err(error) => {
            return (
                DispatchOutcome::Fault(ApiFault::from_runtime(error, &endpoint)),
                None,
            );
        }
    };
    let immediate = ApiImmediate::new(Some(u32::from(true)));
    let outcome = match runtime.complete_api_immediate(argument_words, &call.frame, immediate) {
        Ok(completion) => DispatchOutcome::Immediate(completion),
        Err(fault) => DispatchOutcome::Fault(fault),
    };
    (outcome, Some(runtime_completion))
}

fn grayscale_text_pixel(coverage: u8, text: u32, background: u32) -> u8 {
    let coverage = u32::from(coverage);
    let channels = |color: u32| (color & 255) + ((color >> 8) & 255) + ((color >> 16) & 255);
    let intensity =
        (channels(text) * coverage + channels(background) * (255 - coverage) + 382) / 765;
    (255 - intensity) as u8
}

fn handle_select_object(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let device_context = required_argument(frame, 0, "device context")?;
    let object_handle = required_argument(frame, 1, "object")?;
    let previous = runtime
        .select_gdi_object(device_context, object_handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(previous.unwrap_or(0))))
}

fn handle_delete_object(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let object_handle = required_argument(frame, 0, "object")?;
    let deleted = runtime
        .delete_gdi_object(object_handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(u32::from(deleted))))
}

fn handle_create_font_a(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let request = decode_log_font_a(endpoint, frame, &runtime.memory)?;
    if request.height == 0
        || request.height.unsigned_abs() > 4096
        || !(0..=4096).contains(&request.width)
        || !(0..=1000).contains(&request.weight)
        || request.escapement != 0
        || request.orientation != 0
        || encoding_rs::SHIFT_JIS
            .decode_without_bom_handling(&request.face_name_windows_932)
            .1
    {
        return Err(ApiFault::new(
            "CreateFontA currently supports bounded upright Windows-932 fonts",
        ));
    }
    let handle = runtime
        .create_gdi_font_a(request)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(handle.unwrap_or(0))))
}

fn handle_create_font_indirect_w(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let address = GuestAddress(required_argument(frame, 0, "LOGFONTW")?);
    let ansi =
        matches!(&endpoint.symbol, ApiSymbol::Name { name } if name == "CreateFontIndirectA");
    let bytes = runtime
        .memory
        .read(address, if ansi { 60 } else { 92 })
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let units = if ansi {
        crate::win32::decode_windows_932(&bytes[28..])
    } else {
        bytes[28..]
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
            .collect::<Vec<_>>()
    };
    let end = units
        .iter()
        .position(|&u| u == 0)
        .ok_or_else(|| ApiFault::new("LOGFONTW face name is not terminated within LF_FACESIZE"))?;
    let face = String::from_utf16(&units[..end])
        .map_err(|_| ApiFault::new("LOGFONTW face name is malformed UTF-16"))?;
    let (face, _, invalid) = encoding_rs::SHIFT_JIS.encode(&face);
    if invalid || face.len() >= RUNTIME_LOG_FONT_A_FACE_CAPACITY {
        return Err(ApiFault::new(
            "logical font face requires bounded Windows-932 text",
        ));
    }
    let request = RuntimeLogFontA {
        height: read_i32(&bytes, 0),
        width: read_i32(&bytes, 4),
        escapement: read_i32(&bytes, 8),
        orientation: read_i32(&bytes, 12),
        weight: read_i32(&bytes, 16),
        italic: bytes[20],
        underline: bytes[21],
        strike_out: bytes[22],
        character_set: bytes[23],
        output_precision: bytes[24],
        clip_precision: bytes[25],
        quality: bytes[26],
        pitch_and_family: bytes[27],
        face_name_windows_932: face.into_owned(),
    };
    if request.height.unsigned_abs() > 4096
        || !(0..=4096).contains(&request.width)
        || !(0..=1000).contains(&request.weight)
    {
        return Err(ApiFault::new(
            "logical font dimensions or weight exceed supported bounds",
        ));
    }
    // Retain the logical request. Metrics and rasterization separately validate
    // whether the selected physical font and requested transformation are supported.
    let handle = runtime
        .create_gdi_font_a(request)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(handle.unwrap_or(0))))
}

fn decode_log_font_a(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    memory: &crate::memory::GuestMemory,
) -> Result<RuntimeLogFontA, ApiFault> {
    let face_address = required_argument(frame, 13, "ANSI face name")?;
    Ok(RuntimeLogFontA {
        height: required_argument(frame, 0, "height")? as i32,
        width: required_argument(frame, 1, "width")? as i32,
        escapement: required_argument(frame, 2, "escapement")? as i32,
        orientation: required_argument(frame, 3, "orientation")? as i32,
        weight: required_argument(frame, 4, "weight")? as i32,
        italic: required_byte_argument(frame, 5, "italic")?,
        underline: required_byte_argument(frame, 6, "underline")?,
        strike_out: required_byte_argument(frame, 7, "strike out")?,
        character_set: required_byte_argument(frame, 8, "character set")?,
        output_precision: required_byte_argument(frame, 9, "output precision")?,
        clip_precision: required_byte_argument(frame, 10, "clip precision")?,
        quality: required_byte_argument(frame, 11, "quality")?,
        pitch_and_family: required_byte_argument(frame, 12, "pitch and family")?,
        face_name_windows_932: if face_address == 0 {
            Vec::new()
        } else {
            read_font_face_name(endpoint, memory, GuestAddress(face_address))?
        },
    })
}

fn read_font_face_name(
    endpoint: &ApiEndpointIdentity,
    memory: &crate::memory::GuestMemory,
    address: GuestAddress,
) -> Result<Vec<u8>, ApiFault> {
    let mut bytes = Vec::with_capacity(RUNTIME_LOG_FONT_A_FACE_CAPACITY);
    for offset in 0..RUNTIME_LOG_FONT_A_FACE_CAPACITY {
        let offset = u32::try_from(offset).map_err(|_| {
            ApiFault::new("validated LF_FACESIZE does not fit x86 address accounting")
        })?;
        let current = address
            .0
            .checked_add(offset)
            .map(GuestAddress)
            .ok_or_else(|| {
                ApiFault::new("CreateFontA face-name read overflowed x86 address accounting")
            })?;
        let byte = memory
            .read(current, 1)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?[0];
        if byte == 0 {
            return Ok(bytes);
        }
        bytes.push(byte);
    }
    Err(ApiFault::new(
        "CreateFontA face name is not NUL-terminated within LF_FACESIZE",
    ))
}

fn handle_get_stock_object(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let index = frame
        .argument_u32(0)
        .ok_or_else(|| ApiFault::new("decoded GetStockObject frame is missing object index 0"))?
        as i32;
    match runtime.get_gdi_stock_object(index) {
        Ok(handle) => Ok(ApiImmediate::new(Some(handle.unwrap_or(0)))),
        Err(error) => Err(ApiFault::from_runtime(error, endpoint)),
    }
}

fn handle_enum_fonts_w(
    callback_return_sentinel: GuestAddress,
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> ApiHandlerResult {
    let result = (|| {
        let device_context = required_argument(frame, 0, "device context")?;
        let face_address = required_argument(frame, 1, "typeface name")?;
        let callback = GuestAddress(required_argument(frame, 2, "font enumeration callback")?);
        let parameter = required_argument(frame, 3, "callback parameter")?;
        let valid_dc = runtime
            .gdi_device_context_option(device_context)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
            .is_some_and(|dc| dc.owner_thread_id == runtime.current_thread_id());
        if !valid_dc {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(Some(0)).with_last_error(ERROR_INVALID_HANDLE),
            ));
        }
        if face_address != 0 {
            let face = crate::win32::read_utf16_nul(
                endpoint,
                runtime.memory(),
                GuestAddress(face_address),
                RUNTIME_LOG_FONT_A_FACE_CAPACITY,
            )?;
            if face != "ＭＳ ゴシック" && !face.eq_ignore_ascii_case("MS Gothic") {
                // Native EnumFonts succeeds without calling back for an empty
                // or unavailable face. Memory-added fonts are not enumerable.
                return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(1))));
            }
        }
        runtime
            .memory
            .validate_access(callback, 1, MemoryAccess::Execute)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;

        // Enumerate the same virtual font used by the ANSI GDI metrics path.
        // LOGFONTW is 92 bytes; NEWTEXTMETRICW is 76 bytes on PE32.
        let ansi_font = enum_log_font_ex_a_bytes();
        let ansi_metrics = new_text_metric_ex_a_bytes();
        let mut scratch = vec![0; 92 + 76];
        scratch[..28].copy_from_slice(&ansi_font[..28]);
        for (index, unit) in "ＭＳ ゴシック".encode_utf16().enumerate() {
            scratch[28 + index * 2..30 + index * 2].copy_from_slice(&unit.to_le_bytes());
        }
        let metrics = &mut scratch[92..];
        metrics[..44].copy_from_slice(&ansi_metrics[..44]);
        for (index, character) in [0_u16, 0xffee, 0xff65, 2].into_iter().enumerate() {
            metrics[44 + index * 2..46 + index * 2].copy_from_slice(&character.to_le_bytes());
        }
        metrics[52..57].copy_from_slice(&ansi_metrics[48..53]);
        metrics[60..76].copy_from_slice(&ansi_metrics[56..72]);
        Ok(ApiHandlerResult::GuestCallback(Box::new(
            GuestCallbackRequest {
                target: callback,
                arguments: vec![
                    GuestCallbackArgument::ScratchPointer { offset: 0 },
                    GuestCallbackArgument::ScratchPointer { offset: 92 },
                    GuestCallbackArgument::U32(TRUETYPE_FONTTYPE),
                    GuestCallbackArgument::U32(parameter),
                ],
                scratch,
                scratch_pointer_relocations: Vec::new(),
                return_sentinel: callback_return_sentinel,
                completion_policy: GuestCallbackCompletionPolicy::ReturnU32,
            },
        )))
    })();
    result.unwrap_or_else(ApiHandlerResult::Fault)
}

fn handle_enum_font_families_ex_a(
    callback_return_sentinel: GuestAddress,
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
) -> ApiHandlerResult {
    let result = (|| {
        let device_context = required_argument(frame, 0, "device context")?;
        let log_font_address = required_argument(frame, 1, "LOGFONTA filter")?;
        let callback = required_argument(frame, 2, "font enumeration callback")?;
        let callback_parameter = required_argument(frame, 3, "callback parameter")?;
        let flags = required_argument(frame, 4, "enumeration flags")?;
        if flags != 0 {
            return Err(ApiFault::new(
                "EnumFontFamiliesExA currently supports only the reached zero-flags variant",
            ));
        }
        if callback == 0 {
            return Err(ApiFault::new("EnumFontFamiliesExA requires a callback"));
        }
        let log_font = runtime
            .memory
            .read(
                GuestAddress(log_font_address),
                X86_LOGFONTA_BYTE_LENGTH as usize,
            )
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        let face_storage = log_font.get(28..60).ok_or_else(|| {
            ApiFault::new("validated LOGFONTA bytes do not contain the fixed face-name field")
        })?;
        let face_length = face_storage
            .iter()
            .position(|&byte| byte == 0)
            .ok_or_else(|| {
                ApiFault::new(
                    "EnumFontFamiliesExA face filter is not NUL-terminated within LF_FACESIZE",
                )
            })?;
        if log_font[23] != SHIFT_JIS_CHARSET
            || face_storage[..face_length] != *MS_GOTHIC_WINDOWS_932
        {
            return Err(ApiFault::new(
                "EnumFontFamiliesExA currently supports only the reached CP932 MS Gothic filter",
            ));
        }
        runtime
            .memory
            .validate_access(GuestAddress(callback), 1, MemoryAccess::Execute)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        runtime
            .validate_screen_dc_operation(device_context)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;

        let mut scratch =
            Vec::with_capacity(X86_ENUMLOGFONTEXA_BYTE_LENGTH + X86_NEWTEXTMETRICEXA_BYTE_LENGTH);
        scratch.extend_from_slice(&enum_log_font_ex_a_bytes());
        scratch.extend_from_slice(&new_text_metric_ex_a_bytes());
        Ok(GuestCallbackRequest {
            target: GuestAddress(callback),
            arguments: vec![
                GuestCallbackArgument::ScratchPointer { offset: 0 },
                GuestCallbackArgument::ScratchPointer {
                    offset: X86_ENUMLOGFONTEXA_BYTE_LENGTH as u32,
                },
                GuestCallbackArgument::U32(TRUETYPE_FONTTYPE),
                GuestCallbackArgument::U32(callback_parameter),
            ],
            scratch,
            scratch_pointer_relocations: Vec::new(),
            return_sentinel: callback_return_sentinel,
            completion_policy: GuestCallbackCompletionPolicy::ReturnU32,
        })
    })();
    match result {
        Ok(request) => ApiHandlerResult::GuestCallback(Box::new(request)),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

pub(crate) fn gdi32_argument_words(endpoint: &ApiEndpointIdentity) -> Option<u8> {
    if !endpoint.module.eq_ignore_ascii_case("gdi32.dll") {
        return None;
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return None;
    };
    Some(match name.as_str() {
        "GetStockObject"
        | "CreateCompatibleDC"
        | "DeleteObject"
        | "DeleteDC"
        | "CreateFontIndirectW"
        | "CreateFontIndirectA"
        | "RemoveFontMemResourceEx" => 1,
        "SelectObject" | "SetTextColor" | "SetBkColor" | "SetBkMode" => 2,
        "GetObjectA"
        | "GetObjectW"
        | "GetOutlineTextMetricsA"
        | "GetOutlineTextMetricsW"
        | "CreateCompatibleBitmap"
        | "GetPixel" => 3,
        "SetPixel" | "AddFontMemResourceEx" | "EnumFontsW" => 4,
        "EnumFontFamiliesExA" | "TextOutA" => 5,
        "CreateDIBSection" => 6,
        "CreateFontA" => 14,
        _ => return None,
    })
}

fn outline_text_metrics_a_bytes(fixed_fields: &[(usize, i32)]) -> Vec<u8> {
    let mut bytes = vec![0; X86_OUTLINE_TEXT_METRIC_A_BYTE_LENGTH];
    for &(offset, value) in fixed_fields {
        write_i32(&mut bytes, offset, value);
    }
    bytes[48] = 0;
    bytes[49] = 255;
    bytes[50] = 165;
    bytes[51] = 2;
    bytes[55] = 54;
    bytes[56] = SHIFT_JIS_CHARSET;
    bytes[61..71].copy_from_slice(&[2, 11, 6, 9, 7, 2, 5, 8, 2, 4]);
    write_u32(&mut bytes, 72, 64);
    write_u32(&mut bytes, 76, 8);
    write_u32(&mut bytes, 92, 256);
    for (offset, value) in [(196, 212_u32), (200, 226), (204, 240), (208, 245)] {
        write_u32(&mut bytes, offset, value);
    }
    write_fixed_ansi(&mut bytes, 212, 14, MS_GOTHIC_WINDOWS_932);
    write_fixed_ansi(&mut bytes, 226, 14, MS_GOTHIC_WINDOWS_932);
    write_fixed_ansi(&mut bytes, 240, 5, REGULAR_WINDOWS_932);
    bytes[245..245 + MICROSOFT_PREFIX.len()].copy_from_slice(MICROSOFT_PREFIX);
    bytes[245 + MICROSOFT_PREFIX.len()..268].copy_from_slice(MS_GOTHIC_WINDOWS_932);
    bytes
}

fn enum_log_font_ex_a_bytes() -> Vec<u8> {
    let mut bytes = vec![0; X86_ENUMLOGFONTEXA_BYTE_LENGTH];
    write_i32(&mut bytes, 0, 32);
    write_i32(&mut bytes, 4, 16);
    write_i32(&mut bytes, 16, 400);
    bytes[23] = SHIFT_JIS_CHARSET;
    bytes[24] = 3;
    bytes[25] = 2;
    bytes[26] = 1;
    bytes[27] = 49;
    write_fixed_ansi(&mut bytes, 28, 32, MS_GOTHIC_WINDOWS_932);
    write_fixed_ansi(&mut bytes, 60, 64, MS_GOTHIC_WINDOWS_932);
    write_fixed_ansi(&mut bytes, 124, 32, REGULAR_WINDOWS_932);
    write_fixed_ansi(&mut bytes, 156, 32, JAPANESE_WINDOWS_932);
    bytes
}

fn new_text_metric_ex_a_bytes() -> Vec<u8> {
    let mut bytes = vec![0; X86_NEWTEXTMETRICEXA_BYTE_LENGTH];
    for (offset, value) in [
        (0, 32_i32),
        (4, 28),
        (8, 4),
        (20, 16),
        (24, 63),
        (28, 400),
        (36, 96),
        (40, 96),
    ] {
        write_i32(&mut bytes, offset, value);
    }
    bytes[44] = 0;
    bytes[45] = 255;
    bytes[46] = 165;
    bytes[47] = 2;
    bytes[51] = 54;
    bytes[52] = SHIFT_JIS_CHARSET;
    write_u32(&mut bytes, 56, 2_359_360);
    write_u32(&mut bytes, 60, 256);
    write_u32(&mut bytes, 64, 256);
    write_u32(&mut bytes, 68, 128);
    bytes[72..96].copy_from_slice(&FONT_SIGNATURE);
    bytes
}

fn write_i32(bytes: &mut [u8], offset: usize, value: i32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("validated GDI structure contains a u16 field"),
    )
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated GDI structure contains a u32 field"),
    )
}

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated GDI structure contains an i32 field"),
    )
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_fixed_ansi(bytes: &mut [u8], offset: usize, capacity: usize, value: &[u8]) {
    debug_assert!(value.len() < capacity);
    bytes[offset..offset + value.len()].copy_from_slice(value);
}

fn required_argument(
    frame: &ApiCallFrame,
    index: usize,
    description: &str,
) -> Result<u32, ApiFault> {
    frame
        .argument_u32(index)
        .ok_or_else(|| ApiFault::new(format!("decoded GDI32 frame is missing {description}")))
}

fn required_byte_argument(
    frame: &ApiCallFrame,
    index: usize,
    description: &str,
) -> Result<u8, ApiFault> {
    let value = required_argument(frame, index, description)?;
    u8::try_from(value).map_err(|_| {
        ApiFault::new(format!(
            "CreateFontA {description} is outside the supported byte-sized range"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_raster_decoder_checks_fixed_bounds_and_decodes_live_identity() {
        let byte_length = Gdi32TextRasterHostRequest::WORD_COUNT * 4;
        for length in [0, byte_length - 1, byte_length + 1] {
            assert!(Gdi32TextRasterHostRequest::decode(&vec![0; length]).is_err());
        }
        let mut payload = vec![0; byte_length];
        payload[..4].copy_from_slice(&37_u32.to_le_bytes());
        payload[4..8].copy_from_slice(&5_u32.to_le_bytes());
        payload[8..12].copy_from_slice(&2_u32.to_le_bytes());
        let request = Gdi32TextRasterHostRequest::decode(&payload).unwrap();
        assert_eq!(request.device_context, 37);
        assert_eq!(request.selection_generation, (2_u64 << 32) | 5);
    }
}
