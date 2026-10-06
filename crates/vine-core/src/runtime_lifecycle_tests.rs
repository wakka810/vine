use crate::{
    GuestAddress, GuestMemory, GuestRange, MemoryLimits, MemoryOwnerKind, MemoryRegionKind,
    PageProtection, ProcessAddressAllocationClassification, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationReleasePolicy, Runtime, RuntimeBootstrap, RuntimeClockState,
    RuntimeObjectKind, RuntimeProcessId, RuntimeProcessLifecycleState, RuntimeStartupInfo,
    RuntimeThreadId, RuntimeThreadLifecycleState, RuntimeWaitState, SchedulerThreadState,
    X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;

#[test]
fn preloaded_image_ranges_are_reserved_before_virtual_memory_search() {
    for sealed in [false, true] {
        let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
        let image_bases = [GuestAddress(0x7ffd_0000), GuestAddress(0x7ffe_0000)];
        let image_bytes = [0x4d, 0x5a, 0x90, 0x00, 0x5a, 0xa5];
        let mut image_owners = Vec::new();
        for base in image_bases {
            let owner = memory
                .create_owner(MemoryOwnerKind::Image {
                    source_sha256: "0".repeat(64),
                })
                .unwrap();
            let range = memory
                .reserve_fixed(owner, MemoryRegionKind::Image, base, 4096)
                .unwrap();
            memory
                .commit(
                    owner,
                    range,
                    PageProtection {
                        read: true,
                        write: false,
                        execute: true,
                    },
                )
                .unwrap();
            memory
                .write_initialization(owner, base, &image_bytes)
                .unwrap();
            if sealed {
                memory.seal_owner(owner).unwrap();
            }
            image_owners.push(owner);
        }
        let before_initialization = memory.stats();
        let mut runtime = Runtime::new(controlled_bootstrap(), memory).unwrap();
        assert_eq!(runtime.memory().stats(), before_initialization);
        let images = runtime
            .process_address_space_allocator()
            .records()
            .filter(|record| record.purpose == ProcessAddressAllocationPurpose::GuestImage)
            .collect::<Vec<_>>();
        assert_eq!(images.len(), image_bases.len());
        for (image, base) in images.iter().zip(image_bases) {
            assert_eq!(image.range.base, base);
            assert_eq!(image.range.byte_length, 4096);
            assert_eq!(
                image.classification,
                ProcessAddressAllocationClassification::Image
            );
            assert_eq!(
                image.release_policy,
                ProcessAddressAllocationReleasePolicy::Permanent
            );
        }
        runtime.start_initial_thread().unwrap();
        let last_error = runtime.current_last_error().unwrap();
        let reserved = runtime.reserve_virtual_memory(4096).unwrap().unwrap();
        assert_eq!(reserved, GuestAddress(0x7ffc_0000));
        let writable = PageProtection {
            read: true,
            write: true,
            execute: false,
        };
        assert_eq!(
            runtime
                .commit_virtual_memory(reserved, 4096, writable)
                .unwrap(),
            Ok(reserved)
        );
        runtime.memory_mut().write(reserved, &[1, 2, 3, 4]).unwrap();
        if sealed {
            let generation = runtime
                .memory_mut()
                .pending_reflection()
                .unwrap()
                .invalidation
                .generation;
            runtime.memory_mut().acknowledge_delta(generation).unwrap();
        }
        for (owner, base) in image_owners.into_iter().zip(image_bases) {
            assert_eq!(runtime.memory().owner_is_sealed(owner).unwrap(), sealed);
            let bytes = if sealed {
                runtime.memory().read(base, image_bytes.len()).unwrap()
            } else {
                runtime
                    .memory()
                    .read_initialization(owner, base, image_bytes.len())
                    .unwrap()
            };
            assert_eq!(bytes, image_bytes);
        }
        assert_eq!(runtime.current_last_error().unwrap(), last_error);
    }
}

#[test]
fn preloaded_image_conflicting_with_a_runtime_arena_rejects_initialization() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let owner = memory
        .create_owner(MemoryOwnerKind::Image {
            source_sha256: "0".repeat(64),
        })
        .unwrap();
    memory
        .reserve_fixed(
            owner,
            MemoryRegionKind::Image,
            GuestAddress(0x7f00_0000),
            4096,
        )
        .unwrap();
    Runtime::new(controlled_bootstrap(), memory)
        .expect_err("a live image cannot share a runtime control arena");
}

#[test]
fn preloaded_image_region_requires_an_image_owner() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let owner = memory.create_owner(MemoryOwnerKind::Anonymous).unwrap();
    memory
        .reserve_fixed(
            owner,
            MemoryRegionKind::Image,
            GuestAddress(0x7ffe_0000),
            4096,
        )
        .unwrap();
    Runtime::new(controlled_bootstrap(), memory)
        .expect_err("an image region with a different owner kind must fault");
}

#[test]
fn moved_memory_remains_authoritative_across_environment_rejection_and_retry() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
    let base = GuestAddress(0x2200_0000);
    memory
        .reserve_fixed(owner, MemoryRegionKind::Anonymous, base, 4096)
        .unwrap();
    memory
        .commit(
            owner,
            GuestRange::new(base, 4096).unwrap(),
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .unwrap();
    memory
        .write_initialization(owner, base, &[1, 2, 3, 4])
        .unwrap();
    memory.seal_owner(owner).unwrap();
    let generation = memory.pending_reflection().unwrap().invalidation.generation;
    memory.acknowledge_delta(generation).unwrap();
    let before_move = memory.stats();

    let mut runtime = Runtime::new(controlled_bootstrap(), memory).unwrap();
    assert_eq!(runtime.memory().stats(), before_move);
    assert_eq!(runtime.memory().read(base, 4).unwrap(), [1, 2, 3, 4]);
    assert!(runtime.install_process_environment("bad\0command").is_err());
    assert_eq!(runtime.memory().stats(), before_move);
    assert_eq!(runtime.memory().read(base, 4).unwrap(), [1, 2, 3, 4]);
    assert_eq!(runtime.current_last_error().unwrap(), 87);

    let environment = runtime.install_process_environment("fixture.exe").unwrap();
    let generation = runtime.memory_mut().pending_delta().unwrap().generation;
    runtime.memory_mut().acknowledge_delta(generation).unwrap();
    assert_eq!(runtime.memory().read(base, 4).unwrap(), [1, 2, 3, 4]);
    assert_eq!(
        runtime
            .memory()
            .read(GuestAddress(environment.teb_address.0 + 0x18), 4)
            .unwrap(),
        environment.teb_address.0.to_le_bytes()
    );
    runtime.memory_mut().write(base, &[5, 6, 7, 8]).unwrap();
    let generation = runtime.memory_mut().pending_delta().unwrap().generation;
    runtime.memory_mut().acknowledge_delta(generation).unwrap();
    assert_eq!(runtime.memory().read(base, 4).unwrap(), [5, 6, 7, 8]);
}

#[test]
fn repeated_environment_install_preserves_guest_bytes_and_process_authority() {
    let mut runtime = initialized_runtime();
    runtime.install_process_environment("日本語.exe").unwrap();
    runtime.acquire_environment_strings_w().unwrap();
    let process_environment = runtime.process().environment().unwrap().clone();
    let allocations = runtime
        .process_address_space_allocator()
        .records()
        .cloned()
        .collect::<Vec<_>>();
    let reflection = runtime.memory_mut().pending_reflection().unwrap();
    let bytes = reflection
        .pages
        .iter()
        .map(|page| (page.address, page.bytes.clone()))
        .collect::<Vec<_>>();
    runtime
        .memory_mut()
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();

    for pending in [false, true] {
        let delta = if pending {
            runtime.memory_mut().write(bytes[0].0, &bytes[0].1).unwrap();
            Some(runtime.memory_mut().pending_delta().unwrap())
        } else {
            None
        };
        let stats = runtime.memory().stats();
        for command_line in ["replacement.exe", "invalid\0replacement.exe"] {
            runtime
                .install_process_environment(command_line)
                .expect_err("an installed environment cannot be replaced");
            assert_eq!(runtime.memory().stats(), stats);
            assert_eq!(
                runtime.process().environment().unwrap(),
                &process_environment
            );
            assert_eq!(
                runtime
                    .process_address_space_allocator()
                    .records()
                    .cloned()
                    .collect::<Vec<_>>(),
                allocations
            );
            assert_eq!(
                runtime.process().lifecycle(),
                RuntimeProcessLifecycleState::Bootstrapping
            );
            assert_eq!(runtime.current_last_error().unwrap(), 87);
            if let Some(expected) = &delta {
                assert_eq!(runtime.memory_mut().pending_delta().unwrap(), *expected);
            } else {
                assert!(!runtime.memory().has_pending_delta());
            }
        }
        if let Some(delta) = delta {
            runtime
                .memory_mut()
                .acknowledge_delta(delta.generation)
                .unwrap();
        }
        for (address, expected) in &bytes {
            assert_eq!(
                runtime.memory().read(*address, expected.len()).unwrap(),
                *expected
            );
        }
    }
}

#[test]
fn thread_exit_cancels_pending_work_before_waitable_completion() {
    let mut runtime = initialized_runtime();
    let process_handle = runtime.process_handle().expect("bootstrap process handle");
    let thread_handle = runtime
        .current_thread_handle()
        .expect("bootstrap thread handle");
    runtime
        .complete_current_thread_exit()
        .expect_err("thread cannot exit before start and request");

    assert_eq!(
        runtime.process().lifecycle(),
        RuntimeProcessLifecycleState::Bootstrapping
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().lifecycle,
        RuntimeThreadLifecycleState::Initialized
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Initialized
    );

    runtime
        .start_initial_thread()
        .expect("start initial runtime thread");
    runtime
        .start_initial_thread()
        .expect_err("initial thread cannot start twice");

    assert_eq!(
        runtime.process().lifecycle(),
        RuntimeProcessLifecycleState::Running
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().lifecycle,
        RuntimeThreadLifecycleState::Running
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );

    let callback = runtime
        .push_callback_frame(GuestAddress(0x1000_1000), GuestAddress(0x7f01_0000))
        .expect("active callback before thread exit");
    let pending_request = runtime
        .begin_host_request("browser.storage.flush")
        .expect("pending host request before thread exit");
    runtime
        .complete_host_request(pending_request.request_id + 1)
        .expect_err("unknown completion must preserve scheduler state");
    assert_eq!(runtime.scheduler().host_requests().len(), 1);
    assert_eq!(
        runtime
            .scheduler()
            .host_requests()
            .get(&pending_request.request_id),
        Some(&pending_request)
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: pending_request.request_id
        }
    );
    assert_eq!(
        runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .as_slice(),
        &[callback]
    );
    assert!(runtime.scheduler().events().is_empty());
    runtime
        .request_current_thread_exit(7)
        .expect("request thread exit");
    assert!(runtime.scheduler().host_requests().is_empty());
    assert!(
        runtime
            .current_thread_state()
            .unwrap()
            .callback_frames
            .is_empty()
    );
    runtime
        .request_current_thread_exit(7)
        .expect_err("duplicate thread exit request is rejected");

    assert_eq!(
        runtime.process().lifecycle(),
        RuntimeProcessLifecycleState::Running
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().lifecycle,
        RuntimeThreadLifecycleState::ExitRequested { exit_code: 7 }
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Terminating { exit_code: 7 }
    );
    assert_eq!(
        runtime
            .object_wait_state(process_handle, RuntimeObjectKind::Process)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );
    assert_eq!(
        runtime
            .object_wait_state(thread_handle, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );

    runtime
        .complete_current_thread_exit()
        .expect("complete thread exit");
    runtime
        .complete_current_thread_exit()
        .expect_err("thread exit cannot complete twice");

    assert_eq!(
        runtime.process().lifecycle(),
        RuntimeProcessLifecycleState::Running
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().lifecycle,
        RuntimeThreadLifecycleState::Exited { exit_code: 7 }
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Terminated { exit_code: 7 }
    );
    assert_eq!(
        runtime
            .object_wait_state(thread_handle, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Signaled { exit_code: 7 }
    );
    assert_eq!(
        runtime
            .object_wait_state(process_handle, RuntimeObjectKind::Process)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );
}

#[test]
fn process_image_path_bootstrap_rejects_invalid_ansi_bytes() {
    for mutate in [
        |bootstrap: &mut RuntimeBootstrap| {
            bootstrap.process_image_path_a.bytes_with_nul = b"C:\\Vine\\fixture.exe".to_vec()
        },
        |bootstrap: &mut RuntimeBootstrap| {
            bootstrap.process_image_path_a.bytes_with_nul = b"C:\\Vine\0fixture.exe\0".to_vec()
        },
    ] {
        let mut bootstrap = controlled_bootstrap();
        mutate(&mut bootstrap);
        Runtime::new(
            bootstrap,
            crate::GuestMemory::new(crate::MemoryLimits::default()).unwrap(),
        )
        .expect_err("invalid process image path must stop bootstrap");
    }
}

#[test]
fn current_directory_bootstrap_rejects_invalid_windows_932_paths() {
    for mutate in [
        (|bootstrap: &mut RuntimeBootstrap| {
            bootstrap.initial_current_directory_a.bytes_with_nul = b".\0".to_vec()
        }) as fn(&mut RuntimeBootstrap),
        |bootstrap: &mut RuntimeBootstrap| {
            bootstrap.initial_current_directory_a.bytes_with_nul = b"C:\\Vine\0other\0".to_vec()
        },
        |bootstrap: &mut RuntimeBootstrap| {
            bootstrap.initial_current_directory_a.bytes_with_nul = b"C:\\Vine\\\x82\0".to_vec()
        },
        |bootstrap: &mut RuntimeBootstrap| {
            bootstrap.initial_current_directory_a.bytes_with_nul = b"C:\\Vine\\\x82\x01\0".to_vec()
        },
    ] {
        let mut bootstrap = controlled_bootstrap();
        mutate(&mut bootstrap);
        Runtime::new(
            bootstrap,
            crate::GuestMemory::new(crate::MemoryLimits::default()).unwrap(),
        )
        .expect_err("invalid current-directory bytes must stop bootstrap");
    }
}

#[test]
fn clock_bootstrap_rejects_invalid_performance_clock_values() {
    for (counter, frequency) in [
        (12_345_678_901, 0),
        (12_345_678_901, i64::MAX as u64 + 1),
        (i64::MAX as u64 + 1, 10_000_000),
    ] {
        let mut bootstrap = controlled_bootstrap();
        bootstrap.clock.performance_counter = counter;
        bootstrap.clock.performance_frequency = frequency;
        Runtime::new(
            bootstrap,
            crate::GuestMemory::new(crate::MemoryLimits::default()).unwrap(),
        )
        .expect_err("invalid performance clock must stop bootstrap");
    }
}

fn controlled_bootstrap() -> RuntimeBootstrap {
    RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: RuntimeThreadId(THREAD_ID),
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
        process_image_path_a: crate::RuntimeProcessImagePathA {
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
        },
        initial_current_directory_a: crate::RuntimeCurrentDirectoryA {
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
        },
        encoded_pointer_cookie: 0xec01_9d3b,
    }
}

fn initialized_runtime() -> Runtime {
    Runtime::new(
        controlled_bootstrap(),
        crate::GuestMemory::new(crate::MemoryLimits::default()).unwrap(),
    )
    .expect("initialized controlled runtime")
}
