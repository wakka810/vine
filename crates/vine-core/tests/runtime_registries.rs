use vine_core::{
    DYNAMIC_EXPORT_ENDPOINT_NAMESPACE, GuestAddress, LOGICAL_ENCODED_POINTER_NAMESPACE,
    ProcessAddressSpaceLayout, RuntimeBootstrap, RuntimeClockState, RuntimeDynamicEndpoint,
    RuntimeProcessId, RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeState, RuntimeThreadId,
    RuntimeTlsValue, X86_STARTUP_INFO_W_BYTE_LENGTH,
};

#[test]
fn registry_plans_preserve_cross_subsystem_state_and_serialization_shape() {
    let mut runtime = running_runtime();
    let endpoint = RuntimeDynamicEndpoint {
        namespace: DYNAMIC_EXPORT_ENDPOINT_NAMESPACE.to_owned(),
        id: 2,
        address: GuestAddress(0x7f02_0010),
        module: "kernel32.dll".to_owned(),
        name: "FlsGetValue".to_owned(),
        lifetime: "process-runtime".to_owned(),
    };
    runtime
        .register_dynamic_endpoint(endpoint.clone())
        .expect("register controlled endpoint");

    let before_duplicate = runtime.clone();
    assert_eq!(
        runtime
            .register_dynamic_endpoint(endpoint.clone())
            .expect_err("duplicate endpoint must be mutation-free")
            .code,
        "dynamic-endpoint-duplicate"
    );
    assert_eq!(runtime, before_duplicate);

    let tls_index = runtime.allocate_tls_index().expect("allocate TLS index");
    runtime
        .set_tls_value(tls_index, endpoint.address.0)
        .expect("store endpoint identity in thread TLS");
    assert_eq!(
        runtime.tls_value(tls_index),
        Ok(&RuntimeTlsValue::DynamicEndpoint {
            endpoint_id: endpoint.id,
            address: endpoint.address,
        })
    );

    let first = runtime
        .encode_pointer(endpoint.address.0)
        .expect("encode endpoint pointer")
        .clone();
    let before_reuse = runtime.clone();
    assert_eq!(
        runtime
            .encode_pointer(endpoint.address.0)
            .expect("reuse encoded identity"),
        &first
    );
    assert_eq!(runtime, before_reuse);

    let before_invalid_decode = runtime.clone();
    assert_eq!(
        runtime
            .decode_pointer(0xec02_0001)
            .expect_err("foreign token prefix must be rejected")
            .code,
        "encoded-pointer-token-invalid"
    );
    assert_eq!(runtime, before_invalid_decode);

    let serialized = serde_json::to_value(&runtime).expect("serialize registry compatibility view");
    assert_eq!(
        serialized["registries"]["dynamicEndpoints"]["2"]["name"],
        serde_json::json!("FlsGetValue")
    );
    assert_eq!(
        serialized["registries"]["encodedPointers"][first.token.to_string()]["source"],
        serde_json::json!(endpoint.address.0)
    );
    assert_eq!(
        serialized["encodedPointerNamespace"],
        serde_json::json!(LOGICAL_ENCODED_POINTER_NAMESPACE)
    );
    assert_eq!(
        serialized["encodedPointerTokenPrefix"],
        serde_json::json!(0xec01_0000_u32)
    );
    assert_eq!(
        serialized["encodedPointerSources"][endpoint.address.0.to_string()],
        serde_json::json!(first.token)
    );
    assert_eq!(serialized["nextEncodedPointerId"], serde_json::json!(2));
    assert!(
        serialized["registries"]
            .get("encodedPointerSources")
            .is_none()
    );
    assert!(
        serialized["registries"]
            .get("nextEncodedPointerId")
            .is_none()
    );
}

fn running_runtime() -> RuntimeState {
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(0x1000),
        initial_thread_id: RuntimeThreadId(0x2000),
        initial_last_error: 87,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "runtime-registry-test".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        process_image_path_a: RuntimeProcessImagePathA {
            source_authority: "runtime-registry-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
            source_authority: "runtime-registry-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .expect("controlled runtime");
    runtime
        .start_initial_thread()
        .expect("start initial thread");
    runtime
}
