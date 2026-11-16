//! Optional host observation boundaries for exact execution captures.
//! This owns no guest state and never substitutes bytes in GuestMemory.
use crate::{Backend, REGISTRY, backend_mut, decode::CodeSource, memory::MirrorMemory, result};

pub(crate) struct CodeView<'a> {
    pub memory: &'a MirrorMemory,
    pub stop: Option<u32>,
}

impl CodeSource for CodeView<'_> {
    fn read_data(&self, address: u32, maximum: usize) -> Vec<u8> {
        self.memory.read_data(address, maximum)
    }
    fn read_code(&self, address: u32, maximum: usize) -> Vec<u8> {
        self.memory.read_code(address, maximum)
    }

    fn diagnostic_stop(&self, address: u32) -> bool {
        self.stop == Some(address)
    }
}

impl Backend {
    pub(crate) fn validate_diagnostic_mode(&self) -> Result<(), String> {
        if (self.diagnostic_stop.is_some() || self.sampled_pc)
            && (self.predicate_ir
                || self.flow_ir
                || self.value_ir
                || self.rooted_ssa
                || self.structured_loops
                || self.affine_memory
                || self.loop_memory)
        {
            return Err("CPU diagnostics require ordinary control-flow lowering".into());
        }
        Ok(())
    }

    fn set_sampled_pc(&mut self, enabled: u32) -> Result<(), String> {
        if enabled > 1 {
            return Err("invalid PC sampling option".into());
        }
        let next = enabled != 0;
        if next == self.sampled_pc {
            return Ok(());
        }
        if self.pending_timestamp.is_some() {
            return Err("complete the pending timestamp before changing PC sampling".into());
        }
        if next
            && (self.predicate_ir
                || self.flow_ir
                || self.value_ir
                || self.rooted_ssa
                || self.structured_loops
                || self.affine_memory
                || self.loop_memory)
        {
            return Err("PC sampling requires ordinary control-flow lowering".into());
        }
        self.cache.clear();
        self.sampled_pc = next;
        Ok(())
    }

    fn set_diagnostic_stop(&mut self, enabled: u32, eip: u32) -> Result<(), String> {
        if enabled > 1 {
            return Err("invalid diagnostic stop option".into());
        }
        let next = (enabled != 0).then_some(eip);
        if next == self.diagnostic_stop {
            return Ok(());
        }
        if self.pending_timestamp.is_some() {
            return Err("complete the pending timestamp before changing diagnostic stops".into());
        }
        if next.is_some()
            && (self.predicate_ir
                || self.flow_ir
                || self.value_ir
                || self.rooted_ssa
                || self.structured_loops
                || self.affine_memory
                || self.loop_memory)
        {
            return Err("diagnostic stops require ordinary control-flow lowering".into());
        }
        // Old ordinary or diagnostic bodies, including bound links, cannot
        // bypass a new boundary or keep an obsolete one after disarming.
        self.cache.clear();
        self.diagnostic_stop = next;
        Ok(())
    }
}

/// Arm one host diagnostic PC, or clear it with enabled=0 (eip ignored).
/// The consumer must drain retired instances and perform normal admission.
/// A hit returns exit17 before fetch/operand access/retirement, after earlier
/// completed effects. Zero fuel still yields BUDGET first. Clearing or moving
/// the boundary resumes the same CPU snapshot; never apply a Runtime resume
/// merely to acknowledge this host observation. New handles start unarmed.
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_diagnostic_stop(handle: u32, enabled: u32, eip: u32) -> u32 {
    REGISTRY.with_borrow_mut(|registry| {
        let value = backend_mut(registry, handle)
            .and_then(|backend| backend.set_diagnostic_stop(enabled, eip));
        result(registry, value)
    })
}

/// Diagnostic-only active block/phase observation. Generated modules import
/// env.sampled_pc as memory1 (shared, min=max=1 page) and atomically store one
/// u64 at byte0: high32=phase, low32=guest block entry for phase1 or unit root
/// for phase3. Other phases carry PC0: 2=dispatch, 4=PC lookup, and
/// 5=exit publication plus link resolution/transfer.
/// Phase1 includes helpers and intra-block management; it is not an exclusive
/// instruction timer. The host must clear/set its phase around CPU invocations.
/// No observer value is read by guest code. Disabled modules are unchanged.
#[unsafe(no_mangle)]
pub extern "C" fn backend_set_sampled_pc(handle: u32, enabled: u32) -> u32 {
    REGISTRY.with_borrow_mut(|registry| {
        let value =
            backend_mut(registry, handle).and_then(|backend| backend.set_sampled_pc(enabled));
        result(registry, value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        decode,
        ir::Opcode,
        memory::{EXECUTE, PAGE_SIZE, READ, SEALED},
        state::{ExitState, exit_kind},
    };

    fn backend(bytes: &[u8]) -> Backend {
        let mut backend = Backend::new(2).unwrap();
        let mut page = [0xcc; PAGE_SIZE];
        page[..bytes.len()].copy_from_slice(bytes);
        backend
            .memory
            .map_page(0x1000, 1, READ | EXECUTE | SEALED, &page)
            .unwrap();
        backend
    }

    #[test]
    fn sampling_mode_retires_bindings_without_touching_guest_state() {
        let mut backend = backend(&[0x40, 0xcc]);
        backend.compile(0x1000, 8).unwrap();
        let old = backend.cache.selected_id();
        let state = backend.state.clone();
        let bytes = backend.memory.read_code(0x1000, 15);
        assert!(backend.set_sampled_pc(2).is_err());
        assert_eq!(backend.cache.selected_id(), old);
        backend.set_sampled_pc(1).unwrap();
        assert!(backend.cache.is_empty());
        assert_eq!(backend.cache.pop_retired(), old);
        backend.compile(0x1000, 8).unwrap();
        let sampled = backend.cache.selected_id();
        backend.set_sampled_pc(1).unwrap();
        assert_eq!(backend.cache.selected_id(), sampled);
        backend.set_sampled_pc(0).unwrap();
        assert_eq!(backend.cache.pop_retired(), sampled);
        assert_eq!(backend.state, state);
        assert_eq!(backend.memory.read_code(0x1000, 15), bytes);
        backend.rooted_ssa = true;
        assert!(backend.set_sampled_pc(1).is_err());
        assert!(!backend.sampled_pc);
    }

    #[test]
    fn discovery_stops_before_fetch_and_makes_an_interior_pc_a_leader() {
        let backend = backend(&[0x40, 0x90, 0x43]);
        let source = CodeView {
            memory: &backend.memory,
            stop: Some(0x1001),
        };
        let region = decode::discover(&source, 0x1000, 16);
        assert_eq!(region.blocks[&0x1000].instructions.len(), 1);
        let stop = &region.blocks[&0x1001];
        assert_eq!(stop.instructions.len(), 1);
        assert_eq!(stop.instructions[0].opcode, Opcode::DiagnosticStop);
        assert_eq!(stop.instructions[0].len, 0);
        assert!(stop.successors.is_empty());
        let absent = decode::discover(
            &CodeView {
                memory: &backend.memory,
                stop: Some(0x2000),
            },
            0x2000,
            1,
        );
        assert_eq!(
            absent.blocks[&0x2000].instructions[0].opcode,
            Opcode::DiagnosticStop
        );
        // An address within an instruction is not an executed PC boundary.
        let other = backend_with_stop_inside_instruction();
        assert!(
            other
                .blocks
                .values()
                .flat_map(|b| &b.instructions)
                .all(|i| i.opcode != Opcode::DiagnosticStop)
        );
    }

    fn backend_with_stop_inside_instruction() -> crate::ir::Region {
        let backend = backend(&[0xb8, 1, 2, 3, 4, 0xcc]);
        decode::discover(
            &CodeView {
                memory: &backend.memory,
                stop: Some(0x1002),
            },
            0x1000,
            16,
        )
    }

    #[test]
    fn boundary_changes_retire_units_without_changing_cpu_or_memory() {
        let mut backend = backend(&[0x90, 0x90, 0xcc]);
        backend.compile(0x1000, 8).unwrap();
        let old = backend.cache.selected_id();
        let cpu = backend.state.clone();
        let memory = backend.memory.read_code(0x1000, 15);
        backend.set_diagnostic_stop(1, 0x1001).unwrap();
        assert!(backend.cache.is_empty());
        assert_eq!(backend.cache.pop_retired(), old);
        assert_eq!(backend.state, cpu);
        assert_eq!(backend.memory.read_code(0x1000, 15), memory);
        backend.compile(0x1000, 8).unwrap();
        let observed = backend.cache.selected_id();
        backend.set_diagnostic_stop(1, 0x1001).unwrap();
        assert_eq!(backend.cache.selected_id(), observed);
        backend.set_diagnostic_stop(0, 0xdeadbeef).unwrap();
        assert_eq!(backend.cache.pop_retired(), observed);
        assert!(backend.cache.is_empty());
        assert_eq!(backend.state, cpu);
        backend.value_ir = true;
        assert!(backend.set_diagnostic_stop(1, 0x1001).is_err());
        assert_eq!(backend.diagnostic_stop, None);
    }

    #[test]
    fn pending_timestamp_rejects_boundary_change_without_losing_ticket() {
        let mut backend = backend(&[0x0f, 0x31]);
        backend.compile(0x1000, 1).unwrap();
        backend.state.eip = 0x1000;
        backend.state.exit = ExitState {
            kind: exit_kind::TIMESTAMP_COUNTER,
            fault_eip: 0x1000,
            error: 2,
            ..Default::default()
        };
        let id = backend.cache.selected_id();
        backend.begin_timestamp(id).unwrap();
        let cpu = backend.state.clone();
        assert!(backend.set_diagnostic_stop(1, 0x1002).is_err());
        assert_eq!(backend.state, cpu);
        assert_eq!(backend.cache.selected_id(), id);
        backend.complete_timestamp(0x1000, 1, 2).unwrap();
        backend.set_diagnostic_stop(1, 0x1002).unwrap();
    }
}
