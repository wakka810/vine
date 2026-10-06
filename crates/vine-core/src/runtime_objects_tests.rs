use crate::{
    Runtime, RuntimeBootstrap, RuntimeClockState, RuntimeHandle, RuntimeModule, RuntimeModuleClass,
    RuntimeModuleLoadState, RuntimeModuleReferenceOwner, RuntimeObjectKey, RuntimeObjectKind,
    RuntimeProcessId, RuntimeStartupInfo, RuntimeThreadId, RuntimeWaitState,
    X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;

#[test]
fn typed_runtime_handles_preserve_object_lifetime_and_reject_stale_generations() {
    let mut runtime = controlled_runtime();
    let process_handle = runtime.process_handle().expect("initial process handle");
    let thread_handle = runtime
        .current_thread_handle()
        .expect("initial thread handle");
    assert_eq!(process_handle, RuntimeHandle(0x5001_0001));
    assert_eq!(thread_handle, RuntimeHandle(0x5401_0001));
    let process_object = runtime
        .object_for_handle(process_handle, RuntimeObjectKind::Process)
        .expect("process object");
    assert_eq!(
        process_object.key,
        RuntimeObjectKey::Process {
            process_id: PROCESS_ID
        }
    );
    assert_eq!(
        runtime
            .object_wait_state(process_handle, RuntimeObjectKind::Process)
            .expect("process wait state"),
        RuntimeWaitState::Nonsignaled
    );
    assert_eq!(
        runtime
            .object_wait_state(thread_handle, RuntimeObjectKind::Thread)
            .expect("thread wait state"),
        RuntimeWaitState::Nonsignaled
    );

    let second_thread_handle = runtime
        .open_current_thread_handle()
        .expect("second thread handle");
    assert_eq!(second_thread_handle, RuntimeHandle(0x5401_0002));
    assert_eq!(
        runtime
            .object_for_handle(thread_handle, RuntimeObjectKind::Thread)
            .expect("thread object")
            .open_handles,
        2
    );

    runtime
        .close_handle(thread_handle)
        .expect("close first handle");
    runtime
        .object_for_handle(thread_handle, RuntimeObjectKind::Thread)
        .expect_err("closed handle is stale");
    runtime
        .close_handle(thread_handle)
        .expect_err("double close is stale");
    runtime
        .object_for_handle(process_handle, RuntimeObjectKind::Thread)
        .expect_err("handle types cannot alias");

    let reused_thread_handle = runtime
        .open_current_thread_handle()
        .expect("reuse closed thread slot");
    assert_eq!(reused_thread_handle, RuntimeHandle(0x5402_0001));
    assert_eq!(reused_thread_handle.slot(), thread_handle.slot());
    assert_eq!(reused_thread_handle.generation(), 2);

    let heap_handle = runtime
        .allocate_private_heap(0x1000, 0)
        .expect("logical private heap");
    assert_eq!(heap_handle, 0x6801_0001);
    assert_eq!(
        runtime
            .object_for_handle(RuntimeHandle(heap_handle), RuntimeObjectKind::PrivateHeap)
            .expect("heap object")
            .key,
        RuntimeObjectKey::PrivateHeap {
            handle: heap_handle
        }
    );
    runtime
        .close_handle(RuntimeHandle(heap_handle))
        .expect_err("heap lifetime belongs to the heap subsystem");

    let module_handle = 0x4d01_0001;
    runtime
        .load_module(RuntimeModule {
            handle: module_handle,
            requested_name: "KERNEL32.DLL".to_owned(),
            canonical_name: "kernel32.dll".to_owned(),
            runtime_system_module: true,
        })
        .expect("logical module");
    assert_eq!(
        runtime
            .object_for_handle(RuntimeHandle(module_handle), RuntimeObjectKind::Module)
            .expect("module object")
            .key,
        RuntimeObjectKey::Module {
            handle: module_handle
        }
    );
    let module_graph_node = runtime
        .module_graph()
        .module_by_name("KERNEL32.dll")
        .expect("module graph registration");
    assert_eq!(module_graph_node.handle, module_handle);
    assert_eq!(
        module_graph_node.class,
        RuntimeModuleClass::RuntimeSystemModule
    );
    assert_eq!(module_graph_node.reference_count(), 1);
    let system_module_id = module_graph_node.id;
    let main_handle = 0x4d01_0002;
    runtime
        .register_main_module(RuntimeModule {
            handle: main_handle,
            requested_name: "CONTROLLED.EXE".to_owned(),
            canonical_name: "controlled.exe".to_owned(),
            runtime_system_module: false,
        })
        .expect("register main executable");
    let main_graph_node = runtime
        .module_graph()
        .module_by_name("controlled.exe")
        .expect("main graph node");
    assert_eq!(
        runtime.module_graph().main_module(),
        Some(main_graph_node.id)
    );
    assert_eq!(main_graph_node.class, RuntimeModuleClass::MainExecutable);
    assert_eq!(main_graph_node.reference_count(), 1);
    let main_module_id = main_graph_node.id;
    runtime
        .add_module_dependency(main_handle, module_handle)
        .expect("main imports runtime system module");
    assert_eq!(
        runtime
            .module_graph()
            .module(system_module_id)
            .expect("system module after dependency")
            .reference_count_for(&RuntimeModuleReferenceOwner::StaticImport {
                requester: main_module_id,
            }),
        1
    );
    assert_eq!(
        runtime
            .add_current_thread_module_reference(module_handle)
            .expect("current thread dynamic module reference"),
        1
    );
    assert_eq!(
        runtime
            .release_current_thread_module_reference(module_handle)
            .expect("release current thread module reference"),
        0
    );
    runtime
        .mark_module_ready(module_handle)
        .expect("system module ready");
    runtime
        .mark_module_ready(main_handle)
        .expect("main module ready");
    assert_eq!(
        runtime.attach_modules().expect("dependency-first attach"),
        vec![system_module_id, main_module_id]
    );
    assert_eq!(
        runtime
            .module_graph()
            .module(system_module_id)
            .expect("attached system module")
            .state,
        RuntimeModuleLoadState::ProcessAttached { sequence: 1 }
    );
    assert_eq!(
        runtime.detach_modules().expect("strict reverse detach"),
        vec![main_module_id, system_module_id]
    );
    assert_eq!(
        runtime
            .module_graph()
            .module(system_module_id)
            .expect("detached system module")
            .state,
        RuntimeModuleLoadState::ProcessDetached { sequence: 2 }
    );
    runtime
        .load_module(RuntimeModule {
            handle: 0x6801_0002,
            requested_name: "BROKEN.DLL".to_owned(),
            canonical_name: "broken.dll".to_owned(),
            runtime_system_module: false,
        })
        .expect_err("a heap-shaped handle cannot publish a module");
    assert!(runtime.module_graph().module_by_name("broken.dll").is_err());
    assert_eq!(runtime.module_graph().nodes().len(), 2);
    assert_eq!(runtime.module_graph().main_module(), Some(main_module_id));
    assert_eq!(
        runtime.module(module_handle).unwrap().canonical_name,
        "kernel32.dll"
    );
    assert_eq!(
        runtime.module(main_handle).unwrap().canonical_name,
        "controlled.exe"
    );
    assert!(
        runtime
            .object_for_handle(RuntimeHandle(heap_handle), RuntimeObjectKind::PrivateHeap)
            .is_ok()
    );
    runtime
        .load_module(RuntimeModule {
            handle: 0x4d01_0003,
            requested_name: "BROKEN.DLL".to_owned(),
            canonical_name: "broken.dll".to_owned(),
            runtime_system_module: false,
        })
        .expect("retry with a valid module handle");
    let registered = runtime.module_graph().module_by_name("broken.dll").unwrap();
    assert_eq!(registered.id.0, main_module_id.0 + 1);
    assert_eq!(registered.handle, 0x4d01_0003);
    assert_eq!(registered.state, RuntimeModuleLoadState::Registered);
    assert_eq!(
        runtime
            .object_for_handle(RuntimeHandle(registered.handle), RuntimeObjectKind::Module)
            .unwrap()
            .key,
        RuntimeObjectKey::Module {
            handle: registered.handle
        }
    );
    runtime
        .object_wait_state(RuntimeHandle(module_handle), RuntimeObjectKind::Module)
        .expect_err("module handle is not waitable");

    runtime
        .request_current_thread_exit(0xc000_013a)
        .expect("request controlled thread exit");
    runtime
        .complete_current_thread_exit()
        .expect("complete controlled thread exit");
    for handle in [second_thread_handle, reused_thread_handle] {
        assert_eq!(
            runtime
                .object_wait_state(handle, RuntimeObjectKind::Thread)
                .expect("terminated thread wait state"),
            RuntimeWaitState::Signaled {
                exit_code: 0xc000_013a
            }
        );
        runtime.close_handle(handle).expect("close thread handle");
    }
    let reopened_after_termination = runtime
        .open_current_thread_handle()
        .expect("reopen terminated thread object");
    assert_eq!(reopened_after_termination, RuntimeHandle(0x5403_0001));
    assert_eq!(
        runtime
            .object_wait_state(reopened_after_termination, RuntimeObjectKind::Thread)
            .expect("reopened thread preserves signal state"),
        RuntimeWaitState::Signaled {
            exit_code: 0xc000_013a
        }
    );
    assert_eq!(
        runtime
            .object_wait_state(process_handle, RuntimeObjectKind::Process)
            .expect("thread cancellation does not signal process"),
        RuntimeWaitState::Nonsignaled
    );
}

fn controlled_runtime() -> Runtime {
    let mut runtime = Runtime::new(
        RuntimeBootstrap {
            process_id: RuntimeProcessId(PROCESS_ID),
            initial_thread_id: RuntimeThreadId(THREAD_ID),
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
        crate::GuestMemory::new(crate::MemoryLimits::default()).unwrap(),
    )
    .expect("controlled runtime");
    runtime
        .start_initial_thread()
        .expect("start controlled initial thread");
    runtime
}
