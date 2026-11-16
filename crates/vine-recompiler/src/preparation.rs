//! Bounded pre-execution discovery. Roots and successors are compiler hints,
//! never permission to execute or to replace guest CALL/RET/indirect effects.
use crate::decode::CodeSource;
use crate::{Backend, REGISTRY, backend_mut, result};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
mod layout;

const MAX_ROOTS: usize = 4096;
const MAX_DISCOVERED: usize = 32768;
const READY: u32 = 1;
const COMPLETE: u32 = 2;
const LIMITED: u32 = 3;

pub(super) struct Preparation {
    pending: VecDeque<u32>,
    seen: BTreeSet<u32>,
    limited: bool,
    status: u32,
    planned: Option<PlannedBatch>,
}

struct PlannedBatch {
    units: VecDeque<crate::control_ir::Graph>,
    dependencies: BTreeMap<u32, Option<u64>>,
    roots: Vec<u32>,
    limit: usize,
    stop: Option<u32>,
    control_graph: bool,
}

pub(super) struct CodeView<'a> {
    pub source: crate::diagnostic::CodeView<'a>,
    // Prepared residency outlives the preparation session. Demand discovery
    // shares those same bodies through checked links instead of cloning them.
    pub prepared: &'a crate::code_cache::CodeCache,
    pub preparation: Option<&'a Preparation>,
}
impl CodeSource for CodeView<'_> {
    fn read_code(&self, address: u32, maximum: usize) -> Vec<u8> {
        self.source.read_code(address, maximum)
    }
    fn read_data(&self, address: u32, maximum: usize) -> Vec<u8> {
        self.source.read_data(address, maximum)
    }
    fn diagnostic_stop(&self, address: u32) -> bool {
        self.source.diagnostic_stop(address)
    }
    fn entry_hint(&self, address: u32) -> bool {
        self.preparation.is_some_and(|p| p.seen.contains(&address))
    }
    fn external_entry(&self, address: u32) -> bool {
        self.prepared
            .has_prepared_entry(address, self.source.memory)
    }
}

impl Backend {
    fn uses_preparation_layout(&self) -> bool {
        // Placement is part of lowering. Ordinary code can retain cheap
        // internal branch/return edges; SSA and structured emitters build
        // different entry frames and keep their own discovery granularity.
        !(self.function_cfg
            || self.loop_memory
            || self.structured_loops
            || self.affine_memory
            || self.value_ir
            || self.predicate_ir
            || self.flow_ir
            || self.rooted_ssa)
    }

    fn begin_preparation(&mut self, roots: &[u32]) -> Result<(), String> {
        if roots.is_empty() || roots.len() > MAX_ROOTS {
            return Err("preparation roots outside 1..4096".into());
        }
        if self.preparation.is_some() {
            return Err("code preparation is already active".into());
        }
        self.cache.enable_preparation();
        let mut seen = BTreeSet::new();
        let pending = roots
            .iter()
            .copied()
            .filter(|root| seen.insert(*root))
            .collect();
        self.preparation = Some(Preparation {
            pending,
            seen,
            limited: false,
            status: READY,
            planned: None,
        });
        Ok(())
    }

    fn prepare_next(&mut self, limit: usize) -> Result<(), String> {
        if !(1..=8192).contains(&limit) {
            return Err("preparation unit bound outside 1..8192".into());
        }
        self.validate_diagnostic_mode()?;
        let preparation = self
            .preparation
            .as_mut()
            .ok_or("code preparation is not active")?;
        if preparation.status != READY {
            return Ok(());
        }
        if preparation.limited {
            preparation.status = LIMITED;
            return Ok(());
        }
        self.cache.reconcile(&mut self.memory);
        let stale =
            self.preparation
                .as_ref()
                .and_then(|p| p.planned.as_ref())
                .is_some_and(|plan| {
                    !self.uses_preparation_layout()
                        || plan.limit != limit
                        || plan.stop != self.diagnostic_stop
                        || plan.control_graph != self.control_graph
                        || !plan.dependencies.iter().all(|(&page, &stamp)| {
                            self.memory.mapping_stamp_is_current(page, stamp)
                        })
                });
        if stale {
            let preparation = self.preparation.as_mut().expect("active");
            let plan = preparation.planned.take().expect("stale batch");
            // This is a real browser handoff: source mappings, bytes, aliases
            // and diagnostic boundaries may have changed since discovery.
            for root in plan.roots.into_iter().rev() {
                preparation.pending.push_front(root);
            }
        }
        if self.preparation.as_ref().expect("active").planned.is_some() {
            if self.materialize_planned()? {
                self.publish_frontier();
            }
            return Ok(());
        }
        let preparation = self.preparation.as_mut().expect("active");
        let Some(entry) = preparation.pending.pop_front() else {
            preparation.status = COMPLETE;
            return Ok(());
        };
        // Select validates dependency stamps even when an asynchronous browser
        // instantiation/yield allowed a memory change since the preceding step.
        if self.cache.select(entry, &self.memory).is_none() {
            if !self.cache.fits_prepared(0) {
                self.preparation.as_mut().expect("active").status = LIMITED;
                return Ok(());
            }
            if !self.uses_preparation_layout() {
                let (id, unit) = self.compile_unit(&[entry], limit, false)?;
                if !self.cache.fits_prepared(unit.code.len()) {
                    self.preparation.as_mut().expect("active").status = LIMITED;
                    return Ok(());
                }
                self.cache.insert_prepared(id, unit)?;
            } else {
                self.plan_batch(entry, limit);
                if !self.materialize_planned()? {
                    return Ok(());
                }
            }
        } else if !self.cache.retain_selected() {
            self.preparation.as_mut().expect("active").status = LIMITED;
            return Ok(());
        }
        self.publish_frontier();
        Ok(())
    }

    fn plan_batch(&mut self, entry: u32, limit: usize) {
        // One bounded IR window, then the existing native-unit allowance. This
        // changes placement, not the resident-code or execution-unit limit.
        let discovery_limit = (limit * 16).min(8192);
        let maximum_roots = discovery_limit.min(256);
        let mut roots = vec![entry];
        let preparation = self.preparation.as_mut().expect("active");
        for _ in 0..preparation.pending.len() {
            if roots.len() == maximum_roots {
                break;
            }
            let root = preparation.pending.pop_front().expect("bounded queue");
            if self.cache.has_current_entry(root, &self.memory) {
                preparation.pending.push_back(root);
            } else {
                roots.push(root);
            }
        }
        let source = CodeView {
            source: crate::diagnostic::CodeView {
                memory: &self.memory,
                stop: self.diagnostic_stop,
            },
            prepared: &self.cache,
            preparation: self.preparation.as_ref(),
        };
        let graph = if self.control_graph {
            crate::control_ir::discover(&source, &roots, discovery_limit)
        } else {
            crate::control_ir::Graph::new(
                crate::decode::discover_many(&source, &roots, discovery_limit),
                BTreeMap::new(),
            )
        };
        let dependencies = self.code_dependencies(&graph.region);
        self.preparation.as_mut().expect("active").planned = Some(PlannedBatch {
            units: layout::partition(graph, limit).into(),
            dependencies,
            roots,
            limit,
            stop: self.diagnostic_stop,
            control_graph: self.control_graph,
        });
    }

    fn materialize_planned(&mut self) -> Result<bool, String> {
        if !self.cache.fits_prepared(0) {
            self.preparation.as_mut().expect("active").status = LIMITED;
            return Ok(false);
        }
        let plan = self
            .preparation
            .as_ref()
            .expect("active")
            .planned
            .as_ref()
            .expect("planned batch");
        if !plan
            .dependencies
            .iter()
            .all(|(&page, &stamp)| self.memory.mapping_stamp_is_current(page, stamp))
        {
            return Err("preparation code has pending writes or changed mappings".into());
        }
        let graph = plan
            .units
            .front()
            .ok_or("empty preparation layout")?
            .clone();
        let (id, unit) = self.compile_graph_unit(&graph)?;
        if !self.cache.fits_prepared(unit.code.len()) {
            self.preparation.as_mut().expect("active").status = LIMITED;
            return Ok(false);
        }
        self.cache.insert_prepared(id, unit)?;
        let preparation = self.preparation.as_mut().expect("active");
        let plan = preparation.planned.as_mut().expect("planned batch");
        plan.units.pop_front();
        if plan.units.is_empty() {
            preparation.planned = None;
        }
        Ok(true)
    }

    fn publish_frontier(&mut self) {
        let unit = self.cache.selected().expect("selected preparation unit");
        let preparation = self.preparation.as_mut().expect("active");
        for &target in &unit.preparation_frontier {
            if preparation.seen.contains(&target) {
                continue;
            }
            if preparation.seen.len() == MAX_DISCOVERED {
                preparation.limited = true;
                break;
            }
            preparation.seen.insert(target);
            preparation.pending.push_back(target);
        }
    }
}

/// Root bytes belong to this backend handle. No guest CPU state is consumed.
#[unsafe(no_mangle)]
pub extern "C" fn backend_prepare_begin(handle: u32, ptr: usize, count: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = (|| {
            if count == 0
                || count as usize > MAX_ROOTS
                || r.buffer_owners.get(&ptr) != Some(&handle)
            {
                return Err("invalid owned preparation roots".into());
            }
            let bytes = r
                .buffers
                .get(&ptr)
                .filter(|b| b.len() == count as usize * 4)
                .ok_or("invalid preparation roots buffer extent")?;
            let roots: Vec<_> = bytes
                .chunks_exact(4)
                .map(|b| u32::from_le_bytes(b.try_into().expect("four root bytes")))
                .collect();
            backend_mut(r, handle)?.begin_preparation(&roots)
        })();
        result(r, value)
    })
}

/// Materialize at most one 512-instruction unit from a bounded shared IR batch.
/// READY selects a unit for instantiation; COMPLETE/LIMITED execute no code.
#[unsafe(no_mangle)]
pub extern "C" fn backend_prepare_next(handle: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = backend_mut(r, handle).and_then(|b| b.prepare_next(512));
        result(r, value)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn backend_prepare_status(handle: u32) -> u32 {
    REGISTRY.with_borrow(|r| {
        r.backends
            .get(&handle)
            .and_then(|b| b.preparation.as_ref())
            .map_or(0, |p| p.status)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn backend_prepare_end(handle: u32) -> u32 {
    REGISTRY.with_borrow_mut(|r| {
        let value = backend_mut(r, handle).map(|b| b.preparation = None);
        result(r, value)
    })
}

#[cfg(test)]
mod tests;
