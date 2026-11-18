//! DrawText layout, clipping, selected-font identity and DIB writes belong to Rust.
//! Chrome only supplies coverage for the already positioned glyphs.
use crate::api::{
    ApiCallFrame, ApiDeferredCall, ApiDispatcher, ApiFault, ApiHandlerResult, ApiImmediate,
    ApiSymbol, DispatchOutcome, HostRequest,
};
use crate::gdi32::{Gdi32TextRasterHostResponse, RUNTIME_GDI_TEXT_RASTER_SERVICE};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::{Runtime, RuntimeGdiTextRasterTarget, RuntimeHostCompletion};

const MAGIC: u32 = 0x32545854;
const TEXT_LIMIT: usize = 8192;
#[derive(Debug, Clone, Copy)]
struct Glyph {
    character: char,
    x: i32,
    y: i32,
    width: i32,
    underline: bool,
}
#[derive(Debug)]
pub(crate) struct PendingTextLayout {
    target: RuntimeGdiTextRasterTarget,
    before: Vec<u8>,
    payload: Vec<u8>,
    glyphs: Vec<Glyph>,
    clip: [i32; 4],
    height: u32,
    result: u32,
}

fn text(frame: &ApiCallFrame, runtime: &Runtime, wide: bool) -> Result<String, ApiFault> {
    let pointer = frame.argument_u32(1).unwrap();
    let count = frame.argument_u32(2).unwrap();
    if count != u32::MAX && count as usize > TEXT_LIMIT {
        return Err(ApiFault::new("DrawText string exceeds its bound"));
    }
    if pointer == 0 && count != 0 {
        return Err(ApiFault::new("DrawText string is NULL"));
    }
    let unit = if wide { 2 } else { 1 };
    let mut bytes = Vec::new();
    for index in 0..if count == u32::MAX {
        TEXT_LIMIT + 1
    } else {
        count as usize
    } {
        let address = pointer
            .checked_add((index * unit) as u32)
            .ok_or_else(|| ApiFault::new("DrawText pointer overflow"))?;
        let value = runtime
            .memory
            .read(GuestAddress(address), unit)
            .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))?;
        if value.iter().all(|b| *b == 0) {
            break;
        }
        if index == TEXT_LIMIT {
            return Err(ApiFault::new(
                "DrawText string is not terminated within its bound",
            ));
        }
        bytes.extend(value);
    }
    if wide {
        String::from_utf16(
            &bytes
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
                .collect::<Vec<_>>(),
        )
        .map_err(|_| ApiFault::new("DrawTextW string contains invalid UTF-16"))
    } else {
        let (text, invalid) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(&bytes);
        if invalid {
            Err(ApiFault::new(
                "DrawTextA string contains invalid Windows-932",
            ))
        } else {
            Ok(text.into_owned())
        }
    }
}

fn cells(character: char) -> i32 {
    if character as u32 >= 0x1100 && !matches!(character as u32, 0xff61..=0xffdc) {
        2
    } else {
        1
    }
}

// The Runtime's virtual fixed-pitch font uses the same advances for A and W.
fn layout(
    text: &str,
    rect: [i32; 4],
    flags: u32,
    height: i32,
    width: i32,
) -> Result<(Vec<Glyph>, i32, i32), ApiFault> {
    let single = flags & 0x20 != 0;
    let mut lines: Vec<Vec<(char, bool)>> = vec![Vec::new()];
    let mut prefix = false;
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if flags & 0x800 == 0 && character == '&' {
            if chars.peek() == Some(&'&') {
                chars.next();
            } else {
                prefix = true;
                continue;
            }
        }
        if character == '\r' || character == '\n' {
            if character == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            if !single {
                lines.push(Vec::new());
                continue;
            }
        }
        let character = if matches!(character, '\r' | '\n') {
            ' '
        } else {
            character
        };
        let line = lines.last_mut().unwrap();
        if character == '\t' && flags & 0x40 != 0 {
            let length: i32 = line.iter().map(|(c, _)| cells(*c)).sum();
            for _ in 0..(8 - length % 8) {
                line.push((' ', false));
            }
        } else {
            line.push((character, prefix));
        }
        prefix = false;
        if flags & 0x10 != 0 && !single && rect[2] > rect[0] {
            let total: i32 = line.iter().map(|(c, _)| cells(*c) * width).sum();
            if total > rect[2] - rect[0] && line.len() > 1 {
                let split = line[..line.len() - 1]
                    .iter()
                    .rposition(|(c, _)| c.is_whitespace())
                    .map_or(line.len() - 1, |index| index + 1);
                let remainder = line.split_off(split);
                while line.last().is_some_and(|(c, _)| c.is_whitespace()) {
                    line.pop();
                }
                lines.push(remainder);
            }
        }
    }
    if text.is_empty() {
        return Ok((Vec::new(), 0, 0));
    }
    let total_height = (lines.len() as i32)
        .checked_mul(height)
        .ok_or_else(|| ApiFault::new("DrawText height overflow"))?;
    let max_width = lines
        .iter()
        .map(|line| line.iter().map(|(c, _)| cells(*c) * width).sum::<i32>())
        .max()
        .unwrap_or(0);
    let mut y = rect[1];
    if single && flags & 0x400 == 0 {
        if flags & 8 != 0 {
            y = rect[3]
                .checked_sub(total_height)
                .ok_or_else(|| ApiFault::new("DrawText vertical alignment overflow"))?;
        } else if flags & 4 != 0 {
            y = y
                .checked_add(
                    ((i64::from(rect[3]) - i64::from(y) - i64::from(total_height)) / 2) as i32,
                )
                .ok_or_else(|| ApiFault::new("DrawText vertical alignment overflow"))?;
        }
    }
    let mut glyphs = Vec::new();
    for line in lines {
        let line_width: i32 = line.iter().map(|(c, _)| cells(*c) * width).sum();
        let mut x = rect[0];
        if flags & 0x400 == 0 {
            if flags & 2 != 0 {
                x = rect[2]
                    .checked_sub(line_width)
                    .ok_or_else(|| ApiFault::new("DrawText alignment overflow"))?;
            } else if flags & 1 != 0 {
                x = x
                    .checked_add(
                        ((i64::from(rect[2]) - i64::from(x) - i64::from(line_width)) / 2) as i32,
                    )
                    .ok_or_else(|| ApiFault::new("DrawText alignment overflow"))?;
            }
        }
        for (character, underline) in line {
            let advance = cells(character) * width;
            glyphs.push(Glyph {
                character,
                x,
                y,
                width: advance,
                underline,
            });
            x = x
                .checked_add(advance)
                .ok_or_else(|| ApiFault::new("DrawText position overflow"))?;
        }
        y = y
            .checked_add(height)
            .ok_or_else(|| ApiFault::new("DrawText position overflow"))?;
    }
    Ok((glyphs, max_width, total_height))
}

pub(crate) fn draw_text(
    frame: &ApiCallFrame,
    runtime: &mut Runtime,
) -> Result<ApiHandlerResult, ApiFault> {
    let dc = frame.argument_u32(0).unwrap();
    let output = GuestAddress(frame.argument_u32(3).unwrap());
    let flags = frame.argument_u32(4).unwrap();
    if flags & !0xf7f != 0 {
        return Err(ApiFault::new(
            "DrawText format requires unsupported tab-stop, ellipsis, edit-control or bidi behavior",
        ));
    }
    runtime
        .memory
        .validate_access(output, 16, MemoryAccess::Read)
        .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))?;
    let bytes = runtime
        .memory
        .read(output, 16)
        .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))?;
    let rect: [i32; 4] =
        std::array::from_fn(|i| i32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap()));
    let font = runtime
        .get_gdi_text_font(dc)
        .map_err(|e| ApiFault::from_runtime(e, &frame.endpoint))?
        .clone();
    let height = font.height.unsigned_abs();
    let width = if font.width == 0 {
        height.div_ceil(2)
    } else {
        font.width.unsigned_abs()
    };
    if height == 0
        || height > 4096
        || width == 0
        || width > 4096
        || font.escapement != 0
        || font.orientation != 0
    {
        return Err(ApiFault::new("DrawText requires a bounded unrotated font"));
    }
    let wide = matches!(&frame.endpoint.symbol,ApiSymbol::Name {name} if name.ends_with('W'));
    let text = text(frame, runtime, wide)?;
    let (glyphs, text_width, text_height) =
        layout(&text, rect, flags, height as i32, width as i32)?;
    if flags & 0x400 != 0 {
        let right = rect[0]
            .checked_add(text_width)
            .ok_or_else(|| ApiFault::new("DrawText rectangle overflow"))?;
        let bottom = rect[1]
            .checked_add(text_height)
            .ok_or_else(|| ApiFault::new("DrawText rectangle overflow"))?;
        runtime
            .memory
            .validate_access(output, 16, MemoryAccess::Write)
            .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))?;
        return Ok(ApiHandlerResult::Immediate(
            ApiImmediate::new(Some(text_height as u32)).with_guest_write(
                output,
                [rect[0], rect[1], right, bottom]
                    .into_iter()
                    .flat_map(i32::to_le_bytes)
                    .collect(),
            ),
        ));
    }
    if glyphs.is_empty() {
        return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(0))));
    }
    let Some(target) = runtime
        .get_gdi_text_raster_target(dc)
        .map_err(|e| ApiFault::from_runtime(e, &frame.endpoint))?
    else {
        return Ok(ApiHandlerResult::Immediate(
            ApiImmediate::new(Some(0)).with_last_error(6),
        ));
    };
    let descriptor = &target.descriptor;
    if descriptor.pixel_byte_length > 2 * 1024 * 1024
        || !matches!(descriptor.bits_per_pixel, 8 | 16 | 24 | 32)
        || descriptor.bits_per_pixel == 8
            && (descriptor.palette.len() != 256
                || !(descriptor
                    .palette
                    .iter()
                    .enumerate()
                    .all(|(index, c)| c.red == c.green && c.red == c.blue && c.red == index as u8)
                    || descriptor.palette.iter().enumerate().all(|(index, c)| {
                        c.red == c.green && c.red == c.blue && c.red == 255 - index as u8
                    })))
    {
        return Err(ApiFault::new(
            "DrawText requires a bounded true-color or grayscale DIB",
        ));
    }
    runtime
        .memory
        .validate_access(
            target.backing.range.start(),
            descriptor.pixel_byte_length as usize,
            MemoryAccess::Write,
        )
        .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))?;
    let before = runtime
        .memory
        .read(
            target.backing.range.start(),
            descriptor.pixel_byte_length as usize,
        )
        .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))?
        .to_vec();
    let clip = if flags & 0x100 != 0 {
        [
            0,
            0,
            descriptor.width,
            descriptor.height.unsigned_abs() as i32,
        ]
    } else {
        [
            rect[0].max(0),
            rect[1].max(0),
            rect[2].min(descriptor.width),
            rect[3].min(descriptor.height.unsigned_abs() as i32),
        ]
    };
    let (face, invalid) =
        encoding_rs::SHIFT_JIS.decode_without_bom_handling(&font.face_name_windows_932);
    if invalid {
        return Err(ApiFault::new("DrawText font face is not Windows-932"));
    }
    let face: Vec<u32> = face.encode_utf16().map(u32::from).collect();
    let mut words = vec![
        MAGIC,
        descriptor.width as u32,
        descriptor.height.unsigned_abs(),
        height,
        font.weight as u32,
        u32::from(font.italic != 0),
        u32::from(font.underline != 0),
        u32::from(font.strike_out != 0),
        face.len() as u32,
        glyphs.len() as u32,
    ];
    words.extend(clip.map(|v| v as u32));
    words.extend(face);
    for glyph in &glyphs {
        words.extend([
            glyph.character as u32,
            glyph.x as u32,
            glyph.y as u32,
            glyph.width as u32,
            u32::from(glyph.underline),
        ]);
    }
    let payload: Vec<u8> = words.into_iter().flat_map(u32::to_le_bytes).collect();
    let request = runtime
        .begin_host_request(RUNTIME_GDI_TEXT_RASTER_SERVICE)
        .map_err(|e| ApiFault::from_runtime(e, &frame.endpoint))?;
    runtime.text_layouts.insert(
        request.request_id,
        PendingTextLayout {
            target,
            before,
            payload: payload.clone(),
            glyphs,
            clip,
            height,
            result: text_height as u32,
        },
    );
    Ok(ApiHandlerResult::Deferred(HostRequest {
        request_id: request.request_id,
        service: request.service,
        payload,
    }))
}

pub(crate) fn text_out_a(
    frame: &ApiCallFrame,
    runtime: &mut Runtime,
) -> Result<ApiHandlerResult, ApiFault> {
    let dc = frame.argument_u32(0).unwrap();
    let x = frame.argument_u32(1).unwrap() as i32;
    let y = frame.argument_u32(2).unwrap() as i32;
    let pointer = frame.argument_u32(3).unwrap();
    let count_i32 = frame.argument_u32(4).unwrap() as i32;
    if count_i32 <= 0 {
        return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(
            u32::from(count_i32 == 0),
        ))));
    }
    let count = count_i32 as usize;
    if count > TEXT_LIMIT {
        return Err(ApiFault::new("TextOutA string exceeds its bound"));
    }
    if pointer == 0 {
        return Err(ApiFault::new("TextOutA string is NULL"));
    }
    let bytes = runtime
        .memory
        .read(GuestAddress(pointer), count)
        .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))?;
    let (decoded, invalid) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(&bytes);
    if invalid {
        return Err(ApiFault::new(
            "TextOutA string contains invalid Windows-932",
        ));
    }
    let font = runtime
        .get_gdi_text_font(dc)
        .map_err(|e| ApiFault::from_runtime(e, &frame.endpoint))?
        .clone();
    let height = font.height.unsigned_abs();
    let width = if font.width == 0 {
        height.div_ceil(2)
    } else {
        font.width.unsigned_abs()
    };
    if height == 0
        || height > 4096
        || width == 0
        || width > 4096
        || font.escapement != 0
        || font.orientation != 0
    {
        return Err(ApiFault::new("TextOutA requires a bounded unrotated font"));
    }
    let Some(target) = runtime
        .get_gdi_text_raster_target(dc)
        .map_err(|e| ApiFault::from_runtime(e, &frame.endpoint))?
    else {
        return Ok(ApiHandlerResult::Immediate(
            ApiImmediate::new(Some(0)).with_last_error(6),
        ));
    };
    let descriptor = &target.descriptor;
    if descriptor.pixel_byte_length > 2 * 1024 * 1024
        || !matches!(descriptor.bits_per_pixel, 16 | 24 | 32)
    {
        return Err(ApiFault::new(
            "TextOutA requires a bounded 16/24/32-bit DIB",
        ));
    }
    let clip = [
        0,
        0,
        descriptor.width,
        descriptor.height.unsigned_abs() as i32,
    ];
    let (glyphs, _, _) = layout(
        &decoded,
        [
            x,
            y,
            descriptor.width,
            descriptor.height.unsigned_abs() as i32,
        ],
        0x20 | 0x100 | 0x800,
        height as i32,
        width as i32,
    )?;
    if glyphs.is_empty() {
        return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(1))));
    }
    runtime
        .memory
        .validate_access(
            target.backing.range.start(),
            descriptor.pixel_byte_length as usize,
            MemoryAccess::Write,
        )
        .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))?;
    let before = runtime
        .memory
        .read(
            target.backing.range.start(),
            descriptor.pixel_byte_length as usize,
        )
        .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))?
        .to_vec();
    let (face, invalid_face) =
        encoding_rs::SHIFT_JIS.decode_without_bom_handling(&font.face_name_windows_932);
    if invalid_face {
        return Err(ApiFault::new("TextOutA font face is not Windows-932"));
    }
    let face: Vec<u32> = face.encode_utf16().map(u32::from).collect();
    let mut words = vec![
        MAGIC,
        descriptor.width as u32,
        descriptor.height.unsigned_abs(),
        height,
        font.weight as u32,
        u32::from(font.italic != 0),
        u32::from(font.underline != 0),
        u32::from(font.strike_out != 0),
        face.len() as u32,
        glyphs.len() as u32,
    ];
    words.extend(clip.map(|v| v as u32));
    words.extend(face);
    for glyph in &glyphs {
        words.extend([
            glyph.character as u32,
            glyph.x as u32,
            glyph.y as u32,
            glyph.width as u32,
            u32::from(glyph.underline),
        ]);
    }
    let payload: Vec<u8> = words.into_iter().flat_map(u32::to_le_bytes).collect();
    let request = runtime
        .begin_host_request(RUNTIME_GDI_TEXT_RASTER_SERVICE)
        .map_err(|e| ApiFault::from_runtime(e, &frame.endpoint))?;
    runtime.text_layouts.insert(
        request.request_id,
        PendingTextLayout {
            target,
            before,
            payload: payload.clone(),
            glyphs,
            clip,
            height,
            result: 1,
        },
    );
    Ok(ApiHandlerResult::Deferred(HostRequest {
        request_id: request.request_id,
        service: request.service,
        payload,
    }))
}

pub(crate) fn complete(
    dispatcher: &ApiDispatcher,
    call: &ApiDeferredCall,
    response: Gdi32TextRasterHostResponse,
    runtime: &mut Runtime,
) -> (DispatchOutcome, Option<RuntimeHostCompletion>) {
    let result = (|| {
        let argument_words =
            dispatcher.deferred_argument_words(call, response.request_id, &runtime.memory)?;
        let pending = runtime
            .text_layouts
            .get(&response.request_id)
            .ok_or_else(|| ApiFault::new("DrawText raster is not pending"))?;
        if !matches!(
            &call.frame.endpoint.symbol,
            ApiSymbol::Name { name } if matches!(name.as_str(), "DrawTextA" | "DrawTextW" | "TextOutA")
        ) || call.request.service != RUNTIME_GDI_TEXT_RASTER_SERVICE
            || call.request.payload != pending.payload
        {
            return Err(ApiFault::new(
                "DrawText completion does not match its owned request",
            ));
        }
        let target = runtime
            .pending_gdi_text_raster_target(
                response.request_id,
                RUNTIME_GDI_TEXT_RASTER_SERVICE,
                pending.target.device_context_handle,
            )
            .map_err(|e| ApiFault::from_runtime(e, &call.frame.endpoint))?;
        let d = &target.descriptor;
        let (width, height) = (d.width as usize, d.height.unsigned_abs() as usize);
        if target != pending.target || response.pixels.len() != width * height {
            return Err(ApiFault::new(
                "DrawText target identity or coverage length changed",
            ));
        }
        let mut pixels = pending.before.clone();
        let bytes_per_pixel = d.bits_per_pixel as usize / 8;
        let inverse = d.bits_per_pixel == 8 && d.palette[0].red == 255;
        let mut opaque_pixels = vec![false; width * height];
        if target.background_mode == 2 {
            for glyph in &pending.glyphs {
                let left = i64::from(glyph.x).clamp(0, width as i64) as usize;
                let right =
                    (i64::from(glyph.x) + i64::from(glyph.width)).clamp(0, width as i64) as usize;
                let top = i64::from(glyph.y).clamp(0, height as i64) as usize;
                let bottom = (i64::from(glyph.y) + i64::from(pending.height))
                    .clamp(0, height as i64) as usize;
                for row in top..bottom {
                    opaque_pixels[row * width + left..row * width + right].fill(true);
                }
            }
        }
        for y in pending.clip[1].max(0) as usize..pending.clip[3].max(0) as usize {
            for x in pending.clip[0].max(0) as usize..pending.clip[2].max(0) as usize {
                let raw_coverage = u32::from(response.pixels[y * width + x]);
                let opaque = opaque_pixels[y * width + x];
                let coverage = if d.bits_per_pixel == 16 {
                    if raw_coverage >= 96 { 255 } else { 0 }
                } else {
                    raw_coverage
                };
                if coverage == 0 && !opaque {
                    continue;
                }
                let row = if d.height < 0 { y } else { height - 1 - y };
                let offset = row * d.stride_bytes as usize + x * bytes_per_pixel;
                let existing = match d.bits_per_pixel {
                    8 => {
                        let c = d.palette[pixels[offset] as usize];
                        u32::from(c.red) | u32::from(c.green) << 8 | u32::from(c.blue) << 16
                    }
                    16 => {
                        let word = u16::from_le_bytes([pixels[offset], pixels[offset + 1]]);
                        match d.color_masks.unwrap_or([0x7c00, 0x03e0, 0x001f]) {
                            [0xf800, 0x07e0, 0x001f] => {
                                let r = (u32::from((word >> 11) & 0x1f) * 255 + 15) / 31;
                                let g = (u32::from((word >> 5) & 0x3f) * 255 + 31) / 63;
                                let b = (u32::from(word & 0x1f) * 255 + 15) / 31;
                                r | (g << 8) | (b << 16)
                            }
                            [0x0f00, 0x00f0, 0x000f] => {
                                let r = u32::from((word >> 8) & 0x0f) * 17;
                                let g = u32::from((word >> 4) & 0x0f) * 17;
                                let b = u32::from(word & 0x0f) * 17;
                                r | (g << 8) | (b << 16)
                            }
                            _ => {
                                let r = (u32::from((word >> 10) & 0x1f) * 255 + 15) / 31;
                                let g = (u32::from((word >> 5) & 0x1f) * 255 + 15) / 31;
                                let b = (u32::from(word & 0x1f) * 255 + 15) / 31;
                                r | (g << 8) | (b << 16)
                            }
                        }
                    }
                    _ => {
                        u32::from(pixels[offset + 2])
                            | u32::from(pixels[offset + 1]) << 8
                            | u32::from(pixels[offset]) << 16
                    }
                };
                let background = if opaque {
                    target.background_color
                } else {
                    existing
                };
                let color: [u8; 3] = std::array::from_fn(|channel| {
                    let shift = channel * 8;
                    ((((target.text_color >> shift) & 255) * coverage
                        + ((background >> shift) & 255) * (255 - coverage)
                        + 127)
                        / 255) as u8
                });
                match d.bits_per_pixel {
                    8 => {
                        let gray =
                            ((u32::from(color[0]) + u32::from(color[1]) + u32::from(color[2]) + 1)
                                / 3) as u8;
                        pixels[offset] = if inverse { 255 - gray } else { gray };
                    }
                    16 => {
                        let word = match d.color_masks.unwrap_or([0x7c00, 0x03e0, 0x001f]) {
                            [0xf800, 0x07e0, 0x001f] => {
                                (((u16::from(color[0]) * 31 + 127) / 255) << 11)
                                    | (((u16::from(color[1]) * 63 + 127) / 255) << 5)
                                    | ((u16::from(color[2]) * 31 + 127) / 255)
                            }
                            [0x0f00, 0x00f0, 0x000f] => {
                                (((u16::from(color[0]) + 8) / 17) << 8)
                                    | (((u16::from(color[1]) + 8) / 17) << 4)
                                    | ((u16::from(color[2]) + 8) / 17)
                            }
                            _ => {
                                (((u16::from(color[0]) * 31 + 127) / 255) << 10)
                                    | (((u16::from(color[1]) * 31 + 127) / 255) << 5)
                                    | ((u16::from(color[2]) * 31 + 127) / 255)
                            }
                        };
                        pixels[offset..offset + 2].copy_from_slice(&word.to_le_bytes());
                    }
                    24 => {
                        pixels[offset..offset + 3].copy_from_slice(&[color[2], color[1], color[0]]);
                    }
                    32 => {
                        pixels[offset..offset + 4]
                            .copy_from_slice(&[color[2], color[1], color[0], 0]);
                    }
                    _ => unreachable!(),
                }
            }
        }
        let value = pending.result;
        let before = pending.before.clone();
        let host = runtime
            .commit_pending_gdi_text_raster(
                response.request_id,
                RUNTIME_GDI_TEXT_RASTER_SERVICE,
                target,
                0,
                &before,
                &pixels,
            )
            .map_err(|e| ApiFault::from_runtime(e, &call.frame.endpoint))?;
        runtime.text_layouts.remove(&response.request_id);
        let outcome = runtime
            .complete_api_immediate(argument_words, &call.frame, ApiImmediate::new(Some(value)))
            .map(DispatchOutcome::Immediate)
            .unwrap_or_else(DispatchOutcome::Fault);
        Ok((outcome, Some(host)))
    })();
    result.unwrap_or_else(|e| (DispatchOutcome::Fault(e), None))
}
