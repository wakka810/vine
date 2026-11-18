//! Direct3D 8 ABI over the Runtime's shared graphics owner.
use crate::api::{
    ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiSymbol,
};
use crate::com::{
    COM_S_OK, ComDispatchOutcome, ComFrontendCall, ComInterfaceSpec, ComMethodSpec, ComObjectSpec,
};
use crate::d3d9::{D3D9_D3DERR_INVALIDCALL, D3D9_D3DERR_NOTAVAILABLE};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::{
    Runtime, RuntimeComClass, RuntimeComIid, RuntimeGraphicsPresentMode, RuntimeStateError,
};

const D3DPRESENTFLAG_LOCKABLE_BACKBUFFER: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeviceState {
    pub parameters: [u32; 13],
    pub transforms: std::collections::BTreeMap<u32, [u32; 16]>,
    pub render_values: std::collections::BTreeMap<u32, u32>,
    pub stage_values: std::collections::BTreeMap<(u32, u32), u32>,
    pub back_buffer_surface: Option<crate::runtime::RuntimeComObjectId>,
}

pub(crate) const D3D8_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0xda, 0xe8, 0xd9, 0x1d, 0x77, 0x1c, 0x40, 0x4d, 0xb0, 0xcf, 0x98, 0xfe, 0xfd, 0xff, 0x95, 0x12,
]);
pub(crate) const DEVICE8_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0xdf, 0xe5, 0x85, 0x73, 0xe8, 0x8f, 0xd5, 0x41, 0x86, 0xb6, 0xd7, 0xb4, 0x85, 0x47, 0xb6, 0xcf,
]);
pub(crate) const SURFACE8_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0xca, 0xeb, 0x6e, 0xb9, 0x26, 0xb3, 0xa5, 0x4e, 0x88, 0x2f, 0x2f, 0xf5, 0xba, 0xe0, 0x21, 0xdd,
]);
pub(crate) const VERTEX_BUFFER8_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0xc7, 0xea, 0xee, 0x8a, 0xf9, 0x05, 0xd4, 0x44, 0xb5, 0x91, 0, 0x0b, 0x0d, 0xf1, 0xcb, 0x95,
]);
pub(crate) const TEXTURE8_IID: RuntimeComIid = RuntimeComIid::from_windows_bytes([
    0x75, 0xd5, 0xcd, 0xe4, 0x66, 0x28, 0x01, 0x4f, 0xb1, 0x2e, 0x7e, 0xec, 0xe1, 0xec, 0x93, 0x58,
]);

pub(crate) fn d3d8_argument_words(endpoint: &ApiEndpointIdentity) -> Option<u8> {
    (endpoint.module.eq_ignore_ascii_case("d3d8.dll")
        && matches!(&endpoint.symbol, ApiSymbol::Name {name} if name == "Direct3DCreate8"))
    .then_some(1)
}

pub(crate) fn dispatch_d3d8_api(frame: &ApiCallFrame, runtime: &mut Runtime) -> ApiHandlerResult {
    if d3d8_argument_words(&frame.endpoint).is_none() {
        return ApiHandlerResult::Fault(ApiFault::new("unsupported D3D8 export"));
    }
    // DirectX 8.0 and 8.1 use the same public COM ABI.
    if !matches!(frame.argument_u32(0), Some(120 | 220)) {
        return ApiHandlerResult::Immediate(ApiImmediate::new(Some(0)));
    }
    match runtime.install_com_object(root_spec()) {
        Ok(object) => ApiHandlerResult::Immediate(ApiImmediate::new(Some(
            object
                .interfaces
                .iter()
                .find(|(iid, _)| *iid == D3D8_IID)
                .expect("D3D8 interface")
                .1
                .0,
        ))),
        Err(error) => ApiHandlerResult::Fault(ApiFault::from_runtime(error, &frame.endpoint)),
    }
}

fn interface_spec(
    class: RuntimeComClass,
    iid: RuntimeComIid,
    name: &str,
    methods: &[(&str, Option<u16>)],
) -> ComObjectSpec {
    let mut slots = vec![
        ComMethodSpec::query_interface(),
        ComMethodSpec::add_ref(),
        ComMethodSpec::release(),
    ];
    slots.extend(methods.iter().map(|(name, words)| match words {
        Some(words) => ComMethodSpec::frontend(*name, *words),
        None => ComMethodSpec::unsupported(*name),
    }));
    ComObjectSpec {
        class,
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            ComInterfaceSpec {
                iid,
                name: name.to_owned(),
                methods: slots,
            },
        ],
    }
}

pub(crate) fn root_spec() -> ComObjectSpec {
    interface_spec(
        RuntimeComClass::D3d8,
        D3D8_IID,
        "IDirect3D8",
        &[
            ("RegisterSoftwareDevice", None),
            ("GetAdapterCount", Some(0)),
            ("GetAdapterIdentifier", Some(3)),
            ("GetAdapterModeCount", Some(1)),
            ("EnumAdapterModes", Some(3)),
            ("GetAdapterDisplayMode", Some(2)),
            ("CheckDeviceType", Some(5)),
            ("CheckDeviceFormat", Some(6)),
            ("CheckDeviceMultiSampleType", Some(5)),
            ("CheckDepthStencilMatch", Some(5)),
            ("GetDeviceCaps", Some(3)),
            ("GetAdapterMonitor", Some(1)),
            ("CreateDevice", Some(6)),
        ],
    )
}

fn write_output(call: ComFrontendCall<'_>, output: u32, bytes: &[u8]) -> ComDispatchOutcome {
    if output == 0 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    if let Err(error) = call.runtime.memory.write(GuestAddress(output), bytes) {
        return call.memory_fault(error);
    }
    call.complete(COM_S_OK.bits())
}

fn caps() -> Vec<u8> {
    // Fixed-function, pretransformed vertices and a single texture stage.
    // Programmable shaders and hardware transform/lighting are not advertised.
    let mut words = [0_u32; 53];
    words[0] = 1; // HAL
    words[2] = 0x20000; // scanline
    words[5] = 0x8000_0001; // immediate / one presentation interval
    words[7] = 0x80040 | 0x400; // hardware rasterization, TL vertices
    words[8] = 0x10; // cull none
    words[11] = 0x10; // source alpha
    words[12] = 0x02 | 0x20; // one / inverse source alpha
    words[15] = 1 | 4; // perspective-correct textures
    words[16] = 0x0300_0300; // point and linear min/mag
    words[19] = 0x5; // wrap/clamp
    words[22] = 4096;
    words[23] = 4096;
    words[25] = 4096;
    words[26] = 4096;
    words[27] = 1;
    words[28] = 1.0_f32.to_bits();
    words[35] = 1;
    words[36] = (1 << 0) | (1 << 1) | (1 << 3) | (1 << 6); // disable/selectarg1/modulate/add
    words[37] = 1;
    words[38] = 1;
    words[45] = 2;
    words[46] = 65535;
    words[47] = 1;
    words[48] = 28;
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}

pub(crate) fn dispatch_d3d8_com(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    if call.class() == RuntimeComClass::D3d8VertexBuffer {
        return dispatch_vertex_buffer(call);
    }
    if call.class() == RuntimeComClass::D3d8Texture {
        return dispatch_texture(call);
    }
    if call.class() == RuntimeComClass::D3d8TextureSurface {
        return dispatch_texture_surface(call);
    }
    if call.class() == RuntimeComClass::D3d8BackBuffer {
        return dispatch_back_buffer(call);
    }
    if call.class() == RuntimeComClass::D3d8Surface {
        return dispatch_surface(call);
    }
    if call.class() == RuntimeComClass::D3d8Device {
        return dispatch_device(call);
    }
    let args = call.frame().explicit_arguments.clone();
    match (call.frame().vtable_slot, args.as_slice()) {
        (4, []) => call.complete(1),
        (6, [adapter]) => call.complete(u32::from(*adapter == 0)),
        (14, [adapter]) => {
            let monitor = if *adapter == 0 {
                call.runtime.primary_monitor().map_or(0, |m| m.handle)
            } else {
                0
            };
            call.complete(monitor)
        }
        (5, [adapter, flags, output]) => {
            if *adapter != 0 || *flags & !2 != 0 {
                return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
            }
            let mut bytes = vec![0; 1068];
            bytes[..13].copy_from_slice(b"vine-d3d8.dll");
            let description = b"Vine Virtual Direct3D8 Adapter";
            bytes[512..512 + description.len()].copy_from_slice(description);
            bytes[1024..1032].copy_from_slice(&0x0001_0000_0000_0000_u64.to_le_bytes());
            write_output(call, *output, &bytes)
        }
        (7, [adapter, index, output]) if *adapter == 0 && *index == 0 => {
            write_output(call, *output, &display_mode(640, 480))
        }
        (8, [adapter, output]) if *adapter == 0 => {
            let Some(monitor) = call.runtime.primary_monitor() else {
                return call.fault("D3D8 requires a display");
            };
            let (width, height) = (
                (monitor.rect.right - monitor.rect.left) as u32,
                (monitor.rect.bottom - monitor.rect.top) as u32,
            );
            write_output(call, *output, &display_mode(width, height))
        }
        (9, [adapter, kind, display, back, windowed]) => {
            let supported = *adapter == 0
                && *kind == 1
                && *display == 22
                && (*back == 22 || (*back == 0 && *windowed != 0));
            call.complete(if supported {
                COM_S_OK.bits()
            } else {
                D3D9_D3DERR_NOTAVAILABLE.bits()
            })
        }
        (10, [adapter, kind, display, usage, resource, format]) => {
            let supported = *adapter == 0
                && *kind == 1
                && *display == 22
                && *usage == 0
                && *resource == 3
                && matches!(*format, 21 | 22);
            call.complete(if supported {
                COM_S_OK.bits()
            } else {
                D3D9_D3DERR_NOTAVAILABLE.bits()
            })
        }
        (11, [adapter, kind, format, _, samples]) => call.complete(
            if *adapter == 0 && *kind == 1 && *format == 22 && *samples == 0 {
                COM_S_OK.bits()
            } else {
                D3D9_D3DERR_NOTAVAILABLE.bits()
            },
        ),
        (12, [..]) => call.complete(D3D9_D3DERR_NOTAVAILABLE.bits()),
        (13, [adapter, kind, output]) if *adapter == 0 && *kind == 1 => {
            write_output(call, *output, &caps())
        }
        (15, [..]) => create_device(call),
        _ => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
    }
}

fn display_mode(width: u32, height: u32) -> Vec<u8> {
    [width, height, 60, 22]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect()
}

fn create_device(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [adapter, kind, focus, behavior, present, output] =
        call.frame().explicit_arguments.as_slice()
    else {
        return call.fault("CreateDevice arguments");
    };
    let (adapter, kind, focus, behavior, present, output) =
        (*adapter, *kind, *focus, *behavior, *present, *output);
    if adapter != 0 || kind != 1 || focus == 0 || present == 0 || output == 0 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    if behavior & !(2 | 4 | 0x20) != 0 || behavior & 0x60 != 0x20 {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    }
    let p = GuestAddress(present);
    let out = GuestAddress(output);
    if u64::from(output) < u64::from(present) + 52 && u64::from(present) < u64::from(output) + 4 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    for (address, length, access) in [
        (p, 52, MemoryAccess::Read),
        (p, 16, MemoryAccess::Write),
        (out, 4, MemoryAccess::Write),
    ] {
        if let Err(error) = call.runtime.memory.validate_access(address, length, access) {
            return call.memory_fault(error);
        }
    }
    let bytes = match call.runtime.memory.read(p, 52) {
        Ok(bytes) => bytes,
        Err(error) => return call.memory_fault(error),
    };
    let w: Vec<u32> = bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    if !matches!(w[2], 0 | 22)
        || w[3] > 1
        || w[4] != 0
        || !matches!(w[5], 1..=4)
        || (w[6] != 0 && w[6] != focus)
        || w[7] > 1
        || w[8] > 1
        || w[8] != 0 && w[9] != 80
        || w[10] & !D3DPRESENTFLAG_LOCKABLE_BACKBUFFER != 0
        || (w[7] != 0 && w[11] != 0)
        || !matches!(w[11], 0 | 60)
        || !matches!(w[12], 0 | 1 | 0x8000_0000)
    {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    }
    let creation = match call.runtime.validate_graphics_device_creation(
        focus,
        w[0],
        w[1],
        if matches!(w[5], 3 | 4) {
            RuntimeGraphicsPresentMode::Copy
        } else {
            RuntimeGraphicsPresentMode::Discard
        },
    ) {
        Ok(Some(c)) => c,
        Ok(None) => return call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(e) => return call.runtime_fault(e),
    };
    let presentation = creation.presentation();
    let resolved: Vec<u8> = [presentation.width, presentation.height, 22, w[3].max(1)]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    let parent = call.frame().interface_id;
    let installed = match call.runtime.install_com_object_with_guest_writes(
        Some(parent),
        device_spec(),
        |object| {
            let pointer = object
                .interfaces
                .iter()
                .find(|(iid, _)| *iid == DEVICE8_IID)
                .ok_or_else(|| RuntimeStateError::new("D3D8 device interface missing"))?
                .1;
            Ok(vec![(p, resolved), (out, pointer.0.to_le_bytes().to_vec())])
        },
    ) {
        Ok(object) => object,
        Err(error) => return call.runtime_fault(error),
    };
    call.runtime
        .insert_graphics_device(creation, installed.object_id);
    call.runtime.configure_d3d8(
        installed.object_id,
        DeviceState {
            parameters: w.as_slice().try_into().unwrap(),
            transforms: std::collections::BTreeMap::new(),
            render_values: [
                (7, w[8]),
                (9, 2),
                (14, 1),
                (15, 0),
                (23, 4),
                (24, 0),
                (25, 8),
                (28, 0),
                (34, 0),
                (35, 0),
                (36, 0),
                (37, 1.0f32.to_bits()),
                (38, 1.0f32.to_bits()),
                (60, 0xffffffff),
                (171, 1),
            ]
            .into_iter()
            .collect(),
            stage_values: [
                (1, 4),
                (2, 2),
                (3, 0),
                (4, 2),
                (5, 2),
                (6, 0),
                (13, 1),
                (14, 1),
                (16, 1),
                (17, 1),
                (18, 0),
                (24, 0),
                (25, 1),
            ]
            .into_iter()
            .map(|(key, value)| ((0, key), value))
            .collect(),
            back_buffer_surface: None,
        },
    );
    call.complete(COM_S_OK.bits())
}

fn dispatch_device(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    match call.frame().vtable_slot {
        3 | 5 => {
            if let Err(e) = call
                .runtime
                .graphics_device_by_com_object(call.frame().object_id)
            {
                return call.runtime_fault(e);
            }
            call.complete(COM_S_OK.bits())
        }
        4 => match call
            .runtime
            .available_graphics_texture_memory(call.frame().object_id)
        {
            Ok(bytes) => call.complete(bytes),
            Err(e) => call.runtime_fault(e),
        },
        6 => {
            let output = GuestAddress(call.frame().explicit_arguments[0]);
            if output.0 == 0 {
                return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
            }
            let parent = match call.runtime.com().object(call.frame().object_id) {
                Ok(object) => match object.parent_interface_id {
                    Some(parent) => parent,
                    None => return call.fault("D3D8 device has no parent interface"),
                },
                Err(e) => return call.runtime_fault(e),
            };
            match call
                .runtime
                .add_ref_interface_with_guest_pointer_write(parent, output)
            {
                Ok(_) => call.complete(0),
                Err(e) => call.runtime_fault(e),
            }
        }
        7 => {
            let output = call.frame().explicit_arguments[0];
            write_output(call, output, &caps())
        }
        8 => {
            let output = call.frame().explicit_arguments[0];
            let p = match call
                .runtime
                .graphics_device_by_com_object(call.frame().object_id)
            {
                Ok(device) => device.presentation,
                Err(e) => return call.runtime_fault(e),
            };
            write_output(
                call,
                output,
                &[p.width, p.height, 60, 22]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
        }
        15 => crate::d3d9::dispatch_present(call),
        16 => get_back_buffer(call),
        20 => create_texture(call),
        23 => create_vertex_buffer(call),
        25 => call.complete(D3D9_D3DERR_NOTAVAILABLE.bits()),
        27 => create_image_surface(call),
        28 => copy_rects(call),
        34 => crate::d3d9::dispatch_begin_scene(call),
        35 => crate::d3d9::dispatch_end_scene(call),
        36 => {
            let args = call.frame().explicit_arguments.clone();
            if args[0] != 0 || args[1] != 0 {
                return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
            }
            match call
                .runtime
                .clear_d3d8(call.frame().object_id, args[2], args[3], args[4])
            {
                Ok(true) => call.complete(COM_S_OK.bits()),
                Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
                Err(e) => call.runtime_fault(e),
            }
        }
        37..=39 => transform(call),
        40 => crate::d3d9::dispatch_set_viewport(call),
        41 => {
            let device = match call
                .runtime
                .graphics_device_by_com_object(call.frame().object_id)
            {
                Ok(d) => d,
                Err(e) => return call.runtime_fault(e),
            };
            let viewport = device.viewport;
            let output = call.frame().explicit_arguments[0];
            write_output(
                call,
                output,
                &viewport
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
        }
        50 => render_state(call),
        51 => {
            let args = call.frame().explicit_arguments.clone();
            let state = match call.runtime.d3d8_state_mut(call.frame().object_id) {
                Ok(s) => s,
                Err(e) => return call.runtime_fault(e),
            };
            let Some(value) = state.render_values.get(&args[0]).copied() else {
                return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
            };
            write_output(call, args[1], &value.to_le_bytes())
        }
        61 => set_texture(call),
        62 | 63 => texture_stage_state(call),
        70 => draw_d3d8(call, false),
        72 => draw_d3d8(call, true),
        76 => set_vertex_shader_d3d8(call),
        83 => {
            let a = call.frame().explicit_arguments.clone();
            if a[0] != 0 || !matches!(a[2], 20 | 24 | 28) {
                return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
            }
            let object = match call.runtime.com_interface_for_pointer(GuestAddress(a[1])) {
                Ok(Some(i)) if i.iid == VERTEX_BUFFER8_IID => i.object_id,
                Ok(_) => return call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
                Err(e) => return call.runtime_fault(e),
            };
            match call.runtime.bind_graphics_stream_source(
                call.frame().object_id,
                0,
                object,
                0,
                a[2],
            ) {
                Ok(true) => call.complete(0),
                Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
                Err(e) => call.runtime_fault(e),
            }
        }
        _ => call.fault("D3D8 device method is not implemented"),
    }
}

fn surface_spec() -> ComObjectSpec {
    surface_spec_for(RuntimeComClass::D3d8Surface)
}
fn surface_spec_for(class: RuntimeComClass) -> ComObjectSpec {
    interface_spec(
        class,
        SURFACE8_IID,
        "IDirect3DSurface8",
        &[
            ("GetDevice", None),
            ("SetPrivateData", None),
            ("GetPrivateData", None),
            ("FreePrivateData", None),
            ("GetContainer", None),
            ("GetDesc", Some(1)),
            ("LockRect", Some(3)),
            ("UnlockRect", Some(0)),
        ],
    )
}

fn get_back_buffer(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let args = call.frame().explicit_arguments.clone();
    if args[0] != 0 || args[1] != 0 || args[2] == 0 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    let output = GuestAddress(args[2]);
    if let Err(e) = call
        .runtime
        .memory
        .validate_access(output, 4, MemoryAccess::Write)
    {
        return call.memory_fault(e);
    }
    let existing = match call.runtime.d3d8_state_mut(call.frame().object_id) {
        Ok(s) => s.back_buffer_surface,
        Err(e) => return call.runtime_fault(e),
    };
    if let Some(object) = existing
        && let Ok(interface) = call
            .runtime
            .com_interface_for_object_iid(object, SURFACE8_IID)
    {
        let interface = interface.id;
        return match call
            .runtime
            .add_ref_interface_with_guest_pointer_write(interface, output)
        {
            Ok(_) => call.complete(0),
            Err(e) => call.runtime_fault(e),
        };
    }
    let parent = call.frame().interface_id;
    let object = match call.runtime.install_com_object_with_guest_writes(
        Some(parent),
        surface_spec_for(RuntimeComClass::D3d8BackBuffer),
        |object| {
            let pointer = object
                .interfaces
                .iter()
                .find(|(iid, _)| *iid == SURFACE8_IID)
                .ok_or_else(|| RuntimeStateError::new("back buffer interface missing"))?
                .1;
            Ok(vec![(output, pointer.0.to_le_bytes().to_vec())])
        },
    ) {
        Ok(o) => o,
        Err(e) => return call.runtime_fault(e),
    };
    call.runtime
        .d3d8_state_mut(call.frame().object_id)
        .expect("validated D3D8 device")
        .back_buffer_surface = Some(object.object_id);
    call.complete(0)
}

fn back_buffer_device(
    call: &ComFrontendCall<'_>,
) -> Result<crate::runtime::RuntimeComObjectId, RuntimeStateError> {
    let object = call.runtime.com().object(call.frame().object_id)?;
    let parent = object
        .parent_interface_id
        .ok_or_else(|| RuntimeStateError::new("back buffer has no device"))?;
    Ok(call.runtime.com().interface(parent)?.object_id)
}
fn dispatch_back_buffer(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let slot = call.frame().vtable_slot;
    if slot == 10 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    if !matches!(slot, 8 | 9) {
        return call.fault("D3D8 back buffer surface method is not implemented");
    }
    let device = match back_buffer_device(&call)
        .and_then(|id| call.runtime.graphics_device_by_com_object(id))
    {
        Ok(d) => d,
        Err(e) => return call.runtime_fault(e),
    };
    if slot == 9 {
        if device
            .d3d8
            .as_ref()
            .is_some_and(|state| state.parameters[10] & D3DPRESENTFLAG_LOCKABLE_BACKBUFFER != 0)
        {
            return call
                .fault("locking a D3D8 render target requires browser framebuffer readback");
        }
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    let p = device.presentation;
    let output = call.frame().explicit_arguments[0];
    write_output(
        call,
        output,
        &[22, 1, 1, 0, p.width * p.height * 4, 0, p.width, p.height]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>(),
    )
}

fn copy_rects(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let args = call.frame().explicit_arguments.clone();
    let resolve = |pointer| {
        call.runtime
            .com_interface_for_pointer(GuestAddress(pointer))
    };
    let source = match resolve(args[0]) {
        Ok(Some(i)) if i.iid == SURFACE8_IID => i.object_id,
        Ok(_) => return call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(e) => return call.runtime_fault(e),
    };
    let destination = match resolve(args[3]) {
        Ok(Some(i)) if i.iid == SURFACE8_IID => i.object_id,
        Ok(_) => return call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(e) => return call.runtime_fault(e),
    };
    let device = call.frame().object_id;
    let expected = match call.runtime.d3d8_state_mut(device) {
        Ok(s) => s.back_buffer_surface,
        Err(e) => return call.runtime_fault(e),
    };
    if expected != Some(destination) {
        return call.fault("CopyRects destination is not an owned back buffer");
    }
    let count = args[2] as usize;
    if count > 64 || (count != 0 && args[1] == 0) || (count == 0 && (args[1] != 0 || args[4] != 0))
    {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    let mut rectangles = Vec::new();
    if count != 0 {
        let rects = match call.runtime.memory.read(GuestAddress(args[1]), count * 16) {
            Ok(b) => b,
            Err(e) => return call.memory_fault(e),
        };
        let points = if args[4] == 0 {
            vec![0; count * 8]
        } else {
            match call.runtime.memory.read(GuestAddress(args[4]), count * 8) {
                Ok(b) => b,
                Err(e) => return call.memory_fault(e),
            }
        };
        for (rect, point) in rects.chunks_exact(16).zip(points.chunks_exact(8)) {
            let w: Vec<_> = rect
                .chunks_exact(4)
                .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
                .collect();
            rectangles.push((
                crate::runtime::RuntimeRect {
                    left: w[0],
                    top: w[1],
                    right: w[2],
                    bottom: w[3],
                },
                i32::from_le_bytes(point[..4].try_into().unwrap()),
                i32::from_le_bytes(point[4..].try_into().unwrap()),
            ));
        }
    }
    match call.runtime.copy_d3d8_surface(device, source, &rectangles) {
        Ok(true) => call.complete(0),
        Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(e) => call.runtime_fault(e),
    }
}

fn d3d8_format_to_runtime(format: u32) -> Option<crate::runtime::RuntimeGraphicsColorFormat> {
    use crate::runtime::RuntimeGraphicsColorFormat;
    match format {
        21 => Some(RuntimeGraphicsColorFormat::B8g8r8a8Unorm),
        22 => Some(RuntimeGraphicsColorFormat::B8g8r8x8Unorm),
        23 => Some(RuntimeGraphicsColorFormat::R5g6b5Unorm),
        24 => Some(RuntimeGraphicsColorFormat::X1r5g5b5Unorm),
        25 => Some(RuntimeGraphicsColorFormat::A1r5g5b5Unorm),
        26 => Some(RuntimeGraphicsColorFormat::A4r4g4b4Unorm),
        _ => None,
    }
}

fn runtime_format_to_d3d8(format: crate::runtime::RuntimeGraphicsColorFormat) -> u32 {
    use crate::runtime::RuntimeGraphicsColorFormat;
    match format {
        RuntimeGraphicsColorFormat::B8g8r8a8Unorm => 21,
        RuntimeGraphicsColorFormat::B8g8r8x8Unorm => 22,
        RuntimeGraphicsColorFormat::R5g6b5Unorm => 23,
        RuntimeGraphicsColorFormat::X1r5g5b5Unorm => 24,
        RuntimeGraphicsColorFormat::A1r5g5b5Unorm => 25,
        RuntimeGraphicsColorFormat::A4r4g4b4Unorm => 26,
    }
}

fn create_image_surface(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    use crate::runtime::{
        RuntimeGraphicsTextureDescriptor, RuntimeGraphicsTexturePool, RuntimeGraphicsTextureUsage,
    };
    let args = call.frame().explicit_arguments.clone();
    let (width, height, format, output) = (args[0], args[1], args[2], args[3]);
    if width == 0 || height == 0 || output == 0 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    let Some(color_format) =
        d3d8_format_to_runtime(format).filter(|_| width <= 4096 && height <= 4096)
    else {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    };
    if let Err(e) =
        call.runtime
            .memory
            .validate_access(GuestAddress(output), 4, MemoryAccess::Write)
    {
        return call.memory_fault(e);
    }
    // Standalone system-memory surfaces share the texel allocation owner, with
    // their own D3D8 COM identity and ABI. They are never exposed as textures.
    let creation = match call.runtime.validate_graphics_texture_creation(
        call.frame().object_id,
        RuntimeGraphicsTextureDescriptor {
            width,
            height,
            levels: 1,
            usage: RuntimeGraphicsTextureUsage::None,
            pool: RuntimeGraphicsTexturePool::SystemMemory,
            color_format,
        },
    ) {
        Ok(c) => c,
        Err(e) => return call.runtime_fault(e),
    };
    let parent = call.frame().interface_id;
    let object = match call.runtime.install_com_object_with_guest_writes(
        Some(parent),
        surface_spec(),
        |object| {
            let pointer = object
                .interfaces
                .iter()
                .find(|(iid, _)| *iid == SURFACE8_IID)
                .ok_or_else(|| RuntimeStateError::new("D3D8 surface interface missing"))?
                .1;
            Ok(vec![(
                GuestAddress(output),
                pointer.0.to_le_bytes().to_vec(),
            )])
        },
    ) {
        Ok(o) => o,
        Err(e) => return call.runtime_fault(e),
    };
    call.runtime
        .insert_graphics_texture(creation, object.object_id);
    call.complete(COM_S_OK.bits())
}

fn dispatch_surface(mut call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    match call.frame().vtable_slot {
        8 => {
            let output = call.frame().explicit_arguments[0];
            let texture = match call
                .runtime
                .graphics_texture_for_com_object(call.frame().object_id)
            {
                Ok(t) => t,
                Err(e) => return call.runtime_fault(e),
            };
            let d = texture.descriptor;
            write_output(
                call,
                output,
                &[
                    runtime_format_to_d3d8(d.color_format),
                    1,
                    0,
                    2,
                    d.width * d.height * d.color_format.bytes_per_pixel(),
                    0,
                    d.width,
                    d.height,
                ]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
            )
        }
        9 => {
            call.frame.explicit_arguments.insert(0, 0);
            crate::d3d9::dispatch_texture_lock(call)
        }
        10 => {
            call.frame.explicit_arguments.push(0);
            crate::d3d9::dispatch_texture_unlock(call)
        }
        _ => call.fault("D3D8 surface method is not implemented"),
    }
}

pub(crate) fn vertex_buffer_spec() -> ComObjectSpec {
    interface_spec(
        RuntimeComClass::D3d8VertexBuffer,
        VERTEX_BUFFER8_IID,
        "IDirect3DVertexBuffer8",
        &[
            ("GetDevice", None),
            ("SetPrivateData", None),
            ("GetPrivateData", None),
            ("FreePrivateData", None),
            ("SetPriority", None),
            ("GetPriority", None),
            ("PreLoad", None),
            ("GetType", Some(0)),
            ("Lock", Some(4)),
            ("Unlock", Some(0)),
            ("GetDesc", None),
        ],
    )
}
fn create_vertex_buffer(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    use crate::runtime::{
        RuntimeGraphicsVertexBufferDescriptor, RuntimeGraphicsVertexBufferPool,
        RuntimeGraphicsVertexBufferUsage,
    };
    let a = call.frame().explicit_arguments.clone();
    if a[0] == 0 || a[4] == 0 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    if a[0] > 1024 * 1024
        || !matches!(a[1], 0 | 8)
        || !matches!(a[2], 0x044 | 0x102 | 0x142 | 0x104 | 0x144)
        || a[3] != 1
    {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    }
    match call.runtime.install_graphics_vertex_buffer_object(
        call.frame().interface_id,
        call.frame().object_id,
        RuntimeGraphicsVertexBufferDescriptor {
            byte_length: a[0],
            usage: if a[1] == 0 {
                RuntimeGraphicsVertexBufferUsage::None
            } else {
                RuntimeGraphicsVertexBufferUsage::WriteOnly
            },
            flexible_vertex_format: a[2],
            pool: RuntimeGraphicsVertexBufferPool::Managed,
        },
        GuestAddress(a[4]),
    ) {
        Ok(()) => call.complete(0),
        Err(e) => call.runtime_fault(e),
    }
}
fn dispatch_vertex_buffer(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let a = call.frame().explicit_arguments.clone();
    match call.frame().vtable_slot {
        10 => call.complete(6),
        11 => match call.runtime.lock_graphics_vertex_buffer(
            call.frame().object_id,
            a[0],
            a[1],
            a[3],
            GuestAddress(a[2]),
        ) {
            Ok(true) => call.complete(0),
            Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
            Err(e) => call.runtime_fault(e),
        },
        12 => crate::d3d9::vertex_buffer_unlock(call),
        _ => call.fault("D3D8 vertex buffer method is not implemented"),
    }
}

fn create_texture(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    use crate::runtime::{
        RuntimeGraphicsColorFormat, RuntimeGraphicsTextureDescriptor, RuntimeGraphicsTexturePool,
        RuntimeGraphicsTextureUsage,
    };
    let a = call.frame().explicit_arguments.clone();
    if a[0] == 0 || a[1] == 0 || a[6] == 0 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    if a[0] > 4096
        || a[1] > 4096
        || a[2] != 1
        || a[3] != 0
        || !matches!(a[4], 21 | 22)
        || !matches!(a[5], 1 | 2)
    {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    }
    let creation = match call.runtime.validate_graphics_texture_creation(
        call.frame().object_id,
        RuntimeGraphicsTextureDescriptor {
            width: a[0],
            height: a[1],
            levels: 1,
            usage: RuntimeGraphicsTextureUsage::None,
            color_format: if a[4] == 21 {
                RuntimeGraphicsColorFormat::B8g8r8a8Unorm
            } else {
                RuntimeGraphicsColorFormat::B8g8r8x8Unorm
            },
            pool: if a[5] == 1 {
                RuntimeGraphicsTexturePool::Managed
            } else {
                RuntimeGraphicsTexturePool::SystemMemory
            },
        },
    ) {
        Ok(c) => c,
        Err(e) => return call.runtime_fault(e),
    };
    let spec = interface_spec(
        RuntimeComClass::D3d8Texture,
        TEXTURE8_IID,
        "IDirect3DTexture8",
        &[
            ("GetDevice", None),
            ("SetPrivateData", None),
            ("GetPrivateData", None),
            ("FreePrivateData", None),
            ("SetPriority", None),
            ("GetPriority", None),
            ("PreLoad", None),
            ("GetType", Some(0)),
            ("SetLOD", None),
            ("GetLOD", Some(0)),
            ("GetLevelCount", Some(0)),
            ("GetLevelDesc", Some(2)),
            ("GetSurfaceLevel", Some(2)),
            ("LockRect", Some(4)),
            ("UnlockRect", Some(1)),
            ("AddDirtyRect", None),
        ],
    );
    let parent = call.frame().interface_id;
    let installed =
        match call
            .runtime
            .install_com_object_with_guest_writes(Some(parent), spec, |object| {
                let pointer = object
                    .interfaces
                    .iter()
                    .find(|(i, _)| *i == TEXTURE8_IID)
                    .ok_or_else(|| RuntimeStateError::new("texture interface missing"))?
                    .1;
                Ok(vec![(GuestAddress(a[6]), pointer.0.to_le_bytes().to_vec())])
            }) {
            Ok(o) => o,
            Err(e) => return call.runtime_fault(e),
        };
    call.runtime
        .insert_graphics_texture(creation, installed.object_id);
    call.complete(0)
}

fn texture_desc(call: ComFrontendCall<'_>, output: u32) -> ComDispatchOutcome {
    let d = match call
        .runtime
        .graphics_texture_for_com_object(call.frame().object_id)
    {
        Ok(t) => t.descriptor,
        Err(e) => return call.runtime_fault(e),
    };
    let pool = match d.pool {
        crate::runtime::RuntimeGraphicsTexturePool::Default => 0,
        crate::runtime::RuntimeGraphicsTexturePool::Managed => 1,
        crate::runtime::RuntimeGraphicsTexturePool::SystemMemory => 2,
    };
    write_output(
        call,
        output,
        &[
            runtime_format_to_d3d8(d.color_format),
            1,
            0,
            pool,
            d.width * d.height * d.color_format.bytes_per_pixel(),
            0,
            d.width,
            d.height,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>(),
    )
}
fn dispatch_texture(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let args = call.frame().explicit_arguments.clone();
    match call.frame().vtable_slot {
        10 => call.complete(3),
        12 => call.complete(0),
        13 => call.complete(1),
        14 if args[0] == 0 => texture_desc(call, args[1]),
        14 => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        15 => texture_surface(call),
        16 => crate::d3d9::dispatch_texture_lock(call),
        17 => crate::d3d9::dispatch_texture_unlock(call),
        _ => call.fault("D3D8 texture method is not implemented"),
    }
}
fn texture_surface(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let args = call.frame().explicit_arguments.clone();
    if args[0] != 0 || args[1] == 0 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    let existing = match call
        .runtime
        .graphics_surface_for_texture_level(call.frame().object_id, 0)
    {
        Ok(s) => s.map(|s| s.com_object_id),
        Err(e) => return call.runtime_fault(e),
    };
    if let Some(id) = existing {
        let interface = match call.runtime.com_interface_for_object_iid(id, SURFACE8_IID) {
            Ok(i) => i.id,
            Err(e) => return call.runtime_fault(e),
        };
        return match call
            .runtime
            .add_ref_interface_with_guest_pointer_write(interface, GuestAddress(args[1]))
        {
            Ok(_) => call.complete(0),
            Err(e) => call.runtime_fault(e),
        };
    }
    let creation = match call
        .runtime
        .validate_graphics_surface_creation(call.frame().object_id, 0)
    {
        Ok(Some(c)) => c,
        Ok(None) => return call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(e) => return call.runtime_fault(e),
    };
    let parent = call.frame().interface_id;
    let object = match call.runtime.install_com_object_with_guest_writes(
        Some(parent),
        surface_spec_for(RuntimeComClass::D3d8TextureSurface),
        |object| {
            let pointer = object
                .interfaces
                .iter()
                .find(|(i, _)| *i == SURFACE8_IID)
                .ok_or_else(|| RuntimeStateError::new("texture surface interface missing"))?
                .1;
            Ok(vec![(
                GuestAddress(args[1]),
                pointer.0.to_le_bytes().to_vec(),
            )])
        },
    ) {
        Ok(o) => o,
        Err(e) => return call.runtime_fault(e),
    };
    call.runtime
        .insert_graphics_surface(creation, object.object_id);
    call.complete(0)
}
fn dispatch_texture_surface(mut call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let parent = match call
        .runtime
        .com()
        .object(call.frame().object_id)
        .and_then(|o| {
            o.parent_interface_id
                .ok_or_else(|| RuntimeStateError::new("texture surface has no parent"))
        }) {
        Ok(p) => p,
        Err(e) => return call.runtime_fault(e),
    };
    let texture = match call.runtime.com().interface(parent) {
        Ok(i) => i.object_id,
        Err(e) => return call.runtime_fault(e),
    };
    call.frame.object_id = texture;
    match call.frame().vtable_slot {
        8 => {
            let output = call.frame().explicit_arguments[0];
            texture_desc(call, output)
        }
        9 => {
            call.frame.explicit_arguments.insert(0, 0);
            crate::d3d9::dispatch_texture_lock(call)
        }
        10 => {
            call.frame.explicit_arguments.push(0);
            crate::d3d9::dispatch_texture_unlock(call)
        }
        _ => call.fault("D3D8 texture surface method is not implemented"),
    }
}
fn set_texture(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let args = call.frame().explicit_arguments.clone();
    if args[0] != 0 {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    }
    let texture = if args[1] == 0 {
        None
    } else {
        let id = match call
            .runtime
            .com_interface_for_pointer(GuestAddress(args[1]))
        {
            Ok(Some(i)) if i.iid == TEXTURE8_IID => i.object_id,
            Ok(_) => return call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
            Err(e) => return call.runtime_fault(e),
        };
        match call.runtime.graphics_texture_for_com_object(id) {
            Ok(t)
                if matches!(
                    t.descriptor.pool,
                    crate::runtime::RuntimeGraphicsTexturePool::Default
                        | crate::runtime::RuntimeGraphicsTexturePool::Managed
                ) => {}
            Ok(_) => return call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
            Err(e) => return call.runtime_fault(e),
        }
        Some(id)
    };
    match call
        .runtime
        .bind_graphics_texture(call.frame().object_id, 0, texture)
    {
        Ok(true) => call.complete(0),
        Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
        Err(e) => call.runtime_fault(e),
    }
}

fn device_spec() -> ComObjectSpec {
    interface_spec(
        RuntimeComClass::D3d8Device,
        DEVICE8_IID,
        "IDirect3DDevice8",
        DEVICE_METHODS,
    )
}

fn transform(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let args = call.frame().explicit_arguments.clone();
    let slot = call.frame().vtable_slot;
    let (kind, pointer) = (args[0], args[1]);
    if !matches!(kind,2|3|16..=23|256..=511) || pointer == 0 {
        return call.complete(D3D9_D3DERR_INVALIDCALL.bits());
    }
    let mut identity = [0u32; 16];
    for index in [0, 5, 10, 15] {
        identity[index] = 1.0f32.to_bits();
    }
    if slot == 38 {
        let state = match call.runtime.d3d8_state_mut(call.frame().object_id) {
            Ok(state) => state,
            Err(e) => return call.runtime_fault(e),
        };
        let matrix = state.transforms.get(&kind).copied().unwrap_or(identity);
        return write_output(
            call,
            pointer,
            &matrix
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
    }
    let bytes = match call.runtime.memory.read(GuestAddress(pointer), 64) {
        Ok(bytes) => bytes,
        Err(e) => return call.memory_fault(e),
    };
    let mut matrix: [u32; 16] =
        std::array::from_fn(|i| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap()));
    let state = match call.runtime.d3d8_state_mut(call.frame().object_id) {
        Ok(state) => state,
        Err(e) => return call.runtime_fault(e),
    };
    if slot == 39 {
        let current = state.transforms.get(&kind).copied().unwrap_or(identity);
        matrix = std::array::from_fn(|i| {
            (0..4)
                .map(|k| {
                    f32::from_bits(matrix[i / 4 * 4 + k]) * f32::from_bits(current[k * 4 + i % 4])
                })
                .sum::<f32>()
                .to_bits()
        });
    }
    state.transforms.insert(kind, matrix);
    call.complete(COM_S_OK.bits())
}

fn set_vertex_shader_d3d8(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let [handle] = call.frame().explicit_arguments.as_slice() else {
        return call.fault("SetVertexShader requires one handle word");
    };
    let Some(format) = crate::runtime::RuntimeGraphicsFlexibleVertexFormat::from_d3d8_code(*handle)
    else {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    };
    match call
        .runtime
        .update_graphics_flexible_vertex_format(call.frame().object_id, format)
    {
        Ok(()) => call.complete(COM_S_OK.bits()),
        Err(e) => call.runtime_fault(e),
    }
}

fn draw_d3d8(call: ComFrontendCall<'_>, is_up: bool) -> ComDispatchOutcome {
    let device = call.frame().object_id;
    let state = match call.runtime.d3d8_state_mut(device) {
        Ok(s) => s,
        Err(e) => return call.runtime_fault(e),
    };
    if state
        .stage_values
        .get(&(0, 24))
        .is_some_and(|v| !matches!(*v, 0 | 2))
    {
        return call.fault("D3D8 draw requires unimplemented texture transform mode");
    }
    if is_up {
        crate::d3d9::dispatch_draw_primitive_up(call)
    } else {
        let [primitive_type, start_vertex, primitive_count] =
            call.frame().explicit_arguments.as_slice()
        else {
            return call
                .fault("DrawPrimitive requires topology, start vertex, and primitive count");
        };
        if *primitive_type != 5 || *primitive_count != 2 {
            return call.fault("D3D8 DrawPrimitive topology or primitive count is not implemented");
        }
        match call
            .runtime
            .submit_graphics_draw(device, *start_vertex, *primitive_count)
        {
            Ok(true) => call.complete(COM_S_OK.bits()),
            Ok(false) => call.complete(D3D9_D3DERR_INVALIDCALL.bits()),
            Err(e) => call.runtime_fault(e),
        }
    }
}

fn render_state(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let args = call.frame().explicit_arguments.clone();
    let (state, value) = (args[0], args[1]);
    let object = call.frame().object_id;
    if (state, value) == (20, 2) {
        if let Err(e) = call.runtime.update_graphics_render_state(
            object,
            crate::runtime::RuntimeGraphicsRenderStateUpdate::DestinationBlendFactor(
                crate::runtime::RuntimeGraphicsBlendFactor::One,
            ),
        ) {
            return call.runtime_fault(e);
        }
        match call.runtime.d3d8_state_mut(object) {
            Ok(s) => {
                s.render_values.insert(state, value);
            }
            Err(e) => return call.runtime_fault(e),
        }
        return call.complete(COM_S_OK.bits());
    }
    if matches!(
        (state, value),
        (7, 0..=1)
            | (14 | 15 | 28, 0..=1)
            | (23 | 25, 1..=8)
            | (9, 1..=2)
            | (24, 0..=255)
            | (34 | 36 | 37 | 38 | 60, _)
            | (35, 0..=3)
    ) {
        if state == 7
            && let Err(e) = call.runtime.update_graphics_render_state(
                object,
                crate::runtime::RuntimeGraphicsRenderStateUpdate::DepthTestEnabled(value != 0),
            )
        {
            return call.runtime_fault(e);
        }
        match call.runtime.d3d8_state_mut(object) {
            Ok(s) => {
                s.render_values.insert(state, value);
            }
            Err(e) => return call.runtime_fault(e),
        }
        return call.complete(COM_S_OK.bits());
    }
    let outcome = crate::d3d9::set_render_state(ComFrontendCall {
        runtime: &mut *call.runtime,
        trap: call.trap,
        target: call.target.clone(),
        frame: call.frame.clone(),
    });
    if matches!(&outcome,ComDispatchOutcome::Completed(done) if done.eax==0) {
        match call.runtime.d3d8_state_mut(object) {
            Ok(s) => {
                s.render_values.insert(state, value);
            }
            Err(e) => return call.runtime_fault(e),
        }
    }
    outcome
}

fn texture_stage_state(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    use crate::runtime::{
        RuntimeGraphicsSamplerStateUpdate as S, RuntimeGraphicsTextureAddress as A,
        RuntimeGraphicsTextureArgument as G, RuntimeGraphicsTextureFilter as F,
        RuntimeGraphicsTextureOperation as O, RuntimeGraphicsTextureStageStateUpdate as T,
    };
    let args = call.frame().explicit_arguments.clone();
    let (stage, kind, value) = (args[0], args[1], args[2]);
    if stage != 0 {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    }
    if call.frame().vtable_slot == 62 {
        let state = match call.runtime.d3d8_state_mut(call.frame().object_id) {
            Ok(s) => s,
            Err(e) => return call.runtime_fault(e),
        };
        let Some(value) = state.stage_values.get(&(stage, kind)).copied() else {
            return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
        };
        return write_output(call, args[2], &value.to_le_bytes());
    }
    let operation = match value {
        1 => Some(O::Disable),
        2 => Some(O::SelectArgumentOne),
        4 => Some(O::Modulate),
        7 => Some(O::Add),
        _ => None,
    };
    let argument = match value {
        0 => Some(G::Diffuse),
        2 => Some(G::Texture),
        3 => Some(G::TFactor),
        _ => None,
    };
    let update = match (kind, operation, argument) {
        (1, Some(operation), _) => Some(T::ColorOperation { stage, operation }),
        (4, Some(operation), _) => Some(T::AlphaOperation { stage, operation }),
        (2, _, Some(argument)) => Some(T::ColorArgumentOne { stage, argument }),
        (3, _, Some(argument)) => Some(T::ColorArgumentTwo { stage, argument }),
        (5, _, Some(argument)) => Some(T::AlphaArgumentOne { stage, argument }),
        (6, _, Some(argument)) => Some(T::AlphaArgumentTwo { stage, argument }),
        _ => None,
    };
    let sampler = match (kind, value) {
        (13, 1 | 3) => Some(S::AddressU {
            sampler: stage,
            address: if value == 1 { A::Wrap } else { A::Clamp },
        }),
        (14, 1 | 3) => Some(S::AddressV {
            sampler: stage,
            address: if value == 1 { A::Wrap } else { A::Clamp },
        }),
        (16, 1 | 2) => Some(S::MagFilter {
            sampler: stage,
            filter: if value == 1 { F::Point } else { F::Linear },
        }),
        (17, 1 | 2) => Some(S::MinFilter {
            sampler: stage,
            filter: if value == 1 { F::Point } else { F::Linear },
        }),
        _ => None,
    };
    // State acquisition is independent of drawing. Deferred effects are retained
    // and explicitly checked by the draw entry point until their lowering exists.
    if update.is_none()
        && sampler.is_none()
        && !matches!(
            (kind, value),
            (2 | 3 | 5 | 6, 3) | (18, 0) | (24, 0 | 2) | (25, 1 | 3)
        )
    {
        return call.complete(D3D9_D3DERR_NOTAVAILABLE.bits());
    }
    let object = call.frame().object_id;
    if let Some(update) = update
        && let Err(e) = call
            .runtime
            .update_graphics_texture_stage_state(object, update)
    {
        return call.runtime_fault(e);
    }
    if let Some(update) = sampler
        && let Err(e) = call.runtime.update_graphics_sampler_state(object, update)
    {
        return call.runtime_fault(e);
    }
    match call.runtime.d3d8_state_mut(object) {
        Ok(state) => {
            state.stage_values.insert((stage, kind), value);
        }
        Err(e) => return call.runtime_fault(e),
    }
    call.complete(COM_S_OK.bits())
}

const DEVICE_METHODS: &[(&str, Option<u16>)] = &[
    ("TestCooperativeLevel", Some(0)),
    ("GetAvailableTextureMem", Some(0)),
    ("ResourceManagerDiscardBytes", Some(1)),
    ("GetDirect3D", Some(1)),
    ("GetDeviceCaps", Some(1)),
    ("GetDisplayMode", Some(1)),
    ("GetCreationParameters", None),
    ("SetCursorProperties", None),
    ("SetCursorPosition", None),
    ("ShowCursor", None),
    ("CreateAdditionalSwapChain", None),
    ("Reset", None),
    ("Present", Some(4)),
    ("GetBackBuffer", Some(3)),
    ("GetRasterStatus", None),
    ("SetGammaRamp", None),
    ("GetGammaRamp", None),
    ("CreateTexture", Some(7)),
    ("CreateVolumeTexture", None),
    ("CreateCubeTexture", None),
    ("CreateVertexBuffer", Some(5)),
    ("CreateIndexBuffer", None),
    ("CreateRenderTarget", Some(6)),
    ("CreateDepthStencilSurface", None),
    ("CreateImageSurface", Some(4)),
    ("CopyRects", Some(5)),
    ("UpdateTexture", None),
    ("GetFrontBuffer", None),
    ("SetRenderTarget", None),
    ("GetRenderTarget", None),
    ("GetDepthStencilSurface", None),
    ("BeginScene", Some(0)),
    ("EndScene", Some(0)),
    ("Clear", Some(6)),
    ("SetTransform", Some(2)),
    ("GetTransform", Some(2)),
    ("MultiplyTransform", Some(2)),
    ("SetViewport", Some(1)),
    ("GetViewport", Some(1)),
    ("SetMaterial", None),
    ("GetMaterial", None),
    ("SetLight", None),
    ("GetLight", None),
    ("LightEnable", None),
    ("GetLightEnable", None),
    ("SetClipPlane", None),
    ("GetClipPlane", None),
    ("SetRenderState", Some(2)),
    ("GetRenderState", Some(2)),
    ("BeginStateBlock", None),
    ("EndStateBlock", None),
    ("ApplyStateBlock", None),
    ("CaptureStateBlock", None),
    ("DeleteStateBlock", None),
    ("CreateStateBlock", None),
    ("SetClipStatus", None),
    ("GetClipStatus", None),
    ("GetTexture", None),
    ("SetTexture", Some(2)),
    ("GetTextureStageState", Some(3)),
    ("SetTextureStageState", Some(3)),
    ("ValidateDevice", None),
    ("GetInfo", None),
    ("SetPaletteEntries", None),
    ("GetPaletteEntries", None),
    ("SetCurrentTexturePalette", None),
    ("GetCurrentTexturePalette", None),
    ("DrawPrimitive", Some(3)),
    ("DrawIndexedPrimitive", None),
    ("DrawPrimitiveUP", Some(4)),
    ("DrawIndexedPrimitiveUP", None),
    ("ProcessVertices", None),
    ("CreateVertexShader", None),
    ("SetVertexShader", Some(1)),
    ("GetVertexShader", None),
    ("DeleteVertexShader", None),
    ("SetVertexShaderConstant", None),
    ("GetVertexShaderConstant", None),
    ("GetVertexShaderDeclaration", None),
    ("GetVertexShaderFunction", None),
    ("SetStreamSource", Some(3)),
    ("GetStreamSource", None),
    ("SetIndices", None),
    ("GetIndices", None),
    ("CreatePixelShader", None),
    ("SetPixelShader", None),
    ("GetPixelShader", None),
    ("DeletePixelShader", None),
    ("SetPixelShaderConstant", None),
    ("GetPixelShaderConstant", None),
    ("GetPixelShaderFunction", None),
    ("DrawRectPatch", None),
    ("DrawTriPatch", None),
    ("DeletePatch", None),
];
