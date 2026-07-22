use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use vine_core::{
    ApiEndpointIdentity, CALLBACK_RETURN_ARENA_BASE, DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH,
    DLL_PROCESS_STATIC_RESERVED, ExecutionSession, GuestAddress, GuestMemory, ImageMaterialization,
    LOGICAL_ENCODED_POINTER_NAMESPACE, LOGICAL_LOADED_MODULE_NAMESPACE, LoaderIngressReport,
    MemoryLimits, ModuleSnapshot, Pe32Limits, ProcessAddressSpaceLayout, RuntimeBootstrap,
    RuntimeCallbackFrameId, RuntimeClockState, RuntimeDllCallbackReason, RuntimeDllCallbackRequest,
    RuntimeDllLifecycleAction, RuntimeDllLifecyclePhase, RuntimeExport, RuntimeExportCatalog,
    RuntimeGuestImportResolution, RuntimeModule, RuntimeModuleId, RuntimeModuleLoadState,
    RuntimeProcessId, RuntimeStartupInfo, RuntimeState, RuntimeThreadId, SchedulerThreadState,
    X86_STARTUP_INFO_W_BYTE_LENGTH, build_pe32_load_plan, materialize_pe32_image,
};

const CODEC_HANDLE: u32 = 0x4d01_0001;
const ENGINE_HANDLE: u32 = 0x4d01_0002;
const KERNEL32_HANDLE: u32 = 0x4d01_0003;
const MAIN_HANDLE: u32 = 0x4d01_0004;

const CODEC_BASE: u32 = 0x3000_0000;
const ENGINE_BASE: u32 = 0x3100_0000;
const MAIN_BASE: u32 = 0x3200_0000;

const CODEC_SHA256: &str = "6e872f31dddb4932f06e0908c0cbe536a552dca24f338d174fa7e538a21c3d55";
const ENGINE_SHA256: &str = "7154f24bec58d4e080de8bb98b512f53b425a54dcde273679c193cb1daaaf90b";
const MAIN_SHA256: &str = "df9553bc5c10dae44d0b6f25f0a96c2a6fcda903895b7ffecebade641bff980a";

#[test]
fn guest_import_graph_drives_atomic_dll_process_lifecycle() {
    let mut memory = fixture_memory();
    let (codec_loader, codec_image) =
        materialize_fixture(&mut memory, "lifecycle_codec.dll", CODEC_SHA256, CODEC_BASE);
    let (engine_loader, engine_image) = materialize_fixture(
        &mut memory,
        "lifecycle_engine.dll",
        ENGINE_SHA256,
        ENGINE_BASE,
    );
    let (main_loader, main_image) =
        materialize_fixture(&mut memory, "dll-lifecycle.exe", MAIN_SHA256, MAIN_BASE);
    assert_eq!(codec_loader.load_plan.imports.len(), 0);
    assert_eq!(engine_loader.load_plan.imports.len(), 1);
    assert_eq!(main_loader.load_plan.imports.len(), 2);

    let catalog = RuntimeExportCatalog::new(vec![RuntimeExport::named(
        "kernel32.dll",
        "GetCurrentProcessId",
    )])
    .unwrap();
    let mut runtime = controlled_runtime();
    load_module(&mut runtime, CODEC_HANDLE, "lifecycle_codec.dll", false);
    load_module(&mut runtime, ENGINE_HANDLE, "lifecycle_engine.dll", false);
    load_module(&mut runtime, KERNEL32_HANDLE, "kernel32.dll", true);
    runtime
        .register_main_module(runtime_module(MAIN_HANDLE, "dll-lifecycle.exe", false))
        .unwrap();

    let graph_before_missing = runtime.module_graph().clone();
    let bindings_before_missing = runtime.module_import_bindings().clone();
    let memory_before_missing = memory.stats();
    let engine_iat_before = iat_values(&memory, &engine_image, &engine_loader, true);
    let missing = ExecutionSession::new(&mut memory, &mut runtime)
        .bind_guest_module_imports(ENGINE_HANDLE, &engine_loader, &engine_image, &catalog)
        .expect_err("engine cannot bind before the codec image is registered");
    assert_eq!(missing.code, "module-image-name-unknown");
    assert_eq!(runtime.module_graph(), &graph_before_missing);
    assert_eq!(runtime.module_import_bindings(), &bindings_before_missing);
    assert_eq!(runtime.runtime_system_exports(), None);
    assert_eq!(memory.stats(), memory_before_missing);
    assert_eq!(
        iat_values(&memory, &engine_image, &engine_loader, true),
        engine_iat_before
    );

    let codec_binding = ExecutionSession::new(&mut memory, &mut runtime)
        .bind_guest_module_imports(CODEC_HANDLE, &codec_loader, &codec_image, &catalog)
        .unwrap();
    assert!(codec_binding.imports.is_empty());
    ExecutionSession::new(&mut memory, &mut runtime)
        .register_module_image(CODEC_HANDLE, &codec_loader, &codec_image)
        .unwrap();

    let engine_binding = ExecutionSession::new(&mut memory, &mut runtime)
        .bind_guest_module_imports(ENGINE_HANDLE, &engine_loader, &engine_image, &catalog)
        .unwrap();
    assert_eq!(engine_binding.imports.len(), 2);
    assert_eq!(
        engine_binding
            .imports
            .iter()
            .map(|import| import.resolved_address())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            GuestAddress(CODEC_BASE + 0x1000),
            GuestAddress(CODEC_BASE + 0x1010),
        ])
    );
    ExecutionSession::new(&mut memory, &mut runtime)
        .register_module_image(ENGINE_HANDLE, &engine_loader, &engine_image)
        .unwrap();

    let main_binding = ExecutionSession::new(&mut memory, &mut runtime)
        .bind_guest_module_imports(MAIN_HANDLE, &main_loader, &main_image, &catalog)
        .unwrap();
    assert_eq!(main_binding.imports.len(), 2);
    let engine_import = main_binding
        .imports
        .iter()
        .find(|import| import.requested_module == "lifecycle_engine.dll")
        .unwrap();
    assert_eq!(
        engine_import.resolved_address(),
        GuestAddress(ENGINE_BASE + 0x1000)
    );
    assert!(matches!(
        &engine_import.resolution,
        RuntimeGuestImportResolution::GuestExport { .. }
    ));
    let kernel_import = main_binding
        .imports
        .iter()
        .find(|import| import.requested_module == "kernel32.dll")
        .unwrap();
    assert_eq!(kernel_import.resolved_address(), GuestAddress(0x7f00_0000));
    assert!(matches!(
        &kernel_import.resolution,
        RuntimeGuestImportResolution::RuntimeExport { endpoint }
            if endpoint.id.0 == 1
                && endpoint.address == GuestAddress(0x7f00_0000)
                && endpoint.export
                    == RuntimeExport::named("kernel32.dll", "GetCurrentProcessId")
    ));
    let kernel_endpoint =
        ApiEndpointIdentity::from_runtime_guest_import(kernel_import).expect("runtime endpoint");
    assert_eq!(kernel_endpoint.id.0, 1);
    assert_eq!(kernel_endpoint.address, GuestAddress(0x7f00_0000));
    assert_eq!(kernel_endpoint.iat_address, Some(kernel_import.iat_address));
    let runtime_exports = runtime.runtime_system_exports().unwrap();
    assert_eq!(runtime_exports.endpoints.len(), 1);
    assert_eq!(runtime_exports.arena.committed_pages, 1);
    assert_eq!(runtime_exports.arena.bound_slots, 1);
    assert_eq!(
        runtime_exports.arena.content_sha256,
        "4a1211e03e37794b9434ba7e8866fdda0d3a7dcffb53d0dc3a92a55cf1438059"
    );
    assert!(runtime_exports.arena.owner_sealed);
    let runtime_export_owner = runtime_exports.arena.owner;
    ExecutionSession::new(&mut memory, &mut runtime)
        .register_module_image(MAIN_HANDLE, &main_loader, &main_image)
        .unwrap();
    let reflection = memory.pending_reflection().unwrap();
    assert_eq!(reflection.pages.len(), 15);
    assert_eq!(reflection.invalidation.generation, 12);
    memory
        .acknowledge_invalidation(reflection.invalidation.generation)
        .unwrap();
    assert_eq!(
        memory.read(GuestAddress(0x7f00_0000), 4).unwrap(),
        [0x0f, 0x0b, 0xf4, 0xf4]
    );
    assert_eq!(reflection.pages.last().unwrap().owner, runtime_export_owner);

    for (binding, loader, image) in [
        (&engine_binding, &engine_loader, &engine_image),
        (&main_binding, &main_loader, &main_image),
    ] {
        assert_eq!(
            iat_values(&memory, image, loader, false),
            binding
                .imports
                .iter()
                .map(|import| import.resolved_address().0)
                .collect::<Vec<_>>()
        );
    }

    let codec_id = module_id(&runtime, CODEC_HANDLE);
    let engine_id = module_id(&runtime, ENGINE_HANDLE);
    let kernel32_id = module_id(&runtime, KERNEL32_HANDLE);
    let main_id = module_id(&runtime, MAIN_HANDLE);
    assert_eq!(
        runtime
            .module_graph()
            .module(engine_id)
            .unwrap()
            .dependencies,
        BTreeSet::from([codec_id])
    );
    assert_eq!(
        runtime.module_graph().module(main_id).unwrap().dependencies,
        BTreeSet::from([engine_id, kernel32_id])
    );

    for handle in [CODEC_HANDLE, ENGINE_HANDLE, KERNEL32_HANDLE, MAIN_HANDLE] {
        runtime.mark_module_ready(handle).unwrap();
    }
    runtime.start_initial_thread().unwrap();

    let graph_before_invalid_sentinel = runtime.module_graph().clone();
    let runtime_before_invalid_sentinel = serde_json::to_value(&runtime).unwrap();
    assert_eq!(
        runtime
            .begin_dll_process_attach(GuestAddress(CALLBACK_RETURN_ARENA_BASE.0 - 1))
            .expect_err("unowned callback sentinel must fail before lifecycle mutation")
            .code,
        "dll-return-sentinel-unowned"
    );
    assert_eq!(runtime.module_graph(), &graph_before_invalid_sentinel);
    assert_eq!(
        serde_json::to_value(&runtime).unwrap(),
        runtime_before_invalid_sentinel
    );
    assert_eq!(
        runtime.dll_lifecycle_phase(),
        RuntimeDllLifecyclePhase::Idle
    );

    let codec_attach = expect_callback(
        runtime
            .begin_dll_process_attach(CALLBACK_RETURN_ARENA_BASE)
            .unwrap(),
    );
    assert_callback(
        &codec_attach,
        RuntimeCallbackFrameId(1),
        codec_id,
        CODEC_HANDLE,
        CODEC_BASE,
        CODEC_BASE + 0x1030,
        RuntimeDllCallbackReason::ProcessAttach,
    );
    assert_eq!(
        runtime
            .complete_dll_callback(RuntimeCallbackFrameId(999), 1)
            .expect_err("DLL callback completion must identify the strict top frame")
            .code,
        "dll-callback-frame-mismatch"
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().callback_frames,
        [codec_attach.frame]
    );
    let engine_attach = expect_callback(
        runtime
            .complete_dll_callback(codec_attach.frame.id, 1)
            .unwrap(),
    );
    assert_callback(
        &engine_attach,
        RuntimeCallbackFrameId(2),
        engine_id,
        ENGINE_HANDLE,
        ENGINE_BASE,
        ENGINE_BASE + 0x1020,
        RuntimeDllCallbackReason::ProcessAttach,
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().callback_frames,
        [engine_attach.frame],
        "DLL callback replacement commits without an observable empty stack"
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );
    assert_eq!(
        runtime
            .add_current_thread_module_reference(CODEC_HANDLE)
            .expect_err("graph mutation during DllMain is structured")
            .code,
        "module-graph-mutation-during-dll-callback-forbidden"
    );
    let codec_unwind = expect_callback(
        runtime
            .complete_dll_callback(engine_attach.frame.id, 0)
            .unwrap(),
    );
    assert_callback(
        &codec_unwind,
        RuntimeCallbackFrameId(3),
        codec_id,
        CODEC_HANDLE,
        CODEC_BASE,
        CODEC_BASE + 0x1030,
        RuntimeDllCallbackReason::ProcessDetach,
    );
    let failure = match runtime
        .complete_dll_callback(codec_unwind.frame.id, 1)
        .unwrap()
    {
        RuntimeDllLifecycleAction::AttachFailed { failure } => failure,
        action => panic!("expected attach failure, got {action:?}"),
    };
    assert_eq!(failure.failed_module_id, engine_id);
    assert_eq!(failure.failed_module, "lifecycle_engine.dll");
    assert_eq!(failure.unwound_modules, [codec_id]);
    assert!(failure.retryable);
    assert_eq!(
        runtime.dll_lifecycle_phase(),
        RuntimeDllLifecyclePhase::Idle
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert!(
        runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .is_empty()
    );
    for id in [codec_id, engine_id, kernel32_id, main_id] {
        assert_eq!(
            runtime.module_graph().module(id).unwrap().state,
            RuntimeModuleLoadState::Ready
        );
    }

    let codec_retry = expect_callback(
        runtime
            .begin_dll_process_attach(CALLBACK_RETURN_ARENA_BASE)
            .unwrap(),
    );
    assert_eq!(codec_retry.frame.id, RuntimeCallbackFrameId(4));
    let engine_retry = expect_callback(
        runtime
            .complete_dll_callback(codec_retry.frame.id, 1)
            .unwrap(),
    );
    assert_eq!(engine_retry.frame.id, RuntimeCallbackFrameId(5));
    let attached = expect_completion(
        runtime
            .complete_dll_callback(engine_retry.frame.id, 1)
            .unwrap(),
    );
    assert_eq!(attached.phase, RuntimeDllLifecyclePhase::ProcessAttached);
    assert_eq!(
        attached.graph_order,
        [kernel32_id, codec_id, engine_id, main_id]
    );
    assert_eq!(attached.callback_order, [codec_id, engine_id]);

    let engine_detach = expect_callback(
        runtime
            .begin_dll_process_detach(CALLBACK_RETURN_ARENA_BASE)
            .unwrap(),
    );
    assert_callback(
        &engine_detach,
        RuntimeCallbackFrameId(6),
        engine_id,
        ENGINE_HANDLE,
        ENGINE_BASE,
        ENGINE_BASE + 0x1020,
        RuntimeDllCallbackReason::ProcessDetach,
    );
    let codec_detach = expect_callback(
        runtime
            .complete_dll_callback(engine_detach.frame.id, 0)
            .unwrap(),
    );
    assert_callback(
        &codec_detach,
        RuntimeCallbackFrameId(7),
        codec_id,
        CODEC_HANDLE,
        CODEC_BASE,
        CODEC_BASE + 0x1030,
        RuntimeDllCallbackReason::ProcessDetach,
    );
    let detached = expect_completion(
        runtime
            .complete_dll_callback(codec_detach.frame.id, 0)
            .unwrap(),
    );
    assert_eq!(detached.phase, RuntimeDllLifecyclePhase::ProcessDetached);
    assert_eq!(
        detached.graph_order,
        [main_id, engine_id, codec_id, kernel32_id]
    );
    assert_eq!(detached.callback_order, [engine_id, codec_id]);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert!(
        runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .is_empty()
    );
}

fn fixture_memory() -> GuestMemory {
    GuestMemory::new(MemoryLimits {
        max_physical_pages: 20,
        max_regions: 8,
        max_owners: 8,
        ..MemoryLimits::default()
    })
    .unwrap()
}

fn materialize_fixture(
    memory: &mut GuestMemory,
    name: &str,
    sha256: &str,
    load_base: u32,
) -> (LoaderIngressReport, ImageMaterialization) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(".work/toolchain-spike")
        .join(name);
    let bytes = fs::read(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {} ({error}); run `npm run check:toolchain` first",
            path.display()
        )
    });
    let limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(bytes, &limits).unwrap();
    snapshot.verify_expected_sha256(sha256).unwrap();
    let loader = build_pe32_load_plan(&snapshot, limits).unwrap();
    assert_eq!(loader.load_plan.image.preferred_image_base, load_base);
    let image =
        materialize_pe32_image(memory, &snapshot, &loader, GuestAddress(load_base)).unwrap();
    (loader, image)
}

fn iat_values(
    memory: &GuestMemory,
    image: &ImageMaterialization,
    loader: &LoaderIngressReport,
    initialization: bool,
) -> Vec<u32> {
    loader
        .load_plan
        .imports
        .iter()
        .flat_map(|module| &module.symbols)
        .map(|symbol| {
            let address = GuestAddress(image.load_base.0 + symbol.iat_rva.0);
            let bytes = if initialization {
                memory
                    .read_initialization(image.layout.owner, address, 4)
                    .unwrap()
            } else {
                memory.read(address, 4).unwrap()
            };
            u32::from_le_bytes(bytes.try_into().unwrap())
        })
        .collect()
}

fn controlled_runtime() -> RuntimeState {
    RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(4096),
        initial_thread_id: RuntimeThreadId(8192),
        initial_last_error: 0,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "runtime-dll-lifecycle-test".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        process_image_path_a: vine_core::RuntimeProcessImagePathA {
            source_authority: "runtime-dll-lifecycle-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
            source_authority: "runtime-dll-lifecycle-process-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .unwrap()
}

fn load_module(runtime: &mut RuntimeState, handle: u32, name: &str, system: bool) {
    runtime
        .load_module(runtime_module(handle, name, system))
        .unwrap();
}

fn runtime_module(handle: u32, name: &str, system: bool) -> RuntimeModule {
    RuntimeModule {
        owner: "runtime-dll-lifecycle-test".to_owned(),
        variant_id: format!("controlled-{name}"),
        namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
        handle,
        requested_name: name.to_owned(),
        canonical_name: name.to_owned(),
        runtime_system_module: system,
    }
}

fn module_id(runtime: &RuntimeState, handle: u32) -> RuntimeModuleId {
    runtime.module_graph().module_by_handle(handle).unwrap().id
}

fn expect_callback(action: RuntimeDllLifecycleAction) -> RuntimeDllCallbackRequest {
    match action {
        RuntimeDllLifecycleAction::Callback { request } => request,
        action => panic!("expected DLL callback, got {action:?}"),
    }
}

fn expect_completion(
    action: RuntimeDllLifecycleAction,
) -> vine_core::RuntimeDllLifecycleCompletion {
    match action {
        RuntimeDllLifecycleAction::Complete { completion } => completion,
        action => panic!("expected DLL lifecycle completion, got {action:?}"),
    }
}

fn assert_callback(
    request: &RuntimeDllCallbackRequest,
    frame_id: RuntimeCallbackFrameId,
    module_id: RuntimeModuleId,
    module_handle: u32,
    module_base: u32,
    entry_point: u32,
    reason: RuntimeDllCallbackReason,
) {
    assert_eq!(request.frame.id, frame_id);
    assert_eq!(request.frame.thread_id, RuntimeThreadId(8192));
    assert_eq!(request.frame.target, GuestAddress(entry_point));
    assert_eq!(request.frame.return_sentinel, CALLBACK_RETURN_ARENA_BASE);
    assert_eq!(request.module_id, module_id);
    assert_eq!(request.module_handle, module_handle);
    assert_eq!(request.module_base, GuestAddress(module_base));
    assert_eq!(request.entry_point, GuestAddress(entry_point));
    assert_eq!(request.reason, reason);
    assert_eq!(request.reserved, DLL_PROCESS_STATIC_RESERVED);
    assert_eq!(
        request.arguments(),
        [
            module_base,
            match reason {
                RuntimeDllCallbackReason::ProcessAttach => DLL_PROCESS_ATTACH,
                RuntimeDllCallbackReason::ProcessDetach => DLL_PROCESS_DETACH,
            },
            DLL_PROCESS_STATIC_RESERVED,
        ]
    );
}
