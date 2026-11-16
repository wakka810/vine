use super::*;

#[test]
fn continuation_templates_preserve_exact_predicates_with_separate_result_input() {
    fn template(t: &Template, args: [u32; 4]) -> u32 {
        let mut values = Vec::new();
        for op in &t.values {
            let at = |id: ValueId| values[id.index()];
            let value = match *op {
                Op::Constant(n) => n,
                Op::Parameter {
                    field: Parameter::Gpr(n),
                    ..
                } => args[n as usize],
                Op::Binary { op, left, right } => v::fold_binary(op, at(left), at(right)),
                Op::Select { condition, yes, no } => {
                    if at(condition) == 0 {
                        at(no)
                    } else {
                        at(yes)
                    }
                }
                Op::Popcnt(id) => at(id).count_ones(),
                _ => panic!("non-pure continuation template"),
            };
            values.push(value);
        }
        values[t.output.index()]
    }
    for width in [ScalarWidth::W8, ScalarWidth::W16, ScalarWidth::W32] {
        for kind in [
            FlagKind::Add,
            FlagKind::Sub,
            FlagKind::Logic,
            FlagKind::Shl,
            FlagKind::Shr,
            FlagKind::Sar,
            FlagKind::Materialized,
        ] {
            let mut b = builder();
            let args = std::array::from_fn(|n| {
                if n < 3 {
                    b.node(Op::Parameter {
                        block: BlockId(0),
                        field: Parameter::Gpr(n as u8),
                    })
                } else {
                    b.constant(0)
                }
            });
            let payloads: [ValueId; 6] =
                std::array::from_fn(|bit| b.leaf(bit as u8, kind, width, args));
            let result = Template::result(kind, width);
            let flags: [Template; 6] =
                std::array::from_fn(|bit| Template::flag(bit as u8, kind, width));
            for a in [
                0, 1, 0x7f, 0x80, 0x7fff, 0x8000, 0x7fffffff, 0x80000000, 0xffffffff,
            ] {
                for count in 0..64 {
                    for carry in 0..2 {
                        let args = [a, count, carry, 0];
                        let r = template(&result, args);
                        let values = evaluate(&b.graph, [a, count, carry]);
                        for bit in 0..6 {
                            assert_eq!(
                                template(&flags[bit], [a, count, carry, r]),
                                values[payloads[bit].index()],
                                "{kind:?} {width:?} bit{bit} a{a:x} b{count} c{carry}"
                            );
                        }
                    }
                }
            }
        }
    }
}

fn builder() -> Builder {
    Builder {
        graph: Graph {
            values: vec![],
            value_map: vec![],
            payloads: vec![],
            conditions: vec![],
            stats: Stats::default(),
        },
        home: Some(BlockId(0)),
        table: BTreeMap::new(),
    }
}
fn evaluate(graph: &Graph, inputs: [u32; 3]) -> Vec<u32> {
    let mut values = Vec::with_capacity(graph.values.len());
    for node in &graph.values {
        let at = |id: ValueId| values[id.index()];
        let value = match node.op {
            Op::Constant(n) => n,
            Op::Parameter {
                field: Parameter::Gpr(n),
                ..
            } => inputs[n as usize],
            Op::Binary { op, left, right } => v::fold_binary(op, at(left), at(right)),
            Op::Select { condition, yes, no } => {
                if at(condition) == 0 {
                    at(no)
                } else {
                    at(yes)
                }
            }
            Op::Popcnt(value) => at(value).count_ones(),
            _ => panic!("unexpected arithmetic fixture input"),
        };
        values.push(value);
    }
    values
}
fn arithmetic_reference(
    width: ScalarWidth,
    subtract: bool,
    a: u32,
    b: u32,
    carry: u32,
) -> [u32; 6] {
    let mask = u64::from(width.mask());
    let a = u64::from(a) & mask;
    let b = u64::from(b) & mask;
    let c = u64::from(carry & 1);
    let result = if subtract {
        a.wrapping_sub(b + c)
    } else {
        a + b + c
    } & mask;
    let sign = 1_i64 << (width.bits() - 1);
    let signed = |x: u64| {
        if x as i64 >= sign {
            x as i64 - (sign * 2)
        } else {
            x as i64
        }
    };
    let exact = if subtract {
        signed(a) - signed(b) - c as i64
    } else {
        signed(a) + signed(b) + c as i64
    };
    [
        u32::from(if subtract {
            a < b + c
        } else {
            a + b + c > mask
        }),
        u32::from((result as u8).count_ones() & 1 == 0),
        u32::from(if subtract {
            (a & 15) < (b & 15) + c
        } else {
            (a & 15) + (b & 15) + c > 15
        }),
        u32::from(result == 0),
        u32::from(result & sign as u64 != 0),
        u32::from(exact < -sign || exact >= sign),
    ]
}
fn arithmetic_fixture(width: ScalarWidth, kind: FlagKind) -> (Graph, [ValueId; 6]) {
    let mut b = builder();
    let mut args = std::array::from_fn(|n| {
        if n < 3 {
            b.node(Op::Parameter {
                block: BlockId(0),
                field: Parameter::Gpr(n as u8),
            })
        } else {
            b.constant(0)
        }
    });
    args[3] = b.constant(0);
    let flags = std::array::from_fn(|bit| {
        let p = b.leaf(bit as u8, kind, width, args);
        b.condition(bit as u8, p)
    });
    (b.graph, flags)
}
#[test]
fn predicates_match_exhaustive_byte_arithmetic_including_carry_and_borrow() {
    for kind in [FlagKind::Add, FlagKind::Sub] {
        let (graph, flags) = arithmetic_fixture(ScalarWidth::W8, kind);
        for a in 0..=255 {
            for b in 0..=255 {
                for carry in 0..=1 {
                    let values = evaluate(&graph, [a, b, carry]);
                    assert_eq!(
                        flags.map(|id| values[id.index()]),
                        arithmetic_reference(ScalarWidth::W8, kind == FlagKind::Sub, a, b, carry),
                        "{kind:?}: {a:x} {b:x} {carry}"
                    );
                }
            }
        }
    }
}
#[test]
fn predicate_widths_and_partial_canonical_flags_are_independent() {
    for width in [ScalarWidth::W16, ScalarWidth::W32] {
        for kind in [FlagKind::Add, FlagKind::Sub] {
            let (graph, flags) = arithmetic_fixture(width, kind);
            let mut n = 0x9e37_79b9_u32;
            for index in 0..8192 {
                n ^= n << 13;
                n ^= n >> 17;
                n ^= n << 5;
                let a = n;
                n ^= n << 13;
                n ^= n >> 17;
                n ^= n << 5;
                let b = n;
                let carry = index & 1;
                let values = evaluate(&graph, [a, b, carry]);
                assert_eq!(
                    flags.map(|id| values[id.index()]),
                    arithmetic_reference(width, kind == FlagKind::Sub, a, b, carry)
                );
            }
        }
    }
    for packed in 0..64 {
        let mut b = builder();
        let flags: [ValueId; 6] = std::array::from_fn(|bit| {
            let value = b.constant((packed >> bit) & 1);
            let p = b.boolean_payload(bit as u8, value);
            b.condition(bit as u8, p)
        });
        let values = evaluate(&b.graph, [0; 3]);
        assert_eq!(
            flags.map(|id| values[id.index()]),
            std::array::from_fn(|bit| (packed >> bit) & 1)
        );
    }
}
#[test]
fn shift_predicates_cover_zero_over_width_and_masked_counts() {
    for width in [ScalarWidth::W8, ScalarWidth::W16, ScalarWidth::W32] {
        for kind in [FlagKind::Shl, FlagKind::Shr, FlagKind::Sar] {
            let (graph, flags) = arithmetic_fixture(width, kind);
            for value in [
                0,
                1,
                0x7f,
                0x80,
                0xff,
                0x7fff,
                0x8000,
                0xffff,
                0x7fff_ffff,
                0x8000_0000,
                u32::MAX,
            ] {
                for count in 0..64 {
                    let values = evaluate(&graph, [value, count, 0]);
                    for bit in 0..6 {
                        assert_eq!(
                            values[flags[bit].index()],
                            v::evaluate_leaf(bit as u8, kind, width, [value, count, 0, 0]),
                            "{kind:?}/{width:?} {value:x} {count} bit{bit}"
                        );
                    }
                }
            }
        }
    }
}

struct Code(Vec<u8>);
impl crate::decode::CodeSource for Code {
    fn read_code(&self, address: u32, maximum: usize) -> Vec<u8> {
        address
            .checked_sub(0x1000)
            .and_then(|n| self.0.get(n as usize..))
            .unwrap_or_default()
            .iter()
            .copied()
            .take(maximum)
            .collect()
    }
}
#[test]
fn scalar_results_and_predicates_share_nodes_without_changing_effects_or_entries() {
    let region = crate::decode::discover(
        &Code(vec![0x01, 0xd8, 0x89, 0x06, 0x75, 0xfa, 0x0f, 0x0b]),
        0x1000,
        64,
    );
    let mut source = v::lift(&region).unwrap();
    v::optimize(&mut source).unwrap();
    let graph = Graph::build(&source).unwrap();
    graph.verify(&source).unwrap();
    let step = &source.blocks[source.entry.index()].steps[0];
    let after = &source.snapshots[step.after.index()].cpu;
    assert_eq!(
        graph.value_map[after.gpr[0].index()],
        graph.payloads[after.flags.arithmetic[1].index()]
    );
    assert_eq!(
        graph.value_map[after.gpr[0].index()],
        graph.payloads[after.flags.arithmetic[3].index()]
    );
    assert_eq!(
        graph.value_map[after.gpr[0].index()],
        graph.payloads[after.flags.arithmetic[4].index()]
    );
    assert_eq!(source.entries.len(), region.blocks.len());
    assert!(
        source
            .effects
            .iter()
            .any(|e| matches!(e.kind, v::EffectKind::Store { .. }))
    );
    let mut malformed = graph.clone();
    let first = malformed
        .values
        .iter()
        .position(|n| matches!(n.op, Op::Binary { .. }))
        .unwrap();
    malformed.values[first].op = Op::Popcnt(ValueId(first as u32));
    assert!(malformed.verify(&source).is_err());
}
