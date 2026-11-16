use super::*;
use crate::decode::{CodeSource, discover};

struct Code(Vec<u8>);
impl CodeSource for Code {
    fn read_code(&self, address: u32, max_len: usize) -> Vec<u8> {
        let Some(offset) = address.checked_sub(0x1000).map(|offset| offset as usize) else {
            return Vec::new();
        };
        let Some(bytes) = self.0.get(offset..) else {
            return Vec::new();
        };
        bytes.iter().copied().take(max_len).collect()
    }
}
fn graph(bytes: &[u8]) -> Graph {
    let region = discover(&Code(bytes.to_vec()), 0x1000, 64);
    lift(&region).expect("lift decoded fixture")
}
fn resolved(graph: &Graph, mut id: ValueId) -> &ValueOp {
    loop {
        match graph.values[id.index()].op {
            ValueOp::Alias(next) => id = next,
            _ => return &graph.values[id.index()].op,
        }
    }
}

#[test]
fn accumulator_sign_extension_uses_pure_values_and_keeps_flags() {
    for bytes in [&[0x98, 0x0f, 0x0b][..], &[0x66, 0x98, 0x0f, 0x0b]] {
        let mut graph = graph(bytes);
        optimize(&mut graph).unwrap();
        verify(&graph).unwrap();
        let step = &graph.blocks[graph.entry.index()].steps[0];
        assert!(step.effects.is_empty(), "register conversion is not opaque");
        let before = &graph.snapshots[step.before.index()];
        let after = &graph.snapshots[step.after.index()];
        assert_eq!(before.cpu.flags, after.cpu.flags);
        assert_eq!(before.completed_prefix, 0);
        assert_eq!(after.completed_prefix, 1);
        assert_ne!(before.cpu.gpr[0], after.cpu.gpr[0]);
        for register in 1..8 {
            assert_eq!(before.cpu.gpr[register], after.cpu.gpr[register]);
        }
        assert!(graph.values.iter().any(|value| matches!(
            value.op,
            ValueOp::Binary {
                op: BinaryOp::ShrS,
                ..
            }
        )));
    }
}

#[test]
fn count_branches_keep_flag_recipes_and_precise_counter_snapshots() {
    for byte in [0xe0, 0xe1, 0xe2, 0xe3] {
        let mut graph = graph(&[byte, 0xfe, 0x0f, 0x0b]);
        optimize(&mut graph).unwrap();
        let block = &graph.blocks[graph.entry.index()];
        let step = &block.steps[0];
        assert!(
            step.effects.is_empty(),
            "count branch is not an opaque adapter"
        );
        let before = &graph.snapshots[step.before.index()];
        let after = &graph.snapshots[step.after.index()];
        assert_eq!(before.cpu.flags, after.cpu.flags);
        assert_eq!(before.completed_prefix, 0);
        assert_eq!(after.completed_prefix, 1);
        for register in [0, 2, 3, 4, 5, 6, 7] {
            assert_eq!(before.cpu.gpr[register], after.cpu.gpr[register]);
        }
        if byte == 0xe3 {
            assert_eq!(before.cpu.gpr[1], after.cpu.gpr[1]);
        } else {
            assert!(matches!(
                resolved(&graph, after.cpu.gpr[1]),
                ValueOp::Binary {
                    op: BinaryOp::Sub,
                    ..
                }
            ));
        }
        let Terminator::Conditional {
            condition,
            taken: Target::Block(target),
            ..
        } = block.terminator
        else {
            panic!("count branch must retain explicit conditional CFG")
        };
        assert_eq!(target, graph.entry);
        let mut visited = std::collections::BTreeSet::new();
        let mut work = vec![condition];
        let mut read_bits = std::collections::BTreeSet::new();
        while let Some(value) = work.pop() {
            if !visited.insert(value) {
                continue;
            }
            match *resolved(&graph, value) {
                ValueOp::EvaluateFlag(flag) => {
                    read_bits.insert(graph.flags[flag.index()].bit);
                }
                ValueOp::Binary { left, right, .. } => work.extend([left, right]),
                ValueOp::Select { condition, yes, no } => work.extend([condition, yes, no]),
                _ => {}
            }
        }
        let expected = if byte < 0xe2 {
            [3].into_iter().collect()
        } else {
            std::collections::BTreeSet::new()
        };
        assert_eq!(read_bits, expected);
    }
}

#[test]
fn unavailable_fetch_retains_unretired_location_and_completed_prefix() {
    for bytes in [&[][..], &[0x40][..]] {
        let mut graph = graph(bytes);
        optimize(&mut graph).unwrap();
        let block = &graph.blocks[graph.entry.index()];
        let step = block.steps.last().unwrap();
        assert_eq!(step.source.len, 0);
        assert_eq!(
            step.source.unsupported,
            Some(ir::UnsupportedReason::FetchUnavailable)
        );
        let before = &graph.snapshots[step.before.index()];
        let after = &graph.snapshots[step.after.index()];
        assert_eq!(before.completed_prefix, bytes.len() as u32);
        assert_eq!(after.completed_prefix, before.completed_prefix);
        assert_eq!(
            resolved(&graph, before.eip),
            &ValueOp::Constant(0x1000 + bytes.len() as u32)
        );
        assert_eq!(resolved(&graph, after.eip), resolved(&graph, before.eip));
    }
    let mut malformed = graph(&[0x90, 0x0f, 0x0b]);
    malformed.blocks[0].steps[0].source.len = 0;
    assert!(
        verify(&malformed).is_err(),
        "ordinary instructions cannot have zero length"
    );
}

#[test]
fn loop_condition_demands_only_zf_and_carries_other_leaf_recipes() {
    // ADD EAX,1; DEC ECX; JNZ loop; UD2.
    let mut graph = graph(&[0x83, 0xc0, 1, 0x49, 0x75, 0xfa, 0x0f, 0x0b]);
    let entries = graph.entries.clone();
    let stats = optimize(&mut graph).expect("optimize loop");
    assert_eq!(graph.entries, entries);
    let block = &graph.blocks[graph.entry.index()];
    let Terminator::Conditional {
        condition,
        taken: Target::Block(target),
        state,
        ..
    } = block.terminator
    else {
        panic!("expected loop")
    };
    assert_eq!(target, graph.entry);
    let mut visited = std::collections::BTreeSet::new();
    let mut work = vec![condition];
    let mut read_bits = std::collections::BTreeSet::new();
    while let Some(value) = work.pop() {
        if !visited.insert(value) {
            continue;
        }
        match *resolved(&graph, value) {
            ValueOp::EvaluateFlag(flag) => {
                read_bits.insert(graph.flags[flag.index()].bit);
            }
            ValueOp::Binary { left, right, .. } => work.extend([left, right]),
            ValueOp::Select { condition, yes, no } => work.extend([condition, yes, no]),
            _ => {}
        }
    }
    assert_eq!(read_bits, [3].into_iter().collect());
    let exit = &graph.snapshots[state.index()];
    // CF still comes from ADD; DEC does not hide it in another recipe.
    let cf = &graph.flags[exit.cpu.flags.arithmetic[0].index()];
    assert!(matches!(
        cf.op,
        FlagOp::Leaf {
            kind: FlagKind::Add,
            ..
        }
    ));
    assert!(stats.exit_only_flag_evaluations > 0);
    assert_eq!(block.parameters.gpr.len(), 8);
}

#[test]
fn fault_recipes_retain_completed_prior_effects_and_pre_store_flags() {
    // ADD EAX,1; MOV [EBX],EAX; ADD [ESI],ECX; UD2.
    let mut graph = graph(&[0x83, 0xc0, 1, 0x89, 0x03, 0x01, 0x0e, 0x0f, 0x0b]);
    optimize(&mut graph).unwrap();
    let block = &graph.blocks[graph.entry.index()];
    let step = &block.steps[2];
    assert_eq!(step.effects.len(), 2);
    let load = &graph.effects[step.effects[0].index()];
    let store = &graph.effects[step.effects[1].index()];
    assert!(matches!(
        load.kind,
        EffectKind::Load {
            access: Access::ReadWrite,
            ..
        }
    ));
    assert!(
        matches!(store.kind, EffectKind::Store { checked_by: Some(id), .. } if id == step.effects[0])
    );
    assert_eq!(load.previous, Some(block.steps[1].effects[0]));
    assert_eq!(graph.snapshots[load.before.index()].completed_prefix, 2);
    assert_eq!(graph.snapshots[store.after.index()].completed_prefix, 3);
    assert_eq!(
        graph.snapshots[load.before.index()].cpu,
        graph.snapshots[block.steps[1].after.index()].cpu
    );
    assert_ne!(
        graph.snapshots[store.before.index()].cpu.flags.arithmetic,
        graph.snapshots[store.after.index()].cpu.flags.arithmetic
    );
}

#[test]
fn zero_shift_keeps_rmw_permission_effect_but_guards_every_store_side_effect() {
    let mut graph = graph(&[0xc1, 0x20, 0, 0x0f, 0x0b]); // SHL dword[EAX],0.
    optimize(&mut graph).unwrap();
    let step = &graph.blocks[0].steps[0];
    assert_eq!(step.effects.len(), 2);
    let load = &graph.effects[step.effects[0].index()];
    let store = &graph.effects[step.effects[1].index()];
    assert_eq!(load.guard, None);
    assert!(matches!(
        load.kind,
        EffectKind::Load {
            access: Access::ReadWrite,
            ..
        }
    ));
    assert_eq!(
        resolved(&graph, store.guard.expect("store guard")),
        &ValueOp::Constant(0)
    );
    assert_eq!(
        graph.snapshots[step.before.index()].cpu,
        graph.snapshots[step.after.index()].cpu
    );
}

#[test]
fn calls_and_nonstandard_returns_keep_guest_stack_effect_and_arbitrary_pc() {
    let mut graph = graph(&[0xff, 0xd0, 0xc2, 0x0c, 0]); // CALL EAX; RET 12.
    optimize(&mut graph).unwrap();
    let call = &graph.blocks[graph.entries[&0x1000].index()];
    let ret = &graph.blocks[graph.entries[&0x1002].index()];
    assert!(matches!(call.terminator, Terminator::Indirect { .. }));
    assert!(matches!(ret.terminator, Terminator::Indirect { .. }));
    let EffectKind::Store { value, address, .. } =
        graph.effects[call.steps[0].effects[0].index()].kind
    else {
        panic!("CALL stack store")
    };
    assert_eq!(resolved(&graph, value), &ValueOp::Constant(0x1002));
    assert_eq!(address.segment, ir::Segment::Ss);
    assert!(matches!(
        graph.effects[ret.steps[0].effects[0].index()].kind,
        EffectKind::Load {
            access: Access::Read,
            ..
        }
    ));
    assert_ne!(
        graph.snapshots[ret.steps[0].before.index()].cpu.gpr[4],
        graph.snapshots[ret.steps[0].after.index()].cpu.gpr[4]
    );
}

#[test]
fn cmov_memory_is_read_even_when_a_pure_condition_is_false() {
    // XOR EAX,EAX; CMOVNE EDX,[EBX]; UD2.
    let mut graph = graph(&[0x31, 0xc0, 0x0f, 0x45, 0x13, 0x0f, 0x0b]);
    optimize(&mut graph).unwrap();
    let step = &graph.blocks[0].steps[1];
    assert_eq!(step.effects.len(), 1);
    assert_eq!(graph.effects[step.effects[0].index()].guard, None);
    assert!(matches!(
        graph.effects[step.effects[0].index()].kind,
        EffectKind::Load {
            access: Access::Read,
            ..
        }
    ));
    assert_eq!(
        graph.snapshots[step.before.index()].cpu.gpr[2],
        graph.snapshots[step.after.index()].cpu.gpr[2]
    );
}

#[test]
fn optimizer_does_not_specialize_an_independently_enterable_successor() {
    // MOV EAX,7; JMP next; ADD EAX,1; UD2.
    let mut graph = graph(&[0xb8, 7, 0, 0, 0, 0xeb, 0, 0x83, 0xc0, 1, 0x0f, 0x0b]);
    optimize(&mut graph).unwrap();
    let next = &graph.blocks[graph.entries[&0x1007].index()];
    assert!(matches!(
        graph.values[next.parameters.gpr[0].index()].op,
        ValueOp::Parameter {
            field: InputField::Gpr(0),
            ..
        }
    ));
    let after = &graph.snapshots[next.steps[0].after.index()];
    assert!(!matches!(
        resolved(&graph, after.cpu.gpr[0]),
        ValueOp::Constant(_)
    ));
}

#[test]
fn verifier_rejects_cycles_stale_rmw_proofs_and_specialized_entries() {
    let base = graph(&[0x83, 0x00, 1, 0x0f, 0x0b]);
    let mut cycle = base.clone();
    let id = cycle.blocks[0].parameters.gpr[0];
    cycle.values[id.index()].op = ValueOp::Alias(id);
    assert!(verify(&cycle).is_err());
    let mut stale = base.clone();
    let store = stale.blocks[0].steps[0].effects[1];
    if let EffectKind::Store { checked_by, .. } = &mut stale.effects[store.index()].kind {
        *checked_by = Some(store);
    }
    assert!(verify(&stale).is_err());
    let mut specialized = base;
    let id = specialized.blocks[0].parameters.gpr[0];
    specialized.values[id.index()] = Value {
        home: None,
        op: ValueOp::Constant(7),
    };
    assert!(verify(&specialized).is_err());
}

#[test]
fn opaque_count_dependent_flags_use_exact_adapter_validity_and_old_raw_input() {
    for bytes in [
        &[0x0f, 0xac, 0xd0, 0, 0x0f, 0x0b][..], // SHRD EAX,EDX,0.
        &[0x0f, 0xad, 0xd0, 0x0f, 0x0b][..],    // SHRD EAX,EDX,CL.
        &[0x0f, 0xa4, 0xd0, 0, 0x0f, 0x0b][..], // SHLD EAX,EDX,0.
        &[0xf2, 0xae, 0x0f, 0x0b][..],          // REPNE SCASB.
        &[0xf3, 0xa7, 0x0f, 0x0b][..],          // REPE CMPSD.
    ] {
        let mut graph = graph(bytes);
        optimize(&mut graph).unwrap();
        let step = &graph.blocks[graph.entry.index()].steps[0];
        let id = step.effects[0];
        let EffectKind::Opaque { contract, .. } = graph.effects[id.index()].kind else {
            panic!("expected complex adapter")
        };
        assert_eq!(contract.flags_read & ARITHMETIC_MASK, ARITHMETIC_MASK);
        assert_eq!(contract.flags_written & ARITHMETIC_MASK, ARITHMETIC_MASK);
        assert!(
            matches!(resolved(&graph, graph.snapshots[step.after.index()].cpu.flags.defined), ValueOp::EffectResult { effect, output: EffectOutput::DefinedFlags } if *effect == id)
        );
    }
}

#[test]
fn scas_restart_demands_complete_flags_but_copy_strings_only_demand_df() {
    for (bytes, scas) in [
        (&[0xf3, 0xa4, 0x0f, 0x0b][..], false),
        (&[0xf3, 0xaa, 0x0f, 0x0b][..], false),
        (&[0xf2, 0xae, 0x0f, 0x0b][..], true),
        (&[0xae, 0x0f, 0x0b][..], true),
        (&[0xf3, 0xa6, 0x0f, 0x0b][..], true),
        (&[0xa7, 0x0f, 0x0b][..], true),
    ] {
        let mut graph = graph(bytes);
        optimize(&mut graph).unwrap();
        let step = &graph.blocks[graph.entry.index()].steps[0];
        let EffectKind::Opaque {
            contract,
            ref inputs,
            ..
        } = graph.effects[step.effects[0].index()].kind
        else {
            panic!("string adapter")
        };
        assert_eq!(contract.flags_read, if scas { u32::MAX } else { 0x400 });
        let (normal, _) = demands(&graph).unwrap();
        assert!(normal.values.contains(&inputs.flags.other));
        assert!(normal.values.contains(&inputs.flags.defined));
        for flag in inputs.flags.arithmetic {
            assert_eq!(normal.evaluated_flags.contains(&flag), scas);
        }
    }
}
