use crate::{
    GuestAddress, Runtime, RuntimeBootstrap, RuntimeClockState, RuntimeDynamicEndpoint,
    RuntimeProcessId, RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeThreadId,
    X86_STARTUP_INFO_W_BYTE_LENGTH,
};

#[test]
fn dynamic_endpoints_and_pointer_permutation_preserve_runtime_state() {
    let mut runtime = running_runtime();
    let endpoint = RuntimeDynamicEndpoint {
        id: 2,
        address: GuestAddress(0x7f02_0010),
        module: "kernel32.dll".to_owned(),
        name: "FlsGetValue".to_owned(),
        ordinal: Some(18),
    };
    runtime
        .register_dynamic_endpoint(endpoint.clone())
        .expect("register controlled endpoint");

    runtime
        .register_dynamic_endpoint(endpoint.clone())
        .expect_err("duplicate endpoint must be rejected");
    let second = RuntimeDynamicEndpoint {
        id: 3,
        address: GuestAddress(0x7f02_0020),
        module: "kernel32.dll".to_owned(),
        name: "FlsAlloc".to_owned(),
        ordinal: None,
    };
    assert_eq!(
        runtime.register_dynamic_endpoint(second.clone()).unwrap(),
        &second
    );

    let tls_index = runtime.allocate_tls_index().expect("allocate TLS index");
    runtime
        .set_tls_value(tls_index, endpoint.address.0)
        .expect("store endpoint identity in thread TLS");
    assert_eq!(runtime.tls_value(tls_index), Ok(endpoint.address.0));

    let clock_before_encoding = runtime.clock().clone();
    let first = runtime.encode_pointer(endpoint.address.0);
    assert_eq!(runtime.encode_pointer(endpoint.address.0), first);
    assert_eq!(runtime.clock(), &clock_before_encoding);
    assert_eq!(runtime.current_last_error().unwrap(), 87);
    assert_eq!(runtime.tls_value(tls_index), Ok(endpoint.address.0));

    // DecodePointer is the inverse process transform, including for tokens not returned by EncodePointer.
    assert_ne!(runtime.decode_pointer(first ^ 1), endpoint.address.0);
    assert_eq!(runtime.decode_pointer(first), endpoint.address.0);
    assert_eq!(runtime.clock(), &clock_before_encoding);
    assert_eq!(runtime.current_last_error().unwrap(), 87);
    assert_eq!(runtime.tls_value(tls_index), Ok(endpoint.address.0));
}

fn running_runtime() -> Runtime {
    let mut runtime = Runtime::new(
        RuntimeBootstrap {
            process_id: RuntimeProcessId(0x1000),
            initial_thread_id: RuntimeThreadId(0x2000),
            initial_last_error: 87,
            clock: RuntimeClockState {
                wall_clock_file_time: 134_282_016_000_000_000,
                tick_count: 3_600_000,
                performance_counter: 12_345_678_901,
                performance_frequency: 10_000_000,
            },
            startup_info: RuntimeStartupInfo {
                bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
            },
            process_image_path_a: RuntimeProcessImagePathA {
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
        .expect("start initial thread");
    runtime
}
