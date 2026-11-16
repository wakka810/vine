//! Private tail-call CFG partitions. Static loop SCCs stay together; direct
//! edges transfer architectural locals without publishing CPU state. Guest
//! CALL/RET still use the ordinary checked guest stack operations.
use super::*;
use std::collections::BTreeSet;
use std::rc::Rc;

const PACK_INSTRUCTIONS: usize = 64;

#[derive(Clone)]
pub(super) struct Context {
    parameters: Vec<u32>,
    router: u32,
    exit: u32,
    targets: BTreeMap<u32, (u32, u32)>,
}

pub(super) struct Output {
    pub parameters: Vec<u8>,
    pub bodies: Vec<Vec<u8>>,
    pub instructions: Vec<usize>,
}

// Iterative Kosaraju avoids compiler stack growth with the public 8192-
// instruction discovery bound. CALL continuation edges are conservative; this
// partitioner makes no assumption about conventional function boundaries.
fn partitions(region: &Region) -> Vec<Vec<u32>> {
    let edges: BTreeMap<u32, Vec<u32>> = region
        .blocks
        .iter()
        .map(|(&pc, block)| {
            (
                pc,
                block
                    .successors
                    .iter()
                    .copied()
                    .filter(|next| region.blocks.contains_key(next))
                    .collect(),
            )
        })
        .collect();
    let mut reverse: BTreeMap<u32, Vec<u32>> = edges.keys().map(|&pc| (pc, Vec::new())).collect();
    for (&pc, next) in &edges {
        for target in next {
            reverse.get_mut(target).unwrap().push(pc);
        }
    }
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    for &root in edges.keys() {
        let mut work = vec![(root, false)];
        while let Some((pc, done)) = work.pop() {
            if done {
                order.push(pc);
            } else if seen.insert(pc) {
                work.push((pc, true));
                work.extend(edges[&pc].iter().rev().map(|&next| (next, false)));
            }
        }
    }
    seen.clear();
    let mut components = Vec::new();
    for root in order.into_iter().rev() {
        if seen.contains(&root) {
            continue;
        }
        let mut work = vec![root];
        let mut component = Vec::new();
        while let Some(pc) = work.pop() {
            if seen.insert(pc) {
                component.push(pc);
                work.extend(&reverse[&pc]);
            }
        }
        component.sort_unstable();
        components.push(component);
    }
    components.sort_by_key(|component| component[0]);
    let mut result: Vec<Vec<u32>> = Vec::new();
    let mut count = PACK_INSTRUCTIONS;
    for component in components {
        let size: usize = component
            .iter()
            .map(|pc| region.blocks[pc].instructions.len())
            .sum();
        if result.is_empty() || count + size > PACK_INSTRUCTIONS {
            result.push(Vec::new());
            count = 0;
        }
        result.last_mut().unwrap().extend(component);
        count += size;
    }
    for group in &mut result {
        group.sort_unstable();
    }
    result
}

// Consume the local declaration prefix, not subsequent entry instructions.
// The core can already contain its effect-free native-preparation guard.
// Keeping types here avoids duplicating optional SIMD/FP/scratch layout rules.
fn local_types(prefix: &[u8]) -> Vec<u8> {
    fn number(bytes: &[u8], cursor: &mut usize) -> u32 {
        let mut value = 0;
        for shift in (0..35).step_by(7) {
            let byte = bytes[*cursor];
            *cursor += 1;
            value |= u32::from(byte & 127) << shift;
            if byte & 128 == 0 {
                return value;
            }
        }
        unreachable!("encoder local count");
    }
    let mut cursor = 0;
    let mut types = vec![0x7f; 18];
    for _ in 0..number(prefix, &mut cursor) {
        let count = number(prefix, &mut cursor);
        let ty = prefix[cursor];
        cursor += 1;
        types.extend(std::iter::repeat_n(ty, count as usize));
    }
    types
}

fn declarations(types: &[u8]) -> Vec<u8> {
    let mut groups: Vec<(u32, u8)> = Vec::new();
    for &ty in types {
        if let Some((count, previous)) = groups.last_mut()
            && *previous == ty
        {
            *count += 1;
        } else {
            groups.push((1, ty));
        }
    }
    let mut result = Vec::new();
    uleb(&mut result, groups.len() as u32);
    for (count, ty) in groups {
        uleb(&mut result, count);
        result.push(ty);
    }
    result
}

impl Emitter {
    fn private_tail(&mut self, function: u32, route: u32) {
        let context = self
            .function_cfg
            .as_ref()
            .expect("private ABI context")
            .clone();
        for local in 0..=13 {
            self.get(local);
        }
        self.get(DEFINED);
        self.get(self.link_enabled);
        // Private parameter16 carries a local entry index, or MAX for an
        // indirect entry. The public ABI still checks the immutable unit ID.
        self.num(route);
        self.get(self.rep_fuel);
        for &local in &context.parameters {
            self.get(local);
        }
        self.op(0x12); // return_call: no cumulative host stack on guest cycles
        uleb(&mut self.code, function);
    }

    fn private_prologue(&mut self) {
        self.get(14);
        self.set(DEFINED);
        self.get(15);
        self.set(self.link_enabled);
        self.get(17);
        self.set(self.rep_fuel);
    }

    pub(super) fn private_exit(&mut self) -> bool {
        if let Some(context) = &self.function_cfg {
            self.private_tail(context.exit, u32::MAX);
            true
        } else {
            false
        }
    }

    pub(super) fn private_jump(&mut self, target: u32) -> bool {
        if let Some((function, route)) = self
            .function_cfg
            .as_ref()
            .and_then(|c| c.targets.get(&target))
            .copied()
        {
            self.private_tail(function, route);
            true
        } else {
            false
        }
    }

    fn private_lookup(&mut self, entries: &[(u32, usize)], miss: Option<u32>) {
        self.num(u32::MAX);
        self.set(ROUTE);
        self.lookup_tree(entries);
        self.get(ROUTE);
        self.num(u32::MAX);
        self.op(0x46);
        self.if_();
        if let Some(function) = miss {
            self.private_tail(function, u32::MAX);
        } else {
            self.exit(2);
        }
        self.end();
    }

    fn private_table(&mut self, count: usize, default: u32) {
        for index in (0..count).rev() {
            self.open(0x02, format!("b{index}"));
        }
        self.get(ROUTE);
        self.op(0x0e);
        uleb(&mut self.code, count as u32);
        for index in 0..count {
            uleb(&mut self.code, index as u32);
        }
        uleb(&mut self.code, default);
    }
}

pub(super) fn emit(
    e: &mut Emitter,
    region: &Region,
    first: u32,
    has_sse: bool,
    has_scas: bool,
    has_timestamp: bool,
    numeric_imports: u32,
) -> Output {
    let types = local_types(&e.code);
    let mut persistent = vec![KIND, FAULT, ERROR, SMC];
    if e.has_mmx {
        persistent.extend(30..=40);
    }
    if has_sse {
        persistent.extend(43..=50);
    }
    if !e.fp_plan.is_empty() {
        persistent.extend(e.fp_base..e.fp_base + 8);
    }
    let mut parameters = vec![0x7f; 18];
    parameters.extend(persistent.iter().map(|&local| types[local as usize]));
    let mut map: Vec<u32> = (0..types.len() as u32).collect();
    let mut locals = Vec::new();
    for logical in 18..types.len() {
        map[logical] = if let Some(index) = persistent.iter().position(|&v| v as usize == logical) {
            18 + index as u32
        } else {
            let index = parameters.len() + locals.len();
            locals.push(types[logical]);
            index as u32
        };
    }
    let groups = partitions(region);
    let context = Rc::new(Context {
        parameters: persistent,
        router: first,
        exit: first + 1 + groups.len() as u32,
        targets: groups
            .iter()
            .enumerate()
            .flat_map(|(index, group)| {
                group
                    .iter()
                    .enumerate()
                    .map(move |(route, &pc)| (pc, (first + 1 + index as u32, route as u32)))
            })
            .collect(),
    });
    let mut template = e.clone();
    template.code = declarations(&locals);
    template.local_map = Some(Rc::new(map));
    template.function_cfg = Some(context.clone());
    template.private_prologue();
    template.get(16);
    template.set(ROUTE);

    // Public entry initializes banks exactly once. Private functions are not
    // exported and are reachable only after this immutable binding check.
    e.function_cfg = Some(context.clone());
    e.sample_location(3, region.entry);
    e.private_prologue();
    if e.has_mmx {
        e.initialize_mmx();
    }
    if has_sse {
        e.initialize_sse();
    }
    e.fp_initialize();
    e.get(16);
    e.unit_identity();
    e.op(0x47);
    e.if_();
    e.num(1);
    e.set(ERROR);
    e.exit(3);
    e.end();
    e.private_tail(context.router, u32::MAX);
    e.op(0x0b);

    let mut router = template.clone();
    router.sample_location(4, 0);
    let entries: Vec<_> = context
        .targets
        .iter()
        .map(|(&pc, &(function, _))| (pc, (function - first - 1) as usize))
        .collect();
    router.private_lookup(&entries, None);
    router.open(0x02, "invalid".into());
    router.private_table(groups.len(), groups.len() as u32);
    for index in 0..groups.len() {
        router.end();
        router.private_tail(first + 1 + index as u32, u32::MAX);
    }
    router.end();
    router.exit(3);
    router.op(0x0b);
    let mut bodies = vec![router.code];
    for group in &groups {
        let mut part = template.clone();
        part.blocks = group
            .iter()
            .enumerate()
            .map(|(index, &pc)| (pc, index))
            .collect();
        part.open(0x02, "invalid".into());
        part.open(0x03, "lookup".into());
        let entries: Vec<_> = part
            .blocks
            .iter()
            .map(|(&pc, &index)| (pc, index))
            .collect();
        // Known edges carry the dense entry index. Only arbitrary/indirect
        // entries search PCs; local backedges remain ordinary branches.
        part.get(ROUTE);
        part.num(u32::MAX);
        part.op(0x46);
        part.if_();
        part.private_lookup(&entries, Some(context.router));
        part.end();
        part.open(0x03, "dispatch".into());
        part.private_table(group.len(), group.len() as u32 + 2);
        for pc in group {
            part.end();
            part.block_body(&region.blocks[pc]);
        }
        part.end();
        part.end();
        part.end();
        part.exit(3);
        part.op(0x0b);
        bodies.push(part.code);
    }
    let mut exit = template;
    exit.function_cfg = None;
    exit.finish(e.has_mmx, has_sse, has_scas, has_timestamp, numeric_imports);
    bodies.push(exit.code);
    Output {
        parameters,
        bodies,
        instructions: groups
            .iter()
            .map(|group| {
                group
                    .iter()
                    .map(|pc| region.blocks[pc].instructions.len())
                    .sum()
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_partition_accepts_a_core_with_native_preparation_entry() {
        use crate::memory::{EXECUTE, MirrorMemory, PAGE_SIZE, READ, SEALED};
        let mut memory = MirrorMemory::new(1).unwrap();
        let mut bytes = [0xcc; PAGE_SIZE];
        let code = [0xff, 0x03, 0x83, 0xc0, 1, 0x8b, 0x16, 0x0f, 0x0b];
        bytes[..code.len()].copy_from_slice(&code);
        memory
            .map_page(0x1000, 1, READ | EXECUTE | SEALED, &bytes)
            .unwrap();
        let region = crate::decode::discover(&memory, 0x1000, 16);
        // Public compilation must not panic when setup instructions follow
        // the declarations; the generated execution is checked in Chrome.
        assert!(
            super::super::compile_with_identity(
                &region,
                CompileOptions {
                    function_cfg: true,
                    fp_ir: true,
                    typed_numeric: true,
                    ..CompileOptions::default()
                },
                17
            )
            .is_ok()
        );
    }

    #[test]
    fn partitions_preserve_cycles_and_arbitrary_entries() {
        let instruction = Instruction {
            eip: 0,
            len: 1,
            width: Width::W32,
            flags_undefined: 0,
            unsupported: None,
            opcode: Opcode::Nop,
            operands: Vec::new(),
            condition: None,
            flags_read: 0,
            flags_written: 0,
        };
        let mut blocks = BTreeMap::new();
        for pc in 0..6 {
            blocks.insert(
                pc,
                Block {
                    start: pc,
                    instructions: vec![instruction.clone(); 40],
                    successors: match pc {
                        0 => vec![1],
                        1 => vec![2],
                        2 => vec![1, 3],
                        3 => vec![4],
                        4 => vec![5],
                        _ => vec![4, 99],
                    },
                },
            );
        }
        let groups = partitions(&Region {
            entry: 0,
            blocks,
            limit_reached: false,
        });
        assert_eq!(groups, vec![vec![0], vec![1, 2], vec![3], vec![4, 5]]);
    }
}
