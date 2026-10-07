use crate::runtime::{
    RuntimeDynamicEndpoint, RuntimeFileBacking, RuntimeHandle, RuntimeObjectKind,
    RuntimeProcessLifecycleState,
};
use crate::win32::dispatch_kernel32_api;
use crate::{
    ApiDeferredCall, ApiDispatcher, ApiEndpointIdentity, ApiImmediate, DispatchOutcome,
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestRange, HostCompletion, MemoryLimits,
    MemoryOwnerKind, MemoryRegionKind, OpfsFileOperation, PageProtection, Runtime,
    RuntimeBootstrap, RuntimeClockState, RuntimeProcessId, RuntimeSchedulerEvent,
    RuntimeSourceFileManifest, RuntimeSourceMountManifest, RuntimeSourceReadHostResponse,
    RuntimeSourceSnapshotManifest, RuntimeStartupInfo, RuntimeThreadId, SchedulerThreadState,
    complete_kernel32_opfs_open, complete_kernel32_opfs_write, complete_kernel32_source_read,
    complete_opfs_file_operation,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
const PATH_ADDRESS: u32 = 0x2000_1000;
const BUFFER_ADDRESS: u32 = 0x2000_2000;
const BYTES_READ_ADDRESS: u32 = 0x2000_2f00;
const FIRST_FILE_HANDLE: u32 = 0x4601_0001;

#[test]
fn core_source_query_decodes_guest_abi_without_publishing_windows_state() {
    let (memory, _) = initialized_memory();
    let mut runtime = controlled_runtime(memory);
    runtime.register_source_snapshot(source_manifest()).unwrap();
    runtime.set_current_last_error(77).unwrap();

    let words = [0x00401000, PATH_ADDRESS, 0x8000_0000, 1, 0, 3, 0, 0];
    runtime
        .memory
        .write(
            MEMORY_BASE,
            &words
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        )
        .unwrap();
    runtime
        .memory
        .write(GuestAddress(PATH_ADDRESS), b"fixtures\\storage\\new.dat\0")
        .unwrap();
    acknowledge_initial_reflection(&mut runtime.memory);
    let before = runtime.memory.stats();
    assert_eq!(
        runtime
            .prepare_api_source_lookup(&endpoint(1, "CreateFileA"), MEMORY_BASE)
            .unwrap(),
        Some(crate::ApiSourceLookup::File {
            object_id: 2,
            relative_path_a: b"fixtures\\storage\\new.dat".to_vec()
        })
    );
    assert_eq!(runtime.memory.stats(), before);
    assert_eq!(runtime.source_snapshot().unwrap().files.len(), 1);
    assert_eq!(runtime.current_last_error().unwrap(), 77);

    runtime
        .memory
        .write(GuestAddress(PATH_ADDRESS), b"fixtures\\storage\\*.dat\0")
        .unwrap();
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime
            .prepare_api_source_lookup(&endpoint(2, "FindFirstFileA"), MEMORY_BASE)
            .unwrap(),
        Some(crate::ApiSourceLookup::Search {
            pattern: b"fixtures\\storage\\*.dat\0".to_vec(),
            parent: b"fixtures\\storage".to_vec()
        })
    );
    assert!(
        runtime
            .prepare_api_source_lookup(&endpoint(3, "Sleep"), GuestAddress(u32::MAX))
            .unwrap()
            .is_none()
    );

    runtime
        .memory
        .write(GuestAddress(PATH_ADDRESS), &[b'x'; 260])
        .unwrap();
    acknowledge_pending_delta(&mut runtime.memory);
    assert!(
        runtime
            .prepare_api_source_lookup(&endpoint(1, "CreateFileA"), MEMORY_BASE)
            .is_err()
    );
    runtime
        .memory
        .write(
            GuestAddress(PATH_ADDRESS),
            b"fixtures\\storage\\source-read-v1.dat\0",
        )
        .unwrap();
    acknowledge_pending_delta(&mut runtime.memory);
    assert!(
        runtime
            .prepare_api_source_lookup(&endpoint(1, "CreateFileA"), MEMORY_BASE)
            .unwrap()
            .is_none()
    );
    assert_eq!(runtime.source_snapshot().unwrap().files.len(), 1);
    assert_eq!(runtime.current_last_error().unwrap(), 77);
}

#[test]
fn wildcard_browser_batch_registers_selected_source_paths_atomically() {
    let mut runtime = controlled_runtime(GuestMemory::new(MemoryLimits::default()).unwrap());
    runtime.register_source_snapshot(source_manifest()).unwrap();
    let pattern = b"fixtures\\storage\\wildcard-*.dat\0";
    assert_eq!(
        runtime.source_search_directory(pattern).unwrap(),
        Some(b"fixtures\\storage".to_vec())
    );
    let (actual_parent, selected) = runtime
        .source_search_matches(
            pattern,
            &["fixtures".to_owned(), "storage".to_owned()],
            &[
                ("wildcard-only.dat".to_owned(), true),
                ("other.bin".to_owned(), true),
            ],
        )
        .unwrap();
    assert_eq!(actual_parent, b"fixtures\\storage");
    assert_eq!(
        selected,
        vec![(b"fixtures\\storage\\wildcard-only.dat".to_vec(), true)]
    );
    let files_before = runtime.source_snapshot().unwrap().files.len();
    let invalid = vec![
        (selected[0].0.clone(), Some(94)),
        (
            b"fixtures\\storage\\bad.dat".to_vec(),
            Some(u64::from(u32::MAX) + 1),
        ),
    ];
    assert!(
        runtime
            .register_source_search_batch(b"fixtures\\storage", invalid)
            .is_err()
    );
    assert_eq!(runtime.source_snapshot().unwrap().files.len(), files_before);
    let mut lookup_path = selected[0].0.clone();
    lookup_path.push(0);
    assert_eq!(
        runtime.source_lookup_relative_path(&lookup_path).unwrap(),
        Some(selected[0].0.clone())
    );
    assert_eq!(
        runtime
            .register_source_search_batch(
                b"fixtures\\storage",
                vec![(selected[0].0.clone(), Some(94))]
            )
            .unwrap(),
        vec![(0, 2)]
    );
    assert_eq!(
        runtime
            .source_snapshot()
            .unwrap()
            .files
            .get(&2)
            .unwrap()
            .relative_path_a,
        b"fixtures\\storage\\wildcard-only.dat"
    );
    assert!(
        runtime
            .source_search_matches(
                pattern,
                &["fixtures".to_owned(), "storage".to_owned()],
                &[("wildcard-only.dat".to_owned(), true)]
            )
            .unwrap()
            .1
            .is_empty()
    );
}

#[test]
fn generic_file_attributes_use_registered_source_and_windows_failures() {
    let endpoint = endpoint(31, "GetFileAttributesA");
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    let (mut memory, owner) = initialized_memory();
    type AttributeCase<'a> = (&'a [u8], u32, u32, Option<(u8, &'a [u8])>);
    let cases: [AttributeCase<'_>; 7] = [
        (b"fixtures\\empty\0", 0x30, 0x1234, None),
        (
            b"fixtures\\storage\\source-read-v1.dat\0",
            0x20,
            0x1234,
            None,
        ),
        (b"fixtures\\storage\0", 0x30, 0x1234, None),
        (
            b"fixtures\\storage\\missing.dat\0",
            u32::MAX,
            2,
            Some((2, b"fixtures/storage/missing.dat")),
        ),
        (
            b"missing\\file.dat\0",
            u32::MAX,
            3,
            Some((3, b"missing/file.dat")),
        ),
        (b"GuestState\0", 0x30, 0x1234, Some((2, b"GuestState"))),
        (
            b"GuestState\\slot.dat\0",
            0x20,
            0x1234,
            Some((3, b"GuestState/slot.dat")),
        ),
    ];
    for (index, (path, _, _, _)) in cases.iter().enumerate() {
        let index = u32::try_from(index).unwrap();
        let path_address = PATH_ADDRESS + index * 0x80;
        write_initialization(&mut memory, owner, GuestAddress(path_address), path);
        write_initialization(
            &mut memory,
            owner,
            GuestAddress(MEMORY_BASE.0 + index * 0x20),
            &[0x1000_1000 + index, path_address]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
    }
    memory.seal_owner(owner).unwrap();
    acknowledge_initial_reflection(&mut memory);
    let mut runtime = controlled_runtime(memory);
    runtime.register_source_snapshot(source_manifest()).unwrap();
    runtime
        .register_source_directory(b"fixtures\\empty")
        .unwrap();
    let source_id = runtime.source_snapshot().unwrap().source_id;
    let source_file_count = runtime.source_snapshot().unwrap().files.len();
    runtime
        .register_source_directory(b"FIXTURES\\EMPTY")
        .unwrap();
    assert_eq!(runtime.source_snapshot().unwrap().source_id, source_id);
    assert_eq!(
        runtime.source_snapshot().unwrap().files.len(),
        source_file_count
    );
    assert!(runtime.register_source_directory(b"..\\escape").is_err());
    assert!(
        runtime
            .register_source_directory(b"fixtures\\storage\\source-read-v1.dat")
            .is_err()
    );
    assert_eq!(runtime.source_snapshot().unwrap().source_id, source_id);
    assert_eq!(
        runtime.source_snapshot().unwrap().files.len(),
        source_file_count
    );
    assert!(
        runtime
            .source_lookup_relative_path(b"fixtures\\empty\0")
            .unwrap()
            .is_none()
    );
    for (index, (_, attributes, last_error, opfs_path)) in cases.into_iter().enumerate() {
        runtime.set_current_last_error(0x1234).unwrap();
        let stack = GuestAddress(MEMORY_BASE.0 + u32::try_from(index).unwrap() * 0x20);
        let completion = dispatcher.dispatch_owned(
            crate::ApiOwnedTrap {
                endpoint_id: endpoint.id,
                endpoint_address: endpoint.address,
                stack_pointer: stack,
                return_address: GuestAddress(0x1000_1000 + u32::try_from(index).unwrap()),
            },
            &mut runtime,
            None,
            dispatch_kernel32_api,
        );
        let completion = if let Some((source_missing, relative_path)) = opfs_path {
            let call = expect_deferred(completion);
            assert_eq!(call.request.service, "browser.opfs-stat");
            assert_eq!(call.request.payload.first(), Some(&source_missing));
            assert_eq!(&call.request.payload[1..], relative_path);
            let request_id = call.request.request_id;
            runtime.complete_host_request(request_id).unwrap();
            expect_immediate(dispatcher.complete_deferred(
                &call,
                HostCompletion {
                    request_id,
                    result: ApiImmediate::new(Some(attributes)).with_last_error(last_error),
                },
                &mut runtime,
            ))
        } else {
            expect_immediate(completion)
        };
        assert_eq!(completion.eax, Some(attributes));
        assert_eq!(runtime.current_last_error().unwrap(), last_error);
        assert_eq!(runtime.current_last_error().unwrap(), last_error);
    }
}

#[test]
fn generic_delete_file_rejects_source_and_finishes_only_after_host_result() {
    let endpoint = endpoint(41, "DeleteFileA");
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    let (mut memory, owner) = initialized_memory();
    let source_path = PATH_ADDRESS;
    let save_path = PATH_ADDRESS + 0x80;
    let source_parent_path = PATH_ADDRESS + 0x100;
    let absent_parent_path = PATH_ADDRESS + 0x180;
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(source_path),
        b"fixtures\\storage\\source-read-v1.dat\0",
    );
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(save_path),
        b"UserData\\slot.dat\0",
    );
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(source_parent_path),
        b"fixtures\\storage\\missing-delete.dat\0",
    );
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(absent_parent_path),
        b"unknown\\missing-delete.dat\0",
    );
    for (index, path) in [
        source_path,
        save_path,
        source_parent_path,
        absent_parent_path,
    ]
    .into_iter()
    .enumerate()
    {
        write_initialization(
            &mut memory,
            owner,
            GuestAddress(MEMORY_BASE.0 + index as u32 * 0x20),
            &[0x1000_1000, path]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
    }
    memory.seal_owner(owner).unwrap();
    acknowledge_initial_reflection(&mut memory);
    let mut runtime = controlled_runtime(memory);
    runtime.register_source_snapshot(source_manifest()).unwrap();
    let run = |index: u32, runtime: &mut Runtime| {
        dispatcher.dispatch_owned(
            crate::ApiOwnedTrap {
                endpoint_id: endpoint.id,
                endpoint_address: endpoint.address,
                stack_pointer: GuestAddress(MEMORY_BASE.0 + index * 0x20),
                return_address: GuestAddress(0x1000_1000),
            },
            runtime,
            None,
            dispatch_kernel32_api,
        )
    };
    let memory_before_denied = runtime.memory.stats();
    let denied = expect_immediate(run(0, &mut runtime));
    assert_eq!(denied.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 5);
    {
        let source = runtime.source_snapshot().unwrap();
        assert_eq!(source.source_id, 1);
        assert_eq!(source.files.len(), 1);
        let file = &source.files[&1];

        assert_eq!(
            file.relative_path_a,
            br"fixtures\storage\source-read-v1.dat"
        );
        assert_eq!(file.byte_length, 94);
    }
    assert_eq!(runtime.current_last_error().unwrap(), 5);
    assert_eq!(runtime.current_thread_id(), RuntimeThreadId(THREAD_ID));
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.memory.stats(), memory_before_denied);

    for (observation, success) in [(1, false), (0, true)] {
        runtime.set_current_last_error(0x4567).unwrap();
        let call = expect_deferred(run(1, &mut runtime));
        assert_eq!(call.request.service, "browser.opfs-delete-file");
        assert_eq!(call.request.payload, b"\x03UserData/slot.dat");
        {
            let source = runtime.source_snapshot().unwrap();
            assert_eq!(source.source_id, 1);
            assert_eq!(source.files.len(), 1);
            let file = &source.files[&1];

            assert_eq!(
                file.relative_path_a,
                br"fixtures\storage\source-read-v1.dat"
            );
            assert_eq!(file.byte_length, 94);
        }
        let request_id = call.request.request_id;
        assert!(
            matches!(runtime.scheduler_state().unwrap(), SchedulerThreadState::WaitingHost { request_id: id } if id == request_id)
        );
        runtime.complete_host_request(request_id).unwrap();
        let last_error = if success { 0x4567 } else { 2 };
        let completed = expect_immediate(
            dispatcher.complete_deferred(
                &call,
                HostCompletion {
                    request_id,
                    result: ApiImmediate::new(Some(u32::from(observation == 0)))
                        .with_last_error(last_error),
                },
                &mut runtime,
            ),
        );
        assert_eq!(completed.eax, Some(u32::from(success)));
        assert_eq!(runtime.current_last_error().unwrap(), last_error);
        {
            let source = runtime.source_snapshot().unwrap();
            assert_eq!(source.source_id, 1);
            assert_eq!(source.files.len(), 1);
            let file = &source.files[&1];

            assert_eq!(
                file.relative_path_a,
                br"fixtures\storage\source-read-v1.dat"
            );
            assert_eq!(file.byte_length, 94);
        }
    }
    for (index, expected_path, expected_error) in [
        (2, b"fixtures/storage/missing-delete.dat".as_slice(), 2),
        (3, b"unknown/missing-delete.dat".as_slice(), 3),
    ] {
        let call = expect_deferred(run(index, &mut runtime));
        assert_eq!(call.request.payload.first(), Some(&(expected_error as u8)));
        assert_eq!(&call.request.payload[1..], expected_path);
        runtime
            .complete_host_request(call.request.request_id)
            .unwrap();
        let completed = expect_immediate(
            dispatcher.complete_deferred(
                &call,
                HostCompletion {
                    request_id: call.request.request_id,
                    result: complete_opfs_file_operation(
                        OpfsFileOperation::DeleteFile {
                            source_missing: expected_error as u8,
                        },
                        2,
                        0x4567,
                    )
                    .unwrap(),
                },
                &mut runtime,
            ),
        );
        assert_eq!(completed.eax, Some(0));
        assert_eq!(runtime.current_last_error().unwrap(), expected_error);
    }
}

#[test]
fn generic_file_search_uses_checked_guest_output_and_retires_its_handle() {
    let endpoints = [
        endpoint(40, "FindFirstFileA"),
        endpoint(41, "FindNextFileA"),
        endpoint(42, "FindClose"),
    ];
    let dispatcher = ApiDispatcher::from_endpoints(endpoints.clone()).unwrap();
    let (mut memory, owner) = initialized_memory();
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(PATH_ADDRESS),
        b"data0????.arc\0",
    );
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(PATH_ADDRESS + 0x100),
        b"missing\\*\0",
    );
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(PATH_ADDRESS + 0x200),
        b"D:\\outside\\*\0",
    );
    let frames = [
        (0, vec![0x1000_1000, PATH_ADDRESS, 0xdead_beef]),
        (0, vec![0x1000_1001, PATH_ADDRESS, BUFFER_ADDRESS]),
        (1, vec![0x1000_1002, 0x5301_0001, BUFFER_ADDRESS + 0x200]),
        (1, vec![0x1000_1003, 0x5301_0001, BUFFER_ADDRESS + 0x400]),
        (1, vec![0x1000_1004, 0x5301_0001, BUFFER_ADDRESS + 0x600]),
        (2, vec![0x1000_1005, 0x5301_0001]),
        (1, vec![0x1000_1006, 0x5301_0001, BUFFER_ADDRESS + 0x800]),
        (0, vec![0x1000_1007, PATH_ADDRESS + 0x100, BUFFER_ADDRESS]),
        (0, vec![0x1000_1008, PATH_ADDRESS + 0x200, BUFFER_ADDRESS]),
    ];
    for (index, (_, words)) in frames.iter().enumerate() {
        write_initialization(
            &mut memory,
            owner,
            GuestAddress(MEMORY_BASE.0 + (index as u32) * 0x20),
            &words
                .iter()
                .copied()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
    }
    memory.seal_owner(owner).unwrap();
    acknowledge_initial_reflection(&mut memory);
    let mut runtime = controlled_runtime(memory);
    runtime
        .register_source_snapshot(find_file_manifest())
        .unwrap();
    runtime.set_current_last_error(0x7654).unwrap();
    let invoke = |index: usize, runtime: &mut Runtime| {
        let which = frames[index].0;
        dispatcher.dispatch_owned(
            crate::ApiOwnedTrap {
                endpoint_id: endpoints[which].id,
                endpoint_address: endpoints[which].address,
                stack_pointer: GuestAddress(MEMORY_BASE.0 + (index as u32) * 0x20),
                return_address: GuestAddress(frames[index].1[0]),
            },
            runtime,
            None,
            dispatch_kernel32_api,
        )
    };
    let memory_before = runtime.memory.stats();
    assert!(matches!(invoke(0, &mut runtime), DispatchOutcome::Fault(_)));
    assert!(
        runtime
            .file_search_for_handle(RuntimeHandle(0x5301_0001))
            .is_err()
    );
    assert_eq!(runtime.current_last_error().unwrap(), 0x7654);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.memory.stats(), memory_before);

    for (index, expected_return, expected_name, expected_size) in [
        (1, 0x5301_0001, b"data01099.arc".as_slice(), 10),
        (2, 1, b"DATA01100.ARC".as_slice(), 20),
        (3, 1, b"data01999.arc".as_slice(), 30),
    ] {
        let completion = expect_immediate(invoke(index, &mut runtime));
        assert_eq!(completion.eax, Some(expected_return));
        assert_eq!(runtime.current_last_error().unwrap(), 0x7654);
        acknowledge_pending_delta(&mut runtime.memory);
        assert_find_data(
            &runtime.memory,
            BUFFER_ADDRESS + (index as u32 - 1) * 0x200,
            0x20,
            expected_size,
            expected_name,
        );
    }
    let terminal = expect_immediate(invoke(4, &mut runtime));
    assert_eq!(terminal.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 18);
    let close = expect_immediate(invoke(5, &mut runtime));
    assert_eq!(close.eax, Some(1));
    let invalid = expect_immediate(invoke(6, &mut runtime));
    assert_eq!(invalid.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 6);
    let memory_before = runtime.memory.stats();
    for index in [7, 8] {
        let missing = expect_immediate(invoke(index, &mut runtime));
        assert_eq!(missing.eax, Some(u32::MAX));
        assert_eq!(runtime.current_last_error().unwrap(), 3); // ERROR_PATH_NOT_FOUND
        assert_eq!(runtime.memory.stats(), memory_before);
        assert!(
            runtime
                .file_search_for_handle(RuntimeHandle(0x5301_0001))
                .is_err()
        );
    }
}

#[test]
fn find_file_a_enumerates_one_deterministic_search_handle_atomically() {
    let endpoints = [
        endpoint(40, "FindFirstFileA"),
        endpoint(41, "FindNextFileA"),
        endpoint(42, "FindClose"),
        endpoint(43, "CloseHandle"),
    ];
    let dispatcher = ApiDispatcher::from_endpoints(endpoints.clone()).expect("FindFile dispatcher");

    let (mut memory, owner) = initialized_memory();
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(PATH_ADDRESS),
        b"data0????.arc\0",
    );
    let invalid_output = MEMORY_BASE.0 + 0x2f80;
    let search_handle = 0x5301_0001;
    let frames = [
        (
            MEMORY_BASE.0,
            vec![0x1000_1000, PATH_ADDRESS, invalid_output],
        ),
        (
            MEMORY_BASE.0 + 0x20,
            vec![0x1000_1010, PATH_ADDRESS, BUFFER_ADDRESS],
        ),
        (
            MEMORY_BASE.0 + 0x40,
            vec![0x1000_1020, search_handle, BUFFER_ADDRESS + 0x200],
        ),
        (
            MEMORY_BASE.0 + 0x60,
            vec![0x1000_1030, search_handle, BUFFER_ADDRESS + 0x400],
        ),
        (
            MEMORY_BASE.0 + 0x80,
            vec![0x1000_1040, search_handle, BUFFER_ADDRESS + 0x600],
        ),
        (
            MEMORY_BASE.0 + 0xa0,
            vec![0x1000_1050, search_handle, BUFFER_ADDRESS + 0x800],
        ),
        (MEMORY_BASE.0 + 0xc0, vec![0x1000_1060, search_handle]),
        (MEMORY_BASE.0 + 0xe0, vec![0x1000_1070, search_handle]),
        (
            MEMORY_BASE.0 + 0x100,
            vec![0x1000_1080, search_handle, BUFFER_ADDRESS + 0xa00],
        ),
        (
            MEMORY_BASE.0 + 0x120,
            vec![0x1000_1090, search_handle, 0xdead_beef],
        ),
    ];
    for (stack, words) in &frames {
        write_initialization(
            &mut memory,
            owner,
            GuestAddress(*stack),
            &words
                .iter()
                .copied()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
    }
    memory.seal_owner(owner).expect("seal FindFile memory");
    acknowledge_initial_reflection(&mut memory);

    let mut runtime = controlled_runtime(memory);
    runtime
        .register_source_snapshot(find_file_manifest())
        .expect("register FindFile snapshot");
    let memory_before_invalid_output = runtime.memory.stats();
    let invalid_output_result = dispatch(&dispatcher, &endpoints[0], frames[0].0, &mut runtime);
    assert!(matches!(invalid_output_result, DispatchOutcome::Fault(_)));
    assert!(
        runtime
            .file_search_for_handle(RuntimeHandle(search_handle))
            .is_err()
    );
    assert_eq!(runtime.current_last_error().unwrap(), 0x1234);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.memory.stats(), memory_before_invalid_output);

    runtime.set_current_last_error(0x7654).unwrap();
    let first = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[0],
        frames[1].0,
        &mut runtime,
    ));
    assert_eq!(first.eax, Some(search_handle));
    assert_eq!(runtime.current_last_error().unwrap(), 0x7654);
    acknowledge_pending_delta(&mut runtime.memory);
    assert_find_data(&runtime.memory, BUFFER_ADDRESS, 0x20, 10, b"data01099.arc");

    let search_before_failed_next = runtime
        .file_search_for_handle(RuntimeHandle(search_handle))
        .unwrap()
        .clone();
    let memory_before_failed_next = runtime.memory.stats();
    assert!(matches!(
        dispatch(&dispatcher, &endpoints[1], frames[9].0, &mut runtime),
        DispatchOutcome::Fault(_)
    ));
    assert_eq!(
        runtime
            .file_search_for_handle(RuntimeHandle(search_handle))
            .unwrap(),
        &search_before_failed_next
    );
    assert_eq!(runtime.current_last_error().unwrap(), 0x7654);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.memory.stats(), memory_before_failed_next);
    assert_find_data(&runtime.memory, BUFFER_ADDRESS, 0x20, 10, b"data01099.arc");

    for (frame_index, expected_name, expected_size) in [
        (2, b"DATA01100.ARC".as_slice(), 20_u64),
        (3, b"data01999.arc".as_slice(), 30_u64),
    ] {
        let next = expect_immediate(dispatch(
            &dispatcher,
            &endpoints[1],
            frames[frame_index].0,
            &mut runtime,
        ));
        assert_eq!(next.eax, Some(1));
        assert_eq!(runtime.current_last_error().unwrap(), 0x7654);
        acknowledge_pending_delta(&mut runtime.memory);
        assert_find_data(
            &runtime.memory,
            BUFFER_ADDRESS + u32::try_from(frame_index - 1).unwrap() * 0x200,
            0x20,
            expected_size,
            expected_name,
        );
    }

    let terminal_output = BUFFER_ADDRESS + 0x600;
    let terminal_before = runtime
        .memory
        .read(GuestAddress(terminal_output), 320)
        .unwrap()
        .to_vec();
    let terminal = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[1],
        frames[4].0,
        &mut runtime,
    ));
    assert_eq!(terminal.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 18);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(terminal_output), 320)
            .unwrap(),
        terminal_before
    );

    let close_handle = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[3],
        frames[6].0,
        &mut runtime,
    ));
    assert_eq!(close_handle.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 6);
    assert!(
        runtime
            .file_search_for_handle(RuntimeHandle(search_handle))
            .is_ok()
    );

    let closed = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[2],
        frames[7].0,
        &mut runtime,
    ));
    assert_eq!(closed.eax, Some(1));
    assert_eq!(runtime.current_last_error().unwrap(), 6);
    assert!(
        runtime
            .file_search_for_handle(RuntimeHandle(search_handle))
            .is_err()
    );
    let stale = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[1],
        frames[8].0,
        &mut runtime,
    ));
    assert_eq!(stale.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 6);
}

#[test]
fn opfs_open_publishes_a_file_handle_only_after_durable_host_success() {
    let endpoints = [
        endpoint(31, "CreateFileA"),
        endpoint(32, "CloseHandle"),
        endpoint(33, "WriteFile"),
        endpoint(34, "GetFileType"),
    ];
    let dispatcher = ApiDispatcher::from_endpoints(endpoints.clone()).unwrap();
    let (mut memory, owner) = initialized_memory();
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(PATH_ADDRESS),
        b"fixtures\\storage\\slot.dat\0",
    );
    for (index, (access, sharing, disposition)) in [(0x4000_0000_u32, 0, 4), (0x8000_0000, 1, 3)]
        .into_iter()
        .enumerate()
    {
        write_initialization(
            &mut memory,
            owner,
            GuestAddress(MEMORY_BASE.0 + index as u32 * 0x40),
            &[
                0x1000_1000,
                PATH_ADDRESS,
                access,
                sharing,
                0,
                disposition,
                0x0800_0020,
                0,
            ]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>(),
        );
    }
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(MEMORY_BASE.0 + 0x80),
        &[0x1000_1000, FIRST_FILE_HANDLE]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(BUFFER_ADDRESS),
        &[49, 0, 255],
    );
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(BYTES_READ_ADDRESS),
        &u32::MAX.to_le_bytes(),
    );
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(MEMORY_BASE.0 + 0xc0),
        &[
            0x1000_1000,
            FIRST_FILE_HANDLE,
            BUFFER_ADDRESS,
            3,
            BYTES_READ_ADDRESS,
            0,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>(),
    );
    memory.seal_owner(owner).unwrap();
    acknowledge_initial_reflection(&mut memory);
    let mut runtime = controlled_runtime(memory);
    runtime.register_source_snapshot(source_manifest()).unwrap();
    let run = |index: usize, frame: u32, runtime: &mut Runtime| {
        dispatcher.dispatch_owned(
            crate::ApiOwnedTrap {
                endpoint_id: endpoints[index].id,
                endpoint_address: endpoints[index].address,
                stack_pointer: GuestAddress(frame),
                return_address: GuestAddress(0x1000_1000),
            },
            runtime,
            None,
            dispatch_kernel32_api,
        )
    };
    let failed_call = expect_deferred(run(0, MEMORY_BASE.0, &mut runtime));
    assert_eq!(runtime.file_type(FIRST_FILE_HANDLE).unwrap(), None);
    assert_eq!(failed_call.request.service, "browser.opfs-open");
    assert_eq!(
        failed_call.request.payload,
        b"\x04\x02\x02fixtures/storage/slot.dat"
    );
    assert_eq!(
        runtime.opfs_delete_path(b"fixtures\\storage\\SLOT.dat\0"),
        Err(32)
    );
    assert!(
        runtime
            .source_file_for_handle(RuntimeHandle(FIRST_FILE_HANDLE))
            .is_err()
    );
    let memory_before_invalid = runtime.memory.stats();
    let last_error_before_invalid = runtime.current_last_error().unwrap();
    let invalid = complete_kernel32_opfs_open(&dispatcher, &failed_call, 99, 0, &mut runtime);
    assert!(matches!(invalid, DispatchOutcome::Fault(_)));
    assert_eq!(runtime.memory.stats(), memory_before_invalid);
    assert_eq!(
        runtime.current_last_error().unwrap(),
        last_error_before_invalid
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: failed_call.request.request_id
        }
    );
    assert!(
        runtime
            .source_file_for_handle(RuntimeHandle(FIRST_FILE_HANDLE))
            .is_err()
    );
    assert_eq!(
        runtime.opfs_delete_path(b"fixtures\\storage\\SLOT.dat\0"),
        Err(32)
    );
    {
        let source = runtime.source_snapshot().unwrap();
        assert_eq!(source.source_id, 1);
        assert_eq!(source.files.len(), 1);
        let file = &source.files[&1];

        assert_eq!(
            file.relative_path_a,
            br"fixtures\storage\source-read-v1.dat"
        );
        assert_eq!(file.byte_length, 94);
    }
    let failed = complete_kernel32_opfs_open(&dispatcher, &failed_call, 5, 0, &mut runtime);
    let failed = expect_immediate(failed);
    assert_eq!(failed.eax, Some(u32::MAX));
    assert_eq!(runtime.current_last_error().unwrap(), 112);
    assert!(
        runtime
            .source_file_for_handle(RuntimeHandle(FIRST_FILE_HANDLE))
            .is_err()
    );
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));
    let call = expect_deferred(run(0, MEMORY_BASE.0, &mut runtime));
    let created = complete_kernel32_opfs_open(&dispatcher, &call, 0, 0, &mut runtime);
    assert_eq!(expect_immediate(created).eax, Some(FIRST_FILE_HANDLE));
    let query_memory = runtime.memory.stats();
    let query_last_error = runtime.current_last_error().unwrap();
    assert_eq!(
        expect_immediate(run(3, MEMORY_BASE.0 + 0x80, &mut runtime)).eax,
        Some(1)
    );
    assert_eq!(runtime.current_last_error().unwrap(), query_last_error);
    assert_eq!(runtime.memory.stats(), query_memory);
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));
    assert!(matches!(
        runtime
            .source_file_for_handle(RuntimeHandle(FIRST_FILE_HANDLE))
            .unwrap()
            .backing,
        RuntimeFileBacking::Opfs { byte_length: 0 }
    ));
    {
        let source = runtime.source_snapshot().unwrap();
        assert_eq!(source.source_id, 1);
        assert_eq!(source.files.len(), 1);
        let file = &source.files[&1];

        assert_eq!(
            file.relative_path_a,
            br"fixtures\storage\source-read-v1.dat"
        );
        assert_eq!(file.byte_length, 94);
    }
    let sharing = expect_immediate(run(0, MEMORY_BASE.0, &mut runtime));
    assert_eq!(sharing.eax, Some(u32::MAX));
    assert_eq!(runtime.current_last_error().unwrap(), 32);
    let failed_write_call = expect_deferred(run(2, MEMORY_BASE.0 + 0xc0, &mut runtime));
    assert_eq!(failed_write_call.request.service, "browser.opfs-write");
    assert_eq!(failed_write_call.request.payload, [49, 0, 255]);
    assert_eq!(
        runtime
            .opfs_write_host_info(failed_write_call.request.request_id)
            .unwrap(),
        ("fixtures/storage/slot.dat", 0, 3)
    );
    let file_before_bad_observation = runtime
        .source_file_for_handle(RuntimeHandle(FIRST_FILE_HANDLE))
        .unwrap()
        .clone();
    let memory_before_bad_observation = runtime.memory.stats();
    let last_error_before_bad_observation = runtime.current_last_error().unwrap();
    let bad = complete_kernel32_opfs_write(&dispatcher, &failed_write_call, 99, &mut runtime);
    assert!(matches!(bad, DispatchOutcome::Fault(_)));
    assert_eq!(
        runtime
            .source_file_for_handle(RuntimeHandle(FIRST_FILE_HANDLE))
            .unwrap(),
        &file_before_bad_observation
    );
    assert!(
        runtime
            .opfs_write_pending(RuntimeHandle(FIRST_FILE_HANDLE))
            .unwrap()
    );
    assert_eq!(
        runtime
            .opfs_write_host_info(failed_write_call.request.request_id)
            .unwrap(),
        ("fixtures/storage/slot.dat", 0, 3)
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: failed_write_call.request.request_id
        }
    );
    assert_eq!(
        runtime.current_last_error().unwrap(),
        last_error_before_bad_observation
    );
    assert_eq!(runtime.memory.stats(), memory_before_bad_observation);
    let denied = complete_kernel32_opfs_write(&dispatcher, &failed_write_call, 3, &mut runtime);
    expect_immediate(denied);
    assert_eq!(runtime.current_last_error().unwrap(), 112);
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));
    let file = runtime
        .source_file_for_handle(RuntimeHandle(FIRST_FILE_HANDLE))
        .unwrap();
    assert_eq!(file.cursor, 0);
    assert_eq!(file.backing, RuntimeFileBacking::Opfs { byte_length: 0 });
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(BYTES_READ_ADDRESS), 4)
            .unwrap(),
        u32::MAX.to_le_bytes()
    );
    let write_call = expect_deferred(run(2, MEMORY_BASE.0 + 0xc0, &mut runtime));
    let written = complete_kernel32_opfs_write(&dispatcher, &write_call, 0, &mut runtime);
    assert_eq!(expect_immediate(written).eax, Some(1));
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(BYTES_READ_ADDRESS), 4)
            .unwrap(),
        3u32.to_le_bytes()
    );
    let file = runtime
        .source_file_for_handle(RuntimeHandle(FIRST_FILE_HANDLE))
        .unwrap();
    assert_eq!(file.cursor, 3);
    assert_eq!(file.backing, RuntimeFileBacking::Opfs { byte_length: 3 });
    assert_eq!(
        expect_immediate(run(1, MEMORY_BASE.0 + 0x80, &mut runtime)).eax,
        Some(1)
    );
    assert!(
        runtime
            .source_file_for_handle(RuntimeHandle(FIRST_FILE_HANDLE))
            .is_err()
    );
    let query_memory = runtime.memory.stats();
    assert_eq!(
        expect_immediate(run(3, MEMORY_BASE.0 + 0x80, &mut runtime)).eax,
        Some(0)
    );
    assert_eq!(runtime.current_last_error().unwrap(), 6);
    assert_eq!(runtime.memory.stats(), query_memory);
    runtime.set_current_last_error(0x4321).unwrap();
    let reopened = expect_deferred(run(0, MEMORY_BASE.0 + 0x40, &mut runtime));
    let loaded = complete_kernel32_opfs_open(&dispatcher, &reopened, 1, 3, &mut runtime);
    let loaded = expect_immediate(loaded);
    assert_ne!(loaded.eax, Some(u32::MAX));
    assert_eq!(runtime.current_last_error().unwrap(), 0x4321);
    let Some(reopened_handle) = loaded.eax else {
        panic!("reopen must return a handle")
    };
    assert_eq!(
        runtime
            .source_file_byte_length(RuntimeHandle(reopened_handle))
            .unwrap(),
        3
    );
    {
        let source = runtime.source_snapshot().unwrap();
        assert_eq!(source.source_id, 1);
        assert_eq!(source.files.len(), 1);
        let file = &source.files[&1];

        assert_eq!(
            file.relative_path_a,
            br"fixtures\storage\source-read-v1.dat"
        );
        assert_eq!(file.byte_length, 94);
    }
}

#[test]
fn directory_creation_suspends_for_opfs_without_a_writable_catalog() {
    let endpoint = endpoint(31, "CreateDirectoryA");
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    for error in [0, 3, 183] {
        let (mut memory, owner) = initialized_memory();
        write_initialization(
            &mut memory,
            owner,
            GuestAddress(PATH_ADDRESS),
            b"UserData\\Slot01\0",
        );
        write_initialization(
            &mut memory,
            owner,
            MEMORY_BASE,
            &[0x1000_1000, PATH_ADDRESS, 0]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        write_initialization(
            &mut memory,
            owner,
            GuestAddress(PATH_ADDRESS + 0x80),
            b"fixtures\\storage\\new-dir\0",
        );
        write_initialization(
            &mut memory,
            owner,
            GuestAddress(MEMORY_BASE.0 + 0x40),
            &[0x1000_1000, PATH_ADDRESS + 0x80, 0]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        memory.seal_owner(owner).unwrap();
        acknowledge_initial_reflection(&mut memory);
        let mut runtime = controlled_runtime(memory);
        runtime.register_source_snapshot(source_manifest()).unwrap();
        let call = expect_deferred(dispatch(
            &dispatcher,
            &endpoint,
            MEMORY_BASE.0,
            &mut runtime,
        ));
        assert_eq!(call.request.service, "browser.opfs-create-directory");
        assert_eq!(call.request.payload, b"\0UserData/Slot01");
        {
            let source = runtime.source_snapshot().unwrap();
            assert_eq!(source.source_id, 1);
            assert_eq!(source.files.len(), 1);
            let file = &source.files[&1];

            assert_eq!(
                file.relative_path_a,
                br"fixtures\storage\source-read-v1.dat"
            );
            assert_eq!(file.byte_length, 94);
        }
        let request_id = call.request.request_id;
        assert!(
            matches!(runtime.scheduler_state().unwrap(), SchedulerThreadState::WaitingHost { request_id: id } if id == request_id)
        );
        let expected_error = if error == 0 {
            runtime.current_last_error().unwrap()
        } else {
            error
        };
        runtime.complete_host_request(request_id).unwrap();
        let completion = expect_immediate(dispatcher.complete_deferred(
            &call,
            HostCompletion {
                request_id,
                result:
                    ApiImmediate::new(Some(u32::from(error == 0))).with_last_error(expected_error),
            },
            &mut runtime,
        ));
        assert_eq!(completion.eax, Some(u32::from(error == 0)));
        assert_eq!(runtime.current_last_error().unwrap(), expected_error);
        {
            let source = runtime.source_snapshot().unwrap();
            assert_eq!(source.source_id, 1);
            assert_eq!(source.files.len(), 1);
            let file = &source.files[&1];

            assert_eq!(
                file.relative_path_a,
                br"fixtures\storage\source-read-v1.dat"
            );
            assert_eq!(file.byte_length, 94);
        }
        let child = expect_deferred(dispatch(
            &dispatcher,
            &endpoint,
            MEMORY_BASE.0 + 0x40,
            &mut runtime,
        ));
        assert_eq!(child.request.payload, b"\x02fixtures/storage/new-dir");
        runtime
            .complete_host_request(child.request.request_id)
            .unwrap();
        expect_immediate(dispatcher.complete_deferred(
            &child,
            HostCompletion {
                request_id: child.request.request_id,
                result: ApiImmediate::new(Some(u32::from(true))).with_last_error(0x4321),
            },
            &mut runtime,
        ));
        {
            let source = runtime.source_snapshot().unwrap();
            assert_eq!(source.source_id, 1);
            assert_eq!(source.files.len(), 1);
            let file = &source.files[&1];

            assert_eq!(
                file.relative_path_a,
                br"fixtures\storage\source-read-v1.dat"
            );
            assert_eq!(file.byte_length, 94);
        }
    }
}

#[test]
fn get_file_type_checks_source_handle_lifetime_without_mutating_file_or_guest_state() {
    let (memory, owner) = initialized_memory();
    let mut runtime = controlled_runtime(memory);
    runtime.register_source_snapshot(source_manifest()).unwrap();
    let handle = runtime
        .open_file_a(
            b"fixtures\\storage\\source-read-v1.dat\0",
            0x8000_0000,
            1,
            3,
            0,
        )
        .unwrap()
        .unwrap()
        .unwrap();
    let event = runtime.create_event(false, true).unwrap();
    let endpoint = endpoint(1, "GetFileType");
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    for (index, value) in [handle.0, event.0, 0, u32::MAX].into_iter().enumerate() {
        write_initialization(
            &mut runtime.memory,
            owner,
            GuestAddress(MEMORY_BASE.0 + index as u32 * 0x20),
            &[0x1000_0010_u32, value]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
    }
    runtime.memory.seal_owner(owner).unwrap();
    acknowledge_initial_reflection(&mut runtime.memory);
    let bytes_before = runtime.memory.read(MEMORY_BASE, 0x3000).unwrap();
    let memory_before = runtime.memory.stats();
    let handles_before = runtime
        .object_for_handle(event, RuntimeObjectKind::Event)
        .unwrap()
        .open_handles;
    let source_handles_before = runtime
        .object_for_handle(handle, RuntimeObjectKind::File)
        .unwrap()
        .open_handles;
    for (index, expected_type) in [1, 0, 0, 0, 1].into_iter().enumerate() {
        runtime.set_current_last_error(0x1234).unwrap();
        let frame_index = if index == 4 { 0 } else { index };
        let result = expect_immediate(dispatch(
            &dispatcher,
            &endpoint,
            MEMORY_BASE.0 + frame_index as u32 * 0x20,
            &mut runtime,
        ));
        assert_eq!(result.eax, Some(expected_type));
        assert_eq!(
            runtime.current_last_error().unwrap(),
            if expected_type == 0 { 6 } else { 0x1234 }
        );
        assert_eq!(runtime.source_file_for_handle(handle).unwrap().cursor, 0);
        assert_eq!(runtime.source_file_byte_length(handle).unwrap(), 94);
        assert_eq!(
            runtime
                .object_for_handle(event, RuntimeObjectKind::Event)
                .unwrap()
                .open_handles,
            handles_before
        );
        assert_eq!(
            runtime
                .object_for_handle(handle, RuntimeObjectKind::File)
                .unwrap()
                .open_handles,
            source_handles_before
        );
        assert_eq!(runtime.memory.stats(), memory_before);
        assert_eq!(
            runtime.memory.read(MEMORY_BASE, 0x3000).unwrap(),
            bytes_before
        );
    }
    runtime.close_handle(handle).unwrap();
    let handles_after_close = runtime
        .object_for_handle(event, RuntimeObjectKind::Event)
        .unwrap()
        .open_handles;
    let closed = expect_immediate(dispatch(
        &dispatcher,
        &endpoint,
        MEMORY_BASE.0,
        &mut runtime,
    ));
    assert_eq!(closed.eax, Some(0));
    assert_eq!(runtime.current_last_error().unwrap(), 6);
    assert_eq!(
        runtime
            .object_for_handle(event, RuntimeObjectKind::Event)
            .unwrap()
            .open_handles,
        handles_after_close
    );
    assert!(
        !runtime
            .is_live_object_handle(handle, RuntimeObjectKind::File)
            .unwrap()
    );
    assert_eq!(runtime.memory.stats(), memory_before);
    assert_eq!(
        runtime.memory.read(MEMORY_BASE, 0x3000).unwrap(),
        bytes_before
    );
}

#[test]
fn source_file_set_file_pointer_64bit_outputs_and_failures_are_atomic() {
    let (mut memory, owner) = initialized_memory();
    memory.seal_owner(owner).unwrap();
    acknowledge_initial_reflection(&mut memory);
    let mut runtime = controlled_runtime(memory);
    runtime.register_source_snapshot(source_manifest()).unwrap();
    let handle = runtime
        .open_file_a(
            b"fixtures\\storage\\source-read-v1.dat\0",
            0x8000_0000,
            1,
            3,
            0,
        )
        .unwrap()
        .unwrap()
        .unwrap();
    let seek = endpoint(1, "SetFilePointer");
    let dispatcher = ApiDispatcher::from_endpoints([seek.clone()]).unwrap();
    let large = (1_i64 << 32) + 144;
    for (distance, origin, has_high, expected_cursor, expected_error, failed) in [
        (144_i64, 0, true, 144_u64, 77, false),
        (large, 0, true, large as u64, 77, false),
        (-32, 1, true, (large - 32) as u64, 77, false),
        (-1, 2, true, 93, 77, false),
        (0x1_ffff_ffff, 0, true, 0x1_ffff_ffff, 0, false),
        (-1, 0, true, 0x1_ffff_ffff, 131, true),
        (0, 3, true, 0x1_ffff_ffff, 87, true),
        (1, 1, false, 0x1_ffff_ffff, 87, true),
        (144, 0, false, 144, 77, false),
    ] {
        let input_high = ((distance as u64) >> 32) as u32;
        runtime
            .memory
            .write(GuestAddress(BYTES_READ_ADDRESS), &input_high.to_le_bytes())
            .unwrap();
        prepare_set_file_pointer_frame(
            &mut runtime,
            handle,
            distance as u32,
            if has_high { BYTES_READ_ADDRESS } else { 0 },
            origin,
        );
        runtime.set_current_last_error(77).unwrap();
        let file_before = runtime.source_file_for_handle(handle).unwrap().clone();
        let memory_before = runtime.memory.stats();
        let result = expect_immediate(dispatch(&dispatcher, &seek, MEMORY_BASE.0, &mut runtime));
        assert_eq!(
            result.eax,
            Some(if failed {
                u32::MAX
            } else {
                expected_cursor as u32
            })
        );
        assert_eq!(runtime.current_last_error().unwrap(), expected_error);
        assert_eq!(
            runtime.source_file_for_handle(handle).unwrap().cursor,
            expected_cursor
        );
        if failed {
            assert_eq!(
                runtime.source_file_for_handle(handle).unwrap(),
                &file_before
            );
            assert_eq!(runtime.memory.stats(), memory_before);
        }
        if runtime.memory.stats().pending_invalidation.is_some() {
            acknowledge_pending_delta(&mut runtime.memory);
        }
        let output_high = runtime
            .memory
            .read(GuestAddress(BYTES_READ_ADDRESS), 4)
            .unwrap();
        assert_eq!(
            output_high,
            if has_high && !failed {
                ((expected_cursor >> 32) as u32).to_le_bytes()
            } else {
                input_high.to_le_bytes()
            }
        );
    }
    runtime
        .memory
        .write(GuestAddress(BUFFER_ADDRESS), &0_u32.to_le_bytes())
        .unwrap();
    runtime
        .memory
        .protect(
            owner,
            GuestRange::new(GuestAddress(BUFFER_ADDRESS), GUEST_PAGE_SIZE).unwrap(),
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .unwrap();
    acknowledge_pending_delta(&mut runtime.memory);
    for high in [0x3000_0000, BUFFER_ADDRESS] {
        prepare_set_file_pointer_frame(&mut runtime, handle, 145, high, 0);
        runtime.set_current_last_error(77).unwrap();
        let file_before = runtime.source_file_for_handle(handle).unwrap().clone();
        let memory_before = runtime.memory.stats();
        let bytes_before = runtime.memory.read(MEMORY_BASE, 0x3000).unwrap();
        assert!(matches!(
            dispatch(&dispatcher, &seek, MEMORY_BASE.0, &mut runtime),
            DispatchOutcome::Fault(_)
        ));
        assert_eq!(
            runtime.source_file_for_handle(handle).unwrap(),
            &file_before
        );
        assert_eq!(runtime.memory.stats(), memory_before);
        assert_eq!(
            runtime.memory.read(MEMORY_BASE, 0x3000).unwrap(),
            bytes_before
        );
        assert_eq!(runtime.current_last_error().unwrap(), 77);
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
    }
}

fn prepare_set_file_pointer_frame(
    runtime: &mut Runtime,
    handle: RuntimeHandle,
    low: u32,
    high: u32,
    origin: u32,
) {
    let bytes = [0x1000_0010, handle.0, low, high, origin]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    runtime.memory.write(MEMORY_BASE, &bytes).unwrap();
    acknowledge_pending_delta(&mut runtime.memory);
}

#[test]
fn source_file_api_slice_opens_reads_seeks_and_closes_atomically() {
    let fixture = include_bytes!("../../../fixtures/storage/source-read-v1.dat");

    let manifest = source_manifest();
    let (mut memory, owner) = initialized_memory();
    let endpoints = [
        endpoint(1, "CreateFileA"),
        endpoint(2, "GetFileSize"),
        endpoint(3, "ReadFile"),
        endpoint(4, "SetFilePointer"),
        endpoint(5, "CloseHandle"),
    ];
    let dispatcher = ApiDispatcher::from_endpoints(endpoints.clone()).expect("file dispatcher");

    write_initialization(
        &mut memory,
        owner,
        GuestAddress(PATH_ADDRESS),
        b"fixtures\\storage\\source-read-v1.dat\0",
    );
    let frames = [
        (
            0x2000_0000,
            vec![
                0x1000_0010,
                PATH_ADDRESS,
                0x8000_0000,
                1,
                0,
                3,
                0x0800_0020,
                0,
            ],
        ),
        (0x2000_0040, vec![0x1000_0020, FIRST_FILE_HANDLE, 0]),
        (
            0x2000_0080,
            vec![
                0x1000_0030,
                FIRST_FILE_HANDLE,
                BUFFER_ADDRESS,
                16,
                BYTES_READ_ADDRESS,
                0,
            ],
        ),
        (0x2000_00c0, vec![0x1000_0040, FIRST_FILE_HANDLE, 5, 0, 0]),
        (
            0x2000_0100,
            vec![
                0x1000_0050,
                FIRST_FILE_HANDLE,
                BUFFER_ADDRESS,
                8,
                BYTES_READ_ADDRESS,
                0,
            ],
        ),
        (0x2000_0140, vec![0x1000_0060, FIRST_FILE_HANDLE]),
        (0x2000_0180, vec![0x1000_0070, FIRST_FILE_HANDLE, 0]),
        (
            0x2000_01c0,
            vec![0x1000_0080, FIRST_FILE_HANDLE, BYTES_READ_ADDRESS],
        ),
        (
            0x2000_0200,
            vec![0x1000_0090, FIRST_FILE_HANDLE, 0x3000_0000],
        ),
        (
            0x2000_0240,
            vec![0x1000_00a0, 0xffff_ffff, BYTES_READ_ADDRESS],
        ),
        (
            0x2000_0280,
            vec![0x1000_00b0, FIRST_FILE_HANDLE, (-6i32) as u32, 0, 1],
        ),
        (0x2000_02c0, vec![0x1000_00c0, FIRST_FILE_HANDLE, 0, 0, 3]),
        (
            0x2000_0300,
            vec![
                0x1000_00d0,
                PATH_ADDRESS,
                0x8000_0000,
                0,
                0,
                3,
                0x0800_0020,
                0,
            ],
        ),
        (
            0x2000_0340,
            vec![
                0x1000_00e0,
                FIRST_FILE_HANDLE,
                fixture.len() as u32 + 1,
                0,
                0,
            ],
        ),
    ];
    for (address, words) in &frames {
        let bytes = words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>();
        write_initialization(&mut memory, owner, GuestAddress(*address), &bytes);
    }
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(BYTES_READ_ADDRESS),
        &u32::MAX.to_le_bytes(),
    );
    memory.seal_owner(owner).expect("seal fixture memory");
    acknowledge_initial_reflection(&mut memory);

    let mut runtime = controlled_runtime(memory);
    runtime
        .register_source_snapshot(manifest)
        .expect("register controlled source snapshot");

    let create = dispatch(&dispatcher, &endpoints[0], frames[0].0, &mut runtime);
    let create = expect_immediate(create);
    assert_eq!(create.eax, Some(FIRST_FILE_HANDLE));
    assert_eq!(runtime.current_last_error().unwrap(), 0x1234);
    let handle = RuntimeHandle(FIRST_FILE_HANDLE);
    let file_before_share_failure = runtime.source_file_for_handle(handle).unwrap().clone();
    let sharing = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[0],
        frames[12].0,
        &mut runtime,
    ));
    assert_eq!(sharing.eax, Some(u32::MAX));
    assert_eq!(runtime.current_last_error().unwrap(), 32);
    assert_eq!(
        runtime.source_file_for_handle(handle).unwrap(),
        &file_before_share_failure
    );
    assert_eq!(
        runtime
            .object_for_handle(handle, RuntimeObjectKind::File)
            .expect("file object")
            .open_handles,
        1
    );
    runtime.set_current_last_error(0x1234).unwrap();

    let size = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[1],
        frames[1].0,
        &mut runtime,
    ));
    assert_eq!(size.eax, Some(fixture.len() as u32));
    assert_eq!(runtime.current_last_error().unwrap(), 0x1234);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(BYTES_READ_ADDRESS), 4)
            .unwrap(),
        u32::MAX.to_le_bytes()
    );

    let size_high = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[1],
        frames[7].0,
        &mut runtime,
    ));
    assert_eq!(size_high.eax, Some(fixture.len() as u32));
    assert_eq!(runtime.current_last_error().unwrap(), 0x1234);
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(BYTES_READ_ADDRESS), 4)
            .unwrap(),
        0_u32.to_le_bytes()
    );

    let file_before_bad_output = runtime.source_file_for_handle(handle).unwrap().clone();
    let memory_before_bad_output = runtime.memory.stats();
    let invalid_output = dispatch(&dispatcher, &endpoints[1], frames[8].0, &mut runtime);
    assert!(matches!(invalid_output, DispatchOutcome::Fault(_)));
    assert_eq!(
        runtime.source_file_for_handle(handle).unwrap(),
        &file_before_bad_output
    );
    assert_eq!(runtime.current_last_error().unwrap(), 0x1234);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.memory.stats(), memory_before_bad_output);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(BYTES_READ_ADDRESS), 4)
            .unwrap(),
        0_u32.to_le_bytes()
    );

    let invalid_handle = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[1],
        frames[9].0,
        &mut runtime,
    ));
    assert_eq!(invalid_handle.eax, Some(u32::MAX));
    assert_eq!(runtime.current_last_error().unwrap(), 6);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(BYTES_READ_ADDRESS), 4)
            .unwrap(),
        0_u32.to_le_bytes()
    );

    let first_call = expect_deferred(dispatch(
        &dispatcher,
        &endpoints[2],
        frames[2].0,
        &mut runtime,
    ));
    assert_eq!(
        decode_words(&first_call.request.payload),
        vec![1, 1, 0, 0, 16]
    );
    assert_eq!(
        runtime.scheduler_state().expect("waiting scheduler"),
        SchedulerThreadState::WaitingHost { request_id: 1 }
    );

    let file_before_wrong = runtime.source_file_for_handle(handle).unwrap().clone();
    let last_error_before_wrong = runtime.current_last_error().unwrap();
    let buffer_before_wrong = runtime
        .memory
        .read(GuestAddress(BUFFER_ADDRESS), 16)
        .expect("buffer before wrong completion")
        .to_vec();
    let wrong = complete_kernel32_source_read(
        &dispatcher,
        &first_call,
        RuntimeSourceReadHostResponse {
            request_id: 1,
            source_id: 1,
            object_id: 99,
            offset: 0,
            requested_bytes: 16,
            transferred_bytes: 16,
            win32_error: 0,
            bytes: fixture[..16].to_vec(),
        },
        &mut runtime,
    );
    assert!(matches!(wrong, DispatchOutcome::Fault(_)));
    assert_eq!(
        runtime.source_file_for_handle(handle).unwrap(),
        &file_before_wrong
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: first_call.request.request_id
        }
    );
    assert_eq!(
        runtime.current_last_error().unwrap(),
        last_error_before_wrong
    );
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(BUFFER_ADDRESS), 16)
            .expect("buffer after wrong completion"),
        buffer_before_wrong
    );

    runtime
        .memory
        .protect(
            owner,
            GuestRange::new(GuestAddress(BUFFER_ADDRESS), GUEST_PAGE_SIZE)
                .expect("source output page range"),
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .expect("make source output page read-only");
    acknowledge_pending_delta(&mut runtime.memory);
    let file_before_memory_fault = runtime.source_file_for_handle(handle).unwrap().clone();
    let memory_before_memory_fault = runtime.memory.stats();
    let memory_fault = complete_kernel32_source_read(
        &dispatcher,
        &first_call,
        RuntimeSourceReadHostResponse {
            request_id: 1,
            source_id: 1,
            object_id: 1,
            offset: 0,
            requested_bytes: 16,
            transferred_bytes: 16,
            win32_error: 0,
            bytes: fixture[..16].to_vec(),
        },
        &mut runtime,
    );
    assert!(matches!(memory_fault, DispatchOutcome::Fault(_)));
    assert_eq!(
        runtime.source_file_for_handle(handle).unwrap(),
        &file_before_memory_fault
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: first_call.request.request_id
        }
    );
    assert_eq!(
        runtime.current_last_error().unwrap(),
        last_error_before_wrong
    );
    assert_eq!(runtime.memory.stats(), memory_before_memory_fault);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(BUFFER_ADDRESS), 16)
            .expect("buffer after failed guest-memory completion"),
        buffer_before_wrong
    );
    runtime
        .memory
        .protect(
            owner,
            GuestRange::new(GuestAddress(BUFFER_ADDRESS), GUEST_PAGE_SIZE)
                .expect("source output page range"),
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .expect("restore source output page writes");
    acknowledge_pending_delta(&mut runtime.memory);

    let first_outcome = complete_kernel32_source_read(
        &dispatcher,
        &first_call,
        RuntimeSourceReadHostResponse {
            request_id: 1,
            source_id: 1,
            object_id: 1,
            offset: 0,
            requested_bytes: 16,
            transferred_bytes: 16,
            win32_error: 0,
            bytes: fixture[..16].to_vec(),
        },
        &mut runtime,
    );
    let first_outcome = expect_immediate(first_outcome);
    assert_eq!(first_outcome.eax, Some(1));
    assert_eq!(runtime.source_file_for_handle(handle).unwrap().cursor, 16);
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(BUFFER_ADDRESS), 16)
            .expect("first source bytes"),
        &fixture[..16]
    );
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(BYTES_READ_ADDRESS), 4)
            .expect("first byte count"),
        16_u32.to_le_bytes()
    );
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));

    let file_before_duplicate = runtime.source_file_for_handle(handle).unwrap().clone();
    let memory_before_duplicate = runtime.memory.stats();
    let last_error_before_duplicate = runtime.current_last_error().unwrap();
    let duplicate = complete_kernel32_source_read(
        &dispatcher,
        &first_call,
        RuntimeSourceReadHostResponse {
            request_id: 1,
            source_id: 1,
            object_id: 1,
            offset: 0,
            requested_bytes: 16,
            transferred_bytes: 16,
            win32_error: 0,
            bytes: fixture[..16].to_vec(),
        },
        &mut runtime,
    );
    assert!(matches!(duplicate, DispatchOutcome::Fault(_)));
    assert_eq!(
        runtime.source_file_for_handle(handle).unwrap(),
        &file_before_duplicate
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(
        runtime.current_last_error().unwrap(),
        last_error_before_duplicate
    );
    assert_eq!(runtime.memory.stats(), memory_before_duplicate);

    let seek = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[3],
        frames[3].0,
        &mut runtime,
    ));
    assert_eq!(seek.eax, Some(5));
    let last_error = runtime.current_last_error().unwrap();
    let beyond_eof = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[3],
        frames[13].0,
        &mut runtime,
    ));
    assert_eq!(beyond_eof.eax, Some(fixture.len() as u32 + 1));
    assert_eq!(runtime.current_last_error().unwrap(), last_error);
    assert_eq!(runtime.seek_source_file(handle, 5, 0, None).unwrap(), Ok(5));
    for (frame, error) in [(10, 131), (11, 87)] {
        let file_before = runtime.source_file_for_handle(handle).unwrap().clone();
        let result = expect_immediate(dispatch(
            &dispatcher,
            &endpoints[3],
            frames[frame].0,
            &mut runtime,
        ));
        assert_eq!(result.eax, Some(u32::MAX));
        assert_eq!(runtime.current_last_error().unwrap(), error);
        assert_eq!(runtime.source_file_for_handle(handle).unwrap().cursor, 5);
        // Failure changes LastError, not file identity, sharing, or cursor.
        assert_eq!(
            runtime.source_file_for_handle(handle).unwrap(),
            &file_before
        );
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
    }
    // Synchronous size and seek successes preserve the preceding invalid-origin error.
    for (endpoint_index, frame_index, expected_eax) in [(1, 1, fixture.len() as u32), (3, 3, 5)] {
        let result = expect_immediate(dispatch(
            &dispatcher,
            &endpoints[endpoint_index],
            frames[frame_index].0,
            &mut runtime,
        ));
        assert_eq!(result.eax, Some(expected_eax));
        assert_eq!(runtime.current_last_error().unwrap(), 87);
    }
    for (distance, origin, error) in [(-6, 1, 131), (0, 3, 87)] {
        assert_eq!(
            runtime
                .seek_source_file(handle, distance, origin, None)
                .unwrap(),
            Err(error)
        );
        assert_eq!(runtime.source_file_for_handle(handle).unwrap().cursor, 5);
    }
    let second_call = expect_deferred(dispatch(
        &dispatcher,
        &endpoints[2],
        frames[4].0,
        &mut runtime,
    ));
    assert_eq!(
        decode_words(&second_call.request.payload),
        vec![1, 1, 5, 0, 8]
    );
    let second_outcome = complete_kernel32_source_read(
        &dispatcher,
        &second_call,
        RuntimeSourceReadHostResponse {
            request_id: 2,
            source_id: 1,
            object_id: 1,
            offset: 5,
            requested_bytes: 8,
            transferred_bytes: 8,
            win32_error: 0,
            bytes: fixture[5..13].to_vec(),
        },
        &mut runtime,
    );
    assert_eq!(expect_immediate(second_outcome).eax, Some(1));
    assert_eq!(runtime.source_file_for_handle(handle).unwrap().cursor, 13);
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));
    acknowledge_pending_delta(&mut runtime.memory);

    let close = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[4],
        frames[5].0,
        &mut runtime,
    ));
    assert_eq!(close.eax, Some(1));
    assert!(runtime.source_file_for_handle(handle).is_err());
    let invalid_size = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[1],
        frames[6].0,
        &mut runtime,
    ));
    assert_eq!(invalid_size.eax, Some(u32::MAX));
    assert_eq!(runtime.current_last_error().unwrap(), 6);
}

#[test]
fn source_file_sharing_mount_and_pending_exit_paths_preserve_authority() {
    let mut runtime = controlled_runtime(GuestMemory::new(MemoryLimits::default()).unwrap());
    runtime
        .register_source_snapshot(source_manifest())
        .expect("register controlled source snapshot");
    let first = runtime
        .open_file_a(
            b"C:\\Vine\\fixtures\\storage\\source-read-v1.dat\0",
            0x8000_0000,
            0,
            3,
            0x0800_0020,
        )
        .expect("first source open")
        .expect("no Windows file error")
        .expect("known source path");
    let file_before_share_rejection = runtime.source_file_for_handle(first).unwrap().clone();
    let sharing = runtime
        .open_file_a(
            b"c:/vine/FIXTURES/STORAGE/SOURCE-READ-V1.DAT\0",
            0x8000_0000,
            1,
            3,
            0x0800_0020,
        )
        .expect("share conflict is a Windows failure")
        .expect_err("read share must conflict");
    assert_eq!(sharing, 32);
    assert_eq!(
        runtime.source_file_for_handle(first).unwrap(),
        &file_before_share_rejection
    );
    assert_eq!(
        runtime
            .object_for_handle(first, RuntimeObjectKind::File)
            .unwrap()
            .open_handles,
        1
    );
    runtime
        .open_file_a(b"..\\outside.dat\0", 0x8000_0000, 1, 3, 0x0800_0020)
        .expect_err("mount escape must fail");
    assert_eq!(
        runtime.source_file_for_handle(first).unwrap(),
        &file_before_share_rejection
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.current_last_error().unwrap(), 0x1234);

    assert!(
        runtime
            .begin_source_file_read(
                first,
                GuestAddress(BUFFER_ADDRESS),
                GuestAddress(BYTES_READ_ADDRESS),
                0,
            )
            .is_err()
    );
    assert_eq!(
        runtime.source_file_for_handle(first).unwrap(),
        &file_before_share_rejection
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.current_last_error().unwrap(), 0x1234);

    let request = runtime
        .begin_source_file_read(
            first,
            GuestAddress(BUFFER_ADDRESS),
            GuestAddress(BYTES_READ_ADDRESS),
            8,
        )
        .expect("begin source read");
    assert_eq!(request.request_id, 1);
    let file_with_pending_read = runtime.source_file_for_handle(first).unwrap().clone();
    assert!(
        runtime
            .begin_source_file_read(
                first,
                GuestAddress(BUFFER_ADDRESS),
                GuestAddress(BYTES_READ_ADDRESS),
                8,
            )
            .is_err()
    );
    assert_eq!(
        runtime.source_file_for_handle(first).unwrap(),
        &file_with_pending_read
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: request.request_id
        }
    );
    assert_eq!(runtime.current_last_error().unwrap(), 0x1234);
    runtime
        .request_current_thread_exit(9)
        .expect("thread exit cancels pending source read");
    assert!(runtime.scheduler().host_requests().is_empty());
    assert!(runtime.complete_host_request(request.request_id).is_err());
    assert_eq!(
        runtime.source_file_for_handle(first).unwrap(),
        &file_before_share_rejection
    );
    runtime
        .complete_current_thread_exit()
        .expect("complete source reader exit");
    assert_eq!(
        runtime.process().lifecycle(),
        RuntimeProcessLifecycleState::Running
    );
    assert_eq!(
        runtime
            .scheduler_state_for_thread(RuntimeThreadId(THREAD_ID))
            .expect("terminated scheduler"),
        SchedulerThreadState::Terminated { exit_code: 9 }
    );
}

#[test]
fn zero_length_read_file_writes_only_the_count_without_browser_work() {
    let endpoint = endpoint(77, "ReadFile");
    let dispatcher = ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap();
    let (mut memory, owner) = initialized_memory();
    let frame = [
        0x1000_0010_u32,
        FIRST_FILE_HANDLE,
        0,
        0,
        BYTES_READ_ADDRESS,
        0,
    ];
    let invalid = [0x1000_0020_u32, FIRST_FILE_HANDLE, 0, 0, 0x3000_0000, 0];
    for (address, words) in [(0x2000_0000, frame), (0x2000_0040, invalid)] {
        write_initialization(
            &mut memory,
            owner,
            GuestAddress(address),
            &words
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
    }
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(BYTES_READ_ADDRESS),
        &u32::MAX.to_le_bytes(),
    );
    memory.seal_owner(owner).unwrap();
    acknowledge_initial_reflection(&mut memory);
    let mut runtime = controlled_runtime(memory);
    runtime.register_source_snapshot(source_manifest()).unwrap();
    let opened = runtime
        .open_file_a(
            b"fixtures\\storage\\source-read-v1.dat\0",
            0x8000_0000,
            1,
            3,
            0x0800_0020,
        )
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(opened.0, FIRST_FILE_HANDLE);
    let call = |stack_pointer, runtime: &mut Runtime| {
        dispatcher.dispatch_owned(
            crate::ApiOwnedTrap {
                endpoint_id: endpoint.id,
                endpoint_address: endpoint.address,
                stack_pointer: GuestAddress(stack_pointer),
                return_address: GuestAddress(if stack_pointer == 0x2000_0000 {
                    frame[0]
                } else {
                    invalid[0]
                }),
            },
            runtime,
            None,
            dispatch_kernel32_api,
        )
    };
    let complete = expect_immediate(call(0x2000_0000, &mut runtime));
    assert_eq!(complete.eax, Some(1));
    assert_eq!(runtime.current_last_error().unwrap(), 0);
    acknowledge_pending_delta(&mut runtime.memory);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(BYTES_READ_ADDRESS), 4)
            .unwrap(),
        [0; 4]
    );
    assert_eq!(runtime.source_file_for_handle(opened).unwrap().cursor, 0);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );

    let file_before_fault = runtime.source_file_for_handle(opened).unwrap().clone();
    let memory_before_fault = runtime.memory.stats();
    let fault = call(0x2000_0040, &mut runtime);
    assert!(matches!(fault, DispatchOutcome::Fault(_)));
    assert_eq!(
        runtime.source_file_for_handle(opened).unwrap(),
        &file_before_fault
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.current_last_error().unwrap(), 0);
    assert_eq!(runtime.memory.stats(), memory_before_fault);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(BYTES_READ_ADDRESS), 4)
            .unwrap(),
        [0; 4]
    );
}

#[test]
fn null_file_handles_complete_with_windows_failure_not_an_abi_fault() {
    for name in ["GetFileType", "FlushFileBuffers"] {
        let api = endpoint(90, name);
        let dispatcher = ApiDispatcher::from_endpoints([api.clone()]).unwrap();
        let (mut memory, owner) = initialized_memory();
        let mut stack = Vec::new();
        stack.extend_from_slice(&0x1000_0030_u32.to_le_bytes());
        stack.extend_from_slice(&0_u32.to_le_bytes());
        write_initialization(&mut memory, owner, MEMORY_BASE, &stack);
        memory.seal_owner(owner).unwrap();
        acknowledge_initial_reflection(&mut memory);
        let mut runtime = controlled_runtime(memory);
        let scheduler_before = runtime.scheduler_state().unwrap();
        let result = expect_immediate(dispatch(&dispatcher, &api, MEMORY_BASE.0, &mut runtime));
        assert_eq!(result.eax, Some(0));
        assert_eq!(runtime.current_last_error().unwrap(), 6);
        assert_eq!(runtime.scheduler_state().unwrap(), scheduler_before);
    }
}

#[test]
fn generic_source_handle_rejects_writes_and_flush_without_touching_bytes_or_cursor() {
    let write = endpoint(78, "WriteFile");
    let flush = endpoint(79, "FlushFileBuffers");
    let dispatcher = ApiDispatcher::from_endpoints([write.clone(), flush.clone()]).unwrap();
    let (mut memory, owner) = initialized_memory();
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(BUFFER_ADDRESS),
        b"never-written",
    );
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(BYTES_READ_ADDRESS),
        &0xdead_beef_u32.to_le_bytes(),
    );
    for (stack, words) in [
        (
            0x2000_0000,
            vec![
                0x1000_0030,
                FIRST_FILE_HANDLE,
                BUFFER_ADDRESS,
                1,
                BYTES_READ_ADDRESS,
                0,
            ],
        ),
        (0x2000_0040, vec![0x1000_0040, FIRST_FILE_HANDLE]),
        (
            0x2000_0080,
            vec![
                0x1000_0050,
                0x4601_9999,
                BUFFER_ADDRESS,
                1,
                BYTES_READ_ADDRESS,
                0,
            ],
        ),
    ] {
        write_initialization(
            &mut memory,
            owner,
            GuestAddress(stack),
            &words
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
    }
    memory.seal_owner(owner).unwrap();
    acknowledge_initial_reflection(&mut memory);
    let mut runtime = controlled_runtime(memory);
    runtime.register_source_snapshot(source_manifest()).unwrap();
    assert_eq!(
        runtime
            .open_file_a(
                b"fixtures\\storage\\source-read-v1.dat\0",
                0x8000_0000,
                1,
                3,
                0x0800_0020,
            )
            .unwrap()
            .unwrap()
            .unwrap()
            .0,
        FIRST_FILE_HANDLE
    );
    let generation = runtime.memory.stats().generation;
    for (endpoint, stack, error) in [
        (&write, 0x2000_0000, 5),
        (&flush, 0x2000_0040, 5),
        (&write, 0x2000_0080, 6),
    ] {
        runtime.set_current_last_error(0x1234).unwrap();
        let completion = expect_immediate(dispatcher.dispatch_owned(
            crate::ApiOwnedTrap {
                endpoint_id: endpoint.id,
                endpoint_address: endpoint.address,
                stack_pointer: GuestAddress(stack),
                return_address: GuestAddress(0x1000_0030 + (stack - 0x2000_0000) / 4),
            },
            &mut runtime,
            None,
            dispatch_kernel32_api,
        ));
        assert_eq!(completion.eax, Some(0));
        assert_eq!(runtime.current_last_error().unwrap(), error);
        assert_eq!(runtime.memory.stats().generation, generation);
        assert_eq!(
            runtime
                .memory
                .read(GuestAddress(BYTES_READ_ADDRESS), 4)
                .unwrap(),
            0xdead_beef_u32.to_le_bytes()
        );
        assert_eq!(
            runtime
                .source_file_for_handle(RuntimeHandle(FIRST_FILE_HANDLE))
                .unwrap()
                .cursor,
            0
        );
    }
}

fn dispatch(
    dispatcher: &ApiDispatcher,
    endpoint: &ApiEndpointIdentity,
    stack_pointer: u32,
    runtime: &mut Runtime,
) -> DispatchOutcome {
    let return_address = u32::from_le_bytes(
        runtime
            .memory
            .read(GuestAddress(stack_pointer), 4)
            .expect("call return address")
            .try_into()
            .expect("x86 word"),
    );
    dispatcher.dispatch_owned(
        crate::ApiOwnedTrap {
            endpoint_id: endpoint.id,
            endpoint_address: endpoint.address,
            stack_pointer: GuestAddress(stack_pointer),
            return_address: GuestAddress(return_address),
        },
        runtime,
        None,
        dispatch_kernel32_api,
    )
}

fn expect_immediate(outcome: DispatchOutcome) -> crate::ApiImmediateCompletion {
    match outcome {
        DispatchOutcome::Immediate(completion) => completion,
        other => panic!("expected immediate completion, got {other:?}"),
    }
}

fn expect_deferred(outcome: DispatchOutcome) -> ApiDeferredCall {
    match outcome {
        DispatchOutcome::Deferred(call) => call,
        other => panic!("expected deferred call, got {other:?}"),
    }
}

fn endpoint(id: u32, name: &str) -> ApiEndpointIdentity {
    ApiEndpointIdentity::from_dynamic_endpoint(&RuntimeDynamicEndpoint {
        id,
        address: GuestAddress(0x7f00_1000 + id * 0x10),
        module: "kernel32.dll".to_owned(),
        name: name.to_owned(),
        ordinal: None,
    })
}

fn initialized_memory() -> (GuestMemory, crate::MemoryOwnerId) {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 4,
        max_regions: 4,
        max_owners: 4,
        max_access_bytes: 1024 * 1024,
    })
    .expect("guest memory");
    let owner = memory
        .create_owner(MemoryOwnerKind::Runtime)
        .expect("memory owner");
    let range = memory
        .reserve_fixed(owner, MemoryRegionKind::Runtime, MEMORY_BASE, 0x3000)
        .expect("memory reservation");
    memory
        .commit(
            owner,
            range,
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .expect("memory commit");
    (memory, owner)
}

fn controlled_runtime(memory: GuestMemory) -> Runtime {
    let mut startup = vec![0; 68];
    startup[..4].copy_from_slice(&68_u32.to_le_bytes());
    let mut runtime = Runtime::new(
        RuntimeBootstrap {
            process_id: RuntimeProcessId(PROCESS_ID),
            initial_thread_id: RuntimeThreadId(THREAD_ID),
            initial_last_error: 0x1234,
            clock: RuntimeClockState {
                wall_clock_file_time: 134_282_016_000_000_000,
                tick_count: 3_600_000,
                performance_counter: 12_345_678_901,
                performance_frequency: 10_000_000,
            },
            startup_info: RuntimeStartupInfo { bytes: startup },
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
    .expect("runtime state");
    runtime
        .start_initial_thread()
        .expect("start initial thread");
    runtime
}

fn assert_find_data(
    memory: &GuestMemory,
    address: u32,
    expected_attributes: u32,
    expected_size: u64,
    expected_name: &[u8],
) {
    let bytes = memory
        .read(GuestAddress(address), 320)
        .expect("WIN32_FIND_DATAA");
    assert_eq!(
        u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
        expected_attributes
    );
    assert_eq!(bytes[4..28], [0; 24]);
    let size_high = u32::from_le_bytes(bytes[28..32].try_into().unwrap());
    let size_low = u32::from_le_bytes(bytes[32..36].try_into().unwrap());
    assert_eq!(
        u64::from(size_low) | (u64::from(size_high) << 32),
        expected_size
    );
    assert_eq!(&bytes[44..44 + expected_name.len()], expected_name);
    assert_eq!(bytes[44 + expected_name.len()], 0);
    assert_eq!(bytes[304..320], [0; 16]);
}

fn find_file_manifest() -> RuntimeSourceSnapshotManifest {
    RuntimeSourceSnapshotManifest {
        mount: RuntimeSourceMountManifest {
            source_id: 1,
            root_path_a: b"C:\\Vine\0".to_vec(),
        },
        files: [
            (1, b"data01099.arc".as_slice(), 10_u64),
            (2, b"DATA01100.ARC".as_slice(), 20_u64),
            (3, b"data01999.arc".as_slice(), 30_u64),
            (4, b"notes.txt".as_slice(), 40_u64),
            (5, b"assets\\nested.bin".as_slice(), 50_u64),
        ]
        .into_iter()
        .map(|(object_id, path, byte_length)| RuntimeSourceFileManifest {
            object_id,
            relative_path_a: path.to_vec(),
            byte_length,
        })
        .collect(),
    }
}

fn source_manifest() -> RuntimeSourceSnapshotManifest {
    RuntimeSourceSnapshotManifest {
        mount: RuntimeSourceMountManifest {
            source_id: 1,
            root_path_a: b"C:\\Vine\0".to_vec(),
        },
        files: vec![RuntimeSourceFileManifest {
            object_id: 1,
            relative_path_a: b"fixtures\\storage\\source-read-v1.dat".to_vec(),
            byte_length: 94,
        }],
    }
}

fn write_initialization(
    memory: &mut GuestMemory,
    owner: crate::MemoryOwnerId,
    address: GuestAddress,
    bytes: &[u8],
) {
    memory
        .write_initialization(owner, address, bytes)
        .expect("fixture initialization");
}

fn acknowledge_initial_reflection(memory: &mut GuestMemory) {
    let generation = memory
        .pending_reflection()
        .expect("initial reflection")
        .invalidation
        .generation;
    memory
        .acknowledge_delta(generation)
        .expect("acknowledge initial reflection");
}

fn acknowledge_pending_delta(memory: &mut GuestMemory) {
    let generation = memory.pending_delta().expect("pending delta").generation;
    memory
        .acknowledge_delta(generation)
        .expect("acknowledge pending delta");
}

fn decode_words(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().expect("x86 word")))
        .collect()
}
