use vine_core::{
    LOGICAL_ENCODED_POINTER_NAMESPACE, LOGICAL_LOADED_MODULE_NAMESPACE,
    LOGICAL_PRIVATE_HEAP_NAMESPACE, ProcessAddressSpaceLayout, RUNTIME_PROCESS_OBJECT_NAMESPACE,
    RuntimeBootstrap, RuntimeClockState, RuntimeHandle, RuntimeModule, RuntimeObjectKey,
    RuntimeObjectKind, RuntimeProcessId, RuntimeStartupInfo, RuntimeState, RuntimeThreadId,
    RuntimeWaitState, X86_STARTUP_INFO_W_BYTE_LENGTH,
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
    assert_eq!(process_object.namespace, RUNTIME_PROCESS_OBJECT_NAMESPACE);
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
    assert_eq!(
        runtime
            .object_for_handle(thread_handle, RuntimeObjectKind::Thread)
            .expect_err("closed handle is stale")
            .code,
        "runtime-handle-stale"
    );
    assert_eq!(
        runtime
            .close_handle(thread_handle)
            .expect_err("double close is stale")
            .code,
        "runtime-handle-stale"
    );
    assert_eq!(
        runtime
            .object_for_handle(process_handle, RuntimeObjectKind::Thread)
            .expect_err("handle types cannot alias")
            .code,
        "runtime-handle-type-mismatch"
    );

    let reused_thread_handle = runtime
        .open_current_thread_handle()
        .expect("reuse closed thread slot");
    assert_eq!(reused_thread_handle, RuntimeHandle(0x5402_0001));
    assert_eq!(reused_thread_handle.slot(), thread_handle.slot());
    assert_eq!(reused_thread_handle.generation(), 2);

    let heap = runtime
        .allocate_private_heap(0, 0x1000, 0)
        .expect("logical private heap");
    assert_eq!(heap.namespace, LOGICAL_PRIVATE_HEAP_NAMESPACE);
    let heap_handle = heap.handle;
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
    assert_eq!(
        runtime
            .close_handle(RuntimeHandle(heap_handle))
            .expect_err("heap lifetime belongs to the heap subsystem")
            .code,
        "runtime-handle-close-policy"
    );

    let module_handle = 0x4d01_0001;
    runtime
        .load_module(RuntimeModule {
            owner: "runtime-object-test".to_owned(),
            variant_id: "controlled-module".to_owned(),
            namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
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
    assert_eq!(
        runtime
            .object_wait_state(RuntimeHandle(module_handle), RuntimeObjectKind::Module)
            .expect_err("module handle is not waitable")
            .code,
        "runtime-object-not-waitable"
    );

    runtime
        .cancel_current_thread(0xc000_013a)
        .expect("terminate controlled thread");
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

    let mut exited = controlled_runtime();
    let exited_process = exited.process_handle().expect("exit process handle");
    let exited_thread = exited.current_thread_handle().expect("exit thread handle");
    exited
        .terminate_current_process(7)
        .expect("terminate controlled process");
    assert_eq!(
        exited
            .object_wait_state(exited_thread, RuntimeObjectKind::Thread)
            .expect("process exit signals thread"),
        RuntimeWaitState::Signaled { exit_code: 7 }
    );
    assert_eq!(
        exited
            .object_wait_state(exited_process, RuntimeObjectKind::Process)
            .expect("process exit signals process"),
        RuntimeWaitState::Signaled { exit_code: 7 }
    );
}

fn controlled_runtime() -> RuntimeState {
    RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: RuntimeThreadId(THREAD_ID),
        initial_last_error: 0,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "runtime-object-test".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .expect("controlled runtime")
}
