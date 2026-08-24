use vine_core::complete_kernel32_writable_delete;
use vine_core::{
    ApiBehavior, ApiDeferredCall, ApiDispatcher, ApiEndpointIdentity, ApiFaultClass,
    ApiReturnSchema, CREATE_DIRECTORY_A_IMPLEMENTATION, DELETE_FILE_A_IMPLEMENTATION,
    DispatchOutcome, ExecutionSession, FIND_CLOSE_IMPLEMENTATION, FIND_FIRST_FILE_A_IMPLEMENTATION,
    FIND_NEXT_FILE_A_IMPLEMENTATION, GET_FILE_ATTRIBUTES_A_IMPLEMENTATION, GUEST_PAGE_SIZE,
    GuestAddress, GuestMemory, GuestRange, Kernel32CompatibilityPolicy, Kernel32DeleteFileAPolicy,
    Kernel32FindFileAPolicy, Kernel32GetFileAttributesAPolicy, LastErrorPolicy, MemoryLimits,
    MemoryOwnerKind, MemoryRegionKind, PageProtection, ProcessAddressSpaceLayout,
    RUNTIME_WRITABLE_PROFILE_FILE_LIMIT, RUNTIME_WRITABLE_PROFILE_SCHEMA,
    RUNTIME_WRITABLE_PROFILE_VERSION, RegisterUpdate, RuntimeBootstrap, RuntimeClockState,
    RuntimeDynamicEndpoint, RuntimeHandle, RuntimeObjectKind, RuntimeProcessId,
    RuntimeProcessLifecycleState, RuntimeSchedulerEvent, RuntimeSourceFileManifest,
    RuntimeSourceMountManifest, RuntimeSourceReadHostResponse, RuntimeSourceSnapshotManifest,
    RuntimeStartupInfo, RuntimeState, RuntimeThreadId, RuntimeWritableCommitHostResponse,
    RuntimeWritableDirectoryCreateHostResponse, RuntimeWritableDirectoryManifest,
    RuntimeWritableFileManifest, RuntimeWritableProfileManifest, SchedulerThreadState,
    complete_kernel32_source_read, complete_kernel32_writable_commit,
    complete_kernel32_writable_directory_create, dispatch_kernel32_api_with_policy,
    kernel32_contract_for_endpoint,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
const PATH_ADDRESS: u32 = 0x2000_1000;
const BUFFER_ADDRESS: u32 = 0x2000_2000;
const BYTES_READ_ADDRESS: u32 = 0x2000_2f00;
const FIRST_FILE_HANDLE: u32 = 0x4601_0001;

#[test]
fn primary_title_save_catalog_creates_a_registered_slot_below_the_profile_bound() {
    assert_eq!(RUNTIME_WRITABLE_PROFILE_FILE_LIMIT, 128);
    let mut files = vec![RuntimeWritableFileManifest {
        object_id: 1,
        relative_path_a: b"BGI.gdb".to_vec(),
        exists: false,
        generation: 0,
        byte_length: 0,
        sha256: None,
    }];
    files.extend((0_u32..=102).map(|slot| RuntimeWritableFileManifest {
        object_id: slot + 2,
        relative_path_a: format!("UserData\\Sakuranouta{slot:03}.sud").into_bytes(),
        exists: false,
        generation: 0,
        byte_length: 0,
        sha256: None,
    }));

    let mut runtime = controlled_runtime();
    runtime
        .register_writable_profile(RuntimeWritableProfileManifest {
            schema: RUNTIME_WRITABLE_PROFILE_SCHEMA.to_owned(),
            schema_version: RUNTIME_WRITABLE_PROFILE_VERSION,
            title_fingerprint: "sha256:primary-title-save-catalog".to_owned(),
            profile_id: "default".to_owned(),
            root_path_a: b"C:\\Vine\0".to_vec(),
            quota_bytes: 16 * 1024 * 1024,
            files,
            directories: vec![RuntimeWritableDirectoryManifest {
                directory_id: 1,
                relative_path_a: b"UserData".to_vec(),
                exists: true,
                generation: 1,
            }],
        })
        .expect("register bounded primary save catalog");
    assert_eq!(runtime.writable_profile().unwrap().files.len(), 104);

    let opened = runtime
        .open_file_a(
            b"UserData\\Sakuranouta000.sud\0",
            0x4000_0000,
            0,
            2,
            0x0800_0020,
        )
        .expect("open registered save slot")
        .expect("create registered save slot");
    assert!(!opened.existed);
    assert!(runtime.writable_profile().unwrap().files[&2].exists);
}

#[test]
fn get_file_attributes_a_projects_only_registered_vfs_paths() {
    let policy: Kernel32CompatibilityPolicy = serde_json::from_str(include_str!(
        "../../../fixtures/runtime-policies/kernel32-get-file-attributes-a-v1.json"
    ))
    .expect("file-attributes policy JSON");
    policy.validate().expect("valid file-attributes policy");
    let (mut memory, owner) = initialized_memory();
    let endpoint = endpoint(30, "GetFileAttributesA");
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("valid GetFileAttributesA contract")
        .expect("implemented GetFileAttributesA contract");
    assert_eq!(
        contract.implementation_id,
        GET_FILE_ATTRIBUTES_A_IMPLEMENTATION
    );
    assert_eq!(contract.abi.return_value, ApiReturnSchema::U32);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");
    let cases = [
        (b".\0".as_slice(), 0x30, 0x1234),
        (b"fixtures\0".as_slice(), 0x30, 0x1234),
        (b"fixtures\\storage\0".as_slice(), 0x30, 0x1234),
        (
            b"fixtures\\storage\\source-read-v1.dat\0".as_slice(),
            0x20,
            0x1234,
        ),
        (b"fixtures\\storage\\missing.dat\0".as_slice(), u32::MAX, 2),
        (b"missing\\file.dat\0".as_slice(), u32::MAX, 3),
        (
            b"D:\\Vine\\fixtures\\storage\\source-read-v1.dat\0".as_slice(),
            u32::MAX,
            3,
        ),
    ];
    for (index, (path, _, _)) in cases.iter().enumerate() {
        let path_address = PATH_ADDRESS + u32::try_from(index).unwrap() * 0x80;
        let stack = MEMORY_BASE.0 + u32::try_from(index).unwrap() * 0x20;
        write_initialization(&mut memory, owner, GuestAddress(path_address), path);
        write_initialization(
            &mut memory,
            owner,
            GuestAddress(stack),
            &[0x1000_1000 + u32::try_from(index).unwrap(), path_address]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
    }
    memory.seal_owner(owner).expect("seal fixture memory");
    acknowledge_initial_reflection(&mut memory);
    let mut runtime = controlled_runtime();
    runtime
        .register_source_snapshot(source_manifest())
        .expect("register controlled source snapshot");

    for (index, (_, expected_attributes, expected_last_error)) in cases.into_iter().enumerate() {
        runtime.set_current_last_error(0x1234).unwrap();
        let before = runtime.clone();
        let completion = expect_immediate(dispatch(
            &dispatcher,
            &endpoint,
            MEMORY_BASE.0 + u32::try_from(index).unwrap() * 0x20,
            &policy,
            &mut memory,
            &mut runtime,
        ));
        assert_eq!(completion.eax, RegisterUpdate::Set(expected_attributes));
        assert_eq!(completion.last_error, expected_last_error);
        let mut expected_runtime = before;
        expected_runtime
            .set_current_last_error(expected_last_error)
            .unwrap();
        assert_eq!(runtime, expected_runtime);
    }

    runtime.set_current_last_error(0x5566_7788).unwrap();
    let before = runtime.clone();
    let disabled = dispatch(
        &dispatcher,
        &endpoint,
        MEMORY_BASE.0,
        &source_policy(),
        &mut memory,
        &mut runtime,
    );
    assert!(matches!(
        disabled,
            DispatchOutcome::Fault(fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "get-file-attributes-a-policy-disabled"
    ));
    assert_eq!(runtime, before);
}

#[test]
fn delete_file_a_durably_removes_only_an_existing_writable_profile_file() {
    let mut policy = source_policy();
    policy
        .source_files
        .as_mut()
        .expect("source file policy")
        .delete_file_a = Some(Kernel32DeleteFileAPolicy {
        missing_file_only: true,
        allow_writable_profile_file: true,
    });
    policy.validate().expect("valid DeleteFileA policy");
    let endpoint = endpoint(38, "DeleteFileA");
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("valid DeleteFileA contract")
        .expect("implemented DeleteFileA contract");
    assert_eq!(contract.implementation_id, DELETE_FILE_A_IMPLEMENTATION);
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Bool32);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");

    let (mut memory, owner) = initialized_memory();
    let paths = [
        b"UserData\\Sakuranouta100.sud\0".as_slice(),
        b"Missing\\Sakuranouta100.sud\0".as_slice(),
        b"data01099.arc\0".as_slice(),
        b"BGI.gdb\0".as_slice(),
        b"UserData\0".as_slice(),
        b"bad<name.sud\0".as_slice(),
    ];
    for (index, path) in paths.iter().enumerate() {
        let path_address = PATH_ADDRESS + u32::try_from(index).unwrap() * 0x80;
        let stack = MEMORY_BASE.0 + u32::try_from(index).unwrap() * 0x20;
        write_initialization(&mut memory, owner, GuestAddress(path_address), path);
        write_initialization(
            &mut memory,
            owner,
            GuestAddress(stack),
            &[0x1000_1000 + u32::try_from(index).unwrap(), path_address]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
    }
    let null_stack = MEMORY_BASE.0 + u32::try_from(paths.len()).unwrap() * 0x20;
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(null_stack),
        &[0x1000_1100, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    memory.seal_owner(owner).expect("seal DeleteFileA memory");
    acknowledge_initial_reflection(&mut memory);

    let mut runtime = controlled_runtime();
    runtime
        .register_source_snapshot(find_file_manifest())
        .expect("register source snapshot");
    runtime
        .register_writable_profile(RuntimeWritableProfileManifest {
            schema: RUNTIME_WRITABLE_PROFILE_SCHEMA.to_owned(),
            schema_version: RUNTIME_WRITABLE_PROFILE_VERSION,
            title_fingerprint: "sha256:find-file-test".to_owned(),
            profile_id: "default".to_owned(),
            root_path_a: b"C:\\Vine\0".to_vec(),
            quota_bytes: 1024,
            files: vec![RuntimeWritableFileManifest {
                object_id: 21,
                relative_path_a: b"BGI.gdb".to_vec(),
                exists: true,
                generation: 1,
                byte_length: 3,
                sha256: Some("0".repeat(64)),
            }],
            directories: vec![RuntimeWritableDirectoryManifest {
                directory_id: 31,
                relative_path_a: b"UserData".to_vec(),
                exists: true,
                generation: 1,
            }],
        })
        .expect("register writable profile");

    for (index, expected_error) in [2_u32, 3].into_iter().enumerate() {
        runtime.set_current_last_error(0x5566_7788).unwrap();
        let before = runtime.clone();
        let result = expect_immediate(dispatch(
            &dispatcher,
            &endpoint,
            MEMORY_BASE.0 + u32::try_from(index).unwrap() * 0x20,
            &policy,
            &mut memory,
            &mut runtime,
        ));
        assert_eq!(result.eax, RegisterUpdate::Set(0));
        assert_eq!(result.last_error, expected_error);
        let mut expected_runtime = before;
        expected_runtime
            .set_current_last_error(expected_error)
            .unwrap();
        assert_eq!(runtime, expected_runtime);
    }

    for (index, expected_code) in [
        (2_usize, "delete-file-a-existing-file-unsupported"),
        (4, "delete-file-a-directory-unsupported"),
        (5, "delete-file-a-path-unsupported"),
    ] {
        let before = runtime.clone();
        let result = dispatch(
            &dispatcher,
            &endpoint,
            MEMORY_BASE.0 + u32::try_from(index).unwrap() * 0x20,
            &policy,
            &mut memory,
            &mut runtime,
        );
        assert!(
            matches!(
                &result,
                DispatchOutcome::Fault(fault)
                    if fault.class == ApiFaultClass::Unsupported && fault.code == expected_code
            ),
            "unexpected DeleteFileA result at case {index}: {result:?}"
        );
        assert_eq!(runtime, before);
    }

    runtime.set_current_last_error(0x5566_7788).unwrap();
    let failed_delete = expect_deferred(dispatch(
        &dispatcher,
        &endpoint,
        MEMORY_BASE.0 + 3 * 0x20,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(
        decode_words(&failed_delete.request.payload),
        vec![1, 21, 1, 0]
    );
    assert!(runtime.writable_profile().unwrap().files[&21].exists);
    let (failed, failed_completion) = complete_kernel32_writable_delete(
        &dispatcher,
        failed_delete,
        RuntimeWritableCommitHostResponse {
            request_id: 1,
            object_id: 21,
            base_generation: 1,
            committed_generation: 1,
            win32_error: 29,
        },
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    let failed = expect_immediate(failed);
    assert_eq!(failed.eax, RegisterUpdate::Set(0));
    assert_eq!(failed.last_error, 29);
    assert!(
        !failed_completion
            .expect("failed delete completion")
            .succeeded
    );
    assert!(runtime.writable_profile().unwrap().files[&21].exists);
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));

    runtime.set_current_last_error(0x1122_3344).unwrap();
    let delete = expect_deferred(dispatch(
        &dispatcher,
        &endpoint,
        MEMORY_BASE.0 + 3 * 0x20,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    let (deleted, completion) = complete_kernel32_writable_delete(
        &dispatcher,
        delete,
        RuntimeWritableCommitHostResponse {
            request_id: 2,
            object_id: 21,
            base_generation: 1,
            committed_generation: 2,
            win32_error: 0,
        },
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    let deleted = expect_immediate(deleted);
    assert_eq!(deleted.eax, RegisterUpdate::Set(1));
    assert_eq!(deleted.last_error, 0x1122_3344);
    assert!(completion.expect("delete completion").succeeded);
    let file = &runtime.writable_profile().unwrap().files[&21];
    assert!(!file.exists);
    assert_eq!(file.generation, 0);
    assert_eq!(file.byte_length, 0);
    assert_eq!(file.sha256, None);
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));

    let before_null = runtime.clone();
    assert!(matches!(
        dispatch(
            &dispatcher,
            &endpoint,
            null_stack,
            &policy,
            &mut memory,
            &mut runtime,
        ),
        DispatchOutcome::Fault(ref fault) if fault.class == ApiFaultClass::InvalidCall
    ));
    assert_eq!(runtime, before_null);

    let before_disabled = runtime.clone();
    assert!(matches!(
        dispatch(
            &dispatcher,
            &endpoint,
            MEMORY_BASE.0,
            &source_policy(),
            &mut memory,
            &mut runtime,
        ),
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "delete-file-a-policy-disabled"
    ));
    assert_eq!(runtime, before_disabled);

    let mut invalid_policy = policy;
    invalid_policy
        .source_files
        .as_mut()
        .unwrap()
        .delete_file_a
        .as_mut()
        .unwrap()
        .missing_file_only = false;
    assert!(invalid_policy.validate().is_err());
}

#[test]
fn find_file_a_enumerates_one_deterministic_search_handle_atomically() {
    let mut policy = source_policy();
    let source_files = policy.source_files.as_mut().expect("source file policy");
    source_files.get_file_attributes_a = Some(Kernel32GetFileAttributesAPolicy {
        regular_file_attributes: 0x20,
        directory_attributes: 0x30,
    });
    source_files.find_file_a = Some(Kernel32FindFileAPolicy {
        maximum_matches: 64,
    });
    policy.validate().expect("valid FindFile policy");

    let endpoints = [
        endpoint(40, "FindFirstFileA"),
        endpoint(41, "FindNextFileA"),
        endpoint(42, "FindClose"),
        endpoint(43, "CloseHandle"),
    ];
    let contracts = endpoints
        .iter()
        .cloned()
        .map(|endpoint| {
            kernel32_contract_for_endpoint(endpoint)
                .expect("valid FindFile contract")
                .expect("implemented FindFile contract")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        contracts[0].implementation_id,
        FIND_FIRST_FILE_A_IMPLEMENTATION
    );
    assert_eq!(
        contracts[1].implementation_id,
        FIND_NEXT_FILE_A_IMPLEMENTATION
    );
    assert_eq!(contracts[2].implementation_id, FIND_CLOSE_IMPLEMENTATION);
    assert_eq!(
        contracts[0].behavior,
        ApiBehavior::ImmediateWithCommittedIndirectGuestWrites
    );
    assert_eq!(
        contracts[1].behavior,
        ApiBehavior::ImmediateWithCommittedIndirectGuestWrites
    );
    let dispatcher = ApiDispatcher::new(endpoints.clone(), contracts).expect("FindFile dispatcher");

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

    let mut runtime = controlled_runtime();
    runtime
        .register_source_snapshot(find_file_manifest())
        .expect("register FindFile snapshot");
    let runtime_before_invalid_output = runtime.clone();
    let invalid_output_result = dispatch(
        &dispatcher,
        &endpoints[0],
        frames[0].0,
        &policy,
        &mut memory,
        &mut runtime,
    );
    assert!(matches!(
        invalid_output_result,
        DispatchOutcome::Fault(ref fault) if fault.class == ApiFaultClass::InvalidCall
    ));
    assert_eq!(runtime, runtime_before_invalid_output);

    runtime.set_current_last_error(0x7654).unwrap();
    let first = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[0],
        frames[1].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(first.eax, RegisterUpdate::Set(search_handle));
    assert_eq!(first.last_error, 0x7654);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        runtime
            .object_for_handle(RuntimeHandle(search_handle), RuntimeObjectKind::FileSearch)
            .expect("live search object")
            .namespace,
        "vine.runtime-file-search.v1"
    );
    assert_find_data(&memory, BUFFER_ADDRESS, 0x20, 10, b"data01099.arc");

    for (frame_index, expected_name, expected_size) in [
        (2, b"DATA01100.ARC".as_slice(), 20_u64),
        (3, b"data01999.arc".as_slice(), 30_u64),
    ] {
        let next = expect_immediate(dispatch(
            &dispatcher,
            &endpoints[1],
            frames[frame_index].0,
            &policy,
            &mut memory,
            &mut runtime,
        ));
        assert_eq!(next.eax, RegisterUpdate::Set(1));
        assert_eq!(next.last_error, 0x7654);
        acknowledge_pending_delta(&mut memory);
        assert_find_data(
            &memory,
            BUFFER_ADDRESS + u32::try_from(frame_index - 1).unwrap() * 0x200,
            0x20,
            expected_size,
            expected_name,
        );
    }

    let terminal_output = BUFFER_ADDRESS + 0x600;
    let terminal_before = memory
        .read(GuestAddress(terminal_output), 320)
        .unwrap()
        .to_vec();
    let terminal = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[1],
        frames[4].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(terminal.eax, RegisterUpdate::Set(0));
    assert_eq!(terminal.last_error, 18);
    assert_eq!(
        memory.read(GuestAddress(terminal_output), 320).unwrap(),
        terminal_before
    );

    let close_handle = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[3],
        frames[6].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(close_handle.eax, RegisterUpdate::Set(0));
    assert_eq!(close_handle.last_error, 6);
    assert!(
        runtime
            .file_search_for_handle(RuntimeHandle(search_handle))
            .is_ok()
    );

    let closed = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[2],
        frames[7].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(closed.eax, RegisterUpdate::Set(1));
    assert_eq!(closed.last_error, 6);
    assert!(
        runtime
            .file_search_for_handle(RuntimeHandle(search_handle))
            .is_err()
    );
    let stale = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[1],
        frames[8].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(stale.eax, RegisterUpdate::Set(0));
    assert_eq!(stale.last_error, 6);
}

#[test]
fn create_directory_a_publishes_only_after_durable_host_completion() {
    let policy: Kernel32CompatibilityPolicy = serde_json::from_str(include_str!(
        "../../../fixtures/runtime-policies/kernel32-create-directory-a-v1.json"
    ))
    .expect("CreateDirectoryA policy JSON");
    policy.validate().expect("valid CreateDirectoryA policy");
    let endpoint = endpoint(31, "CreateDirectoryA");
    let contract = kernel32_contract_for_endpoint(endpoint.clone())
        .expect("valid CreateDirectoryA contract")
        .expect("implemented CreateDirectoryA contract");
    assert_eq!(
        contract.implementation_id,
        CREATE_DIRECTORY_A_IMPLEMENTATION
    );
    assert_eq!(contract.abi.return_value, ApiReturnSchema::Bool32);
    assert_eq!(contract.behavior, ApiBehavior::ImmediateOrDeferred);
    assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
    let dispatcher = ApiDispatcher::new([endpoint.clone()], [contract]).expect("dispatcher");

    let (mut memory, owner) = initialized_memory();
    let paths = [
        (PATH_ADDRESS, b"UserData\0".as_slice()),
        (PATH_ADDRESS + 0x80, b"Missing\\Child\0".as_slice()),
        (PATH_ADDRESS + 0x100, b"BGI.gdb\0".as_slice()),
        (PATH_ADDRESS + 0x180, b"Cache\0".as_slice()),
    ];
    for (address, path) in paths {
        write_initialization(&mut memory, owner, GuestAddress(address), path);
    }
    let frames = [
        (MEMORY_BASE.0, PATH_ADDRESS, 0),
        (MEMORY_BASE.0 + 0x20, PATH_ADDRESS + 0x80, 0),
        (MEMORY_BASE.0 + 0x40, PATH_ADDRESS + 0x100, 0),
        (MEMORY_BASE.0 + 0x60, PATH_ADDRESS, 0x2000_2200),
        (MEMORY_BASE.0 + 0x80, PATH_ADDRESS + 0x180, 0),
    ];
    for (stack, path, security_attributes) in frames {
        write_initialization(
            &mut memory,
            owner,
            GuestAddress(stack),
            &[0x1000_1000, path, security_attributes]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        );
    }
    memory.seal_owner(owner).expect("seal fixture memory");
    acknowledge_initial_reflection(&mut memory);

    let mut runtime = controlled_runtime();
    runtime
        .register_writable_profile(RuntimeWritableProfileManifest {
            schema: RUNTIME_WRITABLE_PROFILE_SCHEMA.to_owned(),
            schema_version: RUNTIME_WRITABLE_PROFILE_VERSION,
            title_fingerprint: "sha256:test-title".to_owned(),
            profile_id: "default".to_owned(),
            root_path_a: b"C:\\Vine\0".to_vec(),
            quota_bytes: 1024,
            files: vec![RuntimeWritableFileManifest {
                object_id: 21,
                relative_path_a: b"BGI.gdb".to_vec(),
                exists: false,
                generation: 0,
                byte_length: 0,
                sha256: None,
            }],
            directories: vec![
                RuntimeWritableDirectoryManifest {
                    directory_id: 31,
                    relative_path_a: b"UserData".to_vec(),
                    exists: false,
                    generation: 0,
                },
                RuntimeWritableDirectoryManifest {
                    directory_id: 32,
                    relative_path_a: b"Missing\\Child".to_vec(),
                    exists: false,
                    generation: 0,
                },
                RuntimeWritableDirectoryManifest {
                    directory_id: 33,
                    relative_path_a: b"Cache".to_vec(),
                    exists: false,
                    generation: 0,
                },
            ],
        })
        .expect("register writable directories");

    let before_security_rejection = runtime.clone();
    let security_rejection = dispatch(
        &dispatcher,
        &endpoint,
        frames[3].0,
        &policy,
        &mut memory,
        &mut runtime,
    );
    assert!(matches!(
        security_rejection,
        DispatchOutcome::Fault(ref fault)
            if fault.class == ApiFaultClass::Unsupported
                && fault.code == "create-directory-a-security-attributes-unsupported"
    ));
    assert_eq!(runtime, before_security_rejection);

    runtime.set_current_last_error(0x1122_3344).unwrap();
    let create = expect_deferred(dispatch(
        &dispatcher,
        &endpoint,
        frames[0].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(decode_words(&create.request.payload), vec![1, 31, 0, 0]);
    assert!(!runtime.writable_profile().unwrap().directories[&31].exists);
    let runtime_before_invalid_completion = runtime.clone();
    let (invalid, invalid_completion) = complete_kernel32_writable_directory_create(
        &dispatcher,
        create.clone(),
        RuntimeWritableDirectoryCreateHostResponse {
            request_id: create.request.request_id,
            directory_id: 31,
            base_generation: 0,
            committed_generation: 2,
            win32_error: 0,
        },
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    assert!(matches!(invalid, DispatchOutcome::Fault(_)));
    assert!(invalid_completion.is_none());
    assert_eq!(runtime, runtime_before_invalid_completion);

    let (created, completion) = complete_kernel32_writable_directory_create(
        &dispatcher,
        create,
        RuntimeWritableDirectoryCreateHostResponse {
            request_id: 1,
            directory_id: 31,
            base_generation: 0,
            committed_generation: 1,
            win32_error: 0,
        },
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    let created = expect_immediate(created);
    assert_eq!(created.eax, RegisterUpdate::Set(1));
    assert_eq!(created.last_error, 0x1122_3344);
    assert!(completion.expect("directory completion").succeeded);
    assert!(runtime.writable_profile().unwrap().directories[&31].exists);
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));

    let duplicate = expect_immediate(dispatch(
        &dispatcher,
        &endpoint,
        frames[0].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(duplicate.eax, RegisterUpdate::Set(0));
    assert_eq!(duplicate.last_error, 183);
    let parent_missing = expect_immediate(dispatch(
        &dispatcher,
        &endpoint,
        frames[1].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(parent_missing.eax, RegisterUpdate::Set(0));
    assert_eq!(parent_missing.last_error, 3);
    let file_conflict = expect_immediate(dispatch(
        &dispatcher,
        &endpoint,
        frames[2].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(file_conflict.eax, RegisterUpdate::Set(0));
    assert_eq!(file_conflict.last_error, 183);

    let failed = expect_deferred(dispatch(
        &dispatcher,
        &endpoint,
        frames[4].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    let (failed, failed_completion) = complete_kernel32_writable_directory_create(
        &dispatcher,
        failed,
        RuntimeWritableDirectoryCreateHostResponse {
            request_id: 2,
            directory_id: 33,
            base_generation: 0,
            committed_generation: 0,
            win32_error: 29,
        },
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    let failed = expect_immediate(failed);
    assert_eq!(failed.eax, RegisterUpdate::Set(0));
    assert_eq!(failed.last_error, 29);
    assert!(!failed_completion.expect("failed completion").succeeded);
    assert!(!runtime.writable_profile().unwrap().directories[&33].exists);
}

#[test]
fn source_file_api_slice_opens_reads_seeks_and_closes_atomically() {
    let fixture = include_bytes!("../../../fixtures/storage/source-read-v1.dat");
    let policy = source_policy();
    let manifest = source_manifest();
    let (mut memory, owner) = initialized_memory();
    let endpoints = [
        endpoint(1, "CreateFileA"),
        endpoint(2, "GetFileSize"),
        endpoint(3, "ReadFile"),
        endpoint(4, "SetFilePointer"),
        endpoint(5, "CloseHandle"),
    ];
    let contracts = endpoints
        .iter()
        .cloned()
        .map(|endpoint| {
            kernel32_contract_for_endpoint(endpoint)
                .expect("valid file API contract")
                .expect("implemented file API contract")
        })
        .collect::<Vec<_>>();
    let dispatcher = ApiDispatcher::new(endpoints.clone(), contracts).expect("file dispatcher");

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
    ];
    for (address, words) in &frames {
        let bytes = words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>();
        write_initialization(&mut memory, owner, GuestAddress(*address), &bytes);
    }
    memory.seal_owner(owner).expect("seal fixture memory");
    acknowledge_initial_reflection(&mut memory);

    let mut runtime = controlled_runtime();
    runtime
        .register_source_snapshot(manifest)
        .expect("register controlled source snapshot");

    let create = dispatch(
        &dispatcher,
        &endpoints[0],
        frames[0].0,
        &policy,
        &mut memory,
        &mut runtime,
    );
    let create = expect_immediate(create);
    assert_eq!(create.eax, RegisterUpdate::Set(FIRST_FILE_HANDLE));
    assert_eq!(create.last_error, 0x1234);
    let handle = RuntimeHandle(FIRST_FILE_HANDLE);
    assert_eq!(
        runtime
            .object_for_handle(handle, RuntimeObjectKind::File)
            .expect("file object")
            .open_handles,
        1
    );

    let size = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[1],
        frames[1].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(size.eax, RegisterUpdate::Set(fixture.len() as u32));

    let first_call = expect_deferred(dispatch(
        &dispatcher,
        &endpoints[2],
        frames[2].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(
        decode_words(&first_call.request.payload),
        vec![1, 1, 1, 0, 0, 16]
    );
    assert_eq!(
        runtime.scheduler_state().expect("waiting scheduler"),
        SchedulerThreadState::WaitingHost { request_id: 1 }
    );

    let runtime_before_wrong = runtime.clone();
    let buffer_before_wrong = memory
        .read(GuestAddress(BUFFER_ADDRESS), 16)
        .expect("buffer before wrong completion")
        .to_vec();
    let (wrong, completion) = complete_kernel32_source_read(
        &dispatcher,
        first_call.clone(),
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
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    assert!(matches!(wrong, DispatchOutcome::Fault(_)));
    assert!(completion.is_none());
    assert_eq!(runtime, runtime_before_wrong);
    assert_eq!(
        memory
            .read(GuestAddress(BUFFER_ADDRESS), 16)
            .expect("buffer after wrong completion"),
        buffer_before_wrong
    );

    memory
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
    acknowledge_pending_delta(&mut memory);
    let runtime_before_memory_fault = runtime.clone();
    let memory_before_memory_fault = memory.stats();
    let (memory_fault, memory_fault_completion) = complete_kernel32_source_read(
        &dispatcher,
        first_call.clone(),
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
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    assert!(matches!(memory_fault, DispatchOutcome::Fault(_)));
    assert!(memory_fault_completion.is_none());
    assert_eq!(runtime, runtime_before_memory_fault);
    assert_eq!(memory.stats(), memory_before_memory_fault);
    assert_eq!(
        memory
            .read(GuestAddress(BUFFER_ADDRESS), 16)
            .expect("buffer after failed guest-memory completion"),
        buffer_before_wrong
    );
    memory
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
    acknowledge_pending_delta(&mut memory);

    let (first_outcome, first_completion) = complete_kernel32_source_read(
        &dispatcher,
        first_call.clone(),
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
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    let first_outcome = expect_immediate(first_outcome);
    assert_eq!(first_outcome.eax, RegisterUpdate::Set(1));
    assert_eq!(
        first_completion.expect("source completion").cursor_after,
        16
    );
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(BUFFER_ADDRESS), 16)
            .expect("first source bytes"),
        &fixture[..16]
    );
    assert_eq!(
        memory
            .read(GuestAddress(BYTES_READ_ADDRESS), 4)
            .expect("first byte count"),
        16_u32.to_le_bytes()
    );
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));

    let runtime_before_duplicate = runtime.clone();
    let (duplicate, duplicate_completion) = complete_kernel32_source_read(
        &dispatcher,
        first_call,
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
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    assert!(matches!(duplicate, DispatchOutcome::Fault(_)));
    assert!(duplicate_completion.is_none());
    assert_eq!(runtime, runtime_before_duplicate);

    let seek = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[3],
        frames[3].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(seek.eax, RegisterUpdate::Set(5));
    let second_call = expect_deferred(dispatch(
        &dispatcher,
        &endpoints[2],
        frames[4].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(
        decode_words(&second_call.request.payload),
        vec![1, 1, 1, 5, 0, 8]
    );
    let (second_outcome, second_completion) = complete_kernel32_source_read(
        &dispatcher,
        second_call,
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
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    assert_eq!(expect_immediate(second_outcome).eax, RegisterUpdate::Set(1));
    assert_eq!(
        second_completion
            .expect("second source completion")
            .cursor_after,
        13
    );
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));
    acknowledge_pending_delta(&mut memory);

    let close = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[4],
        frames[5].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(close.eax, RegisterUpdate::Set(1));
    assert!(runtime.source_file_for_handle(handle).is_err());
    let invalid_size = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[1],
        frames[6].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(invalid_size.eax, RegisterUpdate::Set(u32::MAX));
    assert_eq!(invalid_size.last_error, 6);
}

#[test]
fn writable_profile_api_generates_commits_and_reopens_without_source_shadowing() {
    let policy = source_policy();
    let (mut memory, owner) = initialized_memory();
    let endpoints = [
        endpoint(11, "CreateFileA"),
        endpoint(12, "WriteFile"),
        endpoint(13, "CloseHandle"),
        endpoint(14, "ReadFile"),
    ];
    let contracts = endpoints
        .iter()
        .cloned()
        .map(|endpoint| {
            kernel32_contract_for_endpoint(endpoint)
                .expect("valid writable file API contract")
                .expect("implemented writable file API contract")
        })
        .collect::<Vec<_>>();
    let dispatcher = ApiDispatcher::new(endpoints.clone(), contracts).expect("file dispatcher");
    write_initialization(&mut memory, owner, GuestAddress(PATH_ADDRESS), b"BGI.gdb\0");
    write_initialization(
        &mut memory,
        owner,
        GuestAddress(BUFFER_ADDRESS),
        b"SDC FORMAT 1.00",
    );
    let frames = [
        (
            0x2000_0000,
            vec![
                0x1000_0010,
                PATH_ADDRESS,
                0x4000_0000,
                0,
                0,
                4,
                0x0800_0020,
                0,
            ],
        ),
        (
            0x2000_0040,
            vec![
                0x1000_0020,
                FIRST_FILE_HANDLE,
                BUFFER_ADDRESS,
                15,
                BYTES_READ_ADDRESS,
                0,
            ],
        ),
        (0x2000_0080, vec![0x1000_0030, FIRST_FILE_HANDLE]),
        (
            0x2000_00c0,
            vec![
                0x1000_0040,
                PATH_ADDRESS,
                0x8000_0000,
                1,
                0,
                3,
                0x0800_0020,
                0,
            ],
        ),
    ];
    for (address, words) in &frames {
        write_initialization(
            &mut memory,
            owner,
            GuestAddress(*address),
            &words
                .iter()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<_>>(),
        );
    }
    memory.seal_owner(owner).expect("seal fixture memory");
    acknowledge_initial_reflection(&mut memory);

    let mut runtime = controlled_runtime();
    runtime
        .register_writable_profile(RuntimeWritableProfileManifest {
            schema: RUNTIME_WRITABLE_PROFILE_SCHEMA.to_owned(),
            schema_version: RUNTIME_WRITABLE_PROFILE_VERSION,
            title_fingerprint: "sha256:test-title".to_owned(),
            profile_id: "default".to_owned(),
            root_path_a: b"C:\\Vine\0".to_vec(),
            quota_bytes: 1024,
            files: vec![RuntimeWritableFileManifest {
                object_id: 21,
                relative_path_a: b"BGI.gdb".to_vec(),
                exists: false,
                generation: 0,
                byte_length: 0,
                sha256: None,
            }],
            directories: Vec::new(),
        })
        .expect("register writable profile");

    let create = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[0],
        frames[0].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(create.eax, RegisterUpdate::Set(FIRST_FILE_HANDLE));
    assert_eq!(create.last_error, 0);

    let write = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[1],
        frames[1].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(write.eax, RegisterUpdate::Set(1));
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(BYTES_READ_ADDRESS), 4)
            .expect("write count"),
        15_u32.to_le_bytes()
    );

    let close = expect_deferred(dispatch(
        &dispatcher,
        &endpoints[2],
        frames[2].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(
        decode_words(&close.request.payload),
        vec![1, 21, 0, 0, 15, 0, 1]
    );
    let runtime_before_wrong = runtime.clone();
    let (wrong, wrong_completion) = complete_kernel32_writable_commit(
        &dispatcher,
        close.clone(),
        RuntimeWritableCommitHostResponse {
            request_id: 1,
            object_id: 21,
            base_generation: 0,
            committed_generation: 2,
            win32_error: 0,
        },
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    assert!(matches!(wrong, DispatchOutcome::Fault(_)));
    assert!(wrong_completion.is_none());
    assert_eq!(runtime, runtime_before_wrong);

    let (failed_close, failed_completion) = complete_kernel32_writable_commit(
        &dispatcher,
        close,
        RuntimeWritableCommitHostResponse {
            request_id: 1,
            object_id: 21,
            base_generation: 0,
            committed_generation: 0,
            win32_error: 5,
        },
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    let failed_close = expect_immediate(failed_close);
    assert_eq!(failed_close.eax, RegisterUpdate::Set(0));
    assert_eq!(failed_close.last_error, 5);
    let failed_completion = failed_completion.expect("failed writable commit");
    assert!(!failed_completion.succeeded);
    assert_eq!(failed_completion.committed_generation, 0);
    assert!(
        runtime
            .writable_file_needs_commit(RuntimeHandle(FIRST_FILE_HANDLE))
            .expect("dirty handle after failed close")
    );
    assert_eq!(
        runtime.writable_file_commit_bytes(1).unwrap_err().code,
        "writable-commit-request-unknown"
    );
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));

    runtime.set_current_last_error(0x5a5a).unwrap();
    let retry_close = expect_deferred(dispatch(
        &dispatcher,
        &endpoints[2],
        frames[2].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(
        decode_words(&retry_close.request.payload),
        vec![1, 21, 0, 0, 15, 0, 1]
    );
    assert_eq!(
        runtime.writable_file_commit_bytes(2).expect("retry bytes"),
        b"SDC FORMAT 1.00"
    );
    let (closed, completion) = complete_kernel32_writable_commit(
        &dispatcher,
        retry_close,
        RuntimeWritableCommitHostResponse {
            request_id: 2,
            object_id: 21,
            base_generation: 0,
            committed_generation: 1,
            win32_error: 0,
        },
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    let closed = expect_immediate(closed);
    assert_eq!(closed.eax, RegisterUpdate::Set(1));
    assert_eq!(closed.last_error, 0x5a5a);
    assert_eq!(completion.expect("writable commit").committed_generation, 1);
    assert!(
        runtime
            .source_file_for_handle(RuntimeHandle(FIRST_FILE_HANDLE))
            .is_err()
    );
    assert_eq!(
        runtime.writable_profile().expect("profile").files[&21].byte_length,
        15
    );
    assert!(matches!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::HostCompleted(_))
    ));

    let reopened = expect_immediate(dispatch(
        &dispatcher,
        &endpoints[0],
        frames[3].0,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    let RegisterUpdate::Set(reopened_handle) = reopened.eax else {
        panic!("reopened file did not return a handle");
    };
    let read_frame_address = 0x2000_0140;
    let read_frame = [
        0x1000_0050,
        reopened_handle,
        BUFFER_ADDRESS + 32,
        32,
        BYTES_READ_ADDRESS,
        0,
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect::<Vec<_>>();
    memory
        .write(GuestAddress(read_frame_address), &read_frame)
        .expect("write reopened read frame");
    acknowledge_pending_delta(&mut memory);
    let read = expect_deferred(dispatch(
        &dispatcher,
        &endpoints[3],
        read_frame_address,
        &policy,
        &mut memory,
        &mut runtime,
    ));
    assert_eq!(
        decode_words(&read.request.payload),
        vec![1, 0, 21, 0, 0, 32]
    );
    let (read, read_completion) = complete_kernel32_source_read(
        &dispatcher,
        read,
        RuntimeSourceReadHostResponse {
            request_id: 3,
            source_id: 0,
            object_id: 21,
            offset: 0,
            requested_bytes: 32,
            transferred_bytes: 15,
            win32_error: 0,
            bytes: b"SDC FORMAT 1.00".to_vec(),
        },
        &mut ExecutionSession::new(&mut memory, &mut runtime),
    );
    assert_eq!(expect_immediate(read).eax, RegisterUpdate::Set(1));
    assert_eq!(read_completion.expect("profile read").cursor_after, 15);
    acknowledge_pending_delta(&mut memory);
    assert_eq!(
        memory
            .read(GuestAddress(BUFFER_ADDRESS + 32), 15)
            .expect("reopened bytes"),
        b"SDC FORMAT 1.00"
    );
}

#[test]
fn source_file_sharing_mount_and_pending_exit_paths_preserve_authority() {
    let mut runtime = controlled_runtime();
    runtime
        .register_source_snapshot(source_manifest())
        .expect("register controlled source snapshot");
    let first = runtime
        .open_source_file_a(
            b"C:\\Vine\\fixtures\\storage\\source-read-v1.dat\0",
            0x8000_0000,
            0,
            0x0800_0020,
        )
        .expect("first source open")
        .expect("known source path");
    let before_share_rejection = runtime.clone();
    let sharing = runtime
        .open_source_file_a(
            b"c:/vine/FIXTURES/STORAGE/SOURCE-READ-V1.DAT\0",
            0x8000_0000,
            1,
            0x0800_0020,
        )
        .expect_err("read share must conflict");
    assert_eq!(sharing.code, "source-file-sharing-violation");
    assert_eq!(runtime, before_share_rejection);
    let outside = runtime
        .open_source_file_a(b"..\\outside.dat\0", 0x8000_0000, 1, 0x0800_0020)
        .expect_err("mount escape must fail");
    assert_eq!(outside.code, "source-path-outside-mount");
    assert_eq!(runtime, before_share_rejection);

    let request = runtime
        .begin_source_file_read(
            first.handle,
            GuestAddress(BUFFER_ADDRESS),
            GuestAddress(BYTES_READ_ADDRESS),
            8,
        )
        .expect("begin source read");
    assert_eq!(request.request_id, 1);
    let cancellation = runtime
        .terminate_current_process(9)
        .expect("direct termination cancels pending source read");
    assert_eq!(cancellation.host_requests_cancelled, 1);
    assert_eq!(
        runtime.process().lifecycle(),
        RuntimeProcessLifecycleState::Exited { exit_code: 9 }
    );
    assert!(runtime.source_file_for_handle(first.handle).is_err());
    assert_eq!(
        runtime
            .scheduler_state_for_thread(RuntimeThreadId(THREAD_ID))
            .expect("terminated scheduler"),
        SchedulerThreadState::Terminated { exit_code: 9 }
    );
}

fn dispatch(
    dispatcher: &ApiDispatcher,
    endpoint: &ApiEndpointIdentity,
    stack_pointer: u32,
    policy: &Kernel32CompatibilityPolicy,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
) -> DispatchOutcome {
    let return_address = u32::from_le_bytes(
        memory
            .read(GuestAddress(stack_pointer), 4)
            .expect("call return address")
            .try_into()
            .expect("x86 word"),
    );
    dispatcher.dispatch_owned(
        vine_core::ApiOwnedTrap {
            endpoint_id: endpoint.id,
            endpoint_address: endpoint.address,
            stack_pointer: GuestAddress(stack_pointer),
            return_address: GuestAddress(return_address),
        },
        &mut ExecutionSession::new(memory, runtime),
        |contract, frame, session| {
            dispatch_kernel32_api_with_policy(policy, contract, frame, session)
        },
    )
}

fn expect_immediate(outcome: DispatchOutcome) -> vine_core::ApiImmediateCompletion {
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
        namespace: "vine.test-source-endpoint.v1".to_owned(),
        id,
        address: GuestAddress(0x7f00_1000 + id * 0x10),
        module: "kernel32.dll".to_owned(),
        name: name.to_owned(),
        ordinal: None,
        lifetime: "test-process".to_owned(),
    })
}

fn initialized_memory() -> (GuestMemory, vine_core::MemoryOwnerId) {
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

fn controlled_runtime() -> RuntimeState {
    let mut startup = vec![0; 68];
    startup[..4].copy_from_slice(&68_u32.to_le_bytes());
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: RuntimeThreadId(THREAD_ID),
        initial_last_error: 0x1234,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "source-file-gateway-v1".to_owned(),
            bytes: startup,
        },
        process_image_path_a: vine_core::RuntimeProcessImagePathA {
            source_authority: "source-file-gateway-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
            source_authority: "source-file-gateway-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: vine_core::LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_cookie: 0xec01_9d3b,
    })
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
        schema: vine_core::RUNTIME_SOURCE_SNAPSHOT_SCHEMA.to_owned(),
        schema_version: vine_core::RUNTIME_SOURCE_SNAPSHOT_VERSION,
        title_fingerprint: "sha256:find-file-test".to_owned(),
        snapshot_id: "find-file-test-v1".to_owned(),
        mount: RuntimeSourceMountManifest {
            source_id: 1,
            root_path_a: b"C:\\Vine\0".to_vec(),
        },
        files: [
            (1, b"data01099.arc".as_slice(), 10_u64, '1'),
            (2, b"DATA01100.ARC".as_slice(), 20_u64, '2'),
            (3, b"data01999.arc".as_slice(), 30_u64, '3'),
            (4, b"notes.txt".as_slice(), 40_u64, '4'),
            (5, b"assets\\nested.bin".as_slice(), 50_u64, '5'),
        ]
        .into_iter()
        .map(
            |(object_id, path, byte_length, hash_digit)| RuntimeSourceFileManifest {
                object_id,
                relative_path_a: path.to_vec(),
                byte_length,
                sha256: std::iter::repeat_n(hash_digit, 64).collect(),
            },
        )
        .collect(),
    }
}

fn source_policy() -> Kernel32CompatibilityPolicy {
    let policy: Kernel32CompatibilityPolicy = serde_json::from_str(include_str!(
        "../../../fixtures/runtime-policies/kernel32-source-file-read-v1.json"
    ))
    .expect("source file policy JSON");
    policy.validate().expect("valid source file policy");
    policy
}

fn source_manifest() -> RuntimeSourceSnapshotManifest {
    serde_json::from_str(include_str!(
        "../../../fixtures/source-snapshots/controlled-source-read-v1.json"
    ))
    .expect("source snapshot manifest")
}

fn write_initialization(
    memory: &mut GuestMemory,
    owner: vine_core::MemoryOwnerId,
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
