use vine_core::{
    LOGICAL_ENCODED_POINTER_NAMESPACE, LOGICAL_LOADED_MODULE_NAMESPACE, ProcessAddressSpaceLayout,
    RuntimeBootstrap, RuntimeClockState, RuntimeHandle, RuntimeModule, RuntimeModuleClass,
    RuntimeModuleLoadState, RuntimeModuleReferenceOwner, RuntimeObjectKind, RuntimeProcessId,
    RuntimeStartupInfo, RuntimeState, RuntimeThreadId, X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const KERNEL32_HANDLE: u32 = 0x4d01_0001;
const MAIN_HANDLE: u32 = 0x4d01_0002;

#[test]
fn dynamic_runtime_system_load_owns_identity_reference_and_attached_state() {
    let mut runtime = attached_runtime();
    let object_count = runtime.objects().objects().len();

    let first = runtime
        .load_current_thread_runtime_system_module(
            "D3DX9_43.DLL".to_owned(),
            "d3dx9_43.dll".to_owned(),
        )
        .expect("first runtime-system load");
    assert_eq!(first.handle, 0x4d01_0003);
    assert!(first.newly_registered);
    assert_eq!(first.thread_reference_count, 1);
    assert_eq!(first.attach_sequence, 3);
    assert_eq!(runtime.objects().objects().len(), object_count + 1);
    assert_eq!(
        runtime
            .object_for_handle(RuntimeHandle(first.handle), RuntimeObjectKind::Module)
            .unwrap()
            .key,
        vine_core::RuntimeObjectKey::Module {
            handle: first.handle
        }
    );
    let node = runtime
        .module_graph()
        .module(first.module_id)
        .expect("loaded graph node");
    assert_eq!(node.class, RuntimeModuleClass::RuntimeSystemModule);
    assert_eq!(
        node.state,
        RuntimeModuleLoadState::ProcessAttached { sequence: 3 }
    );
    assert_eq!(
        node.reference_count_for(&RuntimeModuleReferenceOwner::RuntimeService),
        1
    );
    assert_eq!(
        node.reference_count_for(&RuntimeModuleReferenceOwner::DynamicLoad { thread_id: 8192 }),
        1
    );

    let second = runtime
        .load_current_thread_runtime_system_module(
            "d3dx9_43.dll".to_owned(),
            "d3dx9_43.dll".to_owned(),
        )
        .expect("repeated runtime-system load");
    assert_eq!(second.handle, first.handle);
    assert_eq!(second.module_id, first.module_id);
    assert!(!second.newly_registered);
    assert_eq!(second.thread_reference_count, 2);
    assert_eq!(second.attach_sequence, 3);
    assert_eq!(runtime.objects().objects().len(), object_count + 1);
    assert_eq!(
        runtime
            .module_graph()
            .module(first.module_id)
            .unwrap()
            .reference_count_for(&RuntimeModuleReferenceOwner::DynamicLoad { thread_id: 8192 }),
        2
    );
}

fn attached_runtime() -> RuntimeState {
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(4096),
        initial_thread_id: RuntimeThreadId(8192),
        initial_last_error: 0,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "runtime-system-module-load-test".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        process_image_path_a: vine_core::RuntimeProcessImagePathA {
            source_authority: "runtime-system-module-load-test".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
            source_authority: "runtime-system-module-load-test".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .unwrap();
    runtime.start_initial_thread().unwrap();
    runtime
        .load_module(module(KERNEL32_HANDLE, "kernel32.dll", true))
        .unwrap();
    runtime
        .register_main_module(module(MAIN_HANDLE, "fixture.exe", false))
        .unwrap();
    runtime
        .add_module_dependency(MAIN_HANDLE, KERNEL32_HANDLE)
        .unwrap();
    runtime.mark_module_ready(KERNEL32_HANDLE).unwrap();
    runtime.mark_module_ready(MAIN_HANDLE).unwrap();
    assert_eq!(runtime.attach_modules().unwrap().len(), 2);
    runtime
}

fn module(handle: u32, name: &str, runtime_system_module: bool) -> RuntimeModule {
    RuntimeModule {
        owner: "runtime-system-module-load-test".to_owned(),
        variant_id: format!("controlled-{name}"),
        namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
        handle,
        requested_name: name.to_owned(),
        canonical_name: name.to_owned(),
        runtime_system_module,
    }
}
