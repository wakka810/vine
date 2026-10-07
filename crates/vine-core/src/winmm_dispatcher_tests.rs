use crate as vine_core;
use crate::imports::{ImportThunk, ImportThunkDisposition, ImportThunkId};
use crate::winmm::{TIMERR_NOCANDO, dispatch_winmm_api, winmm_argument_words};
use vine_core::{
    ApiDispatcher, ApiEndpointIdentity, ApiOwnedTrap, DispatchOutcome, GuestAddress, GuestMemory,
    ImportTarget, MemoryLimits, MemoryOwnerKind, MemoryRegionKind, PageProtection, Runtime,
    RuntimeBootstrap, RuntimeClockState, RuntimeCurrentDirectoryA, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeThreadId,
};

const BASE: GuestAddress = GuestAddress(0x2000_0000);
const STACK: GuestAddress = GuestAddress(0x2000_0100);
const OUTPUT: GuestAddress = GuestAddress(0x2000_1000);
const RETURN: GuestAddress = GuestAddress(0x1000_0100);
const LAST_ERROR: u32 = 0x1234_5678;
const SYMBOLS: [&str; 4] = [
    "timeGetDevCaps",
    "timeBeginPeriod",
    "timeEndPeriod",
    "timeGetTime",
];

struct Fixture {
    runtime: Runtime,
    dispatcher: ApiDispatcher,
    endpoints: Vec<ApiEndpointIdentity>,
}

impl Fixture {
    fn new() -> Self {
        let mut memory = GuestMemory::new(MemoryLimits {
            max_physical_pages: 3,
            max_regions: 1,
            max_owners: 1,
            max_access_bytes: 0x3000,
        })
        .unwrap();
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
        memory
            .write_initialization(owner, OUTPUT, &[0xa5; 16])
            .unwrap();
        memory
            .write_initialization(owner, GuestAddress(BASE.0 + 0x2ffc), &[0xa5; 4])
            .unwrap();
        let mut endpoints = Vec::new();
        for (index, symbol) in SYMBOLS.into_iter().enumerate() {
            let thunk = ImportThunk {
                id: ImportThunkId(index as u32 + 1),
                address: GuestAddress(0x7f00_0020 + index as u32 * 0x10),
                requester_owner: owner,
                module: "WINMM.dll".to_owned(),
                iat_rva: vine_core::Rva(0x2000 + index as u32 * 4),
                iat_address: GuestAddress(BASE.0 + 0x2000 + index as u32 * 4),
                original_iat_value: 0,
                disposition: ImportThunkDisposition::DeclaredRuntimeExport,
                target: ImportTarget::Name {
                    name: symbol.to_owned(),
                    hint: 0,
                    name_rva: vine_core::Rva(0),
                },
            };
            memory
                .write_initialization(owner, thunk.iat_address, &thunk.address.0.to_le_bytes())
                .unwrap();
            let endpoint = ApiEndpointIdentity::from_import_thunk(&thunk);
            endpoints.push(endpoint);
        }
        memory.seal_owner(owner).unwrap();
        let reflection = memory.pending_reflection().unwrap();
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .unwrap();
        let dispatcher = ApiDispatcher::from_endpoints(endpoints.clone()).unwrap();
        let mut startup = vec![0; 68];
        startup[..4].copy_from_slice(&68_u32.to_le_bytes());
        let mut runtime = Runtime::new(
            RuntimeBootstrap {
                process_id: RuntimeProcessId(4096),
                initial_thread_id: RuntimeThreadId(8192),
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
        Self {
            runtime,
            dispatcher,
            endpoints,
        }
    }

    fn prepare(&mut self, symbol: &str, arguments: &[u32]) -> ApiOwnedTrap {
        let index = SYMBOLS.iter().position(|name| *name == symbol).unwrap();
        let endpoint = &self.endpoints[index];
        let bytes = std::iter::once(RETURN.0)
            .chain(arguments.iter().copied())
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        self.runtime.memory.write(STACK, &bytes).unwrap();
        let delta = self.runtime.memory.pending_delta().unwrap();
        self.runtime
            .memory
            .acknowledge_delta(delta.generation)
            .unwrap();
        ApiOwnedTrap {
            endpoint_id: endpoint.id,
            endpoint_address: endpoint.address,
            stack_pointer: STACK,
            return_address: RETURN,
        }
    }

    fn dispatch(&mut self, trap: ApiOwnedTrap) -> DispatchOutcome {
        self.dispatcher
            .dispatch_owned(trap, &mut self.runtime, None, |frame, runtime, _cpu| {
                dispatch_winmm_api(frame, runtime)
            })
    }

    fn call(&mut self, symbol: &str, arguments: &[u32], result: u32) {
        let trap = self.prepare(symbol, arguments);
        let completion = match self.dispatch(trap) {
            DispatchOutcome::Immediate(completion) => completion,
            other => panic!("expected {symbol} completion, got {other:?}"),
        };
        assert_eq!(completion.eax, Some(result));
        assert_eq!(completion.instruction_pointer, RETURN);
        assert_eq!(
            completion.stack_pointer,
            GuestAddress(STACK.0 + 4 * (arguments.len() as u32 + 1))
        );
        assert_eq!(self.runtime.current_last_error().unwrap(), LAST_ERROR);
        if self.runtime.memory.has_pending_delta() {
            let delta = self.runtime.memory.pending_delta().unwrap();
            self.runtime
                .memory
                .acknowledge_delta(delta.generation)
                .unwrap();
        }
    }
}

#[test]
fn catalog_routes_only_named_winmm_exports() {
    let fixture = Fixture::new();
    let endpoint = fixture.endpoints[0].clone();
    let mut other_module = endpoint.clone();
    other_module.module = "kernel32.dll".to_owned();
    let mut ordinal = endpoint.clone();
    ordinal.symbol = crate::api::ApiSymbol::Ordinal { ordinal: 1 };
    let mut unknown = endpoint.clone();
    unknown.symbol = crate::api::ApiSymbol::Name {
        name: "unknown".to_owned(),
    };
    let mut wrong_case = endpoint;
    wrong_case.symbol = crate::api::ApiSymbol::Name {
        name: "TimeGetDevCaps".to_owned(),
    };
    for unsupported in [other_module, ordinal, unknown, wrong_case] {
        assert!(winmm_argument_words(&unsupported).is_none());
    }
}

#[test]
fn time_caps_write_only_the_fixed_structure_and_preserve_last_error() {
    for size in [8, 12] {
        let mut fixture = Fixture::new();
        fixture.call("timeGetDevCaps", &[OUTPUT.0, size], 0);
        assert_eq!(
            fixture.runtime.memory.read(OUTPUT, 16).unwrap(),
            [
                1, 0, 0, 0, 0x40, 0x42, 0x0f, 0, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5
            ]
        );
    }
    let mut fixture = Fixture::new();
    fixture.call("timeGetDevCaps", &[OUTPUT.0, 7], TIMERR_NOCANDO);
    assert_eq!(fixture.runtime.memory.read(OUTPUT, 16).unwrap(), [0xa5; 16]);
}

#[test]
fn time_caps_invalid_outputs_preserve_bytes_clock_and_last_error() {
    for output in [0, BASE.0 + 0x2ffc] {
        let mut fixture = Fixture::new();
        let trap = fixture.prepare("timeGetDevCaps", &[output, 8]);
        let clock = fixture.runtime.clock().clone();
        let memory = fixture.runtime.memory.stats();
        assert!(matches!(fixture.dispatch(trap), DispatchOutcome::Fault(_)));
        assert_eq!(fixture.runtime.clock(), &clock);
        assert_eq!(fixture.runtime.current_last_error().unwrap(), LAST_ERROR);
        assert_eq!(fixture.runtime.timer_time_100ns(), 0);
        assert_eq!(
            fixture.runtime.scheduler_state().unwrap(),
            vine_core::SchedulerThreadState::Runnable
        );
        assert_eq!(fixture.runtime.memory.stats(), memory);
        assert_eq!(fixture.runtime.memory.read(OUTPUT, 16).unwrap(), [0xa5; 16]);
        assert_eq!(
            fixture
                .runtime
                .memory
                .read(GuestAddress(BASE.0 + 0x2ffc), 4)
                .unwrap(),
            [0xa5; 4]
        );
    }
}

#[test]
fn time_reads_observe_execution_clock_without_advancing_it() {
    let mut fixture = Fixture::new();
    let clock_before = fixture.runtime.clock().clone();
    let execution_before = fixture.runtime.synchronized_instruction_count();
    for _ in 0..2 {
        fixture.call("timeGetTime", &[], 3_600_000);
        assert_eq!(fixture.runtime.clock(), &clock_before);
        assert_eq!(fixture.runtime.timer_time_100ns(), 0);
        assert_eq!(
            fixture.runtime.synchronized_instruction_count(),
            execution_before
        );
    }
    fixture.runtime.advance_scheduled_clock_to(25_000).unwrap();
    fixture.call("timeGetTime", &[], 3_600_002);
    assert_eq!(fixture.runtime.timer_time_100ns(), 25_000);
}

#[test]
fn timer_periods_require_exact_balanced_pairs() {
    let mut fixture = Fixture::new();
    for period in [1, 2, 1] {
        fixture.call("timeBeginPeriod", &[period], 0);
    }
    fixture.call("timeEndPeriod", &[1], 0);
    fixture.call("timeEndPeriod", &[1], 0);
    fixture.call("timeEndPeriod", &[1], TIMERR_NOCANDO);
    fixture.call("timeEndPeriod", &[2], 0);
    fixture.call("timeEndPeriod", &[2], TIMERR_NOCANDO);
    assert_eq!(fixture.runtime.clock().tick_count, 3_600_000);
}

#[test]
fn timer_period_bounds_fail_without_changing_state() {
    let mut fixture = Fixture::new();
    fixture.call("timeBeginPeriod", &[1], 0);
    for period in [0, 1_000_001, u32::MAX] {
        fixture.call("timeBeginPeriod", &[period], TIMERR_NOCANDO);
        fixture.call("timeEndPeriod", &[period], TIMERR_NOCANDO);
    }
    fixture.call("timeEndPeriod", &[1], 0);
    fixture.call("timeEndPeriod", &[1], TIMERR_NOCANDO);
    assert_eq!(fixture.runtime.timer_time_100ns(), 0);
    fixture.call("timeBeginPeriod", &[1_000_000], 0);
    fixture.call("timeEndPeriod", &[1_000_000], 0);
}

#[test]
fn timer_resolution_capacity_returns_mmresult_without_partial_state() {
    let mut fixture = Fixture::new();
    for period in 1..=64 {
        fixture.call("timeBeginPeriod", &[period], 0);
    }
    fixture.call("timeBeginPeriod", &[65], TIMERR_NOCANDO);
    fixture.call("timeBeginPeriod", &[1], 0);
    fixture.call("timeEndPeriod", &[1], 0);
    fixture.call("timeEndPeriod", &[1], 0);
    fixture.call("timeEndPeriod", &[1], TIMERR_NOCANDO);
    for period in 2..=64 {
        fixture.call("timeEndPeriod", &[period], 0);
    }
    fixture.call("timeEndPeriod", &[65], TIMERR_NOCANDO);
    fixture.call("timeBeginPeriod", &[65], 0);
    fixture.call("timeEndPeriod", &[65], 0);
    fixture.call("timeEndPeriod", &[65], TIMERR_NOCANDO);
}
