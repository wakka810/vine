//! Numeric conditions over the independent scalar/effect graph.
//!
//! Architectural flags are predicates of scalar values, not opaque runtime
//! recipes. Every block admits arbitrary architectural state. Internal edges
//! carry one numeric value per flag; equal result values share the same DAG.
//! Packing EFLAGS is an exit operation, never an implicit block-edge effect.
use crate::value_ir::{
    self as v, BinaryOp, BlockId, EffectId, EffectOutput, FlagKind, ScalarWidth,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueId(pub u32);
impl ValueId {
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Parameter {
    Gpr(u8),
    OtherFlags,
    DefinedFlags,
    Predicate(u8),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Op {
    Constant(u32),
    Parameter {
        block: BlockId,
        field: Parameter,
    },
    Binary {
        op: BinaryOp,
        left: ValueId,
        right: ValueId,
    },
    Select {
        condition: ValueId,
        yes: ValueId,
        no: ValueId,
    },
    Popcnt(ValueId),
    EffectResult {
        effect: EffectId,
        output: EffectOutput,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Value {
    pub home: Option<BlockId>,
    pub op: Op,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub folded: usize,
    pub shared: usize,
}

#[derive(Clone, Debug)]
pub struct Graph {
    pub values: Vec<Value>,
    pub value_map: Vec<ValueId>,
    pub payloads: Vec<ValueId>,
    pub conditions: Vec<ValueId>,
    pub stats: Stats,
}

/// A bounded pure continuation recipe. Inputs 0..2 are old operands/carry;
/// input 3 is the already computed, width-truncated result. Keeping that result
/// explicit permits a continuation to share it with a live architectural GPR.
/// There is no runtime opcode/tag or guest-dependent guard in this template.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Template {
    pub values: Vec<Op>,
    pub output: ValueId,
    pub inputs: u8,
}

impl Template {
    fn builder() -> (Builder, [ValueId; 4]) {
        let mut b = Builder {
            graph: Graph {
                values: vec![],
                value_map: vec![],
                payloads: vec![],
                conditions: vec![],
                stats: Stats::default(),
            },
            home: Some(BlockId(0)),
            table: BTreeMap::new(),
        };
        let args = std::array::from_fn(|n| {
            b.node(Op::Parameter {
                block: BlockId(0),
                field: Parameter::Gpr(n as u8),
            })
        });
        (b, args)
    }
    pub fn result(kind: FlagKind, width: ScalarWidth) -> Self {
        let (mut b, args) = Self::builder();
        let output = if kind == FlagKind::Materialized {
            args[0]
        } else {
            b.leaf(1, kind, width, args)
        };
        Self::prune(&b.graph, output, None)
    }
    pub fn flag(bit: u8, kind: FlagKind, width: ScalarWidth) -> Self {
        let (mut b, args) = Self::builder();
        let result = (kind != FlagKind::Materialized).then(|| b.leaf(1, kind, width, args));
        let output = b.leaf(bit, kind, width, args);
        Self::prune(&b.graph, output, result)
    }
    pub fn identity() -> Self {
        Self {
            values: vec![Op::Parameter {
                block: BlockId(0),
                field: Parameter::Gpr(0),
            }],
            output: ValueId(0),
            inputs: 1,
        }
    }
    fn prune(graph: &Graph, output: ValueId, result: Option<ValueId>) -> Self {
        let mut needed = std::collections::BTreeSet::new();
        let mut work = vec![output];
        while let Some(id) = work.pop() {
            if !needed.insert(id) || result == Some(id) {
                continue;
            }
            match graph.values[id.index()].op {
                Op::Binary { left, right, .. } => work.extend([left, right]),
                Op::Select { condition, yes, no } => work.extend([condition, yes, no]),
                Op::Popcnt(id) => work.push(id),
                _ => {}
            }
        }
        let mut values = Vec::new();
        let mut mapping = BTreeMap::new();
        let mut inputs = 0;
        for id in needed {
            let op = if result == Some(id) {
                Op::Parameter {
                    block: BlockId(0),
                    field: Parameter::Gpr(3),
                }
            } else {
                match graph.values[id.index()].op {
                    Op::Binary { op, left, right } => Op::Binary {
                        op,
                        left: mapping[&left],
                        right: mapping[&right],
                    },
                    Op::Select { condition, yes, no } => Op::Select {
                        condition: mapping[&condition],
                        yes: mapping[&yes],
                        no: mapping[&no],
                    },
                    Op::Popcnt(a) => Op::Popcnt(mapping[&a]),
                    ref op => op.clone(),
                }
            };
            if let Op::Parameter {
                field: Parameter::Gpr(n),
                ..
            } = op
            {
                inputs |= 1 << n;
            }
            mapping.insert(id, ValueId(values.len() as u32));
            values.push(op);
        }
        Self {
            values,
            output: mapping[&output],
            inputs,
        }
    }
}

struct Builder {
    graph: Graph,
    home: Option<BlockId>,
    table: BTreeMap<(Option<BlockId>, Op), ValueId>,
}

impl Builder {
    fn node(&mut self, op: Op) -> ValueId {
        let home = if matches!(op, Op::Constant(_)) {
            None
        } else {
            self.home
        };
        let key = (home, op.clone());
        if let Some(&id) = self.table.get(&key) {
            self.graph.stats.shared += 1;
            return id;
        }
        let id = ValueId(self.graph.values.len() as u32);
        self.graph.values.push(Value { home, op });
        self.table.insert(key, id);
        id
    }
    fn constant(&mut self, n: u32) -> ValueId {
        self.node(Op::Constant(n))
    }
    fn number(&self, v: ValueId) -> Option<u32> {
        if let Op::Constant(n) = self.graph.values[v.index()].op {
            Some(n)
        } else {
            None
        }
    }
    fn binary(&mut self, op: BinaryOp, mut left: ValueId, mut right: ValueId) -> ValueId {
        use BinaryOp::*;
        if let (Some(a), Some(b)) = (self.number(left), self.number(right)) {
            self.graph.stats.folded += 1;
            return self.constant(v::fold_binary(op, a, b));
        }
        if matches!(op, Add | Mul | And | Or | Xor | Eq | Ne) && right < left {
            std::mem::swap(&mut left, &mut right);
        }
        let a = self.number(left);
        let b = self.number(right);
        let same = left == right;
        let identity = match op {
            Add | Or | Xor if a == Some(0) => Some(right),
            Add | Sub | Or | Xor | Shl | ShrU | ShrS if b == Some(0) => Some(left),
            And if a == Some(u32::MAX) => Some(right),
            And if b == Some(u32::MAX) => Some(left),
            Mul if a == Some(1) => Some(right),
            Mul if b == Some(1) => Some(left),
            And | Or if same => Some(left),
            _ => None,
        };
        if let Some(id) = identity {
            self.graph.stats.folded += 1;
            return id;
        }
        let constant = match op {
            Mul | And if a == Some(0) || b == Some(0) => Some(0),
            Or if a == Some(u32::MAX) || b == Some(u32::MAX) => Some(u32::MAX),
            Sub | Xor | Ne | LtU | LtS if same => Some(0),
            Eq | LeU | LeS if same => Some(1),
            _ => None,
        };
        if let Some(n) = constant {
            self.graph.stats.folded += 1;
            return self.constant(n);
        }
        self.node(Op::Binary { op, left, right })
    }
    fn immediate(&mut self, op: BinaryOp, left: ValueId, n: u32) -> ValueId {
        let right = self.constant(n);
        self.binary(op, left, right)
    }
    fn select(&mut self, condition: ValueId, yes: ValueId, no: ValueId) -> ValueId {
        if yes == no {
            self.graph.stats.folded += 1;
            return yes;
        }
        if let Some(n) = self.number(condition) {
            self.graph.stats.folded += 1;
            return if n == 0 { no } else { yes };
        }
        self.node(Op::Select { condition, yes, no })
    }
    fn truncate(&mut self, value: ValueId, width: ScalarWidth) -> ValueId {
        self.immediate(BinaryOp::And, value, width.mask())
    }
    fn signed_payload(&mut self, value: ValueId, width: ScalarWidth) -> ValueId {
        self.immediate(BinaryOp::Shl, value, 32 - width.bits())
    }
    fn boolean_payload(&mut self, bit: u8, value: ValueId) -> ValueId {
        let value = self.immediate(BinaryOp::And, value, 1);
        match bit {
            0 => value,
            1 | 3 => self.immediate(BinaryOp::Xor, value, 1),
            2 => self.immediate(BinaryOp::Shl, value, 4),
            4 | 5 => self.immediate(BinaryOp::Shl, value, 31),
            _ => unreachable!("verified architectural flag"),
        }
    }
    fn condition(&mut self, bit: u8, payload: ValueId) -> ValueId {
        match bit {
            0 => self.immediate(BinaryOp::And, payload, 1),
            1 => {
                let byte = self.immediate(BinaryOp::And, payload, 255);
                let count = if let Some(n) = self.number(byte) {
                    self.constant(n.count_ones())
                } else {
                    self.node(Op::Popcnt(byte))
                };
                let parity = self.immediate(BinaryOp::And, count, 1);
                self.immediate(BinaryOp::Xor, parity, 1)
            }
            2 => {
                let bit = self.immediate(BinaryOp::ShrU, payload, 4);
                self.immediate(BinaryOp::And, bit, 1)
            }
            3 => self.immediate(BinaryOp::Eq, payload, 0),
            4 | 5 => self.immediate(BinaryOp::ShrU, payload, 31),
            _ => unreachable!("verified architectural flag"),
        }
    }
    fn shift_carry(
        &mut self,
        kind: FlagKind,
        width: ScalarWidth,
        a: ValueId,
        count: ValueId,
    ) -> ValueId {
        use BinaryOp::*;
        let distance = if kind == FlagKind::Shl {
            let bits = self.constant(width.bits());
            self.binary(Sub, bits, count)
        } else {
            self.immediate(Sub, count, 1)
        };
        // All shifts are wrapping i32 shifts, including discarded candidates.
        let moved = self.binary(ShrU, a, distance);
        let moved = self.immediate(And, moved, 1);
        let fallback = if kind == FlagKind::Sar {
            let sign = self.immediate(ShrU, a, width.bits() - 1);
            self.immediate(And, sign, 1)
        } else {
            self.constant(0)
        };
        let within = self.immediate(LeU, count, width.bits());
        let chosen = self.select(within, moved, fallback);
        let nonzero = self.immediate(Ne, count, 0);
        let zero = self.constant(0);
        self.select(nonzero, chosen, zero)
    }
    fn leaf(&mut self, bit: u8, kind: FlagKind, width: ScalarWidth, args: [ValueId; 4]) -> ValueId {
        use BinaryOp::*;
        if kind == FlagKind::Materialized {
            return self.boolean_payload(bit, args[0]);
        }
        let a = self.truncate(args[0], width);
        let b = self.truncate(args[1], width);
        let carry = self.immediate(And, args[2], 1);
        let result = match kind {
            FlagKind::Add | FlagKind::Sub => {
                let op = if kind == FlagKind::Add { Add } else { Sub };
                let r = self.binary(op, a, b);
                let r = self.binary(op, r, carry);
                self.truncate(r, width)
            }
            FlagKind::Shl | FlagKind::Shr | FlagKind::Sar => {
                let a = if kind == FlagKind::Sar {
                    let r = self.signed_payload(a, width);
                    self.immediate(ShrS, r, 32 - width.bits())
                } else {
                    a
                };
                let r = self.binary(
                    match kind {
                        FlagKind::Shl => Shl,
                        FlagKind::Shr => ShrU,
                        _ => ShrS,
                    },
                    a,
                    args[1],
                );
                self.truncate(r, width)
            }
            FlagKind::Logic => a,
            FlagKind::Materialized => unreachable!(),
        };
        match bit {
            0 => match kind {
                FlagKind::Add | FlagKind::Sub => {
                    let (x, y) = if kind == FlagKind::Add {
                        (result, a)
                    } else {
                        (a, b)
                    };
                    let less = self.binary(LtU, x, y);
                    let equal = self.binary(Eq, x, y);
                    let carry_equal = self.binary(And, carry, equal);
                    self.binary(Or, less, carry_equal)
                }
                FlagKind::Shl | FlagKind::Shr | FlagKind::Sar => {
                    let count = self.immediate(And, args[1], 31);
                    self.shift_carry(kind, width, a, count)
                }
                _ => self.constant(0),
            },
            1 | 3 => result,
            2 => {
                if matches!(kind, FlagKind::Add | FlagKind::Sub) {
                    let operands = self.binary(Xor, a, b);
                    self.binary(Xor, operands, result)
                } else {
                    self.constant(0)
                }
            }
            4 => self.signed_payload(result, width),
            5 => match kind {
                FlagKind::Add | FlagKind::Sub => {
                    let different = self.binary(Xor, a, b);
                    let different = if kind == FlagKind::Add {
                        self.immediate(Xor, different, u32::MAX)
                    } else {
                        different
                    };
                    let changed = self.binary(Xor, a, result);
                    let overflow = self.binary(And, different, changed);
                    self.signed_payload(overflow, width)
                }
                FlagKind::Shl => {
                    let count = self.immediate(And, args[1], 31);
                    let carry = self.shift_carry(kind, width, a, count);
                    let carry = self.immediate(Shl, carry, 31);
                    let sign = self.signed_payload(result, width);
                    self.binary(Xor, sign, carry)
                }
                FlagKind::Shr => self.signed_payload(a, width),
                _ => self.constant(0),
            },
            _ => unreachable!("verified architectural flag"),
        }
    }
}

impl Graph {
    pub fn build(source: &v::Graph) -> Result<Self, v::Error> {
        v::verify(source)?;
        let missing = ValueId(u32::MAX);
        let mut b = Builder {
            graph: Self {
                values: Vec::new(),
                value_map: vec![missing; source.values.len()],
                payloads: vec![missing; source.flags.len()],
                conditions: vec![missing; source.flags.len()],
                stats: Stats::default(),
            },
            home: None,
            table: BTreeMap::new(),
        };
        for index in v::dependency_order(source)? {
            if index < source.values.len() {
                let value = &source.values[index];
                b.home = value.home;
                let op = match value.op {
                    v::ValueOp::Constant(n) => b.constant(n),
                    v::ValueOp::Parameter { block, field } => b.node(Op::Parameter {
                        block,
                        field: match field {
                            v::InputField::Gpr(n) => Parameter::Gpr(n),
                            v::InputField::OtherFlags => Parameter::OtherFlags,
                            v::InputField::DefinedFlags => Parameter::DefinedFlags,
                        },
                    }),
                    v::ValueOp::Binary { op, left, right } => b.binary(
                        op,
                        b.graph.value_map[left.index()],
                        b.graph.value_map[right.index()],
                    ),
                    v::ValueOp::Select { condition, yes, no } => b.select(
                        b.graph.value_map[condition.index()],
                        b.graph.value_map[yes.index()],
                        b.graph.value_map[no.index()],
                    ),
                    v::ValueOp::EvaluateFlag(flag) => b.graph.conditions[flag.index()],
                    v::ValueOp::EffectResult { effect, output } => {
                        b.node(Op::EffectResult { effect, output })
                    }
                    v::ValueOp::Alias(other) => b.graph.value_map[other.index()],
                };
                b.graph.value_map[index] = op;
            } else {
                let index = index - source.values.len();
                let flag = &source.flags[index];
                b.home = Some(flag.home);
                let payload = match flag.op {
                    v::FlagOp::Parameter { block, bit } => b.node(Op::Parameter {
                        block,
                        field: Parameter::Predicate(bit),
                    }),
                    v::FlagOp::Alias(other) => b.graph.payloads[other.index()],
                    v::FlagOp::Leaf { kind, width, args } => b.leaf(
                        flag.bit,
                        kind,
                        width,
                        args.map(|id| b.graph.value_map[id.index()]),
                    ),
                    v::FlagOp::Select { condition, yes, no } => b.select(
                        b.graph.value_map[condition.index()],
                        b.graph.payloads[yes.index()],
                        b.graph.payloads[no.index()],
                    ),
                };
                b.graph.payloads[index] = payload;
                b.graph.conditions[index] = b.condition(flag.bit, payload);
            }
        }
        b.graph.verify(source)?;
        Ok(b.graph)
    }
    pub fn verify(&self, source: &v::Graph) -> Result<(), v::Error> {
        let error = || v::Error("invalid predicate value graph".into());
        if self.value_map.len() != source.values.len()
            || self.payloads.len() != source.flags.len()
            || self.conditions.len() != source.flags.len()
        {
            return Err(error());
        }
        for (index, node) in self.values.iter().enumerate() {
            let operands = match node.op {
                Op::Binary { left, right, .. } => vec![left, right],
                Op::Select { condition, yes, no } => vec![condition, yes, no],
                Op::Popcnt(value) => vec![value],
                Op::Parameter { block, field } => {
                    if node.home != Some(block)
                        || block.index() >= source.blocks.len()
                        || matches!(field, Parameter::Predicate(bit) if bit >= 6)
                        || matches!(field, Parameter::Gpr(reg) if reg >= 8)
                    {
                        return Err(error());
                    }
                    vec![]
                }
                Op::EffectResult { effect, .. } => {
                    if source
                        .effects
                        .get(effect.index())
                        .is_none_or(|e| Some(e.block) != node.home)
                    {
                        return Err(error());
                    }
                    vec![]
                }
                Op::Constant(_) => {
                    if node.home.is_some() {
                        return Err(error());
                    }
                    vec![]
                }
            };
            for operand in operands {
                if operand.index() >= index {
                    return Err(error());
                }
                let home = self.values[operand.index()].home;
                if home.is_some() && home != node.home {
                    return Err(error());
                }
            }
        }
        for (source, target) in source
            .values
            .iter()
            .map(|n| n.home)
            .zip(&self.value_map)
            .chain(
                source
                    .flags
                    .iter()
                    .map(|n| Some(n.home))
                    .zip(&self.payloads),
            )
            .chain(
                source
                    .flags
                    .iter()
                    .map(|n| Some(n.home))
                    .zip(&self.conditions),
            )
        {
            if self
                .values
                .get(target.index())
                .is_none_or(|n| n.home.is_some() && n.home != source)
            {
                return Err(error());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
