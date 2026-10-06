use crate::api::{
    ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiSymbol,
};
use crate::com::{
    COM_S_OK, ComDispatchOutcome, ComFrontendCall, ComHResult, ComInterfaceSpec, ComMethodSpec,
    ComObjectSpec,
};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::{
    RuntimeComClass, RuntimeComIid, RuntimeGraphicsBlendFactor, RuntimeGraphicsBlendOperation,
    RuntimeGraphicsColorFormat, RuntimeGraphicsCullMode, RuntimeGraphicsFlexibleVertexFormat,
    RuntimeGraphicsRenderStateUpdate, RuntimeGraphicsSamplerStateUpdate,
    RuntimeGraphicsTextureArgument, RuntimeGraphicsTextureDescriptor, RuntimeGraphicsTextureFilter,
    RuntimeGraphicsTextureOperation, RuntimeGraphicsTexturePool,
    RuntimeGraphicsTextureStageStateUpdate, RuntimeGraphicsTextureUsage,
    RuntimeGraphicsVertexBufferDescriptor, RuntimeGraphicsVertexBufferPool,
    RuntimeGraphicsVertexBufferUsage, RuntimeRect, RuntimeStateError,
};

pub const D3D9_SDK_VERSION: u32 = 32;
pub const D3D9_D3DADAPTER_IDENTIFIER9_BYTE_LENGTH: u32 = 1_100;
pub const D3D9_D3DPRESENT_PARAMETERS_BYTE_LENGTH: u32 = 56;
pub const D3D9_D3DRASTER_STATUS_BYTE_LENGTH: u32 = 8;
pub const D3D9_D3DDEVTYPE_HAL: u32 = 1;
pub const D3D9_D3DCAPS_READ_SCANLINE: u32 = 0x0002_0000;
pub const D3D9_D3DCAPS2_DYNAMICTEXTURES: u32 = 0x2000_0000;
pub const D3D9_D3DPS_VERSION_3_0: u32 = 0xffff_0300;
pub const D3D9_D3DFVF_XYZRHW: u32 = 0x004;
pub const D3D9_D3DFVF_DIFFUSE: u32 = 0x040;
pub const D3D9_D3DFVF_TEX1: u32 = 0x100;
pub const D3D9_D3DPT_TRIANGLESTRIP: u32 = 5;
pub const D3D9_D3DFMT_X8R8G8B8: u32 = 22;
pub const D3D9_D3DRTYPE_TEXTURE: u32 = 3;
pub const D3D9_D3DUSAGE_DYNAMIC: u32 = 0x0000_0200;
pub const D3D9_D3DUSAGE_WRITEONLY: u32 = 0x0000_0008;
pub const D3D9_D3DLOCK_DISCARD: u32 = 0x0000_2000;
pub const D3D9_D3DLOCK_NO_DIRTY_UPDATE: u32 = 0x0000_8000;
pub const D3D9_D3DCLEAR_TARGET: u32 = 0x0000_0001;
pub const D3D9_D3DRS_ZENABLE: u32 = 7;
pub const D3D9_D3DRS_SRCBLEND: u32 = 19;
pub const D3D9_D3DRS_DESTBLEND: u32 = 20;
pub const D3D9_D3DRS_CULLMODE: u32 = 22;
pub const D3D9_D3DRS_ALPHABLENDENABLE: u32 = 27;
pub const D3D9_D3DRS_LIGHTING: u32 = 137;
pub const D3D9_D3DRS_BLENDOP: u32 = 171;
pub const D3D9_D3DBLEND_SRCALPHA: u32 = 5;
pub const D3D9_D3DBLEND_INVSRCALPHA: u32 = 6;
pub const D3D9_D3DBLENDOP_ADD: u32 = 1;
pub const D3D9_D3DCULL_NONE: u32 = 1;
pub const D3D9_D3DSAMP_MAGFILTER: u32 = 5;
pub const D3D9_D3DSAMP_MINFILTER: u32 = 6;
pub const D3D9_D3DTEXF_LINEAR: u32 = 2;
pub const D3D9_D3DTSS_COLOROP: u32 = 1;
pub const D3D9_D3DTSS_COLORARG1: u32 = 2;
pub const D3D9_D3DTSS_COLORARG2: u32 = 3;
pub const D3D9_D3DTSS_ALPHAOP: u32 = 4;
pub const D3D9_D3DTSS_ALPHAARG1: u32 = 5;
pub const D3D9_D3DTSS_ALPHAARG2: u32 = 6;
pub const D3D9_D3DTOP_MODULATE: u32 = 4;
pub const D3D9_D3DTOP_SELECTARG1: u32 = 2;
pub const D3D9_D3DTA_DIFFUSE: u32 = 0;
pub const D3D9_D3DTA_TEXTURE: u32 = 2;
pub const D3D9_D3DENUM_WHQL_LEVEL: u32 = 0x0000_0002;
pub const D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED: u32 = 0x0000_0044;
pub const D3D9_D3DFMT_UNKNOWN: u32 = 0;
pub const D3D9_D3DMULTISAMPLE_NONE: u32 = 0;
pub const D3D9_D3DSWAPEFFECT_DISCARD: u32 = 3;
pub const D3D9_D3DPOOL_DEFAULT: u32 = 0;
pub const D3D9_D3DPOOL_MANAGED: u32 = 1;
pub const D3D9_D3DPOOL_SYSTEMMEM: u32 = 2;
pub const D3D9_D3DERR_NOTAVAILABLE: ComHResult = ComHResult(0x8876_086a_u32 as i32);
pub const D3D9_D3DERR_INVALIDCALL: ComHResult = ComHResult(0x8876_086c_u32 as i32);
pub const D3D9_VIRTUAL_DRIVER: &str = "vine-d3d9.dll";
pub const D3D9_VIRTUAL_DESCRIPTION: &str = "Vine Virtual Direct3D9 Adapter";
pub const D3D9_VIRTUAL_DEVICE_NAME: &str = "\\\\.\\DISPLAY1";
pub const D3D9_VIRTUAL_DRIVER_VERSION: u64 = 0x0001_0000_0000_0000;
pub const D3D9_VIRTUAL_DEVICE_IDENTIFIER_WINDOWS_BYTES: [u8; 16] = [
    0x45, 0x4e, 0x49, 0x56, 0x33, 0x44, 0x39, 0x44, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
];
pub const D3D9_IDIRECT3D9_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0xca, 0xcb, 0xbd, 0x81, 0xd4, 0x64, 0x6d, 0x42, 0xae, 0x8d, 0xad, 0x01, 0x47, 0xf4, 0x27, 0x5c,
]);
pub const D3D9_IDIRECT3DDEVICE9_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x96, 0x3b, 0x22, 0xd0, 0x7a, 0xbf, 0xfd, 0x43, 0x92, 0xbd, 0xa4, 0x3b, 0x0d, 0x82, 0xb9, 0xeb,
]);
pub const D3D9_IDIRECT3DTEXTURE9_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x27, 0x12, 0xc3, 0x85, 0xe5, 0x3d, 0x00, 0x4f, 0x9b, 0x3a, 0xf1, 0x1a, 0xc3, 0x8c, 0x18, 0xb5,
]);
pub const D3D9_IDIRECT3DSURFACE9_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x3a, 0xaf, 0xfb, 0x0c, 0xf6, 0x9f, 0x9a, 0x42, 0x99, 0xb3, 0xa2, 0x79, 0x6a, 0xf8, 0xb8, 0x9b,
]);
pub const D3D9_IDIRECT3DVERTEXBUFFER9_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0xb5, 0xb1, 0x4b, 0xb6, 0x70, 0xfd, 0xf6, 0x4d, 0xbf, 0x91, 0x19, 0xd0, 0xa1, 0x24, 0x55, 0xe3,
]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct D3d9PresentParameters {
    pub back_buffer_width: u32,
    pub back_buffer_height: u32,
    pub back_buffer_format: u32,
    pub back_buffer_count: u32,
    pub multi_sample_type: u32,
    pub multi_sample_quality: u32,
    pub swap_effect: u32,
    pub device_window: u32,
    pub windowed: u32,
    pub enable_auto_depth_stencil: u32,
    pub auto_depth_stencil_format: u32,
    pub flags: u32,
    pub full_screen_refresh_rate_in_hz: u32,
    pub presentation_interval: u32,
}

impl D3d9PresentParameters {
    pub fn from_x86_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != D3D9_D3DPRESENT_PARAMETERS_BYTE_LENGTH as usize {
            return None;
        }
        let mut words = [0_u32; 14];
        for (word, bytes) in words.iter_mut().zip(bytes.chunks_exact(4)) {
            *word = u32::from_le_bytes(bytes.try_into().ok()?);
        }
        Some(Self {
            back_buffer_width: words[0],
            back_buffer_height: words[1],
            back_buffer_format: words[2],
            back_buffer_count: words[3],
            multi_sample_type: words[4],
            multi_sample_quality: words[5],
            swap_effect: words[6],
            device_window: words[7],
            windowed: words[8],
            enable_auto_depth_stencil: words[9],
            auto_depth_stencil_format: words[10],
            flags: words[11],
            full_screen_refresh_rate_in_hz: words[12],
            presentation_interval: words[13],
        })
    }

    fn resolved_prefix(self, width: u32, height: u32, format: u32) -> Vec<u8> {
        [width, height, format]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct D3d9DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
    pub format: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct D3d9RasterStatus {
    pub in_v_blank: bool,
    pub scan_line: u32,
}

impl D3d9RasterStatus {
    fn to_x86_bytes(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(D3D9_D3DRASTER_STATUS_BYTE_LENGTH as usize);
        bytes.extend_from_slice(&u32::from(self.in_v_blank).to_le_bytes());
        bytes.extend_from_slice(&self.scan_line.to_le_bytes());
        bytes
    }
}

impl D3d9DisplayMode {
    pub fn to_x86_bytes(self) -> Vec<u8> {
        [self.width, self.height, self.refresh_rate, self.format]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct D3d9AdapterIdentifier {
    pub driver: String,
    pub description: String,
    pub device_name: String,
    pub driver_version: u64,
    pub vendor_id: u32,
    pub device_id: u32,
    pub sub_sys_id: u32,
    pub revision: u32,
    pub device_identifier: [u8; 16],
    pub whql_level: u32,
}

impl D3d9AdapterIdentifier {
    pub fn to_x86_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![0; D3D9_D3DADAPTER_IDENTIFIER9_BYTE_LENGTH as usize];
        write_fixed_ansi(&mut bytes, 0, 512, &self.driver);
        write_fixed_ansi(&mut bytes, 512, 512, &self.description);
        write_fixed_ansi(&mut bytes, 1_024, 32, &self.device_name);
        bytes[1_056..1_064].copy_from_slice(&self.driver_version.to_le_bytes());
        for (offset, value) in [
            (1_064, self.vendor_id),
            (1_068, self.device_id),
            (1_072, self.sub_sys_id),
            (1_076, self.revision),
            (1_096, self.whql_level),
        ] {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[1_080..1_096].copy_from_slice(&self.device_identifier);
        bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct D3d9DeviceCaps {
    pub device_type: u32,
    pub adapter_ordinal: u32,
    pub caps: u32,
    pub caps2: u32,
    pub pixel_shader_version: u32,
    pub number_of_adapters_in_group: u32,
}

impl D3d9DeviceCaps {
    pub fn to_x86_bytes(self) -> Vec<u8> {
        let mut words = [0_u32; 76];
        words[0] = self.device_type;
        words[1] = self.adapter_ordinal;
        words[2] = self.caps;
        words[3] = self.caps2;
        words[51] = self.pixel_shader_version;
        words[58] = self.number_of_adapters_in_group;
        words.into_iter().flat_map(u32::to_le_bytes).collect()
    }
}

fn valid_fixed_ansi(value: &str, capacity: usize) -> bool {
    !value.is_empty()
        && value.len() < capacity
        && value.is_ascii()
        && !value.as_bytes().contains(&0)
}

fn write_fixed_ansi(bytes: &mut [u8], offset: usize, capacity: usize, value: &str) {
    debug_assert!(valid_fixed_ansi(value, capacity));
    bytes[offset..offset + value.len()].copy_from_slice(value.as_bytes());
}

fn guest_ranges_overlap(
    left: GuestAddress,
    left_length: usize,
    right: GuestAddress,
    right_length: usize,
) -> bool {
    let left_start = u64::from(left.0);
    let right_start = u64::from(right.0);
    let left_end = left_start + left_length as u64;
    let right_end = right_start + right_length as u64;
    left_start < right_end && right_start < left_end
}

pub(crate) fn dispatch_d3d9_api(
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let endpoint = &frame.endpoint;
    if endpoint.module.eq_ignore_ascii_case("d3d9.dll")
        && matches!(&endpoint.symbol, ApiSymbol::Name { name } if name == "Direct3DCreate9")
    {
        handle_direct3d_create9(endpoint, frame, runtime)
    } else {
        ApiHandlerResult::Fault(ApiFault::new(format!(
            "unsupported D3D9 import {:?}",
            endpoint.symbol
        )))
    }
}

pub(crate) fn dispatch_d3d9_com(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    if call.class() == RuntimeComClass::D3d9Device && call.frame().iid == D3D9_IDIRECT3DDEVICE9_IID
    {
        return match call.frame().vtable_slot {
            3 => {
                if let Err(error) = call
                    .runtime
                    .graphics_device_by_com_object(call.frame().object_id)
                {
                    return call.runtime_fault(error);
                }
                call.complete(COM_S_OK.bits())
            }
            23 => dispatch_create_texture(call),
            26 => dispatch_create_vertex_buffer(call),
            57 => set_render_state(call),
            43 => dispatch_clear(call),
            17 => dispatch_present(call),
            31 => dispatch_update_texture(call),
            41 => dispatch_begin_scene(call),
            42 => dispatch_end_scene(call),
            65 => dispatch_set_texture(call),
            89 => dispatch_set_fvf(call),
            100 => dispatch_set_stream_source(call),
            81 => dispatch_draw_primitive(call),
            69 => dispatch_set_sampler_state(call),
            67 => dispatch_set_texture_stage_state(call),
            19 => dispatch_raster_status(call),
            _ => call.fault("D3D9 device method is not implemented"),
        };
    }
    if call.class() == RuntimeComClass::D3d9Texture
        && call.frame().iid == D3D9_IDIRECT3DTEXTURE9_IID
        && matches!(call.frame().vtable_slot, 19 | 20)
    {
        return if call.frame().vtable_slot == 19 {
            dispatch_texture_lock(call)
        } else {
            dispatch_texture_unlock(call)
        };
    }
    if call.class() == RuntimeComClass::D3d9Texture
        && call.frame().iid == D3D9_IDIRECT3DTEXTURE9_IID
        && call.frame().vtable_slot == 18
    {
        return dispatch_get_surface_level(call);
    }
    if call.class() == RuntimeComClass::D3d9Texture
        && call.frame().iid == D3D9_IDIRECT3DTEXTURE9_IID
        && call.frame().vtable_slot == 21
    {
        return dispatch_add_dirty_rect(call);
    }
    if call.class() == RuntimeComClass::D3d9Surface
        && call.frame().iid == D3D9_IDIRECT3DSURFACE9_IID
    {
        return match call.frame().vtable_slot {
            15 => surface_get_dc(call),
            16 => surface_release_dc(call),
            _ => call.fault("D3D9 surface method is not implemented"),
        };
    }
    if call.class() == RuntimeComClass::D3d9VertexBuffer
        && call.frame().iid == D3D9_IDIRECT3DVERTEXBUFFER9_IID
    {
        return match call.frame().vtable_slot {
            11 => dispatch_vertex_buffer_lock(call),
            12 => vertex_buffer_unlock(call),
            _ => call.fault("D3D9 vertex buffer method is not implemented"),
        };
    }
    if call.class() != RuntimeComClass::D3d9 || call.frame().iid != D3D9_IDIRECT3D9_IID {
        return call.fault("D3D9 device methods need a reached graphics implementation");
    }
    let args = &call.frame().explicit_arguments;
    match (call.frame().vtable_slot, args.as_slice()) {
        (16, [..]) => dispatch_create_device(call),
        (4, []) => {
            if call.runtime.primary_monitor().is_none() {
                return call.fault("D3D9 has no configured display monitor");
            }
            call.complete(1)
        }
        (15, [adapter]) => {
            if *adapter != 0 {
                return call.complete(0);
            }
            let Some(monitor) = call.runtime.primary_monitor() else {
                return call.fault("D3D9 has no configured display monitor");
            };
            let handle = monitor.handle;
            call.complete(handle)
        }
        (5, [adapter, flags, output]) => {
            if *flags == D3D9_D3DENUM_WHQL_LEVEL {
                return call.fault("WHQL certificate discovery is not implemented");
            }
            if *adapter != 0 || *flags != 0 || *output == 0 {
                return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
            }
            if call.runtime.primary_monitor().is_none() {
                return call.fault("D3D9 has no configured display monitor");
            }
            let bytes = D3d9AdapterIdentifier {
                driver: D3D9_VIRTUAL_DRIVER.to_owned(),
                description: D3D9_VIRTUAL_DESCRIPTION.to_owned(),
                device_name: D3D9_VIRTUAL_DEVICE_NAME.to_owned(),
                driver_version: D3D9_VIRTUAL_DRIVER_VERSION,
                vendor_id: 0,
                device_id: 0,
                sub_sys_id: 0,
                revision: 0,
                device_identifier: D3D9_VIRTUAL_DEVICE_IDENTIFIER_WINDOWS_BYTES,
                whql_level: 0,
            }
            .to_x86_bytes();
            let output = GuestAddress(*output);
            complete_d3d9_write(call, output, bytes)
        }
        (
            9,
            [
                adapter,
                device_type,
                adapter_format,
                back_buffer_format,
                _windowed,
            ],
        ) => {
            if *adapter != 0 || *device_type != D3D9_D3DDEVTYPE_HAL {
                return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
            }
            if call.runtime.primary_monitor().is_none() {
                return call.fault("D3D9 has no configured display monitor");
            }
            let result = if *adapter_format == D3D9_D3DFMT_X8R8G8B8
                && *back_buffer_format == D3D9_D3DFMT_X8R8G8B8
            {
                COM_S_OK
            } else {
                D3D9_D3DERR_NOTAVAILABLE
            };
            call.complete(result.bits())
        }
        (
            10,
            [
                adapter,
                device_type,
                adapter_format,
                usage,
                resource_type,
                check_format,
            ],
        ) => {
            if *adapter != 0 || *device_type != D3D9_D3DDEVTYPE_HAL {
                return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
            }
            if call.runtime.primary_monitor().is_none() {
                return call.fault("D3D9 has no configured display monitor");
            }
            let result = if *adapter_format == D3D9_D3DFMT_X8R8G8B8
                && *resource_type == D3D9_D3DRTYPE_TEXTURE
                && *check_format == D3D9_D3DFMT_X8R8G8B8
                && matches!(*usage, 0 | D3D9_D3DUSAGE_DYNAMIC)
            {
                COM_S_OK
            } else {
                D3D9_D3DERR_NOTAVAILABLE
            };
            call.complete(result.bits())
        }
        (14, [adapter, device_type, output]) => {
            if *adapter != 0 || *device_type != D3D9_D3DDEVTYPE_HAL || *output == 0 {
                return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
            }
            if call.runtime.primary_monitor().is_none() {
                return call.fault("D3D9 has no configured display monitor");
            }
            let bytes = D3d9DeviceCaps {
                device_type: D3D9_D3DDEVTYPE_HAL,
                adapter_ordinal: 0,
                caps: D3D9_D3DCAPS_READ_SCANLINE,
                caps2: D3D9_D3DCAPS2_DYNAMICTEXTURES,
                pixel_shader_version: D3D9_D3DPS_VERSION_3_0,
                number_of_adapters_in_group: 1,
            }
            .to_x86_bytes();
            let output = GuestAddress(*output);
            complete_d3d9_write(call, output, bytes)
        }
        (8, [adapter, output]) => {
            if *adapter != 0 || *output == 0 {
                return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
            }
            let Some(monitor) = call.runtime.primary_monitor() else {
                return call.fault("D3D9 has no configured display monitor");
            };
            let width = monitor
                .rect
                .right
                .checked_sub(monitor.rect.left)
                .and_then(|value| u32::try_from(value).ok());
            let height = monitor
                .rect
                .bottom
                .checked_sub(monitor.rect.top)
                .and_then(|value| u32::try_from(value).ok());
            let (Some(width), Some(height)) = (width, height) else {
                return call.fault("D3D9 display dimensions are not positive x86 values");
            };
            let bytes = D3d9DisplayMode {
                width,
                height,
                refresh_rate: 60,
                format: D3D9_D3DFMT_X8R8G8B8,
            }
            .to_x86_bytes();
            let output = GuestAddress(*output);
            complete_d3d9_write(call, output, bytes)
        }
        _ => call.fault("D3D9 method is not yet implemented in the generic graphics path"),
    }
}

fn dispatch_begin_scene(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    if !call.frame().explicit_arguments.is_empty() {
        return call.fault("IDirect3DDevice9::BeginScene takes no arguments");
    }
    let device_com_object_id = call.frame().object_id;
    match call.runtime.begin_graphics_scene(device_com_object_id) {
        Ok(true) => call.complete(COM_S_OK.bits()),
        Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_end_scene(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    if !call.frame().explicit_arguments.is_empty() {
        return call.fault("IDirect3DDevice9::EndScene takes no arguments");
    }
    let device_com_object_id = call.frame().object_id;
    match call.runtime.end_graphics_scene(device_com_object_id) {
        Ok(true) => call.complete(COM_S_OK.bits()),
        Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_present(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [
        source_rect,
        destination_rect,
        destination_window,
        dirty_region,
    ] = call.frame().explicit_arguments.as_slice()
    else {
        return call.fault("Present requires four arguments");
    };
    if *source_rect != 0 || *destination_rect != 0 || *destination_window != 0 || *dirty_region != 0
    {
        return call.fault("Partial or redirected presentation is not implemented");
    }
    let device_com_object_id = call.frame().object_id;
    let device = match call
        .runtime
        .graphics_device_by_com_object(device_com_object_id)
    {
        Ok(device) => device,
        Err(error) => return call.runtime_fault(error),
    };
    let device_id = device.id;
    let mut texture_bytes = 0u64;
    let mut fill_count = 0usize;
    for draw in &device.back_buffer.draw_submissions {
        if draw.texture_width > 4096 || draw.texture_height > 4096 {
            return call.fault("Presentation exceeds the bounded texture dimensions");
        }
        texture_bytes += u64::from(draw.texture_width) * u64::from(draw.texture_height) * 4;
        fill_count += draw.texture_fills.len();
        if texture_bytes > 16 * 1024 * 1024 || fill_count > 16384 {
            return call.fault("Presentation exceeds the bounded texture upload budget");
        }
    }
    if device.presentation.width > 4096 || device.presentation.height > 4096 {
        return call.fault("Presentation exceeds the bounded browser surface");
    }
    match call.runtime.present_graphics(device_com_object_id) {
        Ok(true) => {
            let ComDispatchOutcome::Completed(mut completion) = call.complete(COM_S_OK.bits())
            else {
                unreachable!("successful presentation completes synchronously");
            };
            completion.presented_device = Some(device_id);
            ComDispatchOutcome::Completed(completion)
        }
        Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_clear(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [
        rect_count,
        rects_pointer,
        flags,
        color_argb,
        _depth_bits,
        _stencil,
    ] = call.frame().explicit_arguments.as_slice()
    else {
        return call.fault("IDirect3DDevice9::Clear requires six arguments");
    };
    // A full color-target clear has no guest rectangle pointers to dereference.
    // Depth and stencil values are ignored when their flags are absent.
    if *rect_count != 0 || *rects_pointer != 0 || *flags != D3D9_D3DCLEAR_TARGET {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    let device_com_object_id = call.frame().object_id;
    match call
        .runtime
        .clear_graphics_back_buffer(device_com_object_id, *color_argb)
    {
        Ok(()) => call.complete(COM_S_OK.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_raster_status(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [swap_chain, output] = call.frame().explicit_arguments.as_slice() else {
        return call.fault("GetRasterStatus requires two arguments");
    };
    let (swap_chain, output) = (*swap_chain, *output);
    if swap_chain != 0 || output == 0 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    if let Err(error) = call
        .runtime
        .graphics_device_by_com_object(call.frame().object_id)
    {
        return call.runtime_fault(error);
    }
    let Some(monitor) = call.runtime.primary_monitor() else {
        return call.fault("GetRasterStatus requires the configured display");
    };
    let height = match monitor.rect.bottom.checked_sub(monitor.rect.top) {
        Some(height) if height > 0 => height as u32,
        _ => {
            return call.fault("display has no positive scanline height");
        }
    };
    // A virtual 60 Hz raster, including a short vertical blank. Use the shared
    // Rust clock so guest polling observes progress, not a permanently fixed line.
    let clock = call.runtime.clock();
    let frequency = clock.performance_frequency;
    if frequency == 0 {
        return call.fault("raster clock frequency is zero");
    }
    let total_lines = u64::from(height) + u64::from(height).div_ceil(24);
    let line = ((u128::from(clock.performance_counter) * 60 * u128::from(total_lines)
        / u128::from(frequency))
        % u128::from(total_lines)) as u32;
    let status = D3d9RasterStatus {
        in_v_blank: line >= height,
        scan_line: if line < height { line } else { 0 },
    };
    complete_d3d9_write(call, GuestAddress(output), status.to_x86_bytes())
}

fn dispatch_create_device(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [
        adapter,
        device_type,
        focus_window,
        behavior,
        present_pointer,
        output_pointer,
    ] = call.frame().explicit_arguments.as_slice()
    else {
        return call.fault("IDirect3D9::CreateDevice requires six arguments");
    };
    let (adapter, device_type, focus_window, behavior, present_pointer, output_pointer) = (
        *adapter,
        *device_type,
        *focus_window,
        *behavior,
        *present_pointer,
        *output_pointer,
    );
    if adapter != 0
        || device_type != D3D9_D3DDEVTYPE_HAL
        || focus_window == 0
        || present_pointer == 0
        || output_pointer == 0
    {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    if behavior != D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    }
    let present_address = GuestAddress(present_pointer);
    let output_address = GuestAddress(output_pointer);
    if guest_ranges_overlap(
        present_address,
        D3D9_D3DPRESENT_PARAMETERS_BYTE_LENGTH as usize,
        output_address,
        4,
    ) {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    for (address, size, access) in [
        (
            present_address,
            D3D9_D3DPRESENT_PARAMETERS_BYTE_LENGTH as usize,
            MemoryAccess::Read,
        ),
        (present_address, 12, MemoryAccess::Write),
        (output_address, 4, MemoryAccess::Write),
    ] {
        if let Err(error) = call.runtime.memory.validate_access(address, size, access) {
            return call.memory_fault(error);
        }
    }
    let bytes = match call.runtime.memory.read(
        present_address,
        D3D9_D3DPRESENT_PARAMETERS_BYTE_LENGTH as usize,
    ) {
        Ok(bytes) => bytes,
        Err(error) => return call.memory_fault(error),
    };
    let present = D3d9PresentParameters::from_x86_bytes(&bytes)
        .expect("validated presentation parameters have fourteen words");
    if present.device_window != focus_window {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    let window = match call.runtime.window_option(focus_window) {
        Ok(Some(window)) => window,
        Ok(None) => {
            return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
        }
        Err(error) => return call.runtime_fault(error),
    };
    let client = window.creation.geometry;
    if client.client_width == 0 || client.client_height == 0 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    if (present.back_buffer_width != 0 && present.back_buffer_width != client.client_width)
        || (present.back_buffer_height != 0 && present.back_buffer_height != client.client_height)
        || !matches!(
            present.back_buffer_format,
            D3D9_D3DFMT_UNKNOWN | D3D9_D3DFMT_X8R8G8B8
        )
        || !matches!(present.back_buffer_count, 0 | 1)
        || present.multi_sample_type != D3D9_D3DMULTISAMPLE_NONE
        || present.multi_sample_quality != 0
        || present.swap_effect != D3D9_D3DSWAPEFFECT_DISCARD
        || present.windowed != 1
        || present.enable_auto_depth_stencil != 0
        || present.flags != 0
        || present.full_screen_refresh_rate_in_hz != 0
        || present.presentation_interval != 0
    {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    }
    let graphics = match call.runtime.validate_graphics_device_creation(focus_window) {
        Ok(Some(request)) => request,
        Ok(None) => return call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(error) => return call.runtime_fault(error),
    };
    let presentation = graphics.presentation();
    let resolved_prefix = present.resolved_prefix(
        presentation.width,
        presentation.height,
        D3D9_D3DFMT_X8R8G8B8,
    );
    let parent_interface_id = call.frame().interface_id;
    let installed = match call.runtime.install_com_object_with_guest_writes(
        Some(parent_interface_id),
        direct3ddevice9_object_spec(),
        |object| {
            let interface = object
                .interfaces
                .iter()
                .find(|(iid, _)| *iid == D3D9_IDIRECT3DDEVICE9_IID)
                .ok_or_else(|| RuntimeStateError::new("device object has no D3D9 interface"))?;
            Ok(vec![
                (present_address, resolved_prefix),
                (output_address, interface.1.0.to_le_bytes().to_vec()),
            ])
        },
    ) {
        Ok(result) => result,
        Err(error) => return call.runtime_fault(error),
    };
    call.runtime
        .insert_graphics_device(graphics, installed.object_id);
    call.complete(COM_S_OK.bits())
}

fn dispatch_create_texture(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [width, height, levels, usage, format, pool, output, shared] =
        call.frame().explicit_arguments.as_slice()
    else {
        return call.fault("CreateTexture requires eight arguments");
    };
    let (width, height, levels, usage, format, pool, output, shared) = (
        *width, *height, *levels, *usage, *format, *pool, *output, *shared,
    );
    if width == 0 || height == 0 || output == 0 || shared != 0 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    // One bounded level of XRGB pixels is the implemented texture representation.
    // Other formats/mipmap/shared-resource requests must not appear to succeed.
    if width > 4096
        || height > 4096
        || levels != 1
        || format != D3D9_D3DFMT_X8R8G8B8
        || !matches!(usage, 0 | D3D9_D3DUSAGE_DYNAMIC)
        || !matches!(pool, D3D9_D3DPOOL_DEFAULT | D3D9_D3DPOOL_SYSTEMMEM)
        || (usage == D3D9_D3DUSAGE_DYNAMIC && pool != D3D9_D3DPOOL_DEFAULT)
    {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    }
    let logical_usage = if usage == 0 {
        RuntimeGraphicsTextureUsage::None
    } else {
        RuntimeGraphicsTextureUsage::Dynamic
    };
    let logical_pool = if pool == D3D9_D3DPOOL_DEFAULT {
        RuntimeGraphicsTexturePool::Default
    } else {
        RuntimeGraphicsTexturePool::SystemMemory
    };
    create_texture(
        call,
        width,
        height,
        levels,
        logical_usage,
        logical_pool,
        output,
    )
}

fn create_texture(
    call: ComFrontendCall<'_>,
    width: u32,
    height: u32,
    levels: u32,
    logical_usage: RuntimeGraphicsTextureUsage,
    logical_pool: RuntimeGraphicsTexturePool,
    output_pointer: u32,
) -> ComDispatchOutcome {
    let output_address = GuestAddress(output_pointer);
    if let Err(error) = call
        .runtime
        .memory
        .validate_access(output_address, 4, MemoryAccess::Write)
    {
        return call.memory_fault(error);
    }
    let creation = match call.runtime.validate_graphics_texture_creation(
        call.frame().object_id,
        RuntimeGraphicsTextureDescriptor {
            width,
            height,
            levels,
            usage: logical_usage,
            color_format: RuntimeGraphicsColorFormat::B8g8r8x8Unorm,
            pool: logical_pool,
        },
    ) {
        Ok(plan) => plan,
        Err(error) => return call.runtime_fault(error),
    };
    let parent_interface_id = call.frame().interface_id;
    let installed = match call.runtime.install_com_object_with_guest_writes(
        Some(parent_interface_id),
        direct3dtexture9_object_spec(),
        |object| {
            let interface = object
                .interfaces
                .iter()
                .find(|(iid, _)| *iid == D3D9_IDIRECT3DTEXTURE9_IID)
                .ok_or_else(|| {
                    RuntimeStateError::new(
                        "installed D3D9 texture object has no IDirect3DTexture9 interface",
                    )
                })?;
            Ok(vec![(output_address, interface.1.0.to_le_bytes().to_vec())])
        },
    ) {
        Ok(result) => result,
        Err(error) => return call.runtime_fault(error),
    };
    call.runtime
        .insert_graphics_texture(creation, installed.object_id);
    call.complete(COM_S_OK.bits())
}

fn dispatch_create_vertex_buffer(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [length, usage, fvf, pool, output, shared] = call.frame().explicit_arguments.as_slice()
    else {
        return call.fault("CreateVertexBuffer requires six arguments");
    };
    let (length, usage, fvf, pool, output, shared) =
        (*length, *usage, *fvf, *pool, *output, *shared);
    if length == 0 || output == 0 || shared != 0 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    if length > 1024 * 1024
        || usage != D3D9_D3DUSAGE_WRITEONLY
        || fvf != (D3D9_D3DFVF_XYZRHW | D3D9_D3DFVF_DIFFUSE | D3D9_D3DFVF_TEX1)
        || pool != D3D9_D3DPOOL_MANAGED
    {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    }
    create_vertex_buffer(call, length, fvf, output)
}

fn create_vertex_buffer(
    call: ComFrontendCall<'_>,
    byte_length: u32,
    flexible_vertex_format: u32,
    output_pointer: u32,
) -> ComDispatchOutcome {
    let parent_interface_id = call.frame().interface_id;
    let device_com_object_id = call.frame().object_id;
    let result = call.runtime.install_graphics_vertex_buffer_object(
        parent_interface_id,
        device_com_object_id,
        RuntimeGraphicsVertexBufferDescriptor {
            byte_length,
            usage: RuntimeGraphicsVertexBufferUsage::WriteOnly,
            flexible_vertex_format,
            pool: RuntimeGraphicsVertexBufferPool::Managed,
        },
        GuestAddress(output_pointer),
    );
    if let Err(error) = result {
        return call.runtime_fault(error);
    }
    call.complete(COM_S_OK.bits())
}

fn set_render_state(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [state, value] = call.frame().explicit_arguments.as_slice() else {
        return call.fault("SetRenderState requires two arguments");
    };
    let (state, value) = (*state, *value);
    let update = match (state, value) {
        (D3D9_D3DRS_CULLMODE, D3D9_D3DCULL_NONE) => {
            RuntimeGraphicsRenderStateUpdate::CullMode(RuntimeGraphicsCullMode::None)
        }
        (D3D9_D3DRS_ZENABLE, 0) => RuntimeGraphicsRenderStateUpdate::DepthTestEnabled(false),
        (D3D9_D3DRS_LIGHTING, 0) => RuntimeGraphicsRenderStateUpdate::LightingEnabled(false),
        (D3D9_D3DRS_ALPHABLENDENABLE, 1) => {
            RuntimeGraphicsRenderStateUpdate::AlphaBlendEnabled(true)
        }
        (D3D9_D3DRS_SRCBLEND, D3D9_D3DBLEND_SRCALPHA) => {
            RuntimeGraphicsRenderStateUpdate::SourceBlendFactor(
                RuntimeGraphicsBlendFactor::SourceAlpha,
            )
        }
        (D3D9_D3DRS_DESTBLEND, D3D9_D3DBLEND_INVSRCALPHA) => {
            RuntimeGraphicsRenderStateUpdate::DestinationBlendFactor(
                RuntimeGraphicsBlendFactor::InverseSourceAlpha,
            )
        }
        (D3D9_D3DRS_BLENDOP, D3D9_D3DBLENDOP_ADD) => {
            RuntimeGraphicsRenderStateUpdate::BlendOperation(RuntimeGraphicsBlendOperation::Add)
        }
        _ => {
            return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
        }
    };
    let device_com_object_id = call.frame().object_id;
    match call
        .runtime
        .update_graphics_render_state(device_com_object_id, update)
    {
        Ok(()) => call.complete(COM_S_OK.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_set_stream_source(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [stream, vertex_buffer_pointer, offset_in_bytes, stride] =
        call.frame().explicit_arguments.as_slice()
    else {
        return call.fault("SetStreamSource requires stream, buffer, offset, and stride");
    };
    if *stream != 0 || *offset_in_bytes != 0 || *stride != 28 {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    }
    let interface = match call
        .runtime
        .com_interface_for_pointer(GuestAddress(*vertex_buffer_pointer))
    {
        Ok(Some(interface)) if interface.iid == D3D9_IDIRECT3DVERTEXBUFFER9_IID => interface,
        Ok(_) => {
            return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
        }
        Err(error) => return call.runtime_fault(error),
    };
    let device_com_object_id = call.frame().object_id;
    let vertex_buffer_com_object_id = interface.object_id;
    match call.runtime.bind_graphics_stream_source(
        device_com_object_id,
        *stream,
        vertex_buffer_com_object_id,
        *offset_in_bytes,
        *stride,
    ) {
        Ok(true) => call.complete(COM_S_OK.bits()),
        Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_set_fvf(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [format] = call.frame().explicit_arguments.as_slice() else {
        return call.fault("SetFVF requires one format word");
    };
    if *format != (D3D9_D3DFVF_XYZRHW | D3D9_D3DFVF_DIFFUSE | D3D9_D3DFVF_TEX1) {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    }
    let device_com_object_id = call.frame().object_id;
    match call.runtime.update_graphics_flexible_vertex_format(
        device_com_object_id,
        RuntimeGraphicsFlexibleVertexFormat::XyzRhwDiffuseTexture1,
    ) {
        Ok(()) => call.complete(COM_S_OK.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_set_texture(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [stage, texture_pointer] = call.frame().explicit_arguments.as_slice() else {
        return call.fault("SetTexture requires stage and texture pointer");
    };
    if *stage != 0 {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    }
    let texture_com_object_id = if *texture_pointer == 0 {
        None
    } else {
        let interface = match call
            .runtime
            .com_interface_for_pointer(GuestAddress(*texture_pointer))
        {
            Ok(Some(interface)) if interface.iid == D3D9_IDIRECT3DTEXTURE9_IID => interface,
            Ok(_) => {
                return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
            }
            Err(error) => return call.runtime_fault(error),
        };
        let texture = match call
            .runtime
            .graphics_texture_for_com_object(interface.object_id)
        {
            Ok(texture) => texture,
            Err(error) => return call.runtime_fault(error),
        };
        if texture.descriptor.pool != RuntimeGraphicsTexturePool::Default {
            return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
        }
        Some(interface.object_id)
    };
    let device_com_object_id = call.frame().object_id;
    match call
        .runtime
        .bind_graphics_texture(device_com_object_id, *stage, texture_com_object_id)
    {
        Ok(true) => call.complete(COM_S_OK.bits()),
        Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_set_sampler_state(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [sampler, state, value] = call.frame().explicit_arguments.as_slice() else {
        return call.fault("SetSamplerState requires sampler, state, and value");
    };
    let update = match (*sampler, *state, *value) {
        (0, D3D9_D3DSAMP_MINFILTER, D3D9_D3DTEXF_LINEAR) => {
            RuntimeGraphicsSamplerStateUpdate::MinFilter {
                sampler: 0,
                filter: RuntimeGraphicsTextureFilter::Linear,
            }
        }
        (0, D3D9_D3DSAMP_MAGFILTER, D3D9_D3DTEXF_LINEAR) => {
            RuntimeGraphicsSamplerStateUpdate::MagFilter {
                sampler: 0,
                filter: RuntimeGraphicsTextureFilter::Linear,
            }
        }
        _ => {
            return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
        }
    };
    let device_com_object_id = call.frame().object_id;
    if let Err(error) = call
        .runtime
        .update_graphics_sampler_state(device_com_object_id, update)
    {
        return call.runtime_fault(error);
    }
    call.complete(COM_S_OK.bits())
}

fn dispatch_set_texture_stage_state(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [stage, state, value] = call.frame().explicit_arguments.as_slice() else {
        return call.fault("SetTextureStageState requires stage, state, and value");
    };
    if *stage != 0 {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    }
    let update = match (*state, *value) {
        (D3D9_D3DTSS_COLOROP, D3D9_D3DTOP_MODULATE) => {
            RuntimeGraphicsTextureStageStateUpdate::ColorOperation {
                stage: 0,
                operation: RuntimeGraphicsTextureOperation::Modulate,
            }
        }
        (D3D9_D3DTSS_COLORARG1, D3D9_D3DTA_TEXTURE) => {
            RuntimeGraphicsTextureStageStateUpdate::ColorArgumentOne {
                stage: 0,
                argument: RuntimeGraphicsTextureArgument::Texture,
            }
        }
        (D3D9_D3DTSS_COLORARG2, D3D9_D3DTA_DIFFUSE) => {
            RuntimeGraphicsTextureStageStateUpdate::ColorArgumentTwo {
                stage: 0,
                argument: RuntimeGraphicsTextureArgument::Diffuse,
            }
        }
        (D3D9_D3DTSS_ALPHAOP, D3D9_D3DTOP_SELECTARG1) => {
            RuntimeGraphicsTextureStageStateUpdate::AlphaOperation {
                stage: 0,
                operation: RuntimeGraphicsTextureOperation::SelectArgumentOne,
            }
        }
        (D3D9_D3DTSS_ALPHAARG1, D3D9_D3DTA_TEXTURE) => {
            RuntimeGraphicsTextureStageStateUpdate::AlphaArgumentOne {
                stage: 0,
                argument: RuntimeGraphicsTextureArgument::Texture,
            }
        }
        (D3D9_D3DTSS_ALPHAARG2, D3D9_D3DTA_DIFFUSE) => {
            RuntimeGraphicsTextureStageStateUpdate::AlphaArgumentTwo {
                stage: 0,
                argument: RuntimeGraphicsTextureArgument::Diffuse,
            }
        }
        _ => {
            return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
        }
    };
    let device_com_object_id = call.frame().object_id;
    if let Err(error) = call
        .runtime
        .update_graphics_texture_stage_state(device_com_object_id, update)
    {
        return call.runtime_fault(error);
    }
    call.complete(COM_S_OK.bits())
}

fn dispatch_draw_primitive(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [primitive_type, start_vertex, primitive_count] =
        call.frame().explicit_arguments.as_slice()
    else {
        return call.fault("DrawPrimitive requires topology, start vertex, and primitive count");
    };
    if *primitive_type != D3D9_D3DPT_TRIANGLESTRIP || *start_vertex != 0 || *primitive_count != 2 {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    }
    let device_com_object_id = call.frame().object_id;
    match call
        .runtime
        .submit_graphics_draw(device_com_object_id, *start_vertex, *primitive_count)
    {
        Ok(true) => call.complete(COM_S_OK.bits()),
        Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_vertex_buffer_lock(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [offset, length, output, flags] = call.frame().explicit_arguments.as_slice() else {
        return call.fault("Lock requires four arguments");
    };
    let (offset, length, output, flags) = (*offset, *length, *output, *flags);
    if output == 0 || offset != 0 || length != 0 || flags != 0 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    vertex_buffer_lock(call, offset, length, output, flags)
}

fn vertex_buffer_lock(
    call: ComFrontendCall<'_>,
    offset: u32,
    byte_length: u32,
    output_pointer: u32,
    flags: u32,
) -> ComDispatchOutcome {
    let vertex_buffer_com_object_id = call.frame().object_id;
    let result = call.runtime.lock_graphics_vertex_buffer(
        vertex_buffer_com_object_id,
        offset,
        byte_length,
        flags,
        GuestAddress(output_pointer),
    );
    match result {
        Ok(true) => call.complete(COM_S_OK.bits()),
        Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn vertex_buffer_unlock(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let object_id = call.frame().object_id;
    match call.runtime.unlock_graphics_vertex_buffer(object_id) {
        Ok(true) => call.complete(COM_S_OK.bits()),
        Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_texture_lock(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [level, output, rect, flags] = call.frame().explicit_arguments.as_slice() else {
        return call.fault("LockRect requires four arguments");
    };
    let (level, output, rect, flags) = (*level, *output, *rect, *flags);
    if output == 0 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    const READONLY: u32 = 0x0010;
    const NOSYSLOCK: u32 = 0x0800;
    const VALID_FLAGS: u32 =
        READONLY | NOSYSLOCK | D3D9_D3DLOCK_DISCARD | D3D9_D3DLOCK_NO_DIRTY_UPDATE;
    if level != 0
        || flags & !VALID_FLAGS != 0
        || (flags & D3D9_D3DLOCK_DISCARD != 0 && (rect != 0 || flags & READONLY != 0))
    {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    if flags & (READONLY | NOSYSLOCK) != 0 {
        return call.fault("LockRect READONLY and NOSYSLOCK flags are not implemented");
    }
    let texture = match call
        .runtime
        .graphics_texture_for_com_object(call.frame().object_id)
    {
        Ok(texture) => texture,
        Err(error) => return call.runtime_fault(error),
    };
    let (Ok(width), Ok(height)) = (
        i32::try_from(texture.descriptor.width),
        i32::try_from(texture.descriptor.height),
    ) else {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    };
    let rect = if rect == 0 {
        RuntimeRect {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        }
    } else {
        let address = GuestAddress(rect);
        if let Err(error) = call
            .runtime
            .memory
            .validate_access(address, 16, MemoryAccess::Read)
        {
            return call.memory_fault(error);
        }
        let bytes = match call.runtime.memory.read(address, 16) {
            Ok(bytes) => bytes,
            Err(error) => return call.memory_fault(error),
        };
        RuntimeRect {
            left: i32::from_le_bytes(bytes[0..4].try_into().expect("validated RECT left")),
            top: i32::from_le_bytes(bytes[4..8].try_into().expect("validated RECT top")),
            right: i32::from_le_bytes(bytes[8..12].try_into().expect("validated RECT right")),
            bottom: i32::from_le_bytes(bytes[12..16].try_into().expect("validated RECT bottom")),
        }
    };
    let object_id = call.frame().object_id;
    match call
        .runtime
        .lock_graphics_texture(object_id, level, rect, flags, GuestAddress(output))
    {
        Ok(true) => call.complete(COM_S_OK.bits()),
        Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_texture_unlock(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    if call.frame().explicit_arguments.as_slice() != [0] {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    let object_id = call.frame().object_id;
    match call.runtime.unlock_graphics_texture(object_id) {
        Ok(true) => call.complete(COM_S_OK.bits()),
        Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_get_surface_level(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [level, output] = call.frame().explicit_arguments.as_slice() else {
        return call.fault("GetSurfaceLevel requires two arguments");
    };
    let (level, output) = (*level, *output);
    if level != 0 || output == 0 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    get_surface_level(call, level, output)
}

fn get_surface_level(
    call: ComFrontendCall<'_>,
    level: u32,
    output_pointer: u32,
) -> ComDispatchOutcome {
    let output_address = GuestAddress(output_pointer);
    if let Err(error) = call
        .runtime
        .memory
        .validate_access(output_address, 4, MemoryAccess::Write)
    {
        return call.memory_fault(error);
    }
    let existing_surface = match call
        .runtime
        .graphics_surface_for_texture_level(call.frame().object_id, level)
    {
        Ok(surface) => surface.map(|surface| surface.com_object_id),
        Err(error) => return call.runtime_fault(error),
    };
    if let Some(surface_com_object_id) = existing_surface {
        let interface_id = match call
            .runtime
            .com_interface_for_object_iid(surface_com_object_id, D3D9_IDIRECT3DSURFACE9_IID)
        {
            Ok(interface) => interface.id,
            Err(error) => return call.runtime_fault(error),
        };
        match call
            .runtime
            .add_ref_interface_with_guest_pointer_write(interface_id, output_address)
        {
            Ok(_) => {}
            Err(error) => return call.runtime_fault(error),
        };
        return call.complete(COM_S_OK.bits());
    }

    let creation = match call
        .runtime
        .validate_graphics_surface_creation(call.frame().object_id, level)
    {
        Ok(Some(creation)) => creation,
        Ok(None) => return call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(error) => return call.runtime_fault(error),
    };
    let parent_interface_id = call.frame().interface_id;
    let installed = match call.runtime.install_com_object_with_guest_writes(
        Some(parent_interface_id),
        direct3dsurface9_object_spec(),
        |object| {
            let interface = object
                .interfaces
                .iter()
                .find(|(iid, _)| *iid == D3D9_IDIRECT3DSURFACE9_IID)
                .ok_or_else(|| {
                    RuntimeStateError::new(
                        "installed D3D9 surface object has no IDirect3DSurface9 interface",
                    )
                })?;
            Ok(vec![(output_address, interface.1.0.to_le_bytes().to_vec())])
        },
    ) {
        Ok(result) => result,
        Err(error) => return call.runtime_fault(error),
    };
    call.runtime
        .insert_graphics_surface(creation, installed.object_id);
    call.complete(COM_S_OK.bits())
}

fn dispatch_update_texture(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [source_pointer, destination_pointer] = call.frame().explicit_arguments.as_slice() else {
        return call.fault("IDirect3DDevice9::UpdateTexture requires two texture pointers");
    };
    if *source_pointer == 0 || *destination_pointer == 0 || source_pointer == destination_pointer {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    let resolve = |pointer| {
        call.runtime
            .com_interface_for_pointer(GuestAddress(pointer))
    };
    let source = match resolve(*source_pointer) {
        Ok(Some(interface)) if interface.iid == D3D9_IDIRECT3DTEXTURE9_IID => interface.object_id,
        Ok(_) => {
            return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
        }
        Err(error) => return call.runtime_fault(error),
    };
    let destination = match resolve(*destination_pointer) {
        Ok(Some(interface)) if interface.iid == D3D9_IDIRECT3DTEXTURE9_IID => interface.object_id,
        Ok(_) => {
            return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
        }
        Err(error) => return call.runtime_fault(error),
    };
    let device_com_object_id = call.frame().object_id;
    match call
        .runtime
        .update_graphics_texture(device_com_object_id, source, destination)
    {
        Ok(true) => call.complete(COM_S_OK.bits()),
        Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn dispatch_add_dirty_rect(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [rect_pointer] = call.frame().explicit_arguments.as_slice() else {
        return call.fault("IDirect3DTexture9::AddDirtyRect takes one RECT pointer");
    };
    let texture = match call
        .runtime
        .graphics_texture_for_com_object(call.frame().object_id)
    {
        Ok(texture) => texture,
        Err(error) => return call.runtime_fault(error),
    };
    if texture.descriptor.pool != RuntimeGraphicsTexturePool::SystemMemory {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    let rect = if *rect_pointer == 0 {
        None
    } else {
        let bytes = match call.runtime.memory.read(GuestAddress(*rect_pointer), 16) {
            Ok(bytes) => bytes,
            Err(error) => return call.memory_fault(error),
        };
        Some(RuntimeRect {
            left: i32::from_le_bytes(bytes[0..4].try_into().expect("checked RECT")),
            top: i32::from_le_bytes(bytes[4..8].try_into().expect("checked RECT")),
            right: i32::from_le_bytes(bytes[8..12].try_into().expect("checked RECT")),
            bottom: i32::from_le_bytes(bytes[12..16].try_into().expect("checked RECT")),
        })
    };
    let texture_com_object_id = call.frame().object_id;
    match call
        .runtime
        .mark_graphics_texture_dirty_region(texture_com_object_id, rect)
    {
        Ok(true) => call.complete(COM_S_OK.bits()),
        Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn surface_get_dc(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let output_pointer = call.frame().explicit_arguments[0];
    if output_pointer == 0 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    let output_address = GuestAddress(output_pointer);
    if let Err(error) = call
        .runtime
        .memory
        .validate_access(output_address, 4, MemoryAccess::Write)
    {
        return call.memory_fault(error);
    }
    let surface_com_object_id = call.frame().object_id;
    match call
        .runtime
        .acquire_graphics_surface_dc_with_guest_handle_write(surface_com_object_id, output_address)
    {
        Ok(Some(writes)) => writes,
        Ok(None) => {
            return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
        }
        Err(error) => return call.runtime_fault(error),
    };
    call.complete(COM_S_OK.bits())
}

fn surface_release_dc(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let device_context_handle = call.frame().explicit_arguments[0];
    let surface_com_object_id = call.frame().object_id;
    match call
        .runtime
        .release_graphics_surface_dc(surface_com_object_id, device_context_handle)
    {
        Ok(true) => call.complete(COM_S_OK.bits()),
        Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(error) => call.runtime_fault(error),
    }
}

fn complete_d3d9_write(
    call: ComFrontendCall<'_>,
    output: GuestAddress,
    bytes: Vec<u8>,
) -> ComDispatchOutcome {
    if let Err(error) =
        call.runtime
            .memory
            .validate_access(output, bytes.len(), MemoryAccess::Write)
    {
        return call.memory_fault(error);
    }

    let mut transaction = call.runtime.memory.begin_transaction();
    match transaction.write(output, &bytes) {
        Ok(_) => {}
        Err(error) => {
            drop(transaction);
            return call.memory_fault(error);
        }
    };
    transaction.commit_transaction();
    call.complete(COM_S_OK.bits())
}

fn handle_direct3d_create9(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let Some(sdk_version) = frame.argument_u32(0) else {
        return ApiHandlerResult::Fault(ApiFault::new(
            "decoded Direct3DCreate9 frame is missing its SDK version",
        ));
    };
    if sdk_version != D3D9_SDK_VERSION {
        return ApiHandlerResult::Immediate(ApiImmediate::new(Some(0)));
    }

    let installed = match runtime.install_com_object(direct3d9_object_spec()) {
        Ok(installed) => installed,
        Err(error) => {
            return ApiHandlerResult::Fault(ApiFault::from_runtime(error, endpoint));
        }
    };
    let Some(interface) = installed
        .interfaces
        .iter()
        .find(|(iid, _)| *iid == D3D9_IDIRECT3D9_IID)
    else {
        return ApiHandlerResult::Fault(ApiFault::new(
            "installed D3D9 root object has no IDirect3D9 interface",
        ));
    };
    ApiHandlerResult::Immediate(ApiImmediate::new(Some(interface.1.0)))
}

pub(crate) fn d3d9_argument_words(endpoint: &ApiEndpointIdentity) -> Option<u8> {
    if !endpoint.module.eq_ignore_ascii_case("d3d9.dll") {
        return None;
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return None;
    };
    match name.as_str() {
        "Direct3DCreate9" => Some(1),
        _ => None,
    }
}

pub(crate) fn direct3d9_object_spec() -> ComObjectSpec {
    ComObjectSpec {
        class: RuntimeComClass::D3d9,
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: D3D9_IDIRECT3D9_IID,
                name: "IDirect3D9".to_owned(),
                methods: vec![
                    ComMethodSpec::query_interface(),
                    ComMethodSpec::add_ref(),
                    ComMethodSpec::release(),
                    ComMethodSpec::unsupported("RegisterSoftwareDevice"),
                    ComMethodSpec::frontend("GetAdapterCount", 0),
                    ComMethodSpec::frontend("GetAdapterIdentifier", 3),
                    ComMethodSpec::unsupported("GetAdapterModeCount"),
                    ComMethodSpec::unsupported("EnumAdapterModes"),
                    ComMethodSpec::frontend("GetAdapterDisplayMode", 2),
                    ComMethodSpec::frontend("CheckDeviceType", 5),
                    ComMethodSpec::frontend("CheckDeviceFormat", 6),
                    ComMethodSpec::unsupported("CheckDeviceMultiSampleType"),
                    ComMethodSpec::unsupported("CheckDepthStencilMatch"),
                    ComMethodSpec::unsupported("CheckDeviceFormatConversion"),
                    ComMethodSpec::frontend("GetDeviceCaps", 3),
                    ComMethodSpec::frontend("GetAdapterMonitor", 1),
                    ComMethodSpec::frontend("CreateDevice", 6),
                ],
            },
        ],
    }
}

const D3D9_IDIRECT3DDEVICE9_METHOD_NAMES: [&str; 119] = [
    "QueryInterface",
    "AddRef",
    "Release",
    "TestCooperativeLevel",
    "GetAvailableTextureMem",
    "EvictManagedResources",
    "GetDirect3D",
    "GetDeviceCaps",
    "GetDisplayMode",
    "GetCreationParameters",
    "SetCursorProperties",
    "SetCursorPosition",
    "ShowCursor",
    "CreateAdditionalSwapChain",
    "GetSwapChain",
    "GetNumberOfSwapChains",
    "Reset",
    "Present",
    "GetBackBuffer",
    "GetRasterStatus",
    "SetDialogBoxMode",
    "SetGammaRamp",
    "GetGammaRamp",
    "CreateTexture",
    "CreateVolumeTexture",
    "CreateCubeTexture",
    "CreateVertexBuffer",
    "CreateIndexBuffer",
    "CreateRenderTarget",
    "CreateDepthStencilSurface",
    "UpdateSurface",
    "UpdateTexture",
    "GetRenderTargetData",
    "GetFrontBufferData",
    "StretchRect",
    "ColorFill",
    "CreateOffscreenPlainSurface",
    "SetRenderTarget",
    "GetRenderTarget",
    "SetDepthStencilSurface",
    "GetDepthStencilSurface",
    "BeginScene",
    "EndScene",
    "Clear",
    "SetTransform",
    "GetTransform",
    "MultiplyTransform",
    "SetViewport",
    "GetViewport",
    "SetMaterial",
    "GetMaterial",
    "SetLight",
    "GetLight",
    "LightEnable",
    "GetLightEnable",
    "SetClipPlane",
    "GetClipPlane",
    "SetRenderState",
    "GetRenderState",
    "CreateStateBlock",
    "BeginStateBlock",
    "EndStateBlock",
    "SetClipStatus",
    "GetClipStatus",
    "GetTexture",
    "SetTexture",
    "GetTextureStageState",
    "SetTextureStageState",
    "GetSamplerState",
    "SetSamplerState",
    "ValidateDevice",
    "SetPaletteEntries",
    "GetPaletteEntries",
    "SetCurrentTexturePalette",
    "GetCurrentTexturePalette",
    "SetScissorRect",
    "GetScissorRect",
    "SetSoftwareVertexProcessing",
    "GetSoftwareVertexProcessing",
    "SetNPatchMode",
    "GetNPatchMode",
    "DrawPrimitive",
    "DrawIndexedPrimitive",
    "DrawPrimitiveUP",
    "DrawIndexedPrimitiveUP",
    "ProcessVertices",
    "CreateVertexDeclaration",
    "SetVertexDeclaration",
    "GetVertexDeclaration",
    "SetFVF",
    "GetFVF",
    "CreateVertexShader",
    "SetVertexShader",
    "GetVertexShader",
    "SetVertexShaderConstantF",
    "GetVertexShaderConstantF",
    "SetVertexShaderConstantI",
    "GetVertexShaderConstantI",
    "SetVertexShaderConstantB",
    "GetVertexShaderConstantB",
    "SetStreamSource",
    "GetStreamSource",
    "SetStreamSourceFreq",
    "GetStreamSourceFreq",
    "SetIndices",
    "GetIndices",
    "CreatePixelShader",
    "SetPixelShader",
    "GetPixelShader",
    "SetPixelShaderConstantF",
    "GetPixelShaderConstantF",
    "SetPixelShaderConstantI",
    "GetPixelShaderConstantI",
    "SetPixelShaderConstantB",
    "GetPixelShaderConstantB",
    "DrawRectPatch",
    "DrawTriPatch",
    "DeletePatch",
    "CreateQuery",
];

pub(crate) fn direct3ddevice9_object_spec() -> ComObjectSpec {
    let methods = D3D9_IDIRECT3DDEVICE9_METHOD_NAMES
        .iter()
        .enumerate()
        .map(|(slot, name)| match slot {
            0 => ComMethodSpec::query_interface(),
            1 => ComMethodSpec::add_ref(),
            2 => ComMethodSpec::release(),
            3 | 41 | 42 => ComMethodSpec::frontend(*name, 0),
            17 | 100 => ComMethodSpec::frontend(*name, 4),
            19 | 31 | 57 | 65 => ComMethodSpec::frontend(*name, 2),
            23 => ComMethodSpec::frontend(*name, 8),
            26 | 43 => ComMethodSpec::frontend(*name, 6),
            67 | 69 | 81 => ComMethodSpec::frontend(*name, 3),
            89 => ComMethodSpec::frontend(*name, 1),
            _ => ComMethodSpec::unsupported(*name),
        })
        .collect();
    ComObjectSpec {
        class: RuntimeComClass::D3d9Device,
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: D3D9_IDIRECT3DDEVICE9_IID,
                name: "IDirect3DDevice9".to_owned(),
                methods,
            },
        ],
    }
}

const D3D9_IDIRECT3DTEXTURE9_METHOD_NAMES: [&str; 22] = [
    "QueryInterface",
    "AddRef",
    "Release",
    "GetDevice",
    "SetPrivateData",
    "GetPrivateData",
    "FreePrivateData",
    "SetPriority",
    "GetPriority",
    "PreLoad",
    "GetType",
    "SetLOD",
    "GetLOD",
    "GetLevelCount",
    "SetAutoGenFilterType",
    "GetAutoGenFilterType",
    "GenerateMipSubLevels",
    "GetLevelDesc",
    "GetSurfaceLevel",
    "LockRect",
    "UnlockRect",
    "AddDirtyRect",
];

pub(crate) fn direct3dtexture9_object_spec() -> ComObjectSpec {
    let methods = D3D9_IDIRECT3DTEXTURE9_METHOD_NAMES
        .iter()
        .enumerate()
        .map(|(slot, name)| match slot {
            0 => ComMethodSpec::query_interface(),
            1 => ComMethodSpec::add_ref(),
            2 => ComMethodSpec::release(),
            18 => ComMethodSpec::frontend(*name, 2),
            19 => ComMethodSpec::frontend(*name, 4),
            20 | 21 => ComMethodSpec::frontend(*name, 1),
            _ => ComMethodSpec::unsupported(*name),
        })
        .collect();
    ComObjectSpec {
        class: RuntimeComClass::D3d9Texture,
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: D3D9_IDIRECT3DTEXTURE9_IID,
                name: "IDirect3DTexture9".to_owned(),
                methods,
            },
        ],
    }
}

const D3D9_IDIRECT3DVERTEXBUFFER9_METHOD_NAMES: [&str; 14] = [
    "QueryInterface",
    "AddRef",
    "Release",
    "GetDevice",
    "SetPrivateData",
    "GetPrivateData",
    "FreePrivateData",
    "SetPriority",
    "GetPriority",
    "PreLoad",
    "GetType",
    "Lock",
    "Unlock",
    "GetDesc",
];

pub(crate) fn direct3dvertexbuffer9_object_spec() -> ComObjectSpec {
    let methods = D3D9_IDIRECT3DVERTEXBUFFER9_METHOD_NAMES
        .iter()
        .enumerate()
        .map(|(slot, name)| match slot {
            0 => ComMethodSpec::query_interface(),
            1 => ComMethodSpec::add_ref(),
            2 => ComMethodSpec::release(),
            11 => ComMethodSpec::frontend(*name, 4),
            12 => ComMethodSpec::frontend(*name, 0),
            _ => ComMethodSpec::unsupported(*name),
        })
        .collect();
    ComObjectSpec {
        class: RuntimeComClass::D3d9VertexBuffer,
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: D3D9_IDIRECT3DVERTEXBUFFER9_IID,
                name: "IDirect3DVertexBuffer9".to_owned(),
                methods,
            },
        ],
    }
}

const D3D9_IDIRECT3DSURFACE9_METHOD_NAMES: [&str; 17] = [
    "QueryInterface",
    "AddRef",
    "Release",
    "GetDevice",
    "SetPrivateData",
    "GetPrivateData",
    "FreePrivateData",
    "SetPriority",
    "GetPriority",
    "PreLoad",
    "GetType",
    "GetContainer",
    "GetDesc",
    "LockRect",
    "UnlockRect",
    "GetDC",
    "ReleaseDC",
];

pub(crate) fn direct3dsurface9_object_spec() -> ComObjectSpec {
    let methods = D3D9_IDIRECT3DSURFACE9_METHOD_NAMES
        .iter()
        .enumerate()
        .map(|(slot, name)| match slot {
            0 => ComMethodSpec::query_interface(),
            1 => ComMethodSpec::add_ref(),
            2 => ComMethodSpec::release(),
            15 | 16 => ComMethodSpec::frontend(*name, 1),
            _ => ComMethodSpec::unsupported(*name),
        })
        .collect();
    ComObjectSpec {
        class: RuntimeComClass::D3d9Surface,
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid: D3D9_IDIRECT3DSURFACE9_IID,
                name: "IDirect3DSurface9".to_owned(),
                methods,
            },
        ],
    }
}
