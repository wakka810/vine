use crate::runtime::X86_STARTUP_INFO_W_BYTE_LENGTH;
use crate::{
    Runtime, RuntimeBootstrap, RuntimeClockState, RuntimeProcessId, RuntimeProcessImagePathA,
    RuntimeStartupInfo, RuntimeThreadId,
};

#[test]
fn pointer_encoding_is_reversible_without_changing_thread_values_or_last_error() {
    let mut runtime = running_runtime();
    let index = runtime.allocate_tls_index().unwrap();
    runtime.set_tls_value(index, 42).unwrap();
    for pointer in [0, 1, 0x7f02_0010, u32::MAX] {
        let encoded = runtime.encode_pointer(pointer);
        assert_eq!(runtime.encode_pointer(pointer), encoded);
        assert_eq!(runtime.decode_pointer(encoded), pointer);
        assert_ne!(runtime.decode_pointer(encoded ^ 1), pointer);
    }
    assert_eq!(runtime.tls_value_or_unset(index), Ok(Some(42)));
    assert_eq!(runtime.current_last_error().unwrap(), 87);
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
