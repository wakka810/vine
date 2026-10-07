#![allow(dead_code)]

use std::collections::BTreeMap;
use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiImmediateCompletion, ApiOwnedTrap, DispatchOutcome,
    GuestAddress, GuestMemory, MemoryLimits, MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind,
    ModuleSnapshot, PageProtection, Pe32Limits, Runtime, RuntimeBootstrap, RuntimeClockState,
    RuntimeCurrentDirectoryA, RuntimeExport, RuntimeExportCatalog, RuntimeModule, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeThreadId, build_pe32_load_plan,
    materialize_pe32_image,
};

pub const BASE: GuestAddress = GuestAddress(0x3000_0000);
pub const STACK: GuestAddress = GuestAddress(BASE.0 + 0x1000);
pub const OUTPUT: GuestAddress = GuestAddress(BASE.0 + 0x2000);
pub const RETURN: GuestAddress = GuestAddress(0x0040_1000);
pub const SENTINEL: GuestAddress = GuestAddress(0x0040_1020);
pub const THREAD: RuntimeThreadId = RuntimeThreadId(8192);
pub const LAST_ERROR: u32 = 0x1234_5678;

// A real PE32 import table lets external tests obtain endpoint identities through
// the same public load/bind path used by the browser, without exporting test APIs.
fn imported_pe(imports: &[(&str, &str)]) -> Vec<u8> {
    let mut bytes = vec![0; 0x3200];
    let word = |bytes: &mut [u8], offset: usize, value: u16| {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    };
    let dword = |bytes: &mut [u8], offset: usize, value: u32| {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    };
    bytes[..2].copy_from_slice(b"MZ");
    dword(&mut bytes, 0x3c, 0x80);
    bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
    word(&mut bytes, 0x84, 0x014c);
    word(&mut bytes, 0x86, 1);
    word(&mut bytes, 0x94, 224);
    word(&mut bytes, 0x96, 0x0103);
    let optional = 0x98;
    word(&mut bytes, optional, 0x010b);
    for (offset, value) in [
        (4, 0x3000),
        (16, 0x1000),
        (20, 0x1000),
        (24, 0x1000),
        (28, 0x0040_0000),
        (32, 0x1000),
        (36, 0x200),
        (56, 0x4000),
        (60, 0x200),
        (72, 0x10_0000),
        (76, 0x1000),
        (80, 0x10_0000),
        (84, 0x1000),
        (92, 16),
    ] {
        dword(&mut bytes, optional + offset, value);
    }
    word(&mut bytes, optional + 40, 6);
    word(&mut bytes, optional + 48, 6);
    word(&mut bytes, optional + 68, 2);
    let section = optional + 224;
    bytes[section..section + 5].copy_from_slice(b".text");
    for (offset, value) in [
        (8, 0x3000),
        (12, 0x1000),
        (16, 0x3000),
        (20, 0x200),
        (36, 0xe000_0020),
    ] {
        dword(&mut bytes, section + offset, value);
    }
    bytes[0x200] = 0xc3;
    let mut modules: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for &(module, name) in imports {
        modules.entry(module).or_default().push(name);
    }
    let directory_size = (modules.len() + 1) * 20;
    dword(&mut bytes, optional + 104, 0x1100);
    dword(&mut bytes, optional + 108, directory_size as u32);
    let rva = |offset: usize| u32::try_from(offset + 0xe00).unwrap();
    let mut cursor = 0x300 + directory_size;
    for (index, (module, names)) in modules.into_iter().enumerate() {
        let descriptor = 0x300 + index * 20;
        dword(&mut bytes, descriptor + 12, rva(cursor));
        bytes[cursor..cursor + module.len()].copy_from_slice(module.as_bytes());
        cursor = (cursor + module.len() + 4) & !3;
        let table = cursor;
        cursor += (names.len() + 1) * 4;
        dword(&mut bytes, descriptor, rva(table));
        dword(&mut bytes, descriptor + 16, rva(table));
        for (index, name) in names.into_iter().enumerate() {
            dword(&mut bytes, table + index * 4, rva(cursor));
            bytes[cursor + 2..cursor + 2 + name.len()].copy_from_slice(name.as_bytes());
            cursor = (cursor + name.len() + 4) & !1;
        }
    }
    assert!(
        cursor <= bytes.len(),
        "test import table must fit its PE section"
    );
    bytes
}

pub struct Fixture {
    pub runtime: Runtime,
    pub dispatcher: ApiDispatcher,
    pub owner: MemoryOwnerId,
    endpoints: BTreeMap<(String, String), ApiEndpointIdentity>,
}

impl Fixture {
    pub fn new(imports: &[(&str, &str)]) -> Self {
        let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
        let owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
        let range = memory
            .reserve_fixed(owner, MemoryRegionKind::Runtime, BASE, 0x3000)
            .unwrap();
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
            .unwrap();
        memory.seal_owner(owner).unwrap();
        let limits = Pe32Limits::default();
        let snapshot = ModuleSnapshot::from_bytes(imported_pe(imports), &limits).unwrap();
        let plan = build_pe32_load_plan(&snapshot, limits).unwrap();
        let image =
            materialize_pe32_image(&mut memory, &snapshot, &plan, GuestAddress(0x0040_0000))
                .unwrap();
        let mut startup = vec![0; 68];
        startup[..4].copy_from_slice(&68_u32.to_le_bytes());
        let mut runtime = Runtime::new(
            RuntimeBootstrap {
                process_id: RuntimeProcessId(4096),
                initial_thread_id: THREAD,
                initial_last_error: LAST_ERROR,
                clock: RuntimeClockState {
                    wall_clock_file_time: 134_282_016_000_000_000,
                    tick_count: 3_600_000,
                    performance_counter: 12_345_678_901,
                    performance_frequency: 10_000_000,
                },
                startup_info: RuntimeStartupInfo { bytes: startup },
                process_image_path_a: RuntimeProcessImagePathA {
                    bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
                },
                initial_current_directory_a: RuntimeCurrentDirectoryA {
                    bytes_with_nul: b"C:\\Vine\0".to_vec(),
                },
                encoded_pointer_cookie: 0xec01_9d3b,
            },
            memory,
        )
        .unwrap();
        runtime.start_initial_thread().unwrap();
        runtime
            .register_main_module(RuntimeModule {
                handle: 0x4d01_0001,
                requested_name: "fixture.exe".into(),
                canonical_name: "fixture.exe".into(),
                runtime_system_module: false,
            })
            .unwrap();
        for (index, module) in plan.imports.iter().enumerate() {
            runtime
                .load_module(RuntimeModule {
                    handle: 0x4d01_0002 + index as u32,
                    requested_name: module.name.clone(),
                    canonical_name: module.name.to_ascii_lowercase(),
                    runtime_system_module: true,
                })
                .unwrap();
        }
        let catalog = RuntimeExportCatalog::new(
            imports
                .iter()
                .map(|&(module, name)| RuntimeExport::named(module, name))
                .collect(),
        )
        .unwrap();
        let binding = runtime
            .bind_guest_module_imports(0x4d01_0001, &plan, &image, &catalog)
            .unwrap();
        runtime
            .register_module_image(0x4d01_0001, &plan, &image)
            .unwrap();
        let endpoints: BTreeMap<_, _> = binding
            .imports
            .iter()
            .map(|import| {
                let endpoint = ApiEndpointIdentity::from_runtime_guest_import(import).unwrap();
                (
                    (
                        import.requested_module.to_ascii_lowercase(),
                        endpoint.diagnostic_symbol(),
                    ),
                    endpoint,
                )
            })
            .collect();
        let dispatcher = ApiDispatcher::from_endpoints(endpoints.values().cloned()).unwrap();
        let mut fixture = Self {
            runtime,
            dispatcher,
            owner,
            endpoints,
        };
        fixture.ack();
        fixture
    }

    pub fn ack(&mut self) {
        let memory = self.runtime.memory_mut();
        if memory.has_pending_delta() {
            if memory.pending_reflection().is_err() {
                memory.pending_delta_binary().unwrap();
            }
            let generation = memory.stats().generation;
            memory.acknowledge_delta(generation).unwrap();
        }
    }

    pub fn write(&mut self, address: GuestAddress, bytes: &[u8]) {
        self.runtime.memory_mut().write(address, bytes).unwrap();
        self.ack();
    }

    pub fn words(&mut self, address: GuestAddress, words: &[u32]) {
        self.write(
            address,
            &words
                .iter()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<_>>(),
        );
    }

    pub fn read(&self, address: GuestAddress, length: usize) -> Vec<u8> {
        self.runtime.memory().read(address, length).unwrap()
    }

    pub fn dispatch_at(
        &mut self,
        module: &str,
        name: &str,
        stack: GuestAddress,
        args: &[u32],
    ) -> DispatchOutcome {
        self.words(stack, &[&[RETURN.0], args].concat());
        let endpoint = &self.endpoints[&(module.to_ascii_lowercase(), name.to_owned())];
        self.runtime.dispatch_api_call(
            &self.dispatcher,
            ApiOwnedTrap {
                endpoint_id: endpoint.id,
                endpoint_address: endpoint.address,
                stack_pointer: stack,
                return_address: RETURN,
            },
            SENTINEL,
            None,
        )
    }

    pub fn dispatch(&mut self, module: &str, name: &str, args: &[u32]) -> DispatchOutcome {
        self.dispatch_at(module, name, STACK, args)
    }

    pub fn call(&mut self, module: &str, name: &str, args: &[u32]) -> ApiImmediateCompletion {
        let outcome = self.dispatch(module, name, args);
        let DispatchOutcome::Immediate(completion) = outcome else {
            panic!("{module}!{name}: {outcome:?}")
        };
        assert_eq!(completion.instruction_pointer, RETURN);
        assert_eq!(
            completion.stack_pointer,
            GuestAddress(STACK.0 + 4 * (1 + args.len() as u32))
        );
        self.ack();
        completion
    }
}
