use crate::callbacks::CALLBACK_RETURN_ARENA_BASE;
use crate::runtime::{
    DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_PROCESS_STATIC_RESERVED, RuntimeCallbackFrameId,
    RuntimeDllCallbackReason, RuntimeGuestImportResolution, X86_STARTUP_INFO_W_BYTE_LENGTH,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use crate::{
    ApiEndpointIdentity, GuestAddress, GuestMemory, ImageMaterialization, MemoryLimits,
    ModuleSnapshot, Pe32Limits, Pe32LoadPlan, Runtime, RuntimeBootstrap, RuntimeClockState,
    RuntimeDllLifecycleAction, RuntimeDllLifecyclePhase, RuntimeExport, RuntimeExportCatalog,
    RuntimeModule, RuntimeModuleId, RuntimeModuleLoadState, RuntimeProcessId, RuntimeStartupInfo,
    RuntimeThreadId, SchedulerThreadState, build_pe32_load_plan, materialize_pe32_image,
};

const CODEC_HANDLE: u32 = 0x4d01_0001;
const ENGINE_HANDLE: u32 = 0x4d01_0002;
const KERNEL32_HANDLE: u32 = 0x4d01_0003;
const MAIN_HANDLE: u32 = 0x4d01_0004;

const CODEC_BASE: u32 = 0x1000_0000;
const ENGINE_BASE: u32 = 0x1100_0000;
const MAIN_BASE: u32 = 0x1200_0000;

fn tls_codec_bytes() -> Vec<u8> {
    let mut bytes = fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.work/toolchain-spike/lifecycle_codec.dll"),
    )
    .unwrap();
    let word =
        |bytes: &[u8], offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    let pe = word(&bytes, 0x3c) as usize;
    let optional = pe + 24;
    let base = word(&bytes, optional + 28);
    let section_table = optional + 224;
    let put = |bytes: &mut [u8], offset, value: u32| {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes())
    };
    put(&mut bytes, section_table + 40 + 8, 0x200); // .rdata includes TLS metadata.
    put(&mut bytes, section_table + 80 + 8, 0x40); // Writable TLS index.
    put(&mut bytes, optional + 96 + 9 * 8, 0x2100);
    put(&mut bytes, optional + 96 + 9 * 8 + 4, 24);
    for (offset, value) in [
        base + 0x2180,
        base + 0x2188,
        base + 0x3010,
        base + 0x2120,
        8,
        0x00e0_0000,
    ]
    .into_iter()
    .enumerate()
    {
        put(&mut bytes, 0x700 + offset * 4, value);
    }
    put(&mut bytes, 0x780, base + 0x1234);
    put(&mut bytes, 0x784, 0x8000_0000);
    // A relocated pointer inside the TLS template must be copied after relocation.
    put(&mut bytes, optional + 96 + 5 * 8 + 4, 36);
    put(&mut bytes, section_table + 120 + 8, 36);
    put(&mut bytes, 0x818, 0x2000);
    put(&mut bytes, 0x81c, 12);
    bytes[0x820..0x824].copy_from_slice(&[0x80, 0x31, 0, 0]);
    bytes
}

#[test]
fn pe_tls_validates_metadata_and_rejects_unimplemented_callbacks() {
    let bytes = tls_codec_bytes();
    let parse = |bytes| {
        let snapshot = ModuleSnapshot::from_bytes(bytes, &Pe32Limits::default()).unwrap();
        build_pe32_load_plan(&snapshot, Pe32Limits::default())
    };
    let plan = parse(bytes.clone()).unwrap();
    let tls = plan.tls.unwrap();
    assert_eq!(tls.template_size, 8);
    assert_eq!(tls.zero_fill, 8);
    assert_eq!(tls.alignment, 8192);
    for (offset, value) in [
        (0x704, 0x1000_0000_u32),
        (0x708, 0x1000_1000),
        (0x710, u32::MAX),
        (0x714, 0x00f0_0000),
        (0x720, 0x1000_1000),
    ] {
        let mut malformed = bytes.clone();
        malformed[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(
            parse(malformed).is_err(),
            "invalid TLS field at {offset:#x}"
        );
    }
}

#[test]
fn image_tls_copies_relocated_templates_for_existing_and_new_threads_and_releases_them() {
    use crate::runtime::RuntimeCreateThreadRequest;
    let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let (main_loader, main_image) =
        materialize_fixture(&mut memory, "bootstrap-params.exe", 0x0040_0000);
    let mut runtime = controlled_runtime(memory);
    runtime
        .register_main_module(runtime_module(MAIN_HANDLE, "bootstrap-params.exe", false))
        .unwrap();
    let catalog = RuntimeExportCatalog::new(Vec::new()).unwrap();
    runtime
        .bind_guest_module_imports(MAIN_HANDLE, &main_loader, &main_image, &catalog)
        .unwrap();
    runtime
        .register_module_image(MAIN_HANDLE, &main_loader, &main_image)
        .unwrap();
    runtime.mark_module_ready(MAIN_HANDLE).unwrap();
    install_fixture_thread_stack(&mut runtime);
    let env = runtime.install_process_environment("tls.exe").unwrap();
    runtime.start_initial_thread().unwrap();
    expect_completion(
        runtime
            .begin_dll_process_attach(CALLBACK_RETURN_ARENA_BASE)
            .unwrap(),
    );
    acknowledge_pending_memory(&mut runtime.memory);
    let initial_id = runtime.current_thread_id();
    let new_thread = |runtime: &mut Runtime| {
        runtime
            .create_guest_thread(RuntimeCreateThreadRequest {
                stack_reserve_bytes: 0x10000,
                stack_commit_bytes: 0x1000,
                entry_point: main_image.entry_point_address,
                parameter: 0,
                thread_id_output: None,
                initial_suspend_count: 0,
            })
            .unwrap()
            .unwrap()
            .0
    };
    let existing = new_thread(&mut runtime);
    acknowledge_pending_memory(&mut runtime.memory);
    let snapshot = ModuleSnapshot::from_bytes(tls_codec_bytes(), &Pe32Limits::default()).unwrap();
    let loader = build_pe32_load_plan(&snapshot, Pe32Limits::default()).unwrap();
    let load = |runtime: &mut Runtime| {
        runtime
            .load_dynamic_library(
                &snapshot,
                &loader,
                "tls.dll",
                GuestAddress(0x1500_0000),
                &catalog,
                &[],
            )
            .unwrap()
            .requester_handle
    };
    let teb_region = runtime
        .memory
        .regions()
        .into_iter()
        .find(|region| region.range.start() == env.teb_address)
        .unwrap();
    runtime
        .memory
        .protect(
            teb_region.owner,
            teb_region.range,
            crate::PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .unwrap();
    acknowledge_pending_memory(&mut runtime.memory);
    let before_memory = runtime.memory.stats();
    let before_addresses = runtime.process_address_space_allocator().clone();
    assert!(
        runtime
            .load_dynamic_library(
                &snapshot,
                &loader,
                "tls.dll",
                GuestAddress(0x1500_0000),
                &catalog,
                &[]
            )
            .is_err()
    );
    assert_eq!(runtime.memory.stats(), before_memory);
    assert_eq!(runtime.process_address_space_allocator(), &before_addresses);
    assert!(runtime.module_by_name("tls.dll").unwrap().is_none());
    runtime
        .memory
        .protect(
            teb_region.owner,
            teb_region.range,
            crate::PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .unwrap();
    acknowledge_pending_memory(&mut runtime.memory);
    let first = load(&mut runtime);
    acknowledge_pending_memory(&mut runtime.memory);
    let word = |runtime: &Runtime, address| {
        u32::from_le_bytes(
            runtime
                .memory
                .read(GuestAddress(address), 4)
                .unwrap()
                .try_into()
                .unwrap(),
        )
    };
    let vector = word(&runtime, env.teb_address.0 + 0x2c);
    let failed_block = word(&runtime, vector);
    runtime.finish_dynamic_library(first, false).unwrap();
    acknowledge_pending_memory(&mut runtime.memory);
    assert_eq!(word(&runtime, vector), 0);
    assert!(runtime.memory.read(GuestAddress(failed_block), 1).is_err());
    let second = load(&mut runtime);
    runtime.finish_dynamic_library(second, true).unwrap();
    acknowledge_pending_memory(&mut runtime.memory);
    assert_eq!(word(&runtime, 0x1500_3010), 0);
    let initial_block = word(&runtime, vector);
    runtime
        .memory
        .write(GuestAddress(initial_block), &0xdead_beef_u32.to_le_bytes())
        .unwrap();
    acknowledge_pending_memory(&mut runtime.memory);
    let created = new_thread(&mut runtime);
    acknowledge_pending_memory(&mut runtime.memory);
    let mut blocks = std::collections::BTreeSet::from([initial_block]);
    for id in [existing, created] {
        let teb = runtime
            .thread_state(id)
            .unwrap()
            .environment
            .as_ref()
            .unwrap()
            .teb_address;
        let thread_vector = word(&runtime, teb.0 + 0x2c);
        let block = word(&runtime, thread_vector);
        assert!(blocks.insert(block));
        assert_eq!(block % 8192, 0);
        assert_eq!(word(&runtime, block), 0x1500_1234);
        assert_eq!(word(&runtime, block + 4), 0x8000_0000);
        assert_eq!(
            runtime.memory.read(GuestAddress(block + 8), 8).unwrap(),
            [0; 8]
        );
        runtime.select_thread(id).unwrap();
        runtime.request_current_thread_exit(0).unwrap();
        runtime.complete_current_guest_thread_exit().unwrap();
        acknowledge_pending_memory(&mut runtime.memory);
        assert!(runtime.memory.read(GuestAddress(block), 1).is_err());
        assert!(runtime.memory.read(GuestAddress(thread_vector), 1).is_err());
        runtime.select_thread(initial_id).unwrap();
    }
    assert_eq!(word(&runtime, initial_block), 0xdead_beef);
    free_fixture_library(&mut runtime, 0x1500_0000, true);
    acknowledge_pending_memory(&mut runtime.memory);
    assert_eq!(word(&runtime, vector), 0);
    assert!(runtime.memory.read(GuestAddress(initial_block), 1).is_err());
    assert!(runtime.module_by_name("tls.dll").unwrap().is_none());
    runtime.request_current_thread_exit(0).unwrap();
    runtime.complete_current_thread_exit().unwrap();
    acknowledge_pending_memory(&mut runtime.memory);
    assert!(runtime.memory.read(GuestAddress(initial_block), 1).is_err());
    assert_eq!(word(&runtime, env.teb_address.0 + 0x2c), 0);
}

#[test]
fn image_tls_registered_before_process_environment_initializes_the_first_thread() {
    let snapshot = ModuleSnapshot::from_bytes(tls_codec_bytes(), &Pe32Limits::default()).unwrap();
    let loader = build_pe32_load_plan(&snapshot, Pe32Limits::default()).unwrap();
    let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let image =
        materialize_pe32_image(&mut memory, &snapshot, &loader, GuestAddress(CODEC_BASE)).unwrap();
    let mut runtime = controlled_runtime(memory);
    load_module(&mut runtime, CODEC_HANDLE, "tls.dll", false);
    runtime
        .bind_guest_module_imports(
            CODEC_HANDLE,
            &loader,
            &image,
            &RuntimeExportCatalog::new(Vec::new()).unwrap(),
        )
        .unwrap();
    runtime
        .register_module_image(CODEC_HANDLE, &loader, &image)
        .unwrap();
    let env = runtime.install_process_environment("tls.exe").unwrap();
    acknowledge_pending_memory(&mut runtime.memory);
    let word = |address| {
        u32::from_le_bytes(
            runtime
                .memory
                .read(GuestAddress(address), 4)
                .unwrap()
                .try_into()
                .unwrap(),
        )
    };
    let vector = word(env.teb_address.0 + 0x2c);
    let block = word(vector);
    assert_eq!(word(CODEC_BASE + 0x3010), 0);
    assert_eq!(word(block), CODEC_BASE + 0x1234);
    assert_eq!(
        runtime.memory.read(GuestAddress(block + 8), 8).unwrap(),
        [0; 8]
    );
}

#[test]
fn dynamic_library_load_is_atomic_and_failed_attach_can_be_retried() {
    let mut memory = fixture_memory();
    let (main_loader, main_image) =
        materialize_fixture(&mut memory, "bootstrap-params.exe", 0x0040_0000);
    let mut runtime = controlled_runtime(memory);
    runtime
        .register_main_module(runtime_module(MAIN_HANDLE, "bootstrap-params.exe", false))
        .unwrap();
    let catalog = RuntimeExportCatalog::new(Vec::new()).unwrap();
    runtime
        .bind_guest_module_imports(MAIN_HANDLE, &main_loader, &main_image, &catalog)
        .unwrap();
    runtime
        .register_module_image(MAIN_HANDLE, &main_loader, &main_image)
        .unwrap();
    runtime.mark_module_ready(MAIN_HANDLE).unwrap();
    install_fixture_thread_stack(&mut runtime);
    runtime.start_initial_thread().unwrap();
    expect_completion(
        runtime
            .begin_dll_process_attach(CALLBACK_RETURN_ARENA_BASE)
            .unwrap(),
    );
    acknowledge_pending_memory(&mut runtime.memory);
    let initial_main = runtime
        .module_graph()
        .module_by_handle(MAIN_HANDLE)
        .unwrap()
        .clone();
    let read = |name: &str| {
        let bytes = fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../.work/toolchain-spike")
                .join(name),
        )
        .unwrap();
        let snapshot = ModuleSnapshot::from_bytes(bytes, &Pe32Limits::default()).unwrap();
        let loader = build_pe32_load_plan(&snapshot, Pe32Limits::default()).unwrap();
        (snapshot, loader)
    };
    let (engine, engine_loader) = read("lifecycle_engine.dll");
    let before = runtime.memory.stats();
    // Missing guest dependencies cannot leave the requested image or module handle behind.
    assert!(
        runtime
            .load_dynamic_library(
                &engine,
                &engine_loader,
                "lifecycle_engine.dll",
                GuestAddress(ENGINE_BASE),
                &catalog,
                &[]
            )
            .is_err()
    );
    assert_eq!(runtime.memory.stats(), before);
    assert!(
        runtime
            .module_by_name("lifecycle_engine.dll")
            .unwrap()
            .is_none()
    );
    let (codec, codec_loader) = read("lifecycle_codec.dll");
    let first = runtime
        .load_dynamic_library(
            &codec,
            &codec_loader,
            "C:\\Vine\\plugin\\lifecycle_codec.dll",
            GuestAddress(CODEC_BASE),
            &catalog,
            &[],
        )
        .unwrap();
    let request = runtime
        .dynamic_library_callback(first.requester_handle, CALLBACK_RETURN_ARENA_BASE)
        .unwrap()
        .unwrap();
    assert_eq!(
        request.arguments,
        vec![
            crate::api::GuestCallbackArgument::U32(CODEC_BASE),
            crate::api::GuestCallbackArgument::U32(DLL_PROCESS_ATTACH),
            crate::api::GuestCallbackArgument::U32(0)
        ]
    );
    assert!(
        runtime
            .finish_dynamic_library(first.requester_handle + 1, true)
            .is_err()
    );
    assert_eq!(
        runtime
            .finish_dynamic_library(first.requester_handle, false)
            .unwrap(),
        0
    );
    assert!(
        runtime
            .module_by_name("lifecycle_codec.dll")
            .unwrap()
            .is_none()
    );
    assert!(runtime.memory.read(GuestAddress(CODEC_BASE), 1).is_err());
    let second = runtime
        .load_dynamic_library(
            &codec,
            &codec_loader,
            "lifecycle_codec.dll",
            GuestAddress(CODEC_BASE),
            &catalog,
            &[],
        )
        .unwrap();
    assert_ne!(first.requester_handle, second.requester_handle);
    assert_eq!(
        runtime
            .finish_dynamic_library(second.requester_handle, true)
            .unwrap(),
        CODEC_BASE
    );
    let loaded = runtime
        .module_graph()
        .module_by_handle(second.requester_handle)
        .unwrap();
    assert!(matches!(
        loaded.state,
        RuntimeModuleLoadState::ProcessAttached { .. }
    ));
    assert_eq!(loaded.reference_count(), 1);
    assert_eq!(
        runtime
            .module_graph()
            .module_by_handle(MAIN_HANDLE)
            .unwrap(),
        &initial_main
    );
    let engine = runtime
        .load_dynamic_library(
            &engine,
            &engine_loader,
            "lifecycle_engine.dll",
            GuestAddress(ENGINE_BASE),
            &catalog,
            &[],
        )
        .unwrap();
    assert_eq!(
        runtime
            .finish_dynamic_library(engine.requester_handle, true)
            .unwrap(),
        ENGINE_BASE
    );
    assert_eq!(
        runtime
            .module_graph()
            .module_by_handle(second.requester_handle)
            .unwrap()
            .reference_count(),
        2
    );
    free_fixture_library(&mut runtime, ENGINE_BASE, true);
    assert!(
        runtime
            .module_by_name("lifecycle_engine.dll")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        runtime
            .module_graph()
            .module_by_handle(second.requester_handle)
            .unwrap()
            .reference_count(),
        1
    );
    // A second load keeps the image live until the last release.
    runtime
        .add_current_thread_module_reference(second.requester_handle)
        .unwrap();
    free_fixture_library(&mut runtime, CODEC_BASE, false);
    acknowledge_pending_memory(&mut runtime.memory);
    assert!(runtime.memory.read(GuestAddress(CODEC_BASE), 1).is_ok());
    free_fixture_library(&mut runtime, CODEC_BASE, true);
    assert!(
        runtime
            .module_by_name("lifecycle_codec.dll")
            .unwrap()
            .is_none()
    );
    let reloaded = runtime
        .load_dynamic_library(
            &codec,
            &codec_loader,
            "lifecycle_codec.dll",
            GuestAddress(CODEC_BASE),
            &catalog,
            &[],
        )
        .unwrap();
    assert_ne!(reloaded.requester_handle, second.requester_handle);
    runtime
        .finish_dynamic_library(reloaded.requester_handle, true)
        .unwrap();
}

fn free_fixture_library(runtime: &mut Runtime, base: u32, detach: bool) {
    use crate::api::{
        ApiDispatcher, ApiOwnedTrap, DispatchOutcome, GuestCallbackArgument, GuestCallbackReturn,
    };
    let export =
        crate::dll::runtime_export_endpoint(0, RuntimeExport::named("kernel32.dll", "FreeLibrary"))
            .unwrap();
    let endpoint = ApiEndpointIdentity::from_runtime_system_export(&export);
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    if detach {
        let handle = runtime
            .module_image_by_load_base(GuestAddress(base))
            .unwrap()
            .unwrap()
            .handle;
        let before = runtime.module_graph().clone();
        let invalid_frame = crate::api::ApiCallFrame {
            endpoint: endpoint.clone(),
            stack_pointer: GuestAddress(0),
            return_address: GuestAddress(0x0040_1000),
            arguments: vec![base],
        };
        assert!(
            runtime
                .free_library(handle, &invalid_frame, CALLBACK_RETURN_ARENA_BASE)
                .is_err()
        );
        assert_eq!(runtime.module_graph(), &before);
    }
    let stack = runtime.thread_stack(runtime.current_thread_id()).unwrap();
    let pointer = GuestAddress(stack.initial_stack_pointer.0 - 16);
    let return_address = GuestAddress(0x0040_1000);
    runtime
        .memory
        .write(
            pointer,
            &[return_address.0.to_le_bytes(), base.to_le_bytes()].concat(),
        )
        .unwrap();
    acknowledge_pending_memory(&mut runtime.memory);
    let outcome = runtime.dispatch_api_call(
        &dispatcher,
        ApiOwnedTrap {
            endpoint_id: endpoint.id,
            endpoint_address: endpoint.address,
            stack_pointer: pointer,
            return_address,
        },
        CALLBACK_RETURN_ARENA_BASE,
        None,
    );
    if !detach {
        let DispatchOutcome::Immediate(completion) = outcome else {
            panic!("{outcome:?}")
        };
        assert_eq!(completion.eax, Some(1));
        return;
    }
    let DispatchOutcome::GuestCallback(call) = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(
        call.request.arguments,
        vec![
            GuestCallbackArgument::U32(base),
            GuestCallbackArgument::U32(0),
            GuestCallbackArgument::U32(0)
        ]
    );
    let handle = runtime
        .module_image_by_load_base(GuestAddress(base))
        .unwrap()
        .unwrap()
        .handle;
    assert!(runtime.add_current_thread_module_reference(handle).is_err());
    assert!(runtime.finish_library_release(handle + 1).is_err());
    runtime.begin_guest_callback(call).unwrap();
    acknowledge_pending_memory(&mut runtime.memory);
    assert!(runtime.memory.fetch(GuestAddress(base + 0x1000), 1).is_ok());
    // DllMain's return value is ignored for DLL_PROCESS_DETACH.
    let GuestCallbackReturn::Completed(completion) = runtime
        .complete_guest_callback(&dispatcher, CALLBACK_RETURN_ARENA_BASE, 0)
        .unwrap()
    else {
        panic!("detach did not complete")
    };
    assert_eq!(completion.eax, Some(1));
    assert_eq!(completion.instruction_pointer, return_address);
    acknowledge_pending_memory(&mut runtime.memory);
    assert!(runtime.memory.read(GuestAddress(base), 1).is_err());
    assert!(runtime.module_option(handle).unwrap().is_none());
}

#[test]
fn callback_free_dll_detach_preserves_a_waiting_thread_until_process_exit() {
    let mut memory = fixture_memory();
    let (main_loader, main_image) =
        materialize_fixture(&mut memory, "bootstrap-params.exe", 0x0040_0000);
    let mut runtime = controlled_runtime(memory);
    load_module(&mut runtime, KERNEL32_HANDLE, "kernel32.dll", true);
    runtime
        .register_main_module(runtime_module(MAIN_HANDLE, "bootstrap-params.exe", false))
        .unwrap();
    let binding = runtime
        .bind_guest_module_imports(
            MAIN_HANDLE,
            &main_loader,
            &main_image,
            &RuntimeExportCatalog::new(Vec::new()).unwrap(),
        )
        .unwrap();
    assert!(binding.imports.is_empty());
    runtime
        .register_module_image(MAIN_HANDLE, &main_loader, &main_image)
        .unwrap();
    runtime.mark_module_ready(KERNEL32_HANDLE).unwrap();
    runtime.mark_module_ready(MAIN_HANDLE).unwrap();
    runtime.start_initial_thread().unwrap();

    let attached = expect_completion(
        runtime
            .begin_dll_process_attach(CALLBACK_RETURN_ARENA_BASE)
            .unwrap(),
    );
    assert_eq!(attached, RuntimeDllLifecyclePhase::ProcessAttached);

    let request = runtime.begin_host_request("browser.timer.wait").unwrap();
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: request.request_id
        }
    );
    let detached = expect_completion(
        runtime
            .begin_dll_process_detach(CALLBACK_RETURN_ARENA_BASE)
            .unwrap(),
    );
    assert_eq!(detached, RuntimeDllLifecyclePhase::ProcessDetached);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: request.request_id
        }
    );
}

#[test]
fn rejected_import_registration_rolls_back_iat_writes_and_image_sealing() {
    let mut memory = fixture_memory();
    let (codec_loader, codec_image) =
        materialize_fixture(&mut memory, "lifecycle_codec.dll", CODEC_BASE);
    let (engine_loader, engine_image) =
        materialize_fixture(&mut memory, "lifecycle_engine.dll", ENGINE_BASE);
    let catalog = RuntimeExportCatalog::new(Vec::new()).unwrap();
    let mut runtime = controlled_runtime(memory);
    load_module(&mut runtime, CODEC_HANDLE, "lifecycle_codec.dll", false);
    load_module(&mut runtime, ENGINE_HANDLE, "lifecycle_engine.dll", false);
    runtime
        .bind_guest_module_imports(CODEC_HANDLE, &codec_loader, &codec_image, &catalog)
        .unwrap();
    runtime
        .register_module_image(CODEC_HANDLE, &codec_loader, &codec_image)
        .unwrap();
    runtime.mark_module_ready(ENGINE_HANDLE).unwrap();

    let stats_before = runtime.memory.stats();
    let iat_before = iat_values(&runtime.memory, &engine_image, &engine_loader, true);
    assert!(
        runtime
            .bind_guest_module_imports(ENGINE_HANDLE, &engine_loader, &engine_image, &catalog,)
            .is_err()
    );
    assert_eq!(runtime.module_graph().nodes().len(), 2);
    let engine = runtime
        .module_graph()
        .module_by_handle(ENGINE_HANDLE)
        .unwrap();
    assert_eq!(engine.state, RuntimeModuleLoadState::Ready);
    assert!(engine.dependencies.is_empty());
    let codec = runtime
        .module_graph()
        .module_by_handle(CODEC_HANDLE)
        .unwrap();
    assert_eq!(codec.state, RuntimeModuleLoadState::Registered);
    assert!(codec.dependents.is_empty());
    assert_eq!(
        runtime.dll_lifecycle_phase(),
        RuntimeDllLifecyclePhase::Idle
    );
    assert_eq!(runtime.memory.stats(), stats_before);
    // Initialization reads also verify that the failed registration did not seal the image.
    assert_eq!(
        iat_values(&runtime.memory, &engine_image, &engine_loader, true),
        iat_before
    );
}

#[test]
fn guest_import_binding_rejects_changed_iat_then_accepts_repaired_image() {
    let mut memory = fixture_memory();
    let (codec_loader, codec_image) =
        materialize_fixture(&mut memory, "lifecycle_codec.dll", CODEC_BASE);
    let (engine_loader, engine_image) =
        materialize_fixture(&mut memory, "lifecycle_engine.dll", ENGINE_BASE);
    let catalog = RuntimeExportCatalog::new(Vec::new()).unwrap();
    let mut runtime = controlled_runtime(memory);
    load_module(&mut runtime, CODEC_HANDLE, "lifecycle_codec.dll", false);
    load_module(&mut runtime, ENGINE_HANDLE, "lifecycle_engine.dll", false);
    runtime
        .bind_guest_module_imports(CODEC_HANDLE, &codec_loader, &codec_image, &catalog)
        .unwrap();
    runtime
        .register_module_image(CODEC_HANDLE, &codec_loader, &codec_image)
        .unwrap();
    let symbol = &engine_loader.imports[0].symbols[0];
    let address = GuestAddress(engine_image.layout.image_range.start().0 + symbol.iat_rva.0);
    let changed = symbol.original_iat_value ^ 1;
    runtime
        .memory
        .write_initialization(engine_image.layout.owner, address, &changed.to_le_bytes())
        .unwrap();
    let physical_pages = runtime.memory.stats().physical_pages;
    assert!(
        runtime
            .bind_guest_module_imports(ENGINE_HANDLE, &engine_loader, &engine_image, &catalog,)
            .is_err()
    );
    assert_eq!(runtime.memory.stats().physical_pages, physical_pages);
    assert_eq!(runtime.memory.stats().sealed_owners, 1);
    assert_eq!(
        runtime
            .memory
            .read_initialization(engine_image.layout.owner, address, 4)
            .unwrap(),
        changed.to_le_bytes()
    );
    assert!(
        runtime
            .module_graph()
            .module_by_handle(ENGINE_HANDLE)
            .unwrap()
            .dependencies
            .is_empty()
    );
    runtime
        .memory
        .write_initialization(
            engine_image.layout.owner,
            address,
            &symbol.original_iat_value.to_le_bytes(),
        )
        .unwrap();
    let binding = runtime
        .bind_guest_module_imports(ENGINE_HANDLE, &engine_loader, &engine_image, &catalog)
        .expect("repaired IAT binds normally");
    assert!(!binding.imports.is_empty());
    assert_eq!(runtime.memory.stats().sealed_owners, 2);
}

#[test]
fn dll_boot_frame_is_prepared_before_reflection_but_cannot_return_before_acknowledgement() {
    let mut runtime = codec_runtime_with_stack();
    runtime.set_current_last_error(27).unwrap();
    let (request, pointer) = match runtime
        .begin_dll_process_attach(CALLBACK_RETURN_ARENA_BASE)
        .unwrap()
    {
        RuntimeDllLifecycleAction::Callback {
            target,
            stack_pointer,
        } => {
            let frame = *runtime
                .current_thread_state()
                .unwrap()
                .callback_frames
                .last()
                .unwrap();
            assert_eq!(target, frame.target);
            (frame, stack_pointer)
        }
        other => panic!("expected initial DllMain, got {other:?}"),
    };
    let before = runtime.memory.stats();
    assert!(
        runtime
            .complete_dll_callback(CALLBACK_RETURN_ARENA_BASE, 1)
            .is_err()
    );
    assert_eq!(runtime.memory.stats(), before);
    assert_eq!(
        runtime.current_thread_state().unwrap().callback_frames,
        [request]
    );
    assert_eq!(
        runtime.dll_lifecycle_phase(),
        RuntimeDllLifecyclePhase::ProcessAttaching
    );
    assert_eq!(runtime.current_last_error().unwrap(), 27);
    let reflection = runtime.memory.pending_reflection().unwrap();
    runtime
        .memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    assert_eq!(
        runtime.memory.read(pointer, 16).unwrap(),
        [
            CALLBACK_RETURN_ARENA_BASE.0,
            CODEC_BASE,
            DLL_PROCESS_ATTACH,
            DLL_PROCESS_STATIC_RESERVED
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>()
    );
    let completed = expect_completion(
        runtime
            .complete_dll_callback(CALLBACK_RETURN_ARENA_BASE, 1)
            .unwrap(),
    );
    assert_eq!(completed, RuntimeDllLifecyclePhase::ProcessAttached);
    assert!(
        runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .is_empty()
    );
    assert_eq!(runtime.current_last_error().unwrap(), 27);
}

#[test]
fn rejected_dll_boot_stack_write_does_not_publish_a_frame_or_lifecycle_transition() {
    let mut runtime = codec_runtime_with_stack();
    acknowledge_pending_memory(&mut runtime.memory);
    let stack = runtime.thread_stack(runtime.current_thread_id()).unwrap();
    let owner = stack.memory_owner;
    let committed = stack.committed_range;
    let pointer = GuestAddress(stack.initial_stack_pointer.0 - 16);
    let original = runtime.memory.read(pointer, 16).unwrap();
    runtime
        .memory
        .protect(
            owner,
            committed,
            crate::PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .unwrap();
    acknowledge_pending_memory(&mut runtime.memory);
    let before = runtime.memory.stats();
    assert!(
        runtime
            .begin_dll_process_attach(CALLBACK_RETURN_ARENA_BASE)
            .is_err()
    );
    assert_eq!(runtime.memory.stats(), before);
    assert_eq!(runtime.memory.read(pointer, 16).unwrap(), original);
    assert_eq!(
        runtime.dll_lifecycle_phase(),
        RuntimeDllLifecyclePhase::Idle
    );
    assert_eq!(
        runtime
            .module_graph()
            .module_by_handle(CODEC_HANDLE)
            .unwrap()
            .state,
        RuntimeModuleLoadState::Ready
    );
    assert!(
        runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .is_empty()
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    runtime
        .memory
        .protect(
            owner,
            committed,
            crate::PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .unwrap();
    acknowledge_pending_memory(&mut runtime.memory);
    let (request, _) = expect_callback(
        runtime
            .begin_dll_process_attach(CALLBACK_RETURN_ARENA_BASE)
            .unwrap(),
        &mut runtime,
    );
    assert_eq!(request.id, RuntimeCallbackFrameId(1));
    assert_eq!(runtime.current_last_error().unwrap(), 0);
}

fn codec_runtime_with_stack() -> Runtime {
    let mut memory = fixture_memory();
    let (loader, image) = materialize_fixture(&mut memory, "lifecycle_codec.dll", CODEC_BASE);
    let mut runtime = controlled_runtime(memory);
    load_module(&mut runtime, CODEC_HANDLE, "lifecycle_codec.dll", false);
    runtime
        .bind_guest_module_imports(
            CODEC_HANDLE,
            &loader,
            &image,
            &RuntimeExportCatalog::new(Vec::new()).unwrap(),
        )
        .unwrap();
    runtime
        .register_module_image(CODEC_HANDLE, &loader, &image)
        .unwrap();
    let (main_loader, main_image) =
        materialize_fixture(&mut runtime.memory, "bootstrap-params.exe", 0x0040_0000);
    runtime
        .register_main_module(runtime_module(MAIN_HANDLE, "bootstrap-params.exe", false))
        .unwrap();
    runtime
        .bind_guest_module_imports(
            MAIN_HANDLE,
            &main_loader,
            &main_image,
            &RuntimeExportCatalog::new(Vec::new()).unwrap(),
        )
        .unwrap();
    runtime
        .register_module_image(MAIN_HANDLE, &main_loader, &main_image)
        .unwrap();
    runtime
        .add_current_thread_module_reference(CODEC_HANDLE)
        .unwrap();
    runtime.mark_module_ready(CODEC_HANDLE).unwrap();
    runtime.mark_module_ready(MAIN_HANDLE).unwrap();
    install_fixture_thread_stack(&mut runtime);
    runtime.start_initial_thread().unwrap();
    runtime
}

#[test]
fn guest_import_graph_drives_atomic_dll_process_lifecycle() {
    let mut memory = fixture_memory();
    let (codec_loader, codec_image) =
        materialize_fixture(&mut memory, "lifecycle_codec.dll", CODEC_BASE);
    let (engine_loader, engine_image) =
        materialize_fixture(&mut memory, "lifecycle_engine.dll", ENGINE_BASE);
    let (main_loader, main_image) =
        materialize_fixture(&mut memory, "dll-lifecycle.exe", MAIN_BASE);
    assert_eq!(codec_loader.imports.len(), 0);
    assert_eq!(engine_loader.imports.len(), 1);
    assert_eq!(main_loader.imports.len(), 2);

    let catalog = RuntimeExportCatalog::new(vec![
        RuntimeExport::named("kernel32.dll", "GetCurrentProcessId"),
        RuntimeExport::named("kernel32.dll", "LoadLibraryA"),
    ])
    .unwrap();
    let mut runtime = controlled_runtime(memory);
    load_module(&mut runtime, CODEC_HANDLE, "lifecycle_codec.dll", false);
    load_module(&mut runtime, ENGINE_HANDLE, "lifecycle_engine.dll", false);
    load_module(&mut runtime, KERNEL32_HANDLE, "kernel32.dll", true);
    runtime
        .register_main_module(runtime_module(MAIN_HANDLE, "dll-lifecycle.exe", false))
        .unwrap();

    let initial_modules =
        [CODEC_HANDLE, ENGINE_HANDLE, KERNEL32_HANDLE, MAIN_HANDLE].map(|handle| {
            let node = runtime.module_graph().module_by_handle(handle).unwrap();
            (handle, node.id, node.reference_count(), node.state)
        });
    let memory_before_missing = runtime.memory.stats();
    let engine_iat_before = iat_values(&runtime.memory, &engine_image, &engine_loader, true);
    runtime
        .bind_guest_module_imports(ENGINE_HANDLE, &engine_loader, &engine_image, &catalog)
        .expect_err("engine cannot bind before the codec image is registered");
    for (handle, id, references, state) in initial_modules {
        let node = runtime.module_graph().module_by_handle(handle).unwrap();
        assert_eq!(node.id, id);
        assert_eq!(node.reference_count(), references);
        assert_eq!(node.state, state);
        assert!(node.dependencies.is_empty());
        assert!(node.dependents.is_empty());
    }
    assert!(runtime.module_import_bindings().is_empty());
    assert!(runtime.runtime_system_exports().is_none());
    assert_eq!(runtime.memory.stats(), memory_before_missing);
    assert_eq!(
        iat_values(&runtime.memory, &engine_image, &engine_loader, true),
        engine_iat_before
    );

    let codec_binding = runtime
        .bind_guest_module_imports(CODEC_HANDLE, &codec_loader, &codec_image, &catalog)
        .unwrap();
    assert!(codec_binding.imports.is_empty());
    runtime
        .register_module_image(CODEC_HANDLE, &codec_loader, &codec_image)
        .unwrap();

    let engine_binding = runtime
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
    runtime
        .register_module_image(ENGINE_HANDLE, &engine_loader, &engine_image)
        .unwrap();

    let main_binding = runtime
        .bind_guest_module_imports(MAIN_HANDLE, &main_loader, &main_image, &catalog)
        .unwrap();
    assert_eq!(main_binding.imports.len(), 3);
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
    assert_eq!(runtime_exports.endpoints.len(), 2);
    assert!(main_binding.imports.iter().any(|import| matches!(
        &import.resolution,
        RuntimeGuestImportResolution::RuntimeExport { endpoint }
            if endpoint.export == RuntimeExport::named("kernel32.dll", "LoadLibraryA")
    )));
    runtime
        .register_module_image(MAIN_HANDLE, &main_loader, &main_image)
        .unwrap();
    install_fixture_thread_stack(&mut runtime);
    let reflection = runtime.memory.pending_reflection().unwrap();
    assert!(reflection.pages.len() >= 4);
    runtime
        .memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    assert_eq!(
        runtime.memory.read(GuestAddress(0x7f00_0000), 4).unwrap(),
        [0x0f, 0x0b, 0xf4, 0xf4]
    );
    assert_eq!(
        runtime.memory.fetch(GuestAddress(0x7f00_0000), 2).unwrap(),
        [0x0f, 0x0b]
    );
    assert!(
        runtime
            .memory
            .write(GuestAddress(0x7f00_0000), &[0x90])
            .is_err()
    );

    for (binding, loader, image) in [
        (&engine_binding, &engine_loader, &engine_image),
        (&main_binding, &main_loader, &main_image),
    ] {
        assert_eq!(
            iat_values(&runtime.memory, image, loader, false),
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

    let last_error_before = runtime.current_last_error().unwrap();
    runtime
        .begin_dll_process_attach(GuestAddress(CALLBACK_RETURN_ARENA_BASE.0 - 1))
        .expect_err("unowned callback sentinel must fail before lifecycle mutation");
    for handle in [CODEC_HANDLE, ENGINE_HANDLE, KERNEL32_HANDLE, MAIN_HANDLE] {
        assert_eq!(
            runtime
                .module_graph()
                .module_by_handle(handle)
                .unwrap()
                .state,
            RuntimeModuleLoadState::Ready
        );
    }
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.current_last_error().unwrap(), last_error_before);
    assert_eq!(
        runtime.dll_lifecycle_phase(),
        RuntimeDllLifecyclePhase::Idle
    );

    let (codec_attach, codec_attach_words) = expect_callback(
        runtime
            .begin_dll_process_attach(CALLBACK_RETURN_ARENA_BASE)
            .unwrap(),
        &mut runtime,
    );
    assert_callback(
        &codec_attach,
        RuntimeCallbackFrameId(1),
        codec_attach_words,
        CODEC_BASE,
        CODEC_BASE + 0x1030,
        RuntimeDllCallbackReason::ProcessAttach,
    );
    runtime
        .complete_dll_callback(GuestAddress(CALLBACK_RETURN_ARENA_BASE.0 + 1), 1)
        .expect_err("DLL callback completion must identify the active sentinel");
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().callback_frames,
        [codec_attach]
    );
    let stack = runtime.thread_stack(runtime.current_thread_id()).unwrap();
    let owner = stack.memory_owner;
    let committed = stack.committed_range;
    let pointer = GuestAddress(stack.initial_stack_pointer.0 - 16);
    let previous_words = runtime.memory.read(pointer, 16).unwrap();
    runtime
        .memory
        .protect(
            owner,
            committed,
            crate::PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .unwrap();
    acknowledge_pending_memory(&mut runtime.memory);
    let before = runtime.memory.stats();
    assert!(
        runtime
            .complete_dll_callback(CALLBACK_RETURN_ARENA_BASE, 1)
            .is_err()
    );
    assert_eq!(runtime.memory.stats(), before);
    assert_eq!(runtime.memory.read(pointer, 16).unwrap(), previous_words);
    assert_eq!(
        runtime.current_thread_state().unwrap().callback_frames,
        [codec_attach]
    );
    assert_eq!(
        runtime.dll_lifecycle_phase(),
        RuntimeDllLifecyclePhase::ProcessAttaching
    );
    assert_eq!(runtime.current_last_error().unwrap(), last_error_before);
    runtime
        .memory
        .protect(
            owner,
            committed,
            crate::PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .unwrap();
    acknowledge_pending_memory(&mut runtime.memory);
    let (engine_attach, engine_attach_words) = expect_callback(
        runtime
            .complete_dll_callback(CALLBACK_RETURN_ARENA_BASE, 1)
            .unwrap(),
        &mut runtime,
    );
    assert_callback(
        &engine_attach,
        RuntimeCallbackFrameId(2),
        engine_attach_words,
        ENGINE_BASE,
        ENGINE_BASE + 0x1020,
        RuntimeDllCallbackReason::ProcessAttach,
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().callback_frames,
        [engine_attach],
        "DLL callback replacement commits without an observable empty stack"
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );
    runtime
        .add_current_thread_module_reference(CODEC_HANDLE)
        .expect_err("graph mutation during DllMain is structured");
    let (codec_unwind, codec_unwind_words) = expect_callback(
        runtime
            .complete_dll_callback(CALLBACK_RETURN_ARENA_BASE, 0)
            .unwrap(),
        &mut runtime,
    );
    assert_callback(
        &codec_unwind,
        RuntimeCallbackFrameId(3),
        codec_unwind_words,
        CODEC_BASE,
        CODEC_BASE + 0x1030,
        RuntimeDllCallbackReason::ProcessDetach,
    );
    let failure = match runtime
        .complete_dll_callback(CALLBACK_RETURN_ARENA_BASE, 1)
        .unwrap()
    {
        RuntimeDllLifecycleAction::AttachFailed { failed_module } => failed_module,
        action => panic!("expected attach failure, got {action:?}"),
    };
    assert_eq!(failure, "lifecycle_engine.dll");
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

    let (codec_retry, _) = expect_callback(
        runtime
            .begin_dll_process_attach(CALLBACK_RETURN_ARENA_BASE)
            .unwrap(),
        &mut runtime,
    );
    assert_eq!(codec_retry.id, RuntimeCallbackFrameId(4));
    let (engine_retry, _) = expect_callback(
        runtime
            .complete_dll_callback(CALLBACK_RETURN_ARENA_BASE, 1)
            .unwrap(),
        &mut runtime,
    );
    assert_eq!(engine_retry.id, RuntimeCallbackFrameId(5));
    let attached = expect_completion(
        runtime
            .complete_dll_callback(CALLBACK_RETURN_ARENA_BASE, 1)
            .unwrap(),
    );
    assert_eq!(attached, RuntimeDllLifecyclePhase::ProcessAttached);
    for (index, id) in [kernel32_id, codec_id, engine_id, main_id]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            runtime.module_graph().module(id).unwrap().state,
            RuntimeModuleLoadState::ProcessAttached {
                sequence: (index + 1) as u32
            }
        );
    }

    let request = runtime
        .begin_host_request("browser.lifecycle.wait")
        .unwrap();
    runtime
        .begin_dll_process_detach(CALLBACK_RETURN_ARENA_BASE)
        .expect_err("detach with guest callbacks requires an active thread");
    assert_eq!(
        runtime.dll_lifecycle_phase(),
        RuntimeDllLifecyclePhase::ProcessAttached
    );
    for id in [codec_id, engine_id, kernel32_id, main_id] {
        assert!(matches!(
            runtime.module_graph().module(id).unwrap().state,
            RuntimeModuleLoadState::ProcessAttached { .. }
        ));
    }
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
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: request.request_id
        }
    );
    runtime
        .complete_host_request(request.request_id)
        .expect("resume after rejected detach");
    runtime.pop_scheduler_event();
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );

    let outer_callback = runtime
        .push_callback_frame(GuestAddress(0x1000_1000), CALLBACK_RETURN_ARENA_BASE)
        .expect("enter ordinary callback before process detach");
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );

    let (engine_detach, engine_detach_words) = expect_callback(
        runtime
            .begin_dll_process_detach(CALLBACK_RETURN_ARENA_BASE)
            .unwrap(),
        &mut runtime,
    );
    assert_callback(
        &engine_detach,
        RuntimeCallbackFrameId(7),
        engine_detach_words,
        ENGINE_BASE,
        ENGINE_BASE + 0x1020,
        RuntimeDllCallbackReason::ProcessDetach,
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().callback_frames,
        [outer_callback, engine_detach]
    );
    let (codec_detach, codec_detach_words) = expect_callback(
        runtime
            .complete_dll_callback(CALLBACK_RETURN_ARENA_BASE, 0)
            .unwrap(),
        &mut runtime,
    );
    assert_callback(
        &codec_detach,
        RuntimeCallbackFrameId(8),
        codec_detach_words,
        CODEC_BASE,
        CODEC_BASE + 0x1030,
        RuntimeDllCallbackReason::ProcessDetach,
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().callback_frames,
        [outer_callback, codec_detach]
    );
    let nested = runtime
        .push_callback_frame(codec_detach.target, CALLBACK_RETURN_ARENA_BASE)
        .unwrap();
    let before = runtime.memory.stats();
    assert!(
        runtime
            .complete_dll_callback(CALLBACK_RETURN_ARENA_BASE, 0)
            .is_err()
    );
    assert_eq!(runtime.memory.stats(), before);
    assert_eq!(
        runtime.current_thread_state().unwrap().callback_frames,
        [outer_callback, codec_detach, nested]
    );
    assert_eq!(
        runtime.dll_lifecycle_phase(),
        RuntimeDllLifecyclePhase::ProcessDetaching
    );
    runtime.pop_callback_frame(nested.id).unwrap();
    let detached = expect_completion(
        runtime
            .complete_dll_callback(CALLBACK_RETURN_ARENA_BASE, 0)
            .unwrap(),
    );
    assert_eq!(detached, RuntimeDllLifecyclePhase::ProcessDetached);
    for (index, id) in [main_id, engine_id, codec_id, kernel32_id]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            runtime.module_graph().module(id).unwrap().state,
            RuntimeModuleLoadState::ProcessDetached {
                sequence: (index + 1) as u32
            }
        );
    }
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().callback_frames,
        [outer_callback]
    );
    runtime
        .pop_callback_frame(outer_callback.id)
        .expect("ordinary callback resumes after process detach");
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
}

fn fixture_memory() -> GuestMemory {
    GuestMemory::new(MemoryLimits {
        max_physical_pages: 40,
        max_regions: 8,
        max_owners: 8,
        ..MemoryLimits::default()
    })
    .unwrap()
}

fn materialize_fixture(
    memory: &mut GuestMemory,
    name: &str,
    load_base: u32,
) -> (Pe32LoadPlan, ImageMaterialization) {
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
    let loader = build_pe32_load_plan(&snapshot, limits).unwrap();
    let image =
        materialize_pe32_image(memory, &snapshot, &loader, GuestAddress(load_base)).unwrap();
    (loader, image)
}

fn iat_values(
    memory: &GuestMemory,
    image: &ImageMaterialization,
    loader: &Pe32LoadPlan,
    initialization: bool,
) -> Vec<u32> {
    loader
        .imports
        .iter()
        .flat_map(|module| &module.symbols)
        .map(|symbol| {
            let address = GuestAddress(image.layout.image_range.start().0 + symbol.iat_rva.0);
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

fn controlled_runtime(memory: GuestMemory) -> Runtime {
    Runtime::new(
        RuntimeBootstrap {
            process_id: RuntimeProcessId(4096),
            initial_thread_id: RuntimeThreadId(8192),
            initial_last_error: 0,
            clock: RuntimeClockState {
                wall_clock_file_time: 134_282_016_000_000_000,
                tick_count: 3_600_000,
                performance_counter: 12_345_678_901,
                performance_frequency: 10_000_000,
            },
            startup_info: RuntimeStartupInfo {
                bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
            },
            process_image_path_a: crate::RuntimeProcessImagePathA {
                bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            },
            initial_current_directory_a: crate::RuntimeCurrentDirectoryA {
                bytes_with_nul: b"C:\\Vine\0".to_vec(),
            },
            encoded_pointer_cookie: 0xec01_9d3b,
        },
        memory,
    )
    .unwrap()
}

fn load_module(runtime: &mut Runtime, handle: u32, name: &str, system: bool) {
    runtime
        .load_module(runtime_module(handle, name, system))
        .unwrap();
}

fn runtime_module(handle: u32, name: &str, system: bool) -> RuntimeModule {
    RuntimeModule {
        handle,
        requested_name: name.to_owned(),
        canonical_name: name.to_owned(),
        runtime_system_module: system,
    }
}

fn module_id(runtime: &Runtime, handle: u32) -> RuntimeModuleId {
    runtime.module_graph().module_by_handle(handle).unwrap().id
}

fn expect_callback(
    action: RuntimeDllLifecycleAction,
    runtime: &mut Runtime,
) -> (crate::runtime::RuntimeCallbackFrame, [u32; 4]) {
    match action {
        RuntimeDllLifecycleAction::Callback {
            target,
            stack_pointer,
        } => {
            let request = *runtime
                .current_thread_state()
                .unwrap()
                .callback_frames
                .last()
                .unwrap();
            assert_eq!(target, request.target);
            acknowledge_pending_memory(&mut runtime.memory);
            let bytes = runtime.memory.read(stack_pointer, 16).unwrap();
            let words = std::array::from_fn(|index| {
                u32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap())
            });
            assert_eq!(words[0], request.return_sentinel.0);
            (request, words)
        }
        action => panic!("expected DLL callback, got {action:?}"),
    }
}

fn acknowledge_pending_memory(memory: &mut GuestMemory) {
    if memory.stats().pending_invalidation.is_none() {
        return;
    }
    let generation = if memory.stats().acknowledged_generation == 0 {
        memory.pending_reflection().unwrap().invalidation.generation
    } else {
        memory.pending_delta().unwrap().generation
    };
    memory.acknowledge_delta(generation).unwrap();
}

fn install_fixture_thread_stack(runtime: &mut Runtime) {
    let allocation = runtime
        .address_space_allocator
        .allocation_for_purpose(&crate::ProcessAddressAllocationPurpose::CallbackReturns)
        .unwrap();
    let sentinels =
        crate::install_runtime_return_sentinels(&mut runtime.memory, allocation).unwrap();
    let stack = crate::install_initial_thread_stack(
        &mut runtime.memory,
        &sentinels.process_entry_return,
        0x10000,
        0x1000,
    )
    .unwrap();
    runtime.register_initial_thread_stack(&stack).unwrap();
}

fn expect_completion(action: RuntimeDllLifecycleAction) -> RuntimeDllLifecyclePhase {
    match action {
        RuntimeDllLifecycleAction::Complete { phase } => phase,
        action => panic!("expected DLL lifecycle completion, got {action:?}"),
    }
}

fn assert_callback(
    request: &crate::runtime::RuntimeCallbackFrame,
    frame_id: RuntimeCallbackFrameId,
    words: [u32; 4],
    module_base: u32,
    entry_point: u32,
    reason: RuntimeDllCallbackReason,
) {
    assert_eq!(request.id, frame_id);
    assert_eq!(request.thread_id, RuntimeThreadId(8192));
    assert_eq!(request.target, GuestAddress(entry_point));
    assert_eq!(request.return_sentinel, CALLBACK_RETURN_ARENA_BASE);
    assert_eq!(
        words,
        [
            CALLBACK_RETURN_ARENA_BASE.0,
            module_base,
            match reason {
                RuntimeDllCallbackReason::ProcessAttach => DLL_PROCESS_ATTACH,
                RuntimeDllCallbackReason::ProcessDetach => DLL_PROCESS_DETACH,
            },
            DLL_PROCESS_STATIC_RESERVED,
        ]
    );
}
