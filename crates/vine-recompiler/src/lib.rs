//! Experimental independent PE32 execution backend and in-browser compiler.
//! The exported adapter is a standalone capture/semantic test boundary. Product
//! Runtime authority and thread/callback integration are deliberately separate.
pub mod code_cache;
mod continuation;
mod continuation_ir;
pub mod control_ir;
mod control_layout;
pub mod decode;
mod diagnostic;
mod entry_ir;
mod flag_flow;
mod flag_ir;
pub mod flow_ir;
pub mod fp_ir;
pub mod ir;
pub mod loop_ir;
pub mod memory;
pub mod numeric_kernels;
pub mod numeric_mode;
pub mod optimize;
pub mod predicate_ir;
mod preparation;
mod return_ir;
pub mod rooted_ir;
pub mod runtime_adapter;
mod simd_kernels;
pub mod snapshot;
pub mod state;
mod timestamp;
pub mod value_ir;
pub mod wasm;
pub mod x87;

use decode::CodeSource;
use memory::{Access, MirrorMemory, PAGE_SIZE};
use state::CpuState;
use std::cell::RefCell;
use std::collections::BTreeMap;

impl CodeSource for MirrorMemory {
    fn read_data(&self, address: u32, max_len: usize) -> Vec<u8> {
        // Discovery is optional and bounded. Any failed authority check means
        // no hint; the original generated guest access still owns its fault.
        if max_len > 1024 {
            return Vec::new();
        }
        self.read(address, max_len, Access::Read)
            .unwrap_or_default()
    }
    fn read_code(&self, address: u32, max_len: usize) -> Vec<u8> {
        let mut bytes = Vec::new();
        for offset in 0..max_len.min(15) {
            let Some(at) = address.checked_add(offset as u32) else {
                break;
            };
            let Ok(byte) = self.read(at, 1, Access::Execute) else {
                break;
            };
            bytes.push(byte[0]);
        }
        bytes
    }
}

pub struct Backend {
    pub state: Box<CpuState>,
    pub memory: MirrorMemory,
    runtime_adapter: runtime_adapter::RuntimeMemoryAdapter,
    writeback_control: Vec<u8>,
    writeback_payload: Vec<u8>,
    capture_control: Vec<u8>,
    cache: code_cache::CodeCache,
    structured_loops: bool,
    affine_memory: bool,
    loop_memory: bool,
    function_cfg: bool,
    control_graph: bool,
    fp_ir: bool,
    typed_numeric: bool,
    numeric_mode: numeric_mode::NumericMode,
    value_ir: bool,
    predicate_ir: bool,
    flow_ir: bool,
    rooted_ssa: bool,
    diagnostic_stop: Option<u32>,
    sampled_pc: bool,
    binding_global: bool,
    // Compiler-owned synchronous output; never part of CPU interchange,
    // guest mappings, Runtime reflection, snapshots or code-cache identity.
    numeric_output: Box<fp_ir::NumericOutput>,
    timestamp_next: u32,
    pending_timestamp: Option<timestamp::PendingTimestamp>,
    comparison: Option<continuation::ComparisonContinuation>,
    preparation: Option<preparation::Preparation>,
}
impl Backend {
    pub fn new(maximum_pages: usize) -> Result<Self, String> {
        Ok(Self {
            state: Box::default(),
            memory: MirrorMemory::new(maximum_pages).map_err(|e| e.to_string())?,
            runtime_adapter: runtime_adapter::RuntimeMemoryAdapter::new(),
            writeback_control: Vec::new(),
            writeback_payload: Vec::new(),
            capture_control: Vec::new(),
            cache: code_cache::CodeCache::default(),
            structured_loops: false,
            affine_memory: false,
            loop_memory: false,
            function_cfg: false,
            control_graph: false,
            fp_ir: false,
            typed_numeric: false,
            numeric_mode: numeric_mode::NumericMode::Strict,
            value_ir: false,
            predicate_ir: false,
            flow_ir: false,
            rooted_ssa: false,
            diagnostic_stop: None,
            sampled_pc: false,
            binding_global: false,
            numeric_output: Box::default(),
            timestamp_next: 0,
            pending_timestamp: None,
            comparison: None,
            preparation: None,
        })
    }
    pub fn compile(&mut self, entry: u32, limit: usize) -> Result<(), String> {
        self.compile_entries(&[entry], limit)
    }
    /// The same binary-to-IR-to-Wasm path can run before execution or on an
    /// unknown target. Preparing code never executes guest state or APIs.
    pub fn compile_entries(&mut self, entries: &[u32], limit: usize) -> Result<(), String> {
        let (id, unit) = self.compile_unit(entries, limit, true)?;
        self.cache.insert_reserved(id, unit)?;
        Ok(())
    }
    fn compile_unit(
        &mut self,
        entries: &[u32],
        limit: usize,
        budget_continuation: bool,
    ) -> Result<(u32, code_cache::Unit), String> {
        if entries.is_empty() || entries.len() > 256 || entries.len() > limit {
            return Err("compile roots outside 1..256".into());
        }
        let entry = entries[0];
        if entries.len() > 1 && self.rooted_ssa {
            return Err("rooted SSA cannot admit multiple preparation roots".into());
        }
        if !(1..=8192).contains(&limit) {
            return Err("region instruction limit outside 1..8192".into());
        }
        // Every unit has its own discovery root. Unknown indirect targets add a
        // separate reusable region instead of displacing an ever-growing union.
        self.validate_diagnostic_mode()?;
        self.cache.reconcile(&mut self.memory);
        // A normal instruction-budget exit can stop inside an existing block.
        // Compile only its precise remaining suffix, then use ordinary links
        // to rejoin cached block heads. This does not change the hot region's
        // code or its entry ABI. Non-budget indirect exits still get full
        // regions; no block-boundary return or extra polling is introduced.
        let limit = if budget_continuation
            && entries.len() == 1
            && self.state.exit.kind == state::exit_kind::BUDGET
            && self.state.eip == entry
            && !(self.rooted_ssa || self.value_ir || self.structured_loops || self.affine_memory)
        {
            self.cache
                .budget_continuation_limit(entry, &self.memory)
                .map_or(limit, |suffix| limit.min(suffix))
        } else {
            limit
        };
        let source = preparation::CodeView {
            source: diagnostic::CodeView {
                memory: &self.memory,
                stop: self.diagnostic_stop,
            },
            prepared: &self.cache,
            preparation: self.preparation.as_ref(),
        };
        let graph = if self.control_graph {
            control_ir::discover(&source, entries, limit)
        } else {
            control_ir::Graph::new(
                decode::discover_many(&source, entries, limit),
                BTreeMap::new(),
            )
        };
        self.compile_graph_unit(&graph)
    }
    fn compile_graph_unit(
        &mut self,
        graph: &control_ir::Graph,
    ) -> Result<(u32, code_cache::Unit), String> {
        let region = &graph.region;
        let entry = region.entry;
        let id = self.cache.reserve_id()?;
        let code = wasm::compile_control_graph(
            graph,
            wasm::CompileOptions {
                loop_memory: self.loop_memory,
                function_cfg: self.function_cfg,
                structured_loops: self.structured_loops,
                affine_memory: self.affine_memory,
                fp_ir: self.fp_ir,
                typed_numeric: self.typed_numeric,
                numeric_mode: self.numeric_mode,
                value_ir: self.value_ir,
                predicate_ir: self.predicate_ir,
                flow_ir: self.flow_ir,
                rooted_ssa: self.rooted_ssa,
                sampled_pc: self.sampled_pc,
                binding_global: self.binding_global,
                direct_links: self.cache.direct_pointer() != 0,
            },
            id,
            self.cache.compilation_capacity() as u32,
        )?;
        let dependencies = self.code_dependencies(region);
        Ok((
            id,
            code_cache::Unit {
                code: code.into_boxed_slice(),
                entries: if self.rooted_ssa {
                    std::collections::BTreeSet::from([entry])
                } else {
                    region.blocks.keys().copied().collect()
                },
                dependencies,
                preparation_frontier: region
                    .blocks
                    .values()
                    .flat_map(|block| block.successors.iter().copied())
                    .filter(|target| {
                        if self.rooted_ssa {
                            *target != entry
                        } else {
                            !region.blocks.contains_key(target)
                        }
                    })
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                instruction_count: region
                    .blocks
                    .values()
                    .map(|b| b.instructions.len() as u32)
                    .sum(),
                interior_limits: region
                    .blocks
                    .values()
                    .flat_map(|block| {
                        block.instructions.iter().enumerate().skip(1).map(
                            |(offset, instruction)| {
                                (instruction.eip, block.instructions.len() - offset)
                            },
                        )
                    })
                    .collect(),
            },
        ))
    }
    fn code_dependencies(&self, region: &ir::Region) -> BTreeMap<u32, Option<u64>> {
        let mut dependencies = BTreeMap::new();
        for i in region.blocks.values().flat_map(|block| &block.instructions) {
            let span = if matches!(
                i.unsupported,
                Some(
                    ir::UnsupportedReason::FetchUnavailable
                        | ir::UnsupportedReason::TruncatedInstruction
                )
            ) {
                15
            } else {
                u32::from(i.len.max(1))
            };
            for offset in 0..span {
                if let Some(at) = i.eip.checked_add(offset) {
                    let page = at & !4095;
                    dependencies.insert(page, self.memory.code_stamp(page));
                }
            }
        }
        dependencies
    }
    pub fn contains(&self, entry: u32) -> bool {
        self.cache
            .selected()
            .is_some_and(|unit| unit.entries.contains(&entry))
    }
    pub fn validate(&mut self, entry: u32) -> Result<(), String> {
        self.validate_unit(self.cache.selected_id(), entry)
    }
    pub fn validate_unit(&mut self, id: u32, entry: u32) -> Result<(), String> {
        self.cache.validate(id, entry, &self.memory)?;
        self.validate_cpu()
    }
    pub fn select(&mut self, entry: u32) -> Result<(), String> {
        self.validate_cpu()?;
        self.cache.reconcile(&mut self.memory);
        self.cache.select(entry, &self.memory);
        Ok(())
    }
    fn validate_cpu(&self) -> Result<(), String> {
        self.validate_comparison_admission()?;
        // All generated stores share one deferred journal. Its outer finish
        // needs one new generation; pure in-chain helpers cannot consume it.
        if self.memory.epoch() == u64::MAX {
            return Err("execution memory generation exhausted before admission".into());
        }
        // This first lowering implements the flat PE32 userspace segments and
        // FS base. Other descriptors are an explicit unsupported admission.
        if self.state.segments.iter().enumerate().any(|(n, s)| {
            (n != 4 && s.base != 0)
                || s.limit != u32::MAX
                || !(s.attributes == 0 || s.attributes == if n == 1 { 0xfa } else { 0xf2 })
        }) {
            return Err("unsupported segment descriptor at backend admission".into());
        }
        if self.state.eflags & (0x100 | 0x10000 | 0x20000 | 0x40000) != 0 {
            return Err(
                "unsupported single-step, resume-flag, VM86, or alignment-check state at backend admission"
                    .into(),
            );
        }
        Ok(())
    }
}

#[derive(Default)]
struct Registry {
    next: u32,
    backends: BTreeMap<u32, Backend>,
    buffers: BTreeMap<usize, Box<[u8]>>,
    buffer_owners: BTreeMap<usize, u32>,
    error: Vec<u8>,
}
thread_local! {static REGISTRY:RefCell<Registry>=RefCell::new(Registry::default());}
fn result(r: &mut Registry, value: Result<(), String>) -> u32 {
    match value {
        Ok(()) => {
            r.error.clear();
            0
        }
        Err(e) => {
            r.error = e.into_bytes();
            1
        }
    }
}
fn backend_mut(r: &mut Registry, handle: u32) -> Result<&mut Backend, String> {
    let backend = execution_backend_mut(r, handle)?;
    // Every host operation that can mutate CPU/cache/memory or cross Runtime
    // ends the synchronous invocation proof, including failed operations.
    backend.cache.end_invocation();
    Ok(backend)
}
fn execution_backend_mut(r: &mut Registry, handle: u32) -> Result<&mut Backend, String> {
    r.backends
        .get_mut(&handle)
        .ok_or_else(|| "invalid backend handle".into())
}
fn capture_backend_mut(r: &mut Registry, handle: u32) -> Result<&mut Backend, String> {
    let backend = backend_mut(r, handle)?;
    if backend.runtime_adapter.is_initialized() {
        return Err("capture memory mutation is unavailable after Runtime attachment".into());
    }
    Ok(backend)
}

/// Direct Wasm-to-Wasm arithmetic import. The handle resolves owned state and
/// mappings; generated code cannot pass arbitrary host pointers to this helper.
#[unsafe(no_mangle)]
pub extern "C" fn backend_x87(
    handle: u32,
    descriptor: u32,
    segment: u32,
    offset: u32,
    eip: u32,
    fop: u32,
) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let outcome = execution_backend_mut(r, handle).and_then(|backend| {
            let outcome = x87::execute(
                &mut backend.state,
                &mut backend.memory,
                descriptor,
                segment,
                offset,
                eip,
                fop,
            );
            if !outcome.as_ref().is_ok_and(|&kind| kind == 0) {
                backend.cache.end_invocation();
            }
            outcome
        });
        match outcome {
            Ok(kind) => {
                r.error.clear();
                kind
            }
            Err(error) => {
                if let Some(backend) = r.backends.get_mut(&handle) {
                    backend.state.exit = state::ExitState {
                        kind: state::exit_kind::UNSUPPORTED,
                        fault_eip: eip,
                        ..Default::default()
                    };
                }
                r.error = error.into_bytes();
                state::exit_kind::UNSUPPORTED
            }
        }
    })
}

/// Pure exact arithmetic used directly by generated Wasm. Operands are raw
/// values, never host pointers or guest addresses. Success returns a stable,
/// Backend-owned result valid until the next successful call or backend_drop.
/// The caller consumes it synchronously before any subsequent numeric call.
#[unsafe(no_mangle)]
pub extern "C" fn backend_fp_numeric(
    handle: u32,
    operation: u32,
    control: u32,
    a0: u32,
    a1: u32,
    a2: u32,
    b0: u32,
    b1: u32,
    b2: u32,
) -> usize {
    REGISTRY.with_borrow_mut(|r| {
        let outcome =
            execution_backend_mut(r, handle).and_then(|backend| {
                match fp_ir::numeric_mode(
                    backend.numeric_mode,
                    operation,
                    control,
                    [a0, a1, a2],
                    [b0, b1, b2],
                ) {
                    Ok(output) => {
                        *backend.numeric_output = output;
                        Ok(std::ptr::from_mut(&mut *backend.numeric_output) as usize)
                    }
                    Err(error) => {
                        backend.cache.end_invocation();
                        Err(format!("numeric helper rejected input: {error:?}"))
                    }
                }
            });
        match outcome {
            Ok(pointer) => {
                r.error.clear();
                pointer
            }
            Err(error) => {
                r.error = error.into_bytes();
                0
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn backend_new(maximum_pages: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| match Backend::new(maximum_pages as usize) {
        Ok(b) => {
            let Some(h) = r.next.checked_add(1) else {
                r.error = b"backend handle exhaustion".to_vec();
                return 0;
            };
            r.next = h;
            r.backends.insert(h, b);
            h
        }
        Err(e) => {
            r.error = e.into_bytes();
            0
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_drop(handle: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = r
            .backends
            .remove(&handle)
            .map(|_| ())
            .ok_or_else(|| "invalid backend handle".into());
        if value.is_ok() {
            let retired: Vec<_> = r
                .buffer_owners
                .iter()
                .filter_map(|(&ptr, &owner)| (owner == handle).then_some(ptr))
                .collect();
            for ptr in retired {
                r.buffer_owners.remove(&ptr);
                r.buffers.remove(&ptr);
            }
        }
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_alloc(len: u32) -> usize {
    REGISTRY.with_borrow_mut(|r| {
        if len == 0 || len > 16 * 1024 * 1024 {
            r.error = b"invalid transfer buffer length".to_vec();
            return 0;
        }
        let mut data = Vec::new();
        if data.try_reserve_exact(len as usize).is_err() {
            r.error = b"transfer allocation failed".to_vec();
            return 0;
        }
        data.resize(len as usize, 0);
        let mut data = data.into_boxed_slice();
        let ptr = data.as_mut_ptr() as usize;
        r.buffers.insert(ptr, data);
        ptr
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_free(ptr: usize, len: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = if r.buffers.get(&ptr).is_some_and(|b| b.len() == len as usize) {
            r.buffers.remove(&ptr);
            r.buffer_owners.remove(&ptr);
            Ok(())
        } else {
            Err("invalid owned transfer buffer".into())
        };
        result(r, value)
    })
}

/// Bulk Runtime transport is bounded by this mirror's admitted page quota,
/// including descriptor overhead. Buffers belong to this backend until freed.
#[unsafe(no_mangle)]
pub extern "C" fn backend_input_alloc(handle: u32, len: u32) -> usize {
    REGISTRY.with_borrow_mut(|r| {
        let Some(b) = r.backends.get(&handle) else {
            r.error = b"invalid backend handle".to_vec();
            return 0;
        };
        let cap = b.memory.maximum_pages() as u64 * (PAGE_SIZE as u64 + 16) + 52 + 65536 * 40;
        let outstanding: u64 = r
            .buffer_owners
            .iter()
            .filter(|(_, owner)| **owner == handle)
            .map(|(ptr, _)| r.buffers[ptr].len() as u64)
            .sum();
        if len == 0 || u64::from(len) + outstanding > cap {
            r.error = b"Runtime transfer exceeds backend memory quota".to_vec();
            return 0;
        }
        let mut data = Vec::new();
        if data.try_reserve_exact(len as usize).is_err() {
            r.error = b"Runtime transfer allocation failed".to_vec();
            return 0;
        }
        data.resize(len as usize, 0);
        let mut data = data.into_boxed_slice();
        let ptr = data.as_mut_ptr() as usize;
        r.buffers.insert(ptr, data);
        r.buffer_owners.insert(ptr, handle);
        r.error.clear();
        ptr
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn backend_apply_runtime_update(
    handle: u32,
    initial: u32,
    control_ptr: usize,
    control_len: u32,
    payload_ptr: usize,
    payload_len: u32,
) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            backend_mut(r, handle)?;
            if initial > 1 {
                return Err("invalid initial reflection flag".into());
            }
            let owned = |ptr: usize, len: u32| -> Result<&[u8], String> {
                if ptr == 0 && len == 0 {
                    return Ok(&[]);
                }
                if r.buffer_owners.get(&ptr) != Some(&handle) {
                    return Err("Runtime transport belongs to another backend".into());
                }
                r.buffers
                    .get(&ptr)
                    .filter(|b| b.len() == len as usize)
                    .map(|b| b.as_ref())
                    .ok_or_else(|| "invalid Runtime transport buffer".into())
            };
            let control = owned(control_ptr, control_len)?;
            let payload = owned(payload_ptr, payload_len)?;
            let b = r
                .backends
                .get_mut(&handle)
                .ok_or("invalid backend handle")?;
            b.cache.end_invocation();
            b.runtime_adapter
                .apply_update(&mut b.memory, initial == 1, control, payload)?;
            // The validated VINEDELT flags describe these mapping/byte changes.
            // They are not a request to discard every independent AOT unit.
            if control[44] & 0b110 != 0 {
                b.cache.invalidate_changed(&b.memory);
            }
            Ok(())
        })();
        result(r, value)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn backend_writeback_prepare(handle: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = backend_mut(r, handle).and_then(|b| {
            let (control, payload) = b.runtime_adapter.prepare_writeback(&mut b.memory)?;
            b.writeback_control = control;
            b.writeback_payload = payload;
            Ok(())
        });
        result(r, value)
    })
}
/// Read-only capture metadata. The borrowed result expires at the next prepare
/// or backend drop; memory growth also invalidates host views. No ACK is created.
#[unsafe(no_mangle)]
pub extern "C" fn backend_capture_prepare(handle: u32, maximum_pages: u32, dirty_only: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = execution_backend_mut(r, handle).and_then(|b| {
            b.capture_control.clear();
            if dirty_only > 1 {
                return Err("invalid capture mode".into());
            }
            b.capture_control = b.runtime_adapter.capture_metadata(
                &b.memory,
                maximum_pages as usize,
                dirty_only == 1,
            )?;
            Ok(())
        });
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_capture_control_ptr(handle: u32) -> usize {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .map_or(0, |b| b.capture_control.as_ptr() as usize)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_capture_control_len(handle: u32) -> u32 {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .map_or(0, |b| b.capture_control.len() as u32)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_writeback_ack(
    handle: u32,
    generation_low: u32,
    generation_high: u32,
) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = backend_mut(r, handle).and_then(|b| {
            let generation = u64::from(generation_low) | (u64::from(generation_high) << 32);
            b.runtime_adapter
                .acknowledge_writeback(&mut b.memory, generation)?;
            b.writeback_control.clear();
            b.writeback_payload.clear();
            Ok(())
        });
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_writeback_control_ptr(handle: u32) -> usize {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .map_or(0, |b| b.writeback_control.as_ptr() as usize)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_writeback_control_len(handle: u32) -> u32 {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .map_or(0, |b| b.writeback_control.len() as u32)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_writeback_payload_ptr(handle: u32) -> usize {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .map_or(0, |b| b.writeback_payload.as_ptr() as usize)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_writeback_payload_len(handle: u32) -> u32 {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .map_or(0, |b| b.writeback_payload.len() as u32)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_map_page(
    handle: u32,
    address: u32,
    owner: u32,
    flags: u32,
    ptr: usize,
) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            capture_backend_mut(r, handle)?;
            if r.buffer_owners
                .get(&ptr)
                .is_some_and(|owner| *owner != handle)
            {
                return Err("page source belongs to another backend".into());
            }
            let bytes = r
                .buffers
                .get(&ptr)
                .filter(|b| b.len() >= PAGE_SIZE)
                .ok_or("page source is not an owned bounded transfer buffer")?[..PAGE_SIZE]
                .to_vec();
            let backend = capture_backend_mut(r, handle)?;
            if backend.memory.entry(address).flags & memory::MAPPED != 0 {
                backend.memory.reset_page(address, owner, flags, &bytes)
            } else {
                backend.memory.map_page(address, owner, flags, &bytes)
            }
            .map_err(|e| e.to_string())
        })();
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_protect_page(handle: u32, address: u32, owner: u32, flags: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = capture_backend_mut(r, handle).and_then(|b| {
            b.memory
                .protect_page(address, owner, flags)
                .map_err(|e| e.to_string())
        });
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_map_alias(
    handle: u32,
    address: u32,
    source: u32,
    owner: u32,
    flags: u32,
) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = capture_backend_mut(r, handle).and_then(|b| {
            b.memory
                .map_alias(address, source, owner, flags)
                .map_err(|e| e.to_string())
        });
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_state(handle: u32) -> usize {
    REGISTRY.with_borrow_mut(|r| {
        r.backends
            .get_mut(&handle)
            .map_or(0, |b| std::ptr::from_mut(&mut *b.state) as usize)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_initialize_state(handle: u32, entry: u32, esp: u32, fs: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = backend_mut(r, handle).map(|b| {
            *b.state = CpuState::thread(entry, esp, fs);
            b.pending_timestamp = None;
            b.comparison = None;
        });
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_resume(handle: u32, eip: u32, esp: u32, eax: u32, write_eax: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            let b = backend_mut(r, handle)?;
            if write_eax > 1 {
                return Err("invalid resume accumulator flag".into());
            }
            b.state.eip = eip;
            b.state.gpr[4] = esp;
            if write_eax == 1 {
                b.state.gpr[0] = eax;
            }
            b.state.exit = state::ExitState::default();
            b.pending_timestamp = None;
            b.comparison = None;
            Ok(())
        })();
        result(r, value)
    })
}
/// Called only by a generated RDTSC exit after its CPU state is published.
#[unsafe(no_mangle)]
pub extern "C" fn backend_begin_timestamp_counter(handle: u32, unit_id: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        match backend_mut(r, handle).and_then(|b| b.begin_timestamp(unit_id)) {
            Ok(ticket) => {
                r.error.clear();
                ticket
            }
            Err(error) => {
                r.error = error.into_bytes();
                0
            }
        }
    })
}
/// Complete exactly the pending instruction using a Runtime-owned clock sample.
#[unsafe(no_mangle)]
pub extern "C" fn backend_complete_timestamp_counter(
    handle: u32,
    expected_eip: u32,
    low: u32,
    high: u32,
) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value =
            backend_mut(r, handle).and_then(|b| b.complete_timestamp(expected_eip, low, high));
        result(r, value)
    })
}
/// CPU context replacement invalidates a suspended synchronous completion.
#[unsafe(no_mangle)]
pub extern "C" fn backend_cancel_timestamp_counter(handle: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = backend_mut(r, handle).map(|b| {
            b.pending_timestamp = None;
        });
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_create_context(
    handle: u32,
    entry: u32,
    esp: u32,
    fs: u32,
    ptr: usize,
) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            backend_mut(r, handle)?;
            if r.buffer_owners.get(&ptr) != Some(&handle) {
                return Err("context buffer belongs to another backend".into());
            }
            let bytes = r
                .buffers
                .get_mut(&ptr)
                .filter(|b| b.len() == state::STATE_BYTES)
                .ok_or("invalid context buffer size")?;
            bytes.copy_from_slice(&CpuState::thread(entry, esp, fs).snapshot());
            Ok(())
        })();
        result(r, value)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn backend_snapshot_size() -> u32 {
    snapshot::SNAPSHOT_BYTES as u32
}

fn snapshot_buffer(r: &Registry, handle: u32, ptr: usize, length: u32) -> Result<(), String> {
    if length as usize != snapshot::SNAPSHOT_BYTES
        || r.buffer_owners.get(&ptr) != Some(&handle)
        || !r
            .buffers
            .get(&ptr)
            .is_some_and(|bytes| bytes.len() == snapshot::SNAPSHOT_BYTES)
    {
        return Err("invalid owned backend snapshot buffer".into());
    }
    Ok(())
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_save_snapshot(handle: u32, ptr: usize, length: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            let b = backend_mut(r, handle)?;
            let bytes = snapshot::encode(&b.state, b.comparison.as_ref())?;
            snapshot_buffer(r, handle, ptr, length)?;
            r.buffers
                .get_mut(&ptr)
                .expect("validated buffer")
                .copy_from_slice(&bytes);
            Ok(())
        })();
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_restore_snapshot(handle: u32, ptr: usize, length: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            // Failed host mutation also revokes a synchronous link proof. CPU,
            // continuation and pending timestamp publish only after validation.
            backend_mut(r, handle)?;
            snapshot_buffer(r, handle, ptr, length)?;
            let (state, scas) = snapshot::decode(r.buffers.get(&ptr).expect("validated buffer"))?;
            let b = execution_backend_mut(r, handle)?;
            *b.state = state;
            b.comparison = scas;
            b.pending_timestamp = None;
            Ok(())
        })();
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_create_snapshot(
    handle: u32,
    entry: u32,
    esp: u32,
    fs: u32,
    ptr: usize,
    length: u32,
) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            backend_mut(r, handle)?;
            snapshot_buffer(r, handle, ptr, length)?;
            let bytes = snapshot::encode(&CpuState::thread(entry, esp, fs), None)?;
            r.buffers
                .get_mut(&ptr)
                .expect("validated buffer")
                .copy_from_slice(&bytes);
            Ok(())
        })();
        result(r, value)
    })
}

/// Bounded comparison boundary. The legacy import name also admits CMPS via
/// its checked descriptor; `source` is EAX for SCAS or ESI for CMPS.
/// Generated code still performs every element,
/// checked memory read, arithmetic flag update and architectural progress.
#[unsafe(no_mangle)]
pub extern "C" fn backend_scas_begin(
    handle: u32,
    unit: u32,
    eip: u32,
    flags: u32,
    defined: u32,
    source: u32,
    ecx: u32,
    edi: u32,
    descriptor: u32,
) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = execution_backend_mut(r, handle).and_then(|b| {
            b.begin_comparison(unit, eip, flags, defined, source, ecx, edi, descriptor)
        });
        if value.is_err()
            && let Some(b) = r.backends.get_mut(&handle)
        {
            b.cache.end_invocation();
            // Transport/identity rejection is an explicit terminal engine exit,
            // not a delivered guest fault or a reusable partial continuation.
            b.comparison = None;
        }
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_scas_complete(handle: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = execution_backend_mut(r, handle).map(|b| {
            b.comparison = None;
        });
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_scas_finish(handle: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = execution_backend_mut(r, handle).and_then(Backend::finish_comparison);
        if value.is_err()
            && let Some(b) = r.backends.get_mut(&handle)
        {
            b.cache.end_invocation();
            b.comparison = None;
        }
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_table(handle: u32) -> usize {
    REGISTRY.with_borrow_mut(|r| {
        r.backends
            .get_mut(&handle)
            .map_or(0, |b| b.memory.page_table_ptr())
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_page_ptr(handle: u32, address: u32) -> u32 {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .map_or(0, |b| b.memory.entry(address).data_ptr)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_read_guest(handle: u32, address: u32, ptr: usize, length: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            backend_mut(r, handle)?;
            if r.buffer_owners.get(&ptr) != Some(&handle)
                || !r
                    .buffers
                    .get(&ptr)
                    .is_some_and(|b| b.len() == length as usize)
            {
                return Err("invalid owned guest read destination".into());
            }
            let b = r
                .backends
                .get_mut(&handle)
                .ok_or("invalid backend handle")?;
            b.cache.end_invocation();
            let bytes = b
                .memory
                .read(address, length as usize, Access::Read)
                .map_err(|e| e.to_string())?;
            r.buffers
                .get_mut(&ptr)
                .expect("validated output")
                .copy_from_slice(&bytes);
            Ok(())
        })();
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_compile(handle: u32, entry: u32, limit: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = backend_mut(r, handle).and_then(|b| b.compile(entry, limit as usize));
        result(r, value)
    })
}
/// Bounded roots in a backend-owned transfer buffer. This only prepares code;
/// it neither changes the architectural PC nor executes a guest instruction.
#[unsafe(no_mangle)]
pub extern "C" fn backend_compile_entries(handle: u32, ptr: usize, count: u32, limit: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            if !(1..=256).contains(&count) || r.buffer_owners.get(&ptr) != Some(&handle) {
                return Err("invalid owned compile roots".into());
            }
            let bytes = r
                .buffers
                .get(&ptr)
                .filter(|b| b.len() == count as usize * 4)
                .ok_or("invalid compile roots buffer extent")?;
            let entries: Vec<_> = bytes
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().expect("four root bytes")))
                .collect();
            backend_mut(r, handle)?.compile_entries(&entries, limit as usize)
        })();
        result(r, value)
    })
}
/// Select a reusable unit for EIP. Success with selected id zero is a cache miss.
#[unsafe(no_mangle)]
pub extern "C" fn backend_cache_select(handle: u32, entry: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = backend_mut(r, handle).and_then(|b| b.select(entry));
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_cache_selected(handle: u32) -> u32 {
    REGISTRY.with_borrow(|r| r.backends.get(&handle).map_or(0, |b| b.cache.selected_id()))
}
/// Rust owns a bounded table slot; the host installs `link` before binding it.
#[unsafe(no_mangle)]
pub extern "C" fn backend_link_slot(handle: u32, id: u32) -> u32 {
    REGISTRY.with_borrow(|r| r.backends.get(&handle).map_or(0, |b| b.cache.link_slot(id)))
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_link_bind(handle: u32, id: u32, slot: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = backend_mut(r, handle).and_then(|b| {
            b.cache.reconcile(&mut b.memory);
            b.cache.bind(id, slot, &b.memory)
        });
        result(r, value)
    })
}
/// Full outer admission. Only one synchronous run_linked may follow; callers
/// pair this with link_end in finally, including a native Wasm trap.
#[unsafe(no_mangle)]
pub extern "C" fn backend_link_begin(handle: u32, id: u32, entry: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = backend_mut(r, handle).and_then(|b| {
            b.validate_cpu()?;
            b.cache.reconcile(&mut b.memory);
            b.cache.begin_invocation(id, entry, &b.memory)
        });
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_link_end(handle: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        match backend_mut(r, handle) {
            // Cleanup must retain a generated helper's failure diagnostic.
            Ok(_) => 0,
            Err(error) => result(r, Err(error)),
        }
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_link_memo_hits(handle: u32) -> u32 {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .map_or(0, |b| b.cache.memo_counts().0)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_link_memo_misses(handle: u32) -> u32 {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .map_or(0, |b| b.cache.memo_counts().1)
    })
}
/// Called directly by generated Wasm. High32 is the immutable unit identity,
/// low32 is table slot+1; zero requests an ordinary outer kind2 exit.
#[unsafe(no_mangle)]
pub extern "C" fn backend_link_resolve(handle: u32, entry: u32) -> u64 {
    REGISTRY.with_borrow_mut(|r| {
        let Some(b) = r.backends.get_mut(&handle) else {
            return 0;
        };
        // Generated instructions cannot change admitted segment descriptors or
        // TF/RF/VM/AC bits. Every host mutation ends the invocation, so its CPU
        // profile admission remains valid while GPR/flags live in the link ABI.
        // String helpers can change private continuation state within a chain;
        // retain their dynamic admission check even in the admitted profile.
        let admission = if b.cache.invocation_is_current(&b.memory) {
            b.validate_comparison_admission()
        } else {
            b.validate_cpu()
        };
        if admission.is_err() {
            b.cache.end_invocation();
            return 0;
        }
        b.cache.resolve(entry, &b.memory)
    })
}
/// Required immediately before calling the instance bound to this handle/id.
#[unsafe(no_mangle)]
pub extern "C" fn backend_cache_validate(handle: u32, id: u32, entry: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = backend_mut(r, handle).and_then(|b| b.validate_unit(id, entry));
        result(r, value)
    })
}
/// 0 means empty; FFFFFFFF releases every browser instance. Rust units may remain.
#[unsafe(no_mangle)]
pub extern "C" fn backend_cache_retired(handle: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| backend_mut(r, handle).map_or(0, |b| b.cache.pop_retired()))
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_cache_invalidate(handle: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = backend_mut(r, handle).map(|b| b.cache.clear());
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_cache_unit_count(handle: u32) -> u32 {
    REGISTRY.with_borrow(|r| r.backends.get(&handle).map_or(0, |b| b.cache.len() as u32))
}
/// Unit retirements by cause: 0 explicit clear, 1 demand capacity, 2 dependency.
/// Read-only diagnostics; these counters never authorize code execution.
#[unsafe(no_mangle)]
pub extern "C" fn backend_cache_retirement_count(handle: u32, reason: u32) -> u64 {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .map_or(0, |b| b.cache.retirement_count(reason))
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_cache_bytes(handle: u32) -> u32 {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .map_or(0, |b| b.cache.bytes() as u32)
    })
}
/// Mode zero retains the default 64-unit/16-MiB budget; mode one is a bounded
/// 512-unit/16-MiB experiment. Configure before any compilation is attempted.
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_cache_capacity(handle: u32, mode: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = backend_mut(r, handle).and_then(|b| b.cache.set_capacity_mode(mode));
        result(r, value)
    })
}
/// Number of host table slots and maximum pending individual retirements.
/// Draining may additionally require one read of the zero terminator.
#[unsafe(no_mangle)]
pub extern "C" fn backend_cache_capacity(handle: u32) -> u32 {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .map_or(0, |b| b.cache.capacity() as u32)
    })
}
/// Maximum table extent, including the independently bounded AOT residency.
#[unsafe(no_mangle)]
pub extern "C" fn backend_cache_maximum_capacity() -> u32 {
    code_cache::MAXIMUM_LINK_UNITS as u32
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_cache_byte_limit(handle: u32) -> u32 {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .map_or(0, |b| b.cache.byte_limit() as u32)
    })
}
/// Optional coherent directory. Fixed before compilation; its protected ABI
/// contains no guest state and is never part of a CPU snapshot.
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_direct_links(handle: u32, enabled: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = backend_mut(r, handle).and_then(|b| {
            if enabled > 1 {
                return Err("invalid direct link option".into());
            }
            b.cache.set_direct_links(enabled != 0, &mut b.memory)
        });
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_link_directory(handle: u32) -> usize {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .map_or(0, |b| b.cache.direct_pointer())
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_link_directory_bytes(handle: u32) -> u32 {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .map_or(0, |b| b.cache.direct_bytes() as u32)
    })
}
/// Wrapping generated hit/miss counters and saturating publication-capacity
/// misses. These are diagnostics, not guest progress or FPS measurements.
#[unsafe(no_mangle)]
pub extern "C" fn backend_link_directory_count(handle: u32, counter: u32) -> u32 {
    REGISTRY.with_borrow(|r| {
        r.backends.get(&handle).map_or(0, |b| {
            let (hits, misses, capacity) = b.cache.direct_counts();
            match counter {
                0 => hits,
                1 => misses,
                2 => capacity,
                _ => 0,
            }
        })
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_structured_loops(handle: u32, enabled: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            let b = backend_mut(r, handle)?;
            if enabled > 1 {
                return Err("invalid structured loop option".into());
            }
            if enabled != 0 && (b.diagnostic_stop.is_some() || b.sampled_pc) {
                return Err("CPU diagnostics require ordinary control-flow lowering".into());
            }
            if b.structured_loops != (enabled != 0) {
                b.structured_loops = enabled != 0;
                b.cache.clear();
            }
            Ok(())
        })();
        result(r, value)
    })
}
/// Experimental content-stable code: instances import their immutable unit
/// identity instead of embedding it in generated bytes. Logical identities,
/// admission, retirement and cache bounds remain owned by this backend.
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_binding_global(handle: u32, enabled: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            let b = backend_mut(r, handle)?;
            if enabled > 1 {
                return Err("invalid immutable binding option".into());
            }
            if b.binding_global != (enabled != 0) {
                if b.pending_timestamp.is_some() || b.comparison.is_some() {
                    return Err("cannot change binding mode with a pending CPU continuation".into());
                }
                b.cache.clear();
                b.binding_global = enabled != 0;
            }
            Ok(())
        })();
        result(r, value)
    })
}
/// Change only private generated-function organization. CPU/memory/cache
/// authority and the public link ABI remain unchanged.
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_function_cfg(handle: u32, enabled: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            let b = backend_mut(r, handle)?;
            if enabled > 1 {
                return Err("invalid function CFG option".into());
            }
            if b.function_cfg != (enabled != 0) {
                if b.pending_timestamp.is_some() || b.comparison.is_some() {
                    return Err("cannot change function CFG with a pending CPU continuation".into());
                }
                b.cache.clear();
                b.function_cfg = enabled != 0;
            }
            Ok(())
        })();
        result(r, value)
    })
}
/// Binary-derived control discovery/lowering; no guest-value specialization.
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_control_graph(handle: u32, enabled: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            let b = backend_mut(r, handle)?;
            if enabled > 1 {
                return Err("invalid control graph option".into());
            }
            if b.control_graph != (enabled != 0) {
                if b.pending_timestamp.is_some() || b.comparison.is_some() {
                    return Err(
                        "cannot change control graph with a pending CPU continuation".into(),
                    );
                }
                b.cache.clear();
                b.control_graph = enabled != 0;
            }
            Ok(())
        })();
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_contains(handle: u32, entry: u32) -> u32 {
    REGISTRY.with_borrow(|r| u32::from(r.backends.get(&handle).is_some_and(|b| b.contains(entry))))
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_affine_memory(handle: u32, enabled: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            let b = backend_mut(r, handle)?;
            if enabled > 1 {
                return Err("invalid affine memory option".into());
            }
            if enabled != 0 && (b.diagnostic_stop.is_some() || b.sampled_pc) {
                return Err("CPU diagnostics require ordinary control-flow lowering".into());
            }
            if b.affine_memory != (enabled != 0) {
                b.affine_memory = enabled != 0;
                b.cache.clear();
            }
            Ok(())
        })();
        result(r, value)
    })
}
/// Experimental loop-scoped memory proofs. Changing the lowering retires every
/// old unit; the CPU snapshot and authoritative Runtime contract are unchanged.
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_loop_memory(handle: u32, enabled: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            let b = backend_mut(r, handle)?;
            if enabled > 1 {
                return Err("invalid loop memory option".into());
            }
            if enabled != 0 && (b.diagnostic_stop.is_some() || b.sampled_pc) {
                return Err("CPU diagnostics require ordinary control-flow lowering".into());
            }
            if b.loop_memory != (enabled != 0) {
                if b.pending_timestamp.is_some() || b.comparison.is_some() {
                    return Err(
                        "cannot change loop memory mode with a pending CPU continuation".into(),
                    );
                }
                b.cache.clear();
                b.loop_memory = enabled != 0;
            }
            Ok(())
        })();
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_fp_ir(handle: u32, enabled: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            let backend = backend_mut(r, handle)?;
            if enabled > 1 {
                return Err("invalid explicit floating-point option".into());
            }
            if enabled == 0
                && (backend.typed_numeric
                    || backend.numeric_mode != numeric_mode::NumericMode::Strict)
            {
                return Err(
                    "disable typed/approximate numerical modes before floating-point IR".into(),
                );
            }
            if backend.fp_ir != (enabled != 0) {
                backend.fp_ir = enabled != 0;
                backend.cache.clear();
            }
            Ok(())
        })();
        result(r, value)
    })
}
/// Numerical policy is Rust-owned. Changing it retires every compiled unit
/// and invocation proof; CPU state and GuestMemory remain unchanged.
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_numeric_mode(handle: u32, mode: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            let backend = backend_mut(r, handle)?;
            let mode = numeric_mode::NumericMode::try_from(mode)?;
            if mode != numeric_mode::NumericMode::Strict && !backend.fp_ir {
                return Err("approximation requires explicit floating-point IR".into());
            }
            if backend.numeric_mode != mode {
                backend.numeric_mode = mode;
                backend.cache.clear();
            }
            Ok(())
        })();
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_typed_numeric(handle: u32, enabled: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            let backend = backend_mut(r, handle)?;
            if enabled > 1 || enabled != 0 && !backend.fp_ir {
                return Err("typed numeric kernels require explicit floating-point IR".into());
            }
            if backend.typed_numeric != (enabled != 0) {
                backend.typed_numeric = enabled != 0;
                backend.cache.clear();
            }
            Ok(())
        })();
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_value_ir(handle: u32, enabled: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            let backend = backend_mut(r, handle)?;
            if enabled > 1 {
                return Err("invalid scalar value IR option".into());
            }
            if enabled != 0 && (backend.diagnostic_stop.is_some() || backend.sampled_pc) {
                return Err("CPU diagnostics require ordinary control-flow lowering".into());
            }
            if backend.value_ir != (enabled != 0) {
                backend.value_ir = enabled != 0;
                backend.cache.clear();
            }
            Ok(())
        })();
        result(r, value)
    })
}
/// Numeric predicate/value lowering retains the ordinary multi-entry units.
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_predicate_ir(handle: u32, enabled: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            let backend = backend_mut(r, handle)?;
            if enabled > 1 {
                return Err("invalid predicate IR option".into());
            }
            if enabled != 0
                && (backend.diagnostic_stop.is_some()
                    || backend.sampled_pc
                    || backend.value_ir
                    || backend.flow_ir
                    || backend.rooted_ssa
                    || backend.control_graph
                    || backend.function_cfg
                    || backend.loop_memory
                    || backend.structured_loops
                    || backend.affine_memory
                    || backend.cache.direct_pointer() != 0)
            {
                return Err("predicate IR requires its own value/control lowering".into());
            }
            if backend.predicate_ir != (enabled != 0) {
                if backend.pending_timestamp.is_some() || backend.comparison.is_some() {
                    return Err(
                        "complete the pending CPU continuation before changing predicate IR".into(),
                    );
                }
                backend.predicate_ir = enabled != 0;
                backend.cache.clear();
            }
            Ok(())
        })();
        result(r, value)
    })
}
/// Whole-unit SSA includes all external/indirect entry ports. It shares the
/// ordinary discovery and AOT/JIT cache policy, never the v86 execution ABI.
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_flow_ir(handle: u32, enabled: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            let backend = backend_mut(r, handle)?;
            if enabled > 1 {
                return Err("invalid flow IR option".into());
            }
            if enabled != 0
                && (backend.diagnostic_stop.is_some()
                    || backend.sampled_pc
                    || backend.value_ir
                    || backend.predicate_ir
                    || backend.rooted_ssa
                    || backend.control_graph
                    || backend.function_cfg
                    || backend.loop_memory
                    || backend.structured_loops
                    || backend.affine_memory
                    || backend.cache.direct_pointer() != 0)
            {
                return Err("flow IR requires its own value/control lowering".into());
            }
            if backend.flow_ir != (enabled != 0) {
                if backend.pending_timestamp.is_some() || backend.comparison.is_some() {
                    return Err(
                        "complete the pending CPU continuation before changing flow IR".into(),
                    );
                }
                backend.flow_ir = enabled != 0;
                backend.cache.clear();
            }
            Ok(())
        })();
        result(r, value)
    })
}
/// Rooted SSA units admit exactly their discovery root. An arbitrary guest PC
/// requires another ordinary version, retaining unrestricted CALL/RET behavior.
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_rooted_ssa(handle: u32, enabled: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            let backend = backend_mut(r, handle)?;
            if enabled > 1 {
                return Err("invalid rooted SSA option".into());
            }
            if enabled != 0 && (backend.diagnostic_stop.is_some() || backend.sampled_pc) {
                return Err("CPU diagnostics require ordinary control-flow lowering".into());
            }
            if backend.rooted_ssa != (enabled != 0) {
                backend.rooted_ssa = enabled != 0;
                backend.cache.clear();
            }
            Ok(())
        })();
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_validate(handle: u32, entry: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = backend_mut(r, handle).and_then(|b| b.validate(entry));
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_code_ptr(handle: u32) -> usize {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .and_then(|b| b.cache.selected())
            .map_or(0, |unit| unit.code.as_ptr() as usize)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_code_len(handle: u32) -> u32 {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .and_then(|b| b.cache.selected())
            .map_or(0, |unit| unit.code.len() as u32)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_block_count(handle: u32) -> u32 {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .and_then(|b| b.cache.selected())
            .map_or(0, |unit| unit.entries.len() as u32)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_instruction_count(handle: u32) -> u32 {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .and_then(|b| b.cache.selected())
            .map_or(0, |unit| unit.instruction_count)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_epoch(handle: u32) -> u32 {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .map_or(0, |b| b.memory.epoch() as u32)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_finish(handle: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        // A standalone replay observes writes; this does not acknowledge or publish
        // them to Runtime. Product integration must reconcile the whole writeback
        // batch against GuestMemory before a Runtime call or backend switch.
        let value = backend_mut(r, handle).and_then(|b| {
            b.memory.finish_execution().map_err(|e| e.to_string())?;
            b.finish_suspended_comparison_fetch();
            Ok(())
        });
        result(r, value)
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_error_ptr() -> usize {
    REGISTRY.with_borrow(|r| r.error.as_ptr() as usize)
}

#[cfg(test)]
mod snapshot_boundary_tests {
    use super::*;
    #[test]
    fn owned_snapshot_restore_is_atomic_and_create_does_not_replace_live_cpu() {
        let first = backend_new(1);
        let second = backend_new(1);
        let ptr = backend_input_alloc(first, snapshot::SNAPSHOT_BYTES as u32);
        let foreign = backend_input_alloc(second, snapshot::SNAPSHOT_BYTES as u32);
        assert_eq!(backend_initialize_state(first, 0x1111, 0x2222, 0x3333), 0);
        assert_eq!(backend_save_snapshot(first, ptr, 512), 0);
        let original = REGISTRY.with_borrow(|r| r.backends[&first].state.clone());
        assert_eq!(backend_restore_snapshot(first, foreign, 512), 1);
        assert_eq!(backend_restore_snapshot(first, ptr, 420), 1);
        REGISTRY.with_borrow_mut(|r| {
            r.buffers.get_mut(&ptr).unwrap()[424] = 1;
        });
        assert_eq!(backend_restore_snapshot(first, ptr, 512), 1);
        REGISTRY.with_borrow(|r| assert_eq!(r.backends[&first].state, original));
        assert_eq!(
            backend_create_snapshot(first, 0x4444, 0x5555, 0x6666, ptr, 512),
            0
        );
        REGISTRY.with_borrow(|r| assert_eq!(r.backends[&first].state, original));
        assert_eq!(backend_restore_snapshot(first, ptr, 512), 0);
        REGISTRY.with_borrow(|r| {
            assert_eq!(
                *r.backends[&first].state,
                CpuState::thread(0x4444, 0x5555, 0x6666)
            )
        });
        assert_eq!(backend_drop(first), 0);
        assert_eq!(backend_drop(second), 0);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_error_len() -> u32 {
    REGISTRY.with_borrow(|r| r.error.len() as u32)
}

#[cfg(test)]
mod transport_tests {
    use super::*;

    #[test]
    fn cache_capacity_configuration_is_per_backend_and_fixed_before_compilation() {
        let handle = backend_new(1);
        let other = backend_new(1);
        assert_eq!(backend_cache_capacity(handle), 64);
        assert_eq!(backend_cache_byte_limit(handle), 16 * 1024 * 1024);
        assert_eq!(backend_set_cache_capacity(handle, 2), 1);
        assert_eq!(backend_cache_capacity(handle), 64);
        assert_eq!(backend_set_cache_capacity(handle, 1), 0);
        assert_eq!(backend_cache_capacity(handle), 512);
        assert_eq!(backend_cache_byte_limit(handle), 16 * 1024 * 1024);
        assert_eq!(backend_cache_capacity(other), 64);
        REGISTRY.with_borrow_mut(|r| {
            backend_mut(r, handle)
                .unwrap()
                .memory
                .map_page(
                    0x1000,
                    1,
                    memory::READ | memory::EXECUTE | memory::SEALED,
                    &[0x90; PAGE_SIZE],
                )
                .unwrap();
        });
        assert_eq!(backend_compile(handle, 0x1000, 1), 0);
        let id = backend_cache_selected(handle);
        let before = REGISTRY.with_borrow(|r| {
            let b = &r.backends[&handle];
            (
                b.state.snapshot(),
                b.memory.epoch(),
                b.cache.selected().unwrap().code.clone(),
            )
        });
        assert_eq!(backend_set_cache_capacity(handle, 0), 1);
        assert_eq!(backend_set_cache_capacity(handle, 1), 1);
        assert_eq!(backend_cache_capacity(handle), 512);
        assert_eq!(backend_cache_selected(handle), id);
        REGISTRY.with_borrow(|r| {
            let b = &r.backends[&handle];
            assert_eq!(b.state.snapshot(), before.0);
            assert_eq!(b.memory.epoch(), before.1);
            assert_eq!(b.cache.selected().unwrap().code, before.2);
        });
        assert_eq!(backend_cache_invalidate(handle), 0);
        assert_eq!(backend_set_cache_capacity(handle, 0), 1);
        assert_eq!(backend_drop(handle), 0);
        assert_eq!(backend_cache_capacity(handle), 0);
        assert_eq!(backend_cache_byte_limit(handle), 0);
        assert_eq!(backend_set_cache_capacity(handle, 1), 1);
        assert_eq!(backend_drop(other), 0);
    }

    #[test]
    fn scalar_value_mode_retires_code_without_changing_cpu_or_memory() {
        let handle = backend_new(1);
        REGISTRY.with_borrow_mut(|r| {
            let backend = backend_mut(r, handle).unwrap();
            backend
                .memory
                .map_page(
                    0x1000,
                    7,
                    memory::READ | memory::EXECUTE | memory::SEALED,
                    &[0x90; PAGE_SIZE],
                )
                .unwrap();
            backend.state.eip = 0x1000;
            backend.state.gpr[0] = 0x76543210;
        });
        assert_eq!(backend_compile(handle, 0x1000, 1), 0);
        let old = backend_cache_selected(handle);
        assert_eq!(backend_set_value_ir(handle, 2), 1);
        assert_eq!(backend_cache_selected(handle), old);
        assert_eq!(backend_set_value_ir(handle, 1), 0);
        assert_eq!(backend_cache_retired(handle), old);
        assert_eq!(backend_cache_unit_count(handle), 0);
        REGISTRY.with_borrow(|r| {
            let backend = r.backends.get(&handle).unwrap();
            assert!(backend.value_ir);
            assert_eq!(backend.state.eip, 0x1000);
            assert_eq!(backend.state.gpr[0], 0x76543210);
            assert_eq!(
                backend.memory.read(0x1000, 1, Access::Execute).unwrap(),
                [0x90]
            );
        });
        assert_eq!(backend_compile(handle, 0x1000, 1), 0);
        let value = backend_cache_selected(handle);
        assert_eq!(backend_set_value_ir(handle, 1), 0);
        assert_eq!(backend_cache_selected(handle), value);
        assert_eq!(backend_set_value_ir(handle, 0), 0);
        assert_eq!(backend_cache_retired(handle), value);
        assert_eq!(backend_drop(handle), 0);
        assert_eq!(backend_set_value_ir(handle, 1), 1);
    }

    #[test]
    fn predicate_mode_preserves_pending_timestamp_and_comparison_identities() {
        for comparison in [false, true] {
            let handle = backend_new(1);
            REGISTRY.with_borrow_mut(|r| {
                let backend = backend_mut(r, handle).unwrap();
                let mut page = [0x90; PAGE_SIZE];
                page[..2].copy_from_slice(if comparison {
                    &[0xf3, 0xa6]
                } else {
                    &[0x0f, 0x31]
                });
                backend
                    .memory
                    .map_page(
                        0x1000,
                        1,
                        memory::READ | memory::EXECUTE | memory::SEALED,
                        &page,
                    )
                    .unwrap();
                backend.state.eip = 0x1000;
                backend.state.gpr[1] = 9000;
                backend.compile(0x1000, 1).unwrap();
                let unit = backend.cache.selected_id();
                if comparison {
                    backend
                        .begin_comparison(
                            unit,
                            0x1000,
                            backend.state.eflags,
                            backend.state.defined_flags,
                            0,
                            9000,
                            0,
                            0x501,
                        )
                        .unwrap();
                } else {
                    backend.state.exit = state::ExitState {
                        kind: state::exit_kind::TIMESTAMP_COUNTER,
                        fault_eip: 0x1000,
                        error: 2,
                        ..Default::default()
                    };
                    backend.begin_timestamp(unit).unwrap();
                }
            });
            let unit = backend_cache_selected(handle);
            let before = REGISTRY.with_borrow(|r| (*r.backends[&handle].state).clone());
            assert_eq!(backend_set_predicate_ir(handle, 2), 1);
            assert_eq!(backend_set_predicate_ir(handle, 1), 1);
            assert_eq!(backend_set_predicate_ir(handle, 0), 0); // no change is safe
            assert_eq!(backend_set_flow_ir(handle, 2), 1);
            assert_eq!(backend_set_flow_ir(handle, 1), 1);
            assert_eq!(backend_set_flow_ir(handle, 0), 0);
            assert_eq!(backend_cache_selected(handle), unit);
            REGISTRY.with_borrow_mut(|r| {
                let backend = backend_mut(r, handle).unwrap();
                assert_eq!(*backend.state, before);
                assert!(!backend.predicate_ir);
                assert!(!backend.flow_ir);
                if comparison {
                    assert!(backend.comparison.is_some());
                } else {
                    backend.complete_timestamp(0x1000, 123, 456).unwrap();
                }
            });
            assert_eq!(backend_drop(handle), 0);
        }
        let handle = backend_new(1);
        assert_eq!(backend_set_predicate_ir(handle, 1), 0);
        assert_eq!(backend_set_predicate_ir(handle, 0), 0);
        assert_eq!(backend_set_flow_ir(handle, 1), 0);
        assert_eq!(backend_set_predicate_ir(handle, 1), 1);
        assert_eq!(backend_set_flow_ir(handle, 0), 0);
        assert_eq!(backend_drop(handle), 0);
    }

    #[test]
    fn typed_numeric_mode_requires_fp_ir_and_retires_compiled_units() {
        let handle = backend_new(1);
        assert_eq!(backend_set_typed_numeric(handle, 1), 1);
        assert_eq!(backend_set_fp_ir(handle, 1), 0);
        REGISTRY.with_borrow_mut(|r| {
            backend_mut(r, handle)
                .unwrap()
                .memory
                .map_page(
                    0x1000,
                    1,
                    memory::READ | memory::EXECUTE | memory::SEALED,
                    &[0x90; PAGE_SIZE],
                )
                .unwrap();
        });
        assert_eq!(backend_compile(handle, 0x1000, 1), 0);
        let legacy = backend_cache_selected(handle);
        assert_eq!(backend_set_typed_numeric(handle, 1), 0);
        assert_eq!(backend_cache_retired(handle), legacy);
        assert_eq!(backend_cache_unit_count(handle), 0);
        assert_eq!(backend_set_fp_ir(handle, 0), 1);
        assert_eq!(backend_set_typed_numeric(handle, 2), 1);
        REGISTRY.with_borrow(|r| {
            let backend = r.backends.get(&handle).unwrap();
            assert!(backend.fp_ir && backend.typed_numeric);
        });
        assert_eq!(backend_set_typed_numeric(handle, 0), 0);
        assert_eq!(backend_set_fp_ir(handle, 0), 0);
        assert_eq!(backend_drop(handle), 0);
        assert_eq!(backend_set_typed_numeric(handle, 1), 1);
    }

    #[test]
    fn numeric_policy_is_validated_and_retires_code_without_mutating_cpu() {
        let handle = backend_new(1);
        assert_eq!(backend_set_numeric_mode(handle, 1), 1);
        assert_eq!(backend_set_fp_ir(handle, 1), 0);
        REGISTRY.with_borrow_mut(|r| {
            backend_mut(r, handle)
                .unwrap()
                .memory
                .map_page(
                    0x1000,
                    1,
                    memory::READ | memory::EXECUTE | memory::SEALED,
                    &[0x90; PAGE_SIZE],
                )
                .unwrap();
        });
        let cpu = REGISTRY.with_borrow(|r| (*r.backends[&handle].state).clone());
        for mode in [1, 2, 0] {
            assert_eq!(backend_compile(handle, 0x1000, 1), 0);
            let unit = backend_cache_selected(handle);
            assert_eq!(backend_set_numeric_mode(handle, 3), 1);
            assert_eq!(backend_cache_selected(handle), unit);
            assert_eq!(backend_set_numeric_mode(handle, mode), 0);
            assert_eq!(backend_cache_retired(handle), unit);
            assert_eq!(backend_cache_unit_count(handle), 0);
            REGISTRY.with_borrow(|r| assert_eq!(*r.backends[&handle].state, cpu));
            if mode != 0 {
                assert_eq!(backend_set_fp_ir(handle, 0), 1);
            }
        }
        assert_eq!(backend_set_fp_ir(handle, 0), 0);
        assert_eq!(backend_drop(handle), 0);
    }

    #[test]
    fn cached_units_survive_selection_and_context_switch_but_not_stale_code() {
        let handle = backend_new(2);
        REGISTRY.with_borrow_mut(|r| {
            let backend = backend_mut(r, handle).unwrap();
            for (address, owner) in [(0x1000, 1), (0x2000, 2)] {
                backend
                    .memory
                    .map_page(
                        address,
                        owner,
                        memory::READ | memory::EXECUTE | memory::SEALED,
                        &[0x90; PAGE_SIZE],
                    )
                    .unwrap();
            }
        });
        assert_eq!(backend_compile(handle, 0x1000, 1), 0);
        let first = backend_cache_selected(handle);
        assert_eq!(backend_compile(handle, 0x2000, 1), 0);
        let second = backend_cache_selected(handle);
        assert!(second > first);
        assert_eq!(backend_cache_unit_count(handle), 2);
        assert_eq!(backend_cache_select(handle, 0x1000), 0);
        assert_eq!(backend_cache_selected(handle), first);
        assert_eq!(backend_initialize_state(handle, 0x2000, 0x8000, 0x9000), 0);
        assert_eq!(backend_cache_select(handle, 0x2000), 0);
        assert_eq!(backend_cache_selected(handle), second);
        assert_eq!(backend_cache_validate(handle, first, 0x2000), 1);
        assert_eq!(backend_cache_validate(handle, first, 0x1000), 0);
        assert_eq!(
            backend_protect_page(handle, 0x1000, 1, memory::READ | memory::SEALED),
            0
        );
        assert_eq!(backend_cache_validate(handle, first, 0x1000), 1);
        assert_eq!(backend_cache_retired(handle), first);
        assert_eq!(backend_cache_validate(handle, second, 0x2000), 0);
        assert_eq!(backend_cache_select(handle, 0x1000), 0);
        assert_eq!(backend_cache_selected(handle), 0);
        assert_eq!(backend_set_structured_loops(handle, 1), 0);
        assert_eq!(backend_cache_retired(handle), second);
        assert_eq!(backend_cache_unit_count(handle), 0);
        assert_eq!(backend_compile(handle, 0x2000, 1), 0);
        assert!(backend_cache_selected(handle) > second);
        assert_eq!(backend_drop(handle), 0);
        assert_ne!(backend_cache_validate(handle, second, 0x2000), 0);
    }

    #[test]
    fn code_content_reuse_keeps_distinct_retired_bindings() {
        let mut backend = Backend::new(1).unwrap();
        let mut page = [0xcc; PAGE_SIZE];
        // Both REP SCAS continuation and RDTSC completion carry unit identity.
        page[..4].copy_from_slice(&[0xf2, 0xae, 0x0f, 0x31]);
        backend
            .memory
            .map_page(
                0x1000,
                1,
                memory::READ | memory::EXECUTE | memory::SEALED,
                &page,
            )
            .unwrap();
        backend.binding_global = true;
        backend.compile(0x1000, 4).unwrap();
        let first = backend.cache.selected_id();
        let code = backend.cache.selected().unwrap().code.clone();
        backend.cache.clear();
        assert_eq!(backend.cache.pop_retired(), first);
        backend.compile(0x1000, 4).unwrap();
        let second = backend.cache.selected_id();
        assert_ne!(first, second);
        assert_eq!(code, backend.cache.selected().unwrap().code);
        assert!(backend.validate_unit(first, 0x1000).is_err());
        backend.validate_unit(second, 0x1000).unwrap();
    }

    #[test]
    fn cache_admission_rejects_unobserved_alias_stores_and_unsupported_cpu_state() {
        let mut backend = Backend::new(2).unwrap();
        backend
            .memory
            .map_page(
                0x1000,
                1,
                memory::READ | memory::EXECUTE | memory::SEALED,
                &[0x90; PAGE_SIZE],
            )
            .unwrap();
        backend
            .memory
            .map_alias(0x2000, 0x1000, 1, memory::READ | memory::WRITE)
            .unwrap();
        backend.compile(0x1000, 1).unwrap();
        let id = backend.cache.selected_id();
        backend.state.eflags |= 0x100;
        assert!(backend.validate_unit(id, 0x1000).is_err());
        backend.state.eflags &= !0x100;
        backend.validate_unit(id, 0x1000).unwrap();
        // Simulate the generated-store dirty flag before finish drains its
        // journal. Admission must reject even while the stored stamp matches.
        unsafe {
            let table = backend.memory.page_table_ptr() as *mut memory::PageEntry;
            (*table.add(2)).dirty = 1;
        }
        assert!(backend.validate_unit(id, 0x1000).is_err());
        assert_eq!(backend.cache.pop_retired(), id);
    }

    #[test]
    fn host_operations_and_helper_failures_end_link_proofs_without_losing_diagnostics() {
        let handle = backend_new(1);
        REGISTRY.with_borrow_mut(|r| {
            backend_mut(r, handle)
                .unwrap()
                .memory
                .map_page(
                    0x1000,
                    1,
                    memory::READ | memory::EXECUTE | memory::SEALED,
                    &[0x90; PAGE_SIZE],
                )
                .unwrap();
        });
        assert_eq!(backend_compile(handle, 0x1000, 1), 0);
        let id = backend_cache_selected(handle);
        assert_eq!(
            backend_link_bind(handle, id, backend_link_slot(handle, id)),
            0
        );
        for operation in 0..6 {
            assert_eq!(backend_link_begin(handle, id, 0x1000), 0);
            assert_ne!(backend_link_resolve(handle, 0x1000), 0);
            assert_eq!(backend_link_memo_hits(handle), 1);
            match operation {
                0 => assert_eq!(backend_link_end(handle), 0),
                1 => assert_ne!(backend_resume(handle, 0x1000, 0, 0, 2), 0),
                2 => assert_ne!(backend_compile(handle, 0x1000, 0), 0),
                3 => assert_ne!(backend_apply_runtime_update(handle, 2, 0, 0, 0, 0), 0),
                4 => assert_ne!(backend_read_guest(handle, 0x1000, 0, 1), 0),
                5 => {
                    assert_eq!(
                        backend_x87(handle, u32::MAX, 0, 0, 0x1000, 0),
                        state::exit_kind::UNSUPPORTED
                    );
                    let diagnostic = REGISTRY.with_borrow(|r| r.error.clone());
                    assert!(!diagnostic.is_empty());
                    assert_eq!(backend_link_end(handle), 0);
                    REGISTRY.with_borrow(|r| assert_eq!(r.error, diagnostic));
                }
                _ => unreachable!(),
            }
            assert_ne!(backend_link_resolve(handle, 0x1000), 0);
            assert_eq!(backend_link_memo_hits(handle), 1);
        }
        assert_eq!(backend_drop(handle), 0);
    }

    #[test]
    fn linked_cpu_profile_admission_does_not_survive_outer_state_changes() {
        let handle = backend_new(1);
        REGISTRY.with_borrow_mut(|r| {
            backend_mut(r, handle)
                .unwrap()
                .memory
                .map_page(
                    0x1000,
                    1,
                    memory::READ | memory::EXECUTE | memory::SEALED,
                    &[0x90; PAGE_SIZE],
                )
                .unwrap();
        });
        assert_eq!(backend_compile(handle, 0x1000, 1), 0);
        let id = backend_cache_selected(handle);
        assert_eq!(
            backend_link_bind(handle, id, backend_link_slot(handle, id)),
            0
        );
        for change in 0..3 {
            assert_eq!(backend_link_begin(handle, id, 0x1000), 0);
            assert_ne!(backend_link_resolve(handle, 0x1000), 0);
            assert_eq!(backend_link_end(handle), 0);
            REGISTRY.with_borrow_mut(|r| {
                let b = backend_mut(r, handle).unwrap();
                match change {
                    0 => b.state.eflags |= 0x100,
                    1 => b.state.segments[0].base = 0x1000,
                    2 => b.state.segments[1].limit = 0x7fff_ffff,
                    _ => unreachable!(),
                }
            });
            assert_eq!(backend_link_resolve(handle, 0x1000), 0);
            assert_ne!(backend_link_begin(handle, id, 0x1000), 0);
            REGISTRY.with_borrow_mut(|r| {
                let b = backend_mut(r, handle).unwrap();
                b.state.eflags &= !0x100;
                b.state.segments[0].base = 0;
                b.state.segments[1].limit = u32::MAX;
            });
        }
        assert_eq!(backend_drop(handle), 0);
    }

    #[test]
    fn numeric_scratch_is_handle_owned_stable_and_never_part_of_guest_state() {
        let first = backend_new(3);
        let second = backend_new(1);
        let one = vine_x87::Ext80::ONE.words();
        let two = [0, 0x8000_0000, 0x4000];
        let add = |handle, value: [u32; 3]| {
            backend_fp_numeric(
                handle, 0, 0x37f, value[0], value[1], value[2], value[0], value[1], value[2],
            )
        };
        REGISTRY.with_borrow_mut(|r| {
            backend_mut(r, first)
                .unwrap()
                .memory
                .map_page(0x3000, 1, memory::READ | memory::WRITE, &[9; PAGE_SIZE])
                .unwrap();
        });
        let before = REGISTRY.with_borrow(|r| r.backends[&first].state.snapshot());
        let pointer = add(first, one);
        let other_pointer = add(second, two);
        assert_ne!(pointer, 0);
        assert_ne!(other_pointer, 0);
        assert_ne!(pointer, other_pointer);
        REGISTRY.with_borrow(|r| {
            assert_eq!(r.backends[&first].numeric_output.value, two);
            assert_eq!(
                r.backends[&second].numeric_output.value,
                [0, 0x8000_0000, 0x4001]
            );
            assert_eq!(r.backends[&first].state.snapshot(), before);
            assert_eq!(
                r.backends[&first]
                    .memory
                    .read(0x3000, 4, Access::Read)
                    .unwrap(),
                [9; 4]
            );
            assert!(!r.backends[&first].memory.has_pending_writes());
        });
        let output = REGISTRY.with_borrow(|r| *r.backends[&first].numeric_output);
        for (operation, control, a2, b2) in [
            (u32::MAX, 0x37f, one[2], one[2]),
            (0, 0x1_037f, one[2], one[2]),
            (0, 0x37f, 0x1_0000, one[2]),
            (0, 0x37f, one[2], 0x1_0000),
            (0, 0x17f, one[2], one[2]),
        ] {
            assert_eq!(
                backend_fp_numeric(
                    first, operation, control, one[0], one[1], a2, one[0], one[1], b2
                ),
                0
            );
            REGISTRY.with_borrow(|r| {
                assert_eq!(*r.backends[&first].numeric_output, output);
                assert_eq!(r.backends[&first].state.snapshot(), before);
                assert_eq!(
                    r.backends[&first]
                        .memory
                        .read(0x3000, 4, Access::Read)
                        .unwrap(),
                    [9; 4]
                );
            });
        }
        assert_eq!(backend_set_fp_ir(first, 1), 0);
        assert_eq!(backend_cache_invalidate(first), 0);
        assert_eq!(backend_initialize_state(first, 0x1000, 0x8000, 0x9000), 0);
        REGISTRY.with_borrow_mut(|r| {
            backend_mut(r, first)
                .unwrap()
                .memory
                .map_page(0x4000, 2, memory::READ, &[7; PAGE_SIZE])
                .unwrap();
        });
        assert_eq!(add(first, one), pointer);
        assert_eq!(backend_drop(first), 0);
        let replacement = backend_new(1);
        assert!(replacement > first && replacement > second);
        assert_eq!(add(first, one), 0);
        REGISTRY.with_borrow(|r| {
            assert_eq!(
                *r.backends[&replacement].numeric_output,
                fp_ir::NumericOutput::default()
            );
            assert_eq!(
                r.backends[&second].numeric_output.value,
                [0, 0x8000_0000, 0x4001]
            );
        });
        assert_ne!(add(replacement, one), 0);
        assert_eq!(backend_drop(second), 0);
        assert_eq!(backend_drop(replacement), 0);
    }

    #[test]
    fn numeric_helper_keeps_only_successful_invocation_proofs_and_mode_retires_code() {
        let handle = backend_new(1);
        REGISTRY.with_borrow_mut(|r| {
            backend_mut(r, handle)
                .unwrap()
                .memory
                .map_page(
                    0x1000,
                    1,
                    memory::READ | memory::EXECUTE | memory::SEALED,
                    &[0x90; PAGE_SIZE],
                )
                .unwrap();
        });
        assert_eq!(backend_compile(handle, 0x1000, 1), 0);
        let id = backend_cache_selected(handle);
        assert_eq!(
            backend_link_bind(handle, id, backend_link_slot(handle, id)),
            0
        );
        assert_eq!(backend_link_begin(handle, id, 0x1000), 0);
        assert_ne!(
            backend_fp_numeric(
                handle,
                0,
                0x37f,
                0,
                0x8000_0000,
                0x3fff,
                0,
                0x8000_0000,
                0x3fff
            ),
            0
        );
        assert_ne!(backend_link_resolve(handle, 0x1000), 0);
        assert_eq!(backend_link_memo_hits(handle), 1);
        assert_eq!(backend_fp_numeric(handle, 99, 0x37f, 0, 0, 0, 0, 0, 0), 0);
        assert_ne!(backend_link_resolve(handle, 0x1000), 0);
        assert_eq!(backend_link_memo_hits(handle), 1);
        assert_ne!(backend_set_fp_ir(handle, 2), 0);
        assert_eq!(backend_cache_selected(handle), id);
        assert_eq!(backend_set_fp_ir(handle, 1), 0);
        assert_eq!(backend_cache_retired(handle), id);
        assert_eq!(backend_cache_unit_count(handle), 0);
        assert_eq!(backend_drop(handle), 0);
    }

    #[test]
    fn capture_page_sources_respect_explicit_buffer_ownership() {
        let first = backend_new(1);
        let second = backend_new(1);
        let owned = backend_input_alloc(first, PAGE_SIZE as u32);
        let shared = backend_alloc(PAGE_SIZE as u32);
        assert_ne!(backend_map_page(second, 0x1000, 1, memory::READ, owned), 0);
        REGISTRY.with_borrow(|r| assert_eq!(r.backends[&second].memory.mapped_page_count(), 0));
        assert_eq!(backend_map_page(first, 0x1000, 1, memory::READ, owned), 0);
        assert_eq!(backend_map_page(second, 0x1000, 1, memory::READ, shared), 0);
        assert_eq!(backend_free(shared, PAGE_SIZE as u32), 0);
        assert_eq!(backend_drop(first), 0);
        assert_eq!(backend_drop(second), 0);
    }

    #[test]
    fn attached_runtime_cannot_be_mutated_through_capture_exports() {
        let handle = backend_new(2);
        let scratch = backend_alloc(PAGE_SIZE as u32);
        REGISTRY.with_borrow_mut(|r| {
            let b = backend_mut(r, handle).unwrap();
            let mut control = b"VINEDELT".to_vec();
            control.extend_from_slice(&(PAGE_SIZE as u32).to_le_bytes());
            control.extend_from_slice(&0u64.to_le_bytes());
            control.extend_from_slice(&1u64.to_le_bytes());
            for value in [
                1u32, 40, 1, 16, 7, 0, 1, 1, 8, 0, 0x1000, 0x1000, 0, 0, 0, 0, 0x1000, 1, 11, 0,
            ] {
                control.extend_from_slice(&value.to_le_bytes());
            }
            b.runtime_adapter
                .apply_update(&mut b.memory, true, &control, &[0; PAGE_SIZE])
                .unwrap();
            b.memory.write(0x1000, &[9]).unwrap();
        });
        assert_ne!(
            backend_map_page(
                handle,
                0x1000,
                1,
                memory::READ | memory::WRITE | memory::SEALED,
                scratch
            ),
            0
        );
        assert_ne!(backend_protect_page(handle, 0x1000, 1, memory::READ), 0);
        assert_ne!(
            backend_map_alias(handle, 0x2000, 0x1000, 1, memory::READ),
            0
        );
        REGISTRY.with_borrow(|r| {
            let b = &r.backends[&handle];
            assert_eq!(b.memory.read(0x1000, 1, Access::Read).unwrap(), [9]);
            assert!(b.memory.has_pending_writes());
            assert_eq!(b.memory.mapped_page_count(), 1);
        });
        assert_eq!(backend_free(scratch, PAGE_SIZE as u32), 0);
        assert_eq!(backend_drop(handle), 0);
    }

    #[test]
    fn unimplemented_execution_flags_fail_admission_without_mutation() {
        let mut b = Backend::new(1).unwrap();
        b.memory
            .map_page(
                0x1000,
                1,
                memory::READ | memory::EXECUTE | memory::SEALED,
                &[0x90; PAGE_SIZE],
            )
            .unwrap();
        b.compile(0x1000, 1).unwrap();
        for flag in [0x100, 0x10000, 0x20000, 0x40000] {
            b.state.eflags = flag | 2;
            let before = b.state.clone();
            assert!(b.validate(0x1000).is_err());
            assert_eq!(b.state, before);
        }
        b.state.eflags = 2;
        b.validate(0x1000).unwrap();
    }

    #[test]
    fn runtime_buffers_are_owned_and_failed_reads_preserve_destination() {
        let first = backend_new(2);
        let second = backend_new(2);
        let output = backend_input_alloc(first, 4);
        assert_ne!(output, 0);
        REGISTRY.with_borrow_mut(|r| {
            r.buffers.get_mut(&output).unwrap().fill(0x77);
            backend_mut(r, first)
                .unwrap()
                .memory
                .map_page(0x1000, 1, memory::READ, &[9; PAGE_SIZE])
                .unwrap();
        });
        assert_ne!(backend_read_guest(second, 0x1000, output, 4), 0);
        assert_ne!(backend_read_guest(first, 0x1ffe, output, 4), 0);
        REGISTRY.with_borrow(|r| assert_eq!(r.buffers[&output].as_ref(), &[0x77; 4]));
        assert_eq!(backend_read_guest(first, 0x1000, output, 4), 0);
        REGISTRY.with_borrow(|r| assert_eq!(r.buffers[&output].as_ref(), &[9; 4]));
        assert_eq!(backend_drop(first), 0);
        assert_ne!(backend_free(output, 4), 0);
        assert_eq!(backend_drop(second), 0);
    }

    #[test]
    fn creating_thread_context_does_not_change_active_cpu() {
        let handle = backend_new(1);
        assert_eq!(backend_initialize_state(handle, 0x1000, 0x8000, 0x9000), 0);
        let output = backend_input_alloc(handle, state::STATE_BYTES as u32);
        assert_eq!(
            backend_create_context(handle, 0x2000, 0xa000, 0xb000, output),
            0
        );
        assert_ne!(backend_resume(handle, 0x3000, 0xc000, 7, 2), 0);
        REGISTRY.with_borrow(|r| {
            assert_eq!(
                *r.backends[&handle].state,
                CpuState::thread(0x1000, 0x8000, 0x9000)
            );
            assert_eq!(
                r.buffers[&output].as_ref(),
                CpuState::thread(0x2000, 0xa000, 0xb000).snapshot()
            );
        });
        assert_eq!(backend_resume(handle, 0x1004, 0x8004, 7, 1), 0);
        REGISTRY.with_borrow(|r| {
            let state = &r.backends[&handle].state;
            assert_eq!(
                (
                    state.eip,
                    state.gpr[4],
                    state.gpr[0],
                    state.segments[4].base
                ),
                (0x1004, 0x8004, 7, 0x9000)
            );
        });
        assert_eq!(backend_drop(handle), 0);
    }
}
