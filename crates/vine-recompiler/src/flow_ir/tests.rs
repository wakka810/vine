use super::*;
use crate::decode::{CodeSource, discover};
struct Code(BTreeMap<u32, u8>);
impl CodeSource for Code {
    fn read_code(&self, address: u32, max: usize) -> Vec<u8> {
        (0..max)
            .map_while(|i| self.0.get(&address.wrapping_add(i as u32)).copied())
            .collect()
    }
}
fn graph(parts: &[(u32, &[u8])]) -> (v::Graph, Graph) {
    let code = Code(
        parts
            .iter()
            .flat_map(|(start, bytes)| {
                bytes
                    .iter()
                    .enumerate()
                    .map(move |(i, b)| (start + i as u32, *b))
            })
            .collect(),
    );
    let region = discover(&code, 0x1000, 128);
    let mut source = v::lift(&region).unwrap();
    v::optimize(&mut source).unwrap();
    let graph = Graph::build(&source).unwrap();
    (source, graph)
}
#[test]
fn loops_remove_unchanged_cyclic_phis_without_constraining_arbitrary_entries() {
    let (source, g) = graph(&[
        (0x1000, &[0x40, 0xeb, 0x0d]),
        (0x1010, &[0x49, 0x75, 0xed, 0x0f, 0x0b]),
    ]);
    assert!(g.stats.phis_removed > 0);
    let root_edx = g.parameters[ROOT.0][2];
    for b in &source.blocks {
        let n = NodeId::block(b.id);
        assert!(g.predecessors[n.0].contains(&INGRESS));
        assert_eq!(g.resolve(g.parameters[n.0][2]), root_edx);
    }
    let entry = NodeId::block(source.entry);
    let ecx = g.resolve(g.parameters[entry.0][1]);
    assert!(matches!(g.values[ecx.0].op, Op::Phi { .. }));
    let Op::Phi { incoming } = &g.values[ecx.0].op else {
        unreachable!()
    };
    assert!(incoming.iter().any(|(n, _)| *n == INGRESS));
    assert!(incoming.iter().any(|(n, _)| n.0 >= 2));
}
#[test]
fn call_ret_and_arbitrary_continuation_use_real_stack_effects_and_ingress() {
    let (source, g) = graph(&[(0x1000, &[0xe8, 3, 0, 0, 0, 0x40, 0x0f, 0x0b, 0xc3])]);
    let continuation = NodeId::block(source.entries[&0x1005]);
    let callee = NodeId::block(source.entries[&0x1008]);
    assert!(g.predecessors[continuation.0].contains(&INGRESS));
    assert!(g.successors[callee.0].contains(&INGRESS));
    assert!(
        source
            .effects
            .iter()
            .any(|e| matches!(e.kind, v::EffectKind::Store { .. }))
    );
    assert!(
        source
            .effects
            .iter()
            .any(|e| matches!(e.kind, v::EffectKind::Load { .. }))
    );
    assert!(
        g.values
            .iter()
            .any(|v| matches!(v.op, Op::EffectResult { .. }))
    );
}
#[test]
fn malformed_alias_and_non_dominating_phi_input_are_rejected() {
    let (source, mut g) = graph(&[
        (0x1000, &[0x40, 0xeb, 0x0d]),
        (0x1010, &[0x49, 0x75, 0xed, 0x0f, 0x0b]),
    ]);
    let original = g.clone();
    g.values[0].op = Op::Alias(ValueId(0));
    assert!(g.verify(&source).is_err());
    g = original;
    let phi = g.resolve(g.parameters[NodeId::block(source.entry).0][0]);
    let Op::Phi { incoming } = &mut g.values[phi.0].op else {
        panic!("loop accumulator phi");
    };
    incoming[0].0 = ROOT;
    assert!(g.verify(&source).is_err());
    let (_, mut g) = graph(&[
        (0x1000, &[0x40, 0xeb, 0x0d]),
        (0x1010, &[0x49, 0x75, 0xed, 0x0f, 0x0b]),
    ]);
    let last = ValueId(g.values.len() - 1);
    g.values[last.0].op = Op::Alias(last);
    let phi = g.resolve(g.parameters[NodeId::block(source.entry).0][0]);
    let Op::Phi { incoming } = &mut g.values[phi.0].op else {
        unreachable!()
    };
    incoming[0].1 = last;
    assert!(g.verify(&source).is_err());
}
#[test]
fn iterative_scc_handles_deep_guest_graph_and_preserves_condensation_order() {
    let mut edges: Vec<_> = (0..8192)
        .map(|i| if i < 8191 { vec![i + 1] } else { vec![] })
        .collect();
    edges[8191].push(4096);
    let result = components(&edges);
    assert_eq!(result.len(), 4097);
    assert_eq!(result[0], vec![0]);
    assert_eq!(result.last().unwrap().len(), 4096);
}

#[test]
fn joined_continuations_preserve_raw_ports_without_path_cloning() {
    let (source, _) = graph(&[
        (0x1000, &[0x83, 0xc0, 1, 0xeb, 0x0b]),
        (0x1010, &[0x49, 0x75, 0xed, 0x0f, 0x0b]),
    ]);
    let g = Graph::build_continuations(&source).unwrap();
    assert!(g.node_blocks.len() > source.blocks.len() + 2);
    for block in &source.blocks {
        assert!(g.successors[INGRESS.0].contains(&NodeId::block(block.id)));
    }
    let typed: Vec<_> = (source.blocks.len() + 2..g.node_blocks.len()).collect();
    assert!(typed.iter().all(|n| !g.predecessors[*n].contains(&INGRESS)));
    assert!(g.node_blocks.len() <= 2 + source.blocks.len() * 2);
    let mut joins = BTreeSet::new();
    for n in typed {
        assert!(joins.insert(g.node_blocks[n].unwrap()));
        for pred in &g.predecessors[n] {
            assert!(g.successors[pred.0].contains(&NodeId(n)));
        }
    }
    // Equal edge arguments (including flag results also held in GPRs) share
    // their phi instead of forcing redundant runtime parallel copies.
    let mut phis = BTreeSet::new();
    for value in &g.values {
        if let Op::Phi { incoming } = &value.op {
            assert!(phis.insert((value.home, incoming.clone())));
        }
    }
}

#[test]
fn recovery_rejects_a_future_load_even_when_its_block_dominates() {
    let (source, _) = graph(&[(0x1000, &[0x8b, 0x03, 0x01, 0xc2, 0x0f, 0x0b])]);
    let mut g = Graph::build_continuations(&source).unwrap();
    let node = NodeId::block(source.entry);
    let initial = source.snapshots[source.blocks[0].steps[0].before.index()]
        .cpu
        .gpr[0];
    let future = g
        .values
        .iter()
        .enumerate()
        .find_map(|(index, value)| {
            (value.home == node && matches!(value.op, Op::EffectResult { .. }))
                .then_some(ValueId(index))
        })
        .unwrap();
    g.maps[node.0].values.insert(initial, future);
    assert!(
        g.verify(&source)
            .unwrap_err()
            .0
            .contains("incomplete effect")
    );
}
