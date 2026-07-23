use serde::Serialize;

use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiDynamicPointerConstraint, ApiEndpointIdentity, ApiFault, ApiGuestWrite,
    ApiGuestWriteReceipt, ApiHandlerResult, ApiImmediate, ApiPointeeSchema, ApiPointerConstraint,
    ApiReturnSchema, ApiReturnValue, ApiStringEncoding, ApiSymbol, ApiValueSchema,
    ApiWriteRequirement, CallbackPolicy, CallingConvention, ExecutionSession,
    GuestCallbackArgument, GuestCallbackCompletionPolicy, GuestCallbackRequest, LastErrorPolicy,
    ReentrancyPolicy, StackCleanup,
};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::{
    LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE, LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE,
    RUNTIME_GDI_BITMAP_BYTE_LENGTH, RUNTIME_GDI_DEFAULT_BITMAP_INDEX, RUNTIME_GDI_DIB_BI_RGB,
    RUNTIME_GDI_DIB_RGB_COLORS, RUNTIME_GDI_SYSTEM_FONT_INDEX, RUNTIME_LOG_FONT_A_BYTE_LENGTH,
    RUNTIME_LOG_FONT_A_FACE_CAPACITY, RuntimeGdiDibSectionDescriptor, RuntimeGdiRgbQuad,
    RuntimeLogFontA, RuntimeStateErrorClass,
};

pub const GDI32_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.gdi32-compatibility-policy";
pub const GDI32_COMPATIBILITY_POLICY_VERSION: u32 = 4;
pub const GET_STOCK_OBJECT_IMPLEMENTATION: &str = "gdi32.get-stock-object.black-brush.v1";
pub const ENUM_FONT_FAMILIES_EX_A_IMPLEMENTATION: &str =
    "gdi32.enum-font-families-ex-a.cp932-screen.v1";
pub const CREATE_FONT_A_IMPLEMENTATION: &str = "gdi32.create-font-a.cp932-logical-object.v1";
pub const CREATE_COMPATIBLE_DC_IMPLEMENTATION: &str =
    "gdi32.create-compatible-dc.null-screen-memory.v1";
pub const DELETE_DC_IMPLEMENTATION: &str = "gdi32.delete-dc.logical-memory.v1";
pub const CREATE_DIB_SECTION_IMPLEMENTATION: &str = "gdi32.create-dib-section.primary-8bpp.v1";
pub const GET_OBJECT_A_IMPLEMENTATION: &str = "gdi32.get-object-a.font-or-dib.v1";
pub const SELECT_OBJECT_IMPLEMENTATION: &str = "gdi32.select-object.memory-font-bitmap.v1";
pub const DELETE_OBJECT_IMPLEMENTATION: &str = "gdi32.delete-object.font-or-dib.v1";
pub const X86_LOGFONTA_BYTE_LENGTH: u32 = RUNTIME_LOG_FONT_A_BYTE_LENGTH as u32;
pub const X86_BITMAPINFOHEADER_BYTE_LENGTH: u32 = 40;
pub const X86_BITMAP_BYTE_LENGTH: u32 = RUNTIME_GDI_BITMAP_BYTE_LENGTH as u32;
pub const X86_DIB_RGBQUAD_COUNT: usize = 256;
pub const X86_DIB_BITMAPINFO_BYTE_LENGTH: usize =
    X86_BITMAPINFOHEADER_BYTE_LENGTH as usize + X86_DIB_RGBQUAD_COUNT * 4;
pub const X86_ENUMLOGFONTEXA_BYTE_LENGTH: usize = 188;
pub const X86_NEWTEXTMETRICEXA_BYTE_LENGTH: usize = 96;

const SHIFT_JIS_CHARSET: u8 = 128;
const TRUETYPE_FONTTYPE: u32 = 0x0000_0004;
const MS_GOTHIC_WINDOWS_932: &[u8] = &[
    0x82, 0x6c, 0x82, 0x72, 0x20, 0x83, 0x53, 0x83, 0x56, 0x83, 0x62, 0x83, 0x4e,
];
const REGULAR_WINDOWS_932: &[u8] = &[0x95, 0x57, 0x8f, 0x80];
const JAPANESE_WINDOWS_932: &[u8] = &[0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea];
const FONT_SIGNATURE: [u8; 24] = [
    0xff, 0x02, 0x00, 0xe0, 0xfb, 0xfd, 0xc7, 0x6a, 0x12, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00,
    0x9f, 0x00, 0x02, 0x40, 0x00, 0x00, 0xd7, 0xdf,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gdi32FontRecordPolicy {
    pub font_type: u32,
    pub enum_log_font_ex_a: Vec<u8>,
    pub new_text_metric_ex_a: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gdi32FontEnumerationPolicy {
    pub profile_id: String,
    pub input_character_set: u8,
    pub input_face_name_windows_932: Vec<u8>,
    pub flags: u32,
    pub record: Gdi32FontRecordPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gdi32FontCreationPolicy {
    pub profile_id: String,
    pub log_font: RuntimeLogFontA,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gdi32CompatibleDcPolicy {
    pub profile_id: String,
    pub source_device_context: u32,
    pub initial_font_stock_index: i32,
    pub initial_bitmap_stock_index: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gdi32DibSectionPolicy {
    pub profile_id: String,
    pub bitmap_info_header_size: u32,
    pub size_image: u32,
    pub x_pixels_per_meter: i32,
    pub y_pixels_per_meter: i32,
    pub colors_used: u32,
    pub colors_important: u32,
    pub usage: u32,
    pub section_handle: u32,
    pub offset: u32,
    pub descriptor: RuntimeGdiDibSectionDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gdi32CompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub enum_font_families_ex_a: Option<Gdi32FontEnumerationPolicy>,
    pub create_font_a: Option<Gdi32FontCreationPolicy>,
    pub create_compatible_dc: Option<Gdi32CompatibleDcPolicy>,
    pub create_dib_section: Option<Gdi32DibSectionPolicy>,
}

impl Gdi32CompatibilityPolicy {
    pub fn controlled_v4() -> Self {
        Self {
            schema: GDI32_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: GDI32_COMPATIBILITY_POLICY_VERSION,
            id: "gdi32-primary-cp932-dib-lifecycle-v4".to_owned(),
            enum_font_families_ex_a: Some(controlled_font_enumeration_policy()),
            create_font_a: Some(controlled_font_creation_policy()),
            create_compatible_dc: Some(controlled_compatible_dc_policy()),
            create_dib_section: Some(controlled_dib_section_policy()),
        }
    }

    pub fn validate(&self) -> Result<(), ApiFault> {
        if self.schema != GDI32_COMPATIBILITY_POLICY_SCHEMA
            || self.schema_version != GDI32_COMPATIBILITY_POLICY_VERSION
            || !valid_policy_id(&self.id)
        {
            return Err(ApiFault::runtime_defect(
                "gdi32-compatibility-policy",
                "gdi32-policy-header-invalid",
                "GDI32 compatibility policy has an unsupported schema, version, or ID",
            ));
        }
        if let Some(enumeration) = self.enum_font_families_ex_a.as_ref()
            && enumeration != &controlled_font_enumeration_policy()
        {
            return Err(ApiFault::runtime_defect(
                "gdi32-compatibility-policy",
                "gdi32-font-enumeration-profile-invalid",
                "GDI32 font enumeration policy must match the evidenced single CP932 MS Gothic record",
            ));
        }
        if let Some(creation) = self.create_font_a.as_ref()
            && creation != &controlled_font_creation_policy()
        {
            return Err(ApiFault::runtime_defect(
                "gdi32-compatibility-policy",
                "gdi32-font-creation-profile-invalid",
                "GDI32 font creation policy must match the evidenced CP932 MS Gothic request",
            ));
        }
        if let Some(compatible_dc) = self.create_compatible_dc.as_ref()
            && compatible_dc != &controlled_compatible_dc_policy()
        {
            return Err(ApiFault::runtime_defect(
                "gdi32-compatibility-policy",
                "gdi32-compatible-dc-profile-invalid",
                "GDI32 compatible DC policy must match the evidenced NULL-screen memory DC request",
            ));
        }
        if let Some(dib_section) = self.create_dib_section.as_ref()
            && dib_section != &controlled_dib_section_policy()
        {
            return Err(ApiFault::runtime_defect(
                "gdi32-compatibility-policy",
                "gdi32-dib-section-profile-invalid",
                "GDI32 DIB section policy must match the evidenced 128x-96 reverse-grayscale profile",
            ));
        }
        Ok(())
    }
}

pub fn dispatch_gdi32_api(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    match contract.implementation_id.as_str() {
        GET_STOCK_OBJECT_IMPLEMENTATION => {
            match handle_get_stock_object(contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        DELETE_DC_IMPLEMENTATION => match handle_delete_dc(contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        GET_OBJECT_A_IMPLEMENTATION => match handle_get_object_a(contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        SELECT_OBJECT_IMPLEMENTATION => match handle_select_object(contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        DELETE_OBJECT_IMPLEMENTATION => match handle_delete_object(contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        _ => ApiHandlerResult::Fault(ApiFault::unsupported(
            "gdi32-api-dispatch",
            "implementation-missing",
            format!(
                "no GDI32 handler is registered for {}",
                contract.implementation_id
            ),
        )),
    }
}

pub fn dispatch_gdi32_api_with_policy(
    policy: &Gdi32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    match contract.implementation_id.as_str() {
        CREATE_COMPATIBLE_DC_IMPLEMENTATION => {
            match handle_create_compatible_dc(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        CREATE_FONT_A_IMPLEMENTATION => {
            match handle_create_font_a(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        CREATE_DIB_SECTION_IMPLEMENTATION => {
            match handle_create_dib_section(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        ENUM_FONT_FAMILIES_EX_A_IMPLEMENTATION => handle_enum_font_families_ex_a(
            policy,
            callback_return_sentinel,
            contract,
            frame,
            session,
        ),
        _ => dispatch_gdi32_api(contract, frame, session),
    }
}

fn handle_create_compatible_dc(
    policy: &Gdi32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let compatible_dc = policy.create_compatible_dc.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "gdi32-compatible-dc",
            "create-compatible-dc-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical memory DC creation",
                policy.id
            ),
        )
    })?;
    let source_device_context = required_argument(frame, 0, "source device context")?;
    if source_device_context != compatible_dc.source_device_context {
        return Err(ApiFault::unsupported(
            "gdi32-compatible-dc",
            "create-compatible-dc-source-unsupported",
            "CreateCompatibleDC currently supports only the evidenced NULL screen source",
        ));
    }
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let result = session
        .runtime_mut()
        .create_compatible_memory_dc()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE.to_owned(),
        value: result.handle,
    })
    .with_last_error(preserved_last_error))
}

fn handle_delete_dc(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let device_context = required_argument(frame, 0, "device context")?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session
        .runtime_mut()
        .delete_compatible_memory_dc(device_context)
    {
        Ok(()) => {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
                .with_last_error(preserved_last_error))
        }
        Err(error)
            if error.code == "device-context-handle-unknown"
                || error.code == "delete-dc-relation-mismatch" =>
        {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(preserved_last_error))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_create_dib_section(
    policy: &Gdi32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let dib_policy = policy.create_dib_section.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "gdi32-dib-section",
            "create-dib-section-policy-disabled",
            format!(
                "compatibility policy {} does not enable DIB section creation",
                policy.id
            ),
        )
    })?;
    let device_context = required_argument(frame, 0, "device context")?;
    let bitmap_info_address = GuestAddress(required_argument(frame, 1, "BITMAPINFO")?);
    let usage = required_argument(frame, 2, "color usage")?;
    let bits_output = GuestAddress(required_argument(frame, 3, "bits output")?);
    let section_handle = required_argument(frame, 4, "section handle")?;
    let offset = required_argument(frame, 5, "section offset")?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let header = session
        .memory()
        .read(
            bitmap_info_address,
            X86_BITMAPINFOHEADER_BYTE_LENGTH as usize,
        )
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let header_size = read_u32(&header, 0);
    let width = read_i32(&header, 4);
    let height = read_i32(&header, 8);
    let planes = read_u16(&header, 12);
    let bits_per_pixel = read_u16(&header, 14);
    let compression = read_u32(&header, 16);
    let size_image = read_u32(&header, 20);
    let x_pixels_per_meter = read_i32(&header, 24);
    let y_pixels_per_meter = read_i32(&header, 28);
    let colors_used = read_u32(&header, 32);
    let colors_important = read_u32(&header, 36);
    if header_size != X86_BITMAPINFOHEADER_BYTE_LENGTH
        || width <= 0
        || height == 0
        || height == i32::MIN
        || planes == 0
        || bits_per_pixel == 0
    {
        return Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
            namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
            value: 0,
        })
        .with_guest_write(bits_output, 0_u32.to_le_bytes().to_vec())
        .with_last_error(preserved_last_error));
    }
    if usage != dib_policy.usage
        || section_handle != dib_policy.section_handle
        || offset != dib_policy.offset
        || header_size != dib_policy.bitmap_info_header_size
        || size_image != dib_policy.size_image
        || x_pixels_per_meter != dib_policy.x_pixels_per_meter
        || y_pixels_per_meter != dib_policy.y_pixels_per_meter
        || colors_used != dib_policy.colors_used
        || colors_important != dib_policy.colors_important
        || compression != RUNTIME_GDI_DIB_BI_RGB
        || usage != RUNTIME_GDI_DIB_RGB_COLORS
    {
        return Err(ApiFault::unsupported(
            "gdi32-dib-section",
            "create-dib-section-variant-unsupported",
            "CreateDIBSection currently supports only the evidenced RGB DIB request",
        ));
    }
    let bitmap_info = session
        .memory()
        .read(bitmap_info_address, X86_DIB_BITMAPINFO_BYTE_LENGTH)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let palette = bitmap_info[X86_BITMAPINFOHEADER_BYTE_LENGTH as usize..]
        .chunks_exact(4)
        .map(|quad| RuntimeGdiRgbQuad {
            blue: quad[0],
            green: quad[1],
            red: quad[2],
            reserved: quad[3],
        })
        .collect::<Vec<_>>();
    let width_u32 = u32::try_from(width).map_err(|_| {
        ApiFault::invalid_call(
            "gdi32-dib-section",
            "create-dib-section-width-invalid",
            "CreateDIBSection width does not fit unsigned pixel accounting",
        )
    })?;
    let height_u32 = u32::try_from(height.abs()).map_err(|_| {
        ApiFault::invalid_call(
            "gdi32-dib-section",
            "create-dib-section-height-invalid",
            "CreateDIBSection height does not fit unsigned pixel accounting",
        )
    })?;
    let stride_bytes = width_u32
        .checked_mul(u32::from(bits_per_pixel))
        .and_then(|bits| bits.checked_add(31))
        .map(|bits| bits / 32)
        .and_then(|words| words.checked_mul(4))
        .ok_or_else(|| {
            ApiFault::invalid_call(
                "gdi32-dib-section",
                "create-dib-section-stride-overflow",
                "CreateDIBSection stride overflowed x86 accounting",
            )
        })?;
    let pixel_byte_length = stride_bytes.checked_mul(height_u32).ok_or_else(|| {
        ApiFault::invalid_call(
            "gdi32-dib-section",
            "create-dib-section-pixel-length-overflow",
            "CreateDIBSection pixel byte length overflowed x86 accounting",
        )
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
    };
    if descriptor != dib_policy.descriptor {
        return Err(ApiFault::unsupported(
            "gdi32-dib-section",
            "create-dib-section-variant-unsupported",
            "CreateDIBSection currently supports only the evidenced 128x-96 reverse-grayscale DIB",
        ));
    }
    let (result, write_result) =
        match session.create_gdi_dib_section(device_context, descriptor, bits_output) {
            Ok(result) => result,
            Err(error) if error.class == RuntimeStateErrorClass::ResourceLimit => {
                return Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
                    namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
                    value: 0,
                })
                .with_guest_write(bits_output, 0_u32.to_le_bytes().to_vec())
                .with_last_error(preserved_last_error));
            }
            Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
        };
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
        value: result.handle,
    })
    .with_committed_guest_write(ApiGuestWriteReceipt {
        write: ApiGuestWrite {
            address: bits_output,
            bytes: result.bits_address.0.to_le_bytes().to_vec(),
        },
        result: write_result,
    })
    .with_last_error(preserved_last_error))
}

fn handle_get_object_a(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let object_handle = required_argument(frame, 0, "object")?;
    let requested_byte_length = required_argument(frame, 1, "output byte length")?;
    let output = GuestAddress(required_argument(frame, 2, "object output")?);
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session
        .runtime()
        .get_gdi_object_bytes(object_handle, requested_byte_length)
    {
        Ok(bytes) => Ok(ApiImmediate::new(ApiReturnValue::U32(
            u32::try_from(bytes.len()).map_err(|_| {
                ApiFault::runtime_defect(
                    "gdi32-object",
                    "get-object-a-byte-length-unrepresentable",
                    "GetObjectA output length does not fit x86 accounting",
                )
            })?,
        ))
        .with_guest_write(output, bytes)
        .with_last_error(preserved_last_error)),
        Err(error)
            if matches!(
                error.class,
                RuntimeStateErrorClass::InvalidInput
                    | RuntimeStateErrorClass::NotFound
                    | RuntimeStateErrorClass::Conflict
            ) =>
        {
            Ok(ApiImmediate::new(ApiReturnValue::U32(0)).with_last_error(preserved_last_error))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_select_object(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let device_context = required_argument(frame, 0, "device context")?;
    let object_handle = required_argument(frame, 1, "object")?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session
        .runtime_mut()
        .select_gdi_object(device_context, object_handle)
    {
        Ok(result) => Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
            namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
            value: result.previous_handle,
        })
        .with_last_error(preserved_last_error)),
        Err(error)
            if matches!(
                error.class,
                RuntimeStateErrorClass::InvalidInput
                    | RuntimeStateErrorClass::NotFound
                    | RuntimeStateErrorClass::Conflict
            ) =>
        {
            Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
                namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
                value: 0,
            })
            .with_last_error(preserved_last_error))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_delete_object(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let object_handle = required_argument(frame, 0, "object")?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session.delete_gdi_object(object_handle) {
        Ok(_) => {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
                .with_last_error(preserved_last_error))
        }
        Err(error)
            if matches!(
                error.class,
                RuntimeStateErrorClass::InvalidInput
                    | RuntimeStateErrorClass::NotFound
                    | RuntimeStateErrorClass::Conflict
            ) =>
        {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(preserved_last_error))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_create_font_a(
    policy: &Gdi32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let creation = policy.create_font_a.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "gdi32-fonts",
            "create-font-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical font creation",
                policy.id
            ),
        )
    })?;
    let face_address = required_argument(frame, 13, "ANSI face name")?;
    let request = RuntimeLogFontA {
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
        face_name_windows_932: read_font_face_name(contract, session, GuestAddress(face_address))?,
    };
    if request != creation.log_font {
        return Err(ApiFault::unsupported(
            "gdi32-fonts",
            "create-font-a-variant-unsupported",
            "CreateFontA currently supports only the evidenced CP932 MS Gothic request",
        ));
    }
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let result = session
        .runtime_mut()
        .create_gdi_font_a(request)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
        value: result.handle,
    })
    .with_last_error(preserved_last_error))
}

fn read_font_face_name(
    contract: &ApiContract,
    session: &ExecutionSession<'_>,
    address: GuestAddress,
) -> Result<Vec<u8>, ApiFault> {
    let mut bytes = Vec::with_capacity(RUNTIME_LOG_FONT_A_FACE_CAPACITY);
    for offset in 0..RUNTIME_LOG_FONT_A_FACE_CAPACITY {
        let offset = u32::try_from(offset).map_err(|_| {
            ApiFault::runtime_defect(
                "gdi32-fonts",
                "create-font-a-face-offset-invalid",
                "validated LF_FACESIZE does not fit x86 address accounting",
            )
        })?;
        let current = address
            .0
            .checked_add(offset)
            .map(GuestAddress)
            .ok_or_else(|| {
                ApiFault::invalid_call(
                    "gdi32-fonts",
                    "create-font-a-face-address-overflow",
                    "CreateFontA face-name read overflowed x86 address accounting",
                )
            })?;
        let byte = session
            .memory()
            .read(current, 1)
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?[0];
        if byte == 0 {
            return Ok(bytes);
        }
        bytes.push(byte);
    }
    Err(ApiFault::unsupported(
        "gdi32-fonts",
        "create-font-a-face-name-unterminated",
        "CreateFontA face name is not NUL-terminated within LF_FACESIZE",
    ))
}

fn handle_get_stock_object(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let index = frame.argument_u32(0).ok_or_else(|| {
        ApiFault::runtime_defect(
            "gdi32-api-dispatch",
            "decoded-argument-missing",
            "decoded GetStockObject frame is missing object index 0",
        )
    })? as i32;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session.runtime_mut().get_gdi_stock_object(index) {
        Ok(result) => Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
            namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
            value: result.handle,
        })
        .with_last_error(preserved_last_error)),
        Err(error) if error.code == "stock-object-index-unsupported" => {
            Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
                namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
                value: 0,
            })
            .with_last_error(preserved_last_error))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_enum_font_families_ex_a(
    policy: &Gdi32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> ApiHandlerResult {
    let Some(enumeration) = policy.enum_font_families_ex_a.as_ref() else {
        return ApiHandlerResult::Fault(ApiFault::unsupported(
            "gdi32-fonts",
            "enum-font-families-ex-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical font enumeration",
                policy.id
            ),
        ));
    };
    let result = (|| {
        let device_context = required_argument(frame, 0, "device context")?;
        let log_font_address = required_argument(frame, 1, "LOGFONTA filter")?;
        let callback = required_argument(frame, 2, "font enumeration callback")?;
        let callback_parameter = required_argument(frame, 3, "callback parameter")?;
        let flags = required_argument(frame, 4, "enumeration flags")?;
        if flags != enumeration.flags {
            return Err(ApiFault::unsupported(
                "gdi32-fonts",
                "enum-font-families-ex-a-flags-unsupported",
                "EnumFontFamiliesExA currently supports only the reached zero-flags variant",
            ));
        }
        let log_font = session
            .memory()
            .read(
                GuestAddress(log_font_address),
                X86_LOGFONTA_BYTE_LENGTH as usize,
            )
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
        let face_storage = log_font.get(28..60).ok_or_else(|| {
            ApiFault::runtime_defect(
                "gdi32-fonts",
                "logfonta-layout-invalid",
                "validated LOGFONTA bytes do not contain the fixed face-name field",
            )
        })?;
        let face_length = face_storage
            .iter()
            .position(|&byte| byte == 0)
            .ok_or_else(|| {
                ApiFault::unsupported(
                    "gdi32-fonts",
                    "enum-font-families-ex-a-face-name-unterminated",
                    "EnumFontFamiliesExA face filter is not NUL-terminated within LF_FACESIZE",
                )
            })?;
        if log_font[23] != enumeration.input_character_set
            || face_storage[..face_length] != enumeration.input_face_name_windows_932
        {
            return Err(ApiFault::unsupported(
                "gdi32-fonts",
                "enum-font-families-ex-a-filter-unsupported",
                "EnumFontFamiliesExA currently supports only the reached CP932 MS Gothic filter",
            ));
        }
        session
            .memory()
            .validate_access(GuestAddress(callback), 1, MemoryAccess::Execute)
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
        session
            .runtime()
            .validate_screen_dc_operation(device_context)
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;

        let mut scratch =
            Vec::with_capacity(X86_ENUMLOGFONTEXA_BYTE_LENGTH + X86_NEWTEXTMETRICEXA_BYTE_LENGTH);
        scratch.extend_from_slice(&enumeration.record.enum_log_font_ex_a);
        scratch.extend_from_slice(&enumeration.record.new_text_metric_ex_a);
        Ok(GuestCallbackRequest {
            target: GuestAddress(callback),
            arguments: vec![
                GuestCallbackArgument::ScratchPointer { offset: 0 },
                GuestCallbackArgument::ScratchPointer {
                    offset: X86_ENUMLOGFONTEXA_BYTE_LENGTH as u32,
                },
                GuestCallbackArgument::U32(enumeration.record.font_type),
                GuestCallbackArgument::U32(callback_parameter),
            ],
            scratch,
            scratch_pointer_relocations: Vec::new(),
            return_sentinel: callback_return_sentinel,
            completion_policy: GuestCallbackCompletionPolicy::ReturnU32,
        })
    })();
    match result {
        Ok(request) => ApiHandlerResult::GuestCallback(request),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

pub fn gdi32_contract_for_endpoint(
    endpoint: ApiEndpointIdentity,
) -> Result<Option<ApiContract>, ApiFault> {
    if !endpoint.module.eq_ignore_ascii_case("gdi32.dll") {
        return Ok(None);
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return Ok(None);
    };
    match name.as_str() {
        "GetStockObject" => Ok(Some(get_stock_object_contract(endpoint)?)),
        "EnumFontFamiliesExA" => Ok(Some(enum_font_families_ex_a_contract(endpoint)?)),
        "CreateFontA" => Ok(Some(create_font_a_contract(endpoint)?)),
        "CreateCompatibleDC" => Ok(Some(create_compatible_dc_contract(endpoint)?)),
        "CreateDIBSection" => Ok(Some(create_dib_section_contract(endpoint)?)),
        "GetObjectA" => Ok(Some(get_object_a_contract(endpoint)?)),
        "SelectObject" => Ok(Some(select_object_contract(endpoint)?)),
        "DeleteObject" => Ok(Some(delete_object_contract(endpoint)?)),
        "DeleteDC" => Ok(Some(delete_dc_contract(endpoint)?)),
        _ => Ok(None),
    }
}

pub fn create_compatible_dc_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![logical_handle_argument(
                "sourceDeviceContext",
                LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE,
                true,
            )],
            return_value: ApiReturnSchema::LogicalHandle {
                namespace: LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE.to_owned(),
                nullable: true,
            },
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: CREATE_COMPATIBLE_DC_IMPLEMENTATION.to_owned(),
    })
}

pub fn delete_dc_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![logical_handle_argument(
                "deviceContext",
                LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE,
                true,
            )],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: DELETE_DC_IMPLEMENTATION.to_owned(),
    })
}

pub fn create_dib_section_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_handle_argument(
                    "deviceContext",
                    LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE,
                    false,
                ),
                required_in_struct(
                    "bitmapInfo",
                    "BITMAPINFOHEADER",
                    X86_BITMAPINFOHEADER_BYTE_LENGTH,
                ),
                u32_argument("usage"),
                required_out_struct("bits", "PVOID", 4),
                u32_argument("sectionHandle"),
                u32_argument("offset"),
            ],
            return_value: ApiReturnSchema::LogicalHandle {
                namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
                nullable: true,
            },
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: CREATE_DIB_SECTION_IMPLEMENTATION.to_owned(),
    })
}

pub fn get_object_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_handle_argument("object", LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE, true),
                u32_argument("byteLength"),
                dynamic_out_bytes(
                    "objectBytes",
                    false,
                    4,
                    1,
                    RUNTIME_LOG_FONT_A_BYTE_LENGTH as u32,
                ),
            ],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: GET_OBJECT_A_IMPLEMENTATION.to_owned(),
    })
}

pub fn select_object_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_handle_argument(
                    "deviceContext",
                    LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE,
                    false,
                ),
                logical_handle_argument("object", LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE, false),
            ],
            return_value: ApiReturnSchema::LogicalHandle {
                namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
                nullable: true,
            },
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: SELECT_OBJECT_IMPLEMENTATION.to_owned(),
    })
}

pub fn delete_object_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![logical_handle_argument(
                "object",
                LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE,
                true,
            )],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: DELETE_OBJECT_IMPLEMENTATION.to_owned(),
    })
}

pub fn get_stock_object_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![ApiArgumentSchema {
                name: "objectIndex".to_owned(),
                direction: ApiDirection::In,
                value: ApiValueSchema::U32,
            }],
            return_value: ApiReturnSchema::LogicalHandle {
                namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
                nullable: true,
            },
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: GET_STOCK_OBJECT_IMPLEMENTATION.to_owned(),
    })
}

pub fn enum_font_families_ex_a_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_handle_argument(
                    "deviceContext",
                    LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE,
                    false,
                ),
                required_in_struct("logFont", "LOGFONTA", X86_LOGFONTA_BYTE_LENGTH),
                opaque_pointer_argument("enumerationCallback", false, 1),
                u32_argument("callbackParameter"),
                u32_argument("flags"),
            ],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Required,
        implementation_id: ENUM_FONT_FAMILIES_EX_A_IMPLEMENTATION.to_owned(),
    })
}

pub fn create_font_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: [
                "height",
                "width",
                "escapement",
                "orientation",
                "weight",
                "italic",
                "underline",
                "strikeOut",
                "characterSet",
                "outputPrecision",
                "clipPrecision",
                "quality",
                "pitchAndFamily",
            ]
            .into_iter()
            .map(u32_argument)
            .chain(std::iter::once(input_font_face_argument()))
            .collect(),
            return_value: ApiReturnSchema::LogicalHandle {
                namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
                nullable: true,
            },
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: CREATE_FONT_A_IMPLEMENTATION.to_owned(),
    })
}

fn controlled_font_enumeration_policy() -> Gdi32FontEnumerationPolicy {
    Gdi32FontEnumerationPolicy {
        profile_id: "primary-cp932-ms-gothic-regular-v1".to_owned(),
        input_character_set: SHIFT_JIS_CHARSET,
        input_face_name_windows_932: MS_GOTHIC_WINDOWS_932.to_vec(),
        flags: 0,
        record: Gdi32FontRecordPolicy {
            font_type: TRUETYPE_FONTTYPE,
            enum_log_font_ex_a: controlled_enum_log_font_ex_a(),
            new_text_metric_ex_a: controlled_new_text_metric_ex_a(),
        },
    }
}

fn controlled_font_creation_policy() -> Gdi32FontCreationPolicy {
    Gdi32FontCreationPolicy {
        profile_id: "primary-cp932-ms-gothic-64x32-thin-v1".to_owned(),
        log_font: RuntimeLogFontA {
            height: 64,
            width: 32,
            escapement: 0,
            orientation: 0,
            weight: 100,
            italic: 0,
            underline: 0,
            strike_out: 0,
            character_set: SHIFT_JIS_CHARSET,
            output_precision: 4,
            clip_precision: 0,
            quality: 2,
            pitch_and_family: 0,
            face_name_windows_932: MS_GOTHIC_WINDOWS_932.to_vec(),
        },
    }
}

fn controlled_compatible_dc_policy() -> Gdi32CompatibleDcPolicy {
    Gdi32CompatibleDcPolicy {
        profile_id: "primary-null-screen-memory-dc-v1".to_owned(),
        source_device_context: 0,
        initial_font_stock_index: RUNTIME_GDI_SYSTEM_FONT_INDEX,
        initial_bitmap_stock_index: RUNTIME_GDI_DEFAULT_BITMAP_INDEX,
    }
}

fn controlled_dib_section_policy() -> Gdi32DibSectionPolicy {
    Gdi32DibSectionPolicy {
        profile_id: "primary-128x-96-8bpp-reverse-grayscale-v1".to_owned(),
        bitmap_info_header_size: X86_BITMAPINFOHEADER_BYTE_LENGTH,
        size_image: 0,
        x_pixels_per_meter: 0,
        y_pixels_per_meter: 0,
        colors_used: X86_DIB_RGBQUAD_COUNT as u32,
        colors_important: 0,
        usage: RUNTIME_GDI_DIB_RGB_COLORS,
        section_handle: 0,
        offset: 0,
        descriptor: RuntimeGdiDibSectionDescriptor {
            width: 128,
            height: -96,
            planes: 1,
            bits_per_pixel: 8,
            compression: RUNTIME_GDI_DIB_BI_RGB,
            stride_bytes: 128,
            pixel_byte_length: 12_288,
            palette: (0..X86_DIB_RGBQUAD_COUNT)
                .map(|index| {
                    let intensity = u8::try_from(255 - index)
                        .expect("the controlled DIB palette contains exactly 256 entries");
                    RuntimeGdiRgbQuad {
                        blue: intensity,
                        green: intensity,
                        red: intensity,
                        reserved: 0,
                    }
                })
                .collect(),
        },
    }
}

fn controlled_enum_log_font_ex_a() -> Vec<u8> {
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

fn controlled_new_text_metric_ex_a() -> Vec<u8> {
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
    frame.argument_u32(index).ok_or_else(|| {
        ApiFault::runtime_defect(
            "gdi32-api-dispatch",
            "decoded-argument-missing",
            format!("decoded GDI32 frame is missing {description}"),
        )
    })
}

fn required_byte_argument(
    frame: &ApiCallFrame,
    index: usize,
    description: &str,
) -> Result<u8, ApiFault> {
    let value = required_argument(frame, index, description)?;
    u8::try_from(value).map_err(|_| {
        ApiFault::unsupported(
            "gdi32-fonts",
            "create-font-a-variant-unsupported",
            format!("CreateFontA {description} is outside the evidenced byte-sized variant"),
        )
    })
}

fn u32_argument(name: &str) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::U32,
    }
}

fn logical_handle_argument(name: &str, namespace: &str, nullable: bool) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::LogicalHandle {
            namespace: namespace.to_owned(),
            nullable,
        },
    }
}

fn opaque_pointer_argument(name: &str, nullable: bool, alignment: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::OpaquePointer {
            nullable,
            alignment,
        },
    }
}

fn input_font_face_argument() -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: "faceName".to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: false,
                alignment: 1,
                byte_length: 1,
                pointee: ApiPointeeSchema::String {
                    encoding: ApiStringEncoding::Windows932,
                    maximum_code_units: RUNTIME_LOG_FONT_A_FACE_CAPACITY as u32,
                    nul_terminated: true,
                },
                write_requirement: ApiWriteRequirement::None,
            },
        },
    }
}

fn required_in_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: false,
                alignment: 4,
                byte_length,
                pointee: ApiPointeeSchema::Struct {
                    type_id: type_id.to_owned(),
                },
                write_requirement: ApiWriteRequirement::None,
            },
        },
    }
}

fn required_out_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::Out,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: false,
                alignment: 4,
                byte_length,
                pointee: ApiPointeeSchema::Struct {
                    type_id: type_id.to_owned(),
                },
                write_requirement: ApiWriteRequirement::Required,
            },
        },
    }
}

fn dynamic_out_bytes(
    name: &str,
    nullable: bool,
    alignment: u32,
    minimum_byte_length: u32,
    maximum_byte_length: u32,
) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::Out,
        value: ApiValueSchema::DynamicPointer {
            constraint: ApiDynamicPointerConstraint {
                nullable,
                alignment,
                minimum_byte_length,
                maximum_byte_length,
                pointee: ApiPointeeSchema::Bytes,
                write_requirement: ApiWriteRequirement::Optional,
            },
        },
    }
}

fn valid_policy_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controlled_gdi32_policy_is_canonical_and_bounded() {
        let policy = Gdi32CompatibilityPolicy::controlled_v4();
        policy.validate().unwrap();
        let enumeration = policy.enum_font_families_ex_a.as_ref().unwrap();
        assert_eq!(enumeration.input_character_set, SHIFT_JIS_CHARSET);
        assert_eq!(
            enumeration.input_face_name_windows_932,
            MS_GOTHIC_WINDOWS_932
        );
        assert_eq!(enumeration.record.font_type, TRUETYPE_FONTTYPE);
        assert_eq!(
            enumeration.record.enum_log_font_ex_a.len(),
            X86_ENUMLOGFONTEXA_BYTE_LENGTH
        );
        assert_eq!(
            enumeration.record.new_text_metric_ex_a.len(),
            X86_NEWTEXTMETRICEXA_BYTE_LENGTH
        );
        assert_eq!(
            &enumeration.record.enum_log_font_ex_a[28..41],
            MS_GOTHIC_WINDOWS_932
        );
        assert_eq!(enumeration.record.enum_log_font_ex_a[41], 0);
        assert_eq!(
            enumeration.record.new_text_metric_ex_a[52],
            SHIFT_JIS_CHARSET
        );
        let creation = policy.create_font_a.as_ref().unwrap();
        assert_eq!(creation.log_font.height, 64);
        assert_eq!(creation.log_font.width, 32);
        assert_eq!(creation.log_font.weight, 100);
        assert_eq!(creation.log_font.character_set, SHIFT_JIS_CHARSET);
        assert_eq!(creation.log_font.output_precision, 4);
        assert_eq!(creation.log_font.clip_precision, 0);
        assert_eq!(creation.log_font.quality, 2);
        assert_eq!(creation.log_font.pitch_and_family, 0);
        assert_eq!(
            creation.log_font.face_name_windows_932,
            MS_GOTHIC_WINDOWS_932
        );
        assert_eq!(
            creation.log_font.to_bytes().unwrap()[..46],
            [
                64, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100, 0, 0, 0, 0, 0, 0, 128, 4, 0,
                2, 0, 130, 108, 130, 114, 32, 131, 83, 131, 86, 131, 98, 131, 78, 0, 0, 0, 0, 0,
            ]
        );
        let compatible_dc = policy.create_compatible_dc.as_ref().unwrap();
        assert_eq!(compatible_dc.source_device_context, 0);
        assert_eq!(compatible_dc.initial_font_stock_index, 13);
        assert_eq!(compatible_dc.initial_bitmap_stock_index, 21);
        let dib_section = policy.create_dib_section.as_ref().unwrap();
        assert_eq!(dib_section.bitmap_info_header_size, 40);
        assert_eq!(dib_section.colors_used, 256);
        assert_eq!(dib_section.descriptor.width, 128);
        assert_eq!(dib_section.descriptor.height, -96);
        assert_eq!(dib_section.descriptor.bits_per_pixel, 8);
        assert_eq!(dib_section.descriptor.stride_bytes, 128);
        assert_eq!(dib_section.descriptor.pixel_byte_length, 12_288);
        assert_eq!(dib_section.descriptor.palette.len(), 256);
        assert_eq!(
            dib_section.descriptor.palette[0],
            RuntimeGdiRgbQuad {
                blue: 255,
                green: 255,
                red: 255,
                reserved: 0,
            }
        );
        assert_eq!(
            dib_section.descriptor.palette[255],
            RuntimeGdiRgbQuad {
                blue: 0,
                green: 0,
                red: 0,
                reserved: 0,
            }
        );
    }

    #[test]
    fn mutated_font_policy_is_rejected() {
        let mut policy = Gdi32CompatibilityPolicy::controlled_v4();
        policy
            .enum_font_families_ex_a
            .as_mut()
            .unwrap()
            .record
            .enum_log_font_ex_a[188 - 1] = 1;
        let fault = policy.validate().unwrap_err();
        assert_eq!(fault.code, "gdi32-font-enumeration-profile-invalid");

        let mut policy = Gdi32CompatibilityPolicy::controlled_v4();
        policy.create_font_a.as_mut().unwrap().log_font.quality = 1;
        let fault = policy.validate().unwrap_err();
        assert_eq!(fault.code, "gdi32-font-creation-profile-invalid");

        let mut policy = Gdi32CompatibilityPolicy::controlled_v4();
        policy
            .create_compatible_dc
            .as_mut()
            .unwrap()
            .initial_bitmap_stock_index = 20;
        let fault = policy.validate().unwrap_err();
        assert_eq!(fault.code, "gdi32-compatible-dc-profile-invalid");

        let mut policy = Gdi32CompatibilityPolicy::controlled_v4();
        policy
            .create_dib_section
            .as_mut()
            .unwrap()
            .descriptor
            .palette[0]
            .red = 254;
        let fault = policy.validate().unwrap_err();
        assert_eq!(fault.code, "gdi32-dib-section-profile-invalid");
    }
}
