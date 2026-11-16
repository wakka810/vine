use super::*;
use crate::decode::{CodeSource, discover};

struct Code(BTreeMap<u32, u8>);
impl CodeSource for Code {
    fn read_code(&self, address: u32, max_len: usize) -> Vec<u8> {
        (0..max_len)
            .map_while(|offset| self.0.get(&address.wrapping_add(offset as u32)).copied())
            .collect()
    }
}
fn graph(parts: &[(u32, &[u8])]) -> Graph {
    let code = Code(
        parts
            .iter()
            .flat_map(|(start, bytes)| {
                bytes
                    .iter()
                    .enumerate()
                    .map(move |(offset, &byte)| (start + offset as u32, byte))
            })
            .collect(),
    );
    lift(&discover(&code, 0x1000, 64)).expect("rooted fixture")
}
fn diamond(right_subtract: bool, right_immediate: u8) -> Graph {
    let right = [
        0x83,
        if right_subtract { 0xe8 } else { 0xc0 },
        right_immediate,
        0xeb,
        0x0b,
    ];
    graph(&[
        (0x1000, &[0x83, 0xf8, 0, 0x75, 0x0b]), // CMP EAX,0; JNZ right
        (0x1005, &[0x83, 0xc0, 1, 0xeb, 0x16]), // ADD EAX,1; JMP join
        (0x1010, &right),
        (0x1020, &[0x0f, 0x94, 0xc2, 0x0f, 0x0b]), // SETZ DL; UD2
    ])
}

#[test]
fn call_continuation_is_not_a_rooted_predecessor_or_implicit_return() {
    let graph = graph(&[(0x1000, &[0xe8, 3, 0, 0, 0, 0x40, 0x0f, 0x0b, 0xc3])]);
    let entry = graph.source.entry;
    let continuation = graph.source.entries[&0x1005];
    let callee = graph.source.entries[&0x1008];
    assert_eq!(graph.reachable, BTreeSet::from([entry, callee]));
    assert!(!graph.predecessors.contains_key(&continuation));
    assert_eq!(graph.predecessors[&callee], vec![entry]);
    assert!(matches!(
        graph.source.blocks[callee.index()].terminator,
        v::Terminator::Indirect { .. }
    ));
    assert!(
        graph.source.blocks[entry.index()].steps[0]
            .effects
            .iter()
            .any(|effect| matches!(
                graph.source.effects[effect.index()].kind,
                v::EffectKind::Store { .. }
            ))
    );
    for (index, value) in graph.source.values.iter().enumerate() {
        if value.home == Some(continuation) {
            assert_eq!(
                graph.values[graph.value_map[index].index()].op,
                Op::Unreachable
            );
        }
    }
}

#[test]
fn loop_phi_has_one_invocation_edge_and_real_backedge_without_repeated_inputs() {
    let graph = graph(&[(0x1000, &[0x83, 0xc0, 1, 0x49, 0x75, 0xfa, 0x0f, 0x0b])]);
    let entry = graph.source.entry;
    let ecx = graph.mapped_value(graph.source.blocks[entry.index()].parameters.gpr[1]);
    let Op::Phi { block, incoming } = &graph.values[ecx.index()].op else {
        panic!("changing loop count needs phi")
    };
    assert_eq!(*block, entry);
    assert_eq!(
        incoming.iter().map(|(edge, _)| *edge).collect::<Vec<_>>(),
        vec![None, Some(entry)]
    );
    assert!(matches!(
        graph.values[graph.resolve(incoming[0].1).index()].op,
        Op::Input(Input::Gpr(1))
    ));
    let ebx = graph.mapped_value(graph.source.blocks[entry.index()].parameters.gpr[3]);
    assert!(
        matches!(graph.values[ebx.index()].op, Op::Input(Input::Gpr(3))),
        "unchanged loop register has no copy/phi"
    );
    assert_eq!(
        graph
            .root_inputs
            .iter()
            .filter(|(_, input)| *input == Input::Gpr(1))
            .count(),
        1
    );
    assert!(graph.stats.phis_simplified > 0);
}

#[test]
fn equal_flag_producer_shapes_merge_operands_and_share_phis_across_flag_bits() {
    let graph = diamond(false, 2);
    let join = graph.source.entries[&0x1020];
    let flags = &graph.source.blocks[join.index()]
        .parameters
        .flags
        .arithmetic;
    let mut producer_args = None;
    for (bit, &flag) in flags.iter().enumerate() {
        let id = graph.mapped_flag(flag);
        let Op::Flag {
            bit: actual_bit,
            kind,
            width,
            args,
        } = graph.values[id.index()].op
        else {
            panic!("homogeneous ADD uses static producer")
        };
        assert_eq!(actual_bit, bit as u8);
        assert_eq!(kind, FlagKind::Add);
        assert_eq!(width, ScalarWidth::W32);
        let args = args.map(|id| graph.resolve(id));
        if let Some(previous) = producer_args {
            assert_eq!(args, previous);
        } else {
            producer_args = Some(args);
        }
    }
    let args = producer_args.unwrap();
    assert_eq!(
        args.into_iter()
            .filter(|id| matches!(graph.values[id.index()].op, Op::Phi { .. }))
            .count(),
        1,
        "only differing immediate needs an operand phi, shared by six flags"
    );
    let zero_flag = graph.mapped_flag(flags[3]);
    assert!(graph.normal_values.contains(&zero_flag));
    for &flag in flags {
        assert!(
            graph.exit_values.contains(&graph.mapped_flag(flag)),
            "all snapshot bits remain recoverable"
        );
    }
}

#[test]
fn heterogeneous_flag_producers_use_boolean_phi_without_dynamic_recipe_tags() {
    let graph = diamond(true, 2);
    let join = graph.source.entries[&0x1020];
    for &flag in &graph.source.blocks[join.index()]
        .parameters
        .flags
        .arithmetic
    {
        let id = graph.mapped_flag(flag);
        let Op::Phi { block, incoming } = &graph.values[id.index()].op else {
            panic!("ADD/SUB join needs boolean phi")
        };
        assert_eq!(*block, join);
        assert_eq!(incoming.len(), 2);
        for &(_, source) in incoming {
            assert!(matches!(
                graph.values[graph.resolve(source).index()].op,
                Op::Flag { .. }
            ));
        }
    }
    assert!(graph.stats.boolean_flag_parameters >= 6);
}

#[test]
fn pure_cse_never_uses_a_sibling_definition_that_does_not_dominate() {
    let graph = diamond(false, 1);
    let left = graph.source.entries[&0x1005];
    let right = graph.source.entries[&0x1010];
    let value = |block: BlockId| {
        let step = &graph.source.blocks[block.index()].steps[0];
        graph.mapped_value(graph.source.snapshots[step.after.index()].cpu.gpr[0])
    };
    assert_ne!(
        value(left),
        value(right),
        "same expression in sibling branches is not an available definition"
    );
    assert_eq!(graph.values[value(left).index()].home, Some(left));
    assert_eq!(graph.values[value(right).index()].home, Some(right));
    let mut malformed = graph.clone();
    malformed.values[value(left).index()].op = Op::Alias(value(right));
    assert!(
        verify(&malformed).is_err(),
        "sibling CSE fails dominance verification"
    );
}

#[test]
fn completed_load_results_stay_atomic_and_future_effects_cannot_enter_fault_snapshot() {
    let graph = graph(&[(0x1000, &[0x8b, 0x03, 0x8b, 0x0a, 0x0f, 0x0b])]);
    let entry = &graph.source.blocks[graph.source.entry.index()];
    let second = &entry.steps[1];
    let before = &graph.source.snapshots[second.before.index()];
    let first_result = graph.mapped_value(before.cpu.gpr[0]);
    let Op::EffectResult {
        effect,
        output: EffectOutput::Loaded,
    } = graph.values[first_result.index()].op
    else {
        panic!("first load must have a single atomic result")
    };
    assert_eq!(effect, entry.steps[0].effects[0]);
    assert!(graph.exit_values.contains(&first_result));
    assert_eq!(
        graph
            .values
            .iter()
            .filter(|node| matches!(node.op, Op::EffectResult { effect: id, .. } if id == effect))
            .count(),
        1
    );
    let after = &graph.source.snapshots[second.after.index()];
    let future = graph.mapped_value(after.cpu.gpr[1]);
    let mut malformed = graph.clone();
    malformed.value_map[before.cpu.gpr[0].index()] = future;
    assert!(
        verify(&malformed).is_err(),
        "fault before second load cannot read its output"
    );
}

#[test]
fn verifier_rejects_phi_edge_changes_alias_cycles_and_missing_snapshot_demand() {
    let graph = graph(&[(0x1000, &[0x49, 0x75, 0xfd, 0x0f, 0x0b])]);
    let count = graph.mapped_value(
        graph.source.blocks[graph.source.entry.index()]
            .parameters
            .gpr[1],
    );
    let mut wrong_edge = graph.clone();
    let Op::Phi { incoming, .. } = &mut wrong_edge.values[count.index()].op else {
        panic!("loop count phi")
    };
    incoming.retain(|(edge, _)| edge.is_some());
    assert!(verify(&wrong_edge).is_err());
    let mut cycle = graph.clone();
    let a = cycle.push(Some(graph.source.entry), Op::Constant(0));
    let b = cycle.push(Some(graph.source.entry), Op::Alias(a));
    cycle.values[a.index()].op = Op::Alias(b);
    assert!(verify(&cycle).is_err());
    let mut missing = graph.clone();
    missing.exit_values.clear();
    assert!(verify(&missing).is_err());
}

#[test]
fn partial_register_and_carry_inputs_keep_the_lifted_numeric_semantics() {
    let graph = graph(&[(0x1000, &[0x80, 0xc4, 1, 0x83, 0xd0, 0, 0x0f, 0x0b])]);
    let entry = &graph.source.blocks[graph.source.entry.index()];
    let after = &graph.source.snapshots[entry.steps[1].after.index()];
    let eax = graph.mapped_value(after.cpu.gpr[0]);
    let mut inputs = BTreeSet::new();
    let mut work = vec![eax];
    while let Some(id) = work.pop() {
        let id = graph.resolve(id);
        if inputs.insert(id) {
            work.extend(graph.dependencies(id));
        }
    }
    assert!(
        inputs
            .iter()
            .any(|id| matches!(graph.values[id.index()].op, Op::Input(Input::Gpr(0))))
    );
    assert!(
        inputs.iter().any(|id| matches!(
            graph.values[id.index()].op,
            Op::Flag {
                bit: 0,
                kind: FlagKind::Add,
                width: ScalarWidth::W8,
                ..
            }
        )),
        "ADC demands carry from preceding high-byte ADD"
    );
    verify(&graph).unwrap();
}
