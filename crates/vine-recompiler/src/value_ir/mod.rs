//! Bounded value/effect IR over an already decoded guest CFG.
//!
//! This module owns no memory, clocks, decoder, cache, or execution state.
//! Every guest block remains an admissible entry. In particular, optimizers
//! must not assume that a block parameter has only its known direct inputs.
//! Numeric values are i32 bit patterns with x86 wrapping operations. Flags
//! have a separate type: passing a flag across an edge does not evaluate it.

mod lift;
mod optimize;
mod verify;

use crate::ir;
use std::collections::BTreeMap;

pub use lift::lift;
pub use optimize::{Demand, OptimizationStats, demands, evaluate_leaf, fold_binary, optimize};
pub(crate) use verify::dependency_order;
pub use verify::verify;

macro_rules! id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u32);
        impl $name {
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}
id!(ValueId);
id!(FlagRef);
id!(BlockId);
id!(EffectId);
id!(SnapshotId);

pub const ARITHMETIC_MASK: u32 = 0x8d5;
pub const FLAG_MASKS: [u32; 6] = [1, 4, 16, 64, 128, 2048];
/// Per arithmetic flag: one tag and four numeric operands, never another
/// recipe. Equal producers can share their numeric SSA operands.
pub const MAXIMUM_FLAG_CARRIER_WORDS: usize = 6 * 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarWidth {
    W8,
    W16,
    W32,
}
impl ScalarWidth {
    pub const fn bits(self) -> u32 {
        match self {
            Self::W8 => 8,
            Self::W16 => 16,
            Self::W32 => 32,
        }
    }
    pub const fn bytes(self) -> u32 {
        self.bits() / 8
    }
    pub const fn mask(self) -> u32 {
        match self {
            Self::W8 => 0xff,
            Self::W16 => 0xffff,
            Self::W32 => u32::MAX,
        }
    }
    pub fn from_arch(width: ir::Width) -> Result<Self, Error> {
        match width {
            ir::Width::W8 => Ok(Self::W8),
            ir::Width::W16 => Ok(Self::W16),
            ir::Width::W32 => Ok(Self::W32),
            _ => Err(Error("non-scalar width in scalar value operation".into())),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InputField {
    Gpr(u8),
    OtherFlags,
    DefinedFlags,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    And,
    Or,
    Xor,
    /// Wasm/i32 shifts mask the count to five bits, exactly as these nodes do.
    Shl,
    ShrU,
    ShrS,
    Eq,
    Ne,
    LtU,
    LtS,
    LeU,
    LeS,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EffectOutput {
    Loaded,
    Gpr(u8),
    PackedFlags,
    DefinedFlags,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueOp {
    Constant(u32),
    Parameter {
        block: BlockId,
        field: InputField,
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
    /// This is the only numeric demand of a recipe. It is not inserted merely
    /// because a flag is carried across a block edge or saved in a snapshot.
    EvaluateFlag(FlagRef),
    EffectResult {
        effect: EffectId,
        output: EffectOutput,
    },
    Alias(ValueId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Value {
    /// Constants have no home. Other values are defined in this block; only
    /// block parameters carry values across an edge in the initial lowering.
    pub home: Option<BlockId>,
    pub op: ValueOp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FlagKind {
    /// args[0] is a boolean bit, not a packed EFLAGS word.
    Materialized,
    /// args = [left, right, carry-in, 0]. The result is computed only on demand.
    Add,
    Sub,
    /// args = [result, 0, 0, 0]. CF/OF are zero; AF is retained separately.
    Logic,
    /// args = [unshifted value, masked count, 0, 0]. Count zero selects the
    /// preceding leaf instead of constructing a zero-count shift leaf.
    Shl,
    Shr,
    Sar,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FlagOp {
    Parameter {
        block: BlockId,
        bit: u8,
    },
    Leaf {
        kind: FlagKind,
        width: ScalarWidth,
        args: [ValueId; 4],
    },
    /// Selection copies a leaf carrier; it never embeds either recipe inside
    /// the runtime result and therefore cannot grow a history chain.
    Select {
        condition: ValueId,
        yes: FlagRef,
        no: FlagRef,
    },
    Alias(FlagRef),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flag {
    pub home: BlockId,
    /// Index into FLAG_MASKS, carried statically rather than in the record.
    pub bit: u8,
    pub op: FlagOp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flags {
    pub arithmetic: [FlagRef; 6],
    /// All bits outside ARITHMETIC_MASK, including DF and reserved input bits.
    pub other: ValueId,
    pub defined: ValueId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CpuValues {
    pub gpr: [ValueId; 8],
    pub flags: Flags,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExitState {
    pub block: BlockId,
    pub cpu: CpuValues,
    pub eip: ValueId,
    /// Completed guest instructions since this block's budget admission.
    /// A fault uses the pre-instruction prefix; SMC uses the post-store prefix.
    pub completed_prefix: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    ReadWrite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Address {
    /// Guest effective offset before segment addition, never a host pointer.
    pub offset: ValueId,
    pub segment: ir::Segment,
    pub width: ScalarWidth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryEffects {
    None,
    Read,
    Write,
    ReadWrite,
    Dynamic,
}

pub mod banks {
    pub const X87: u8 = 1;
    pub const MMX: u8 = 2;
    pub const XMM: u8 = 4;
    pub const MXCSR: u8 = 8;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpaqueContract {
    /// Conservative masks are allowed; omission of an actual input is not.
    pub gpr_read: u8,
    pub gpr_written: u8,
    pub flags_read: u32,
    /// Raw output replacement mask. Conditional count-based adapters can
    /// replace undefined raw bits as well; their DefinedFlags output supplies
    /// architectural validity separately, without claiming those bits defined.
    pub flags_written: u32,
    pub flags_undefined: u32,
    pub banks_read: u8,
    pub banks_written: u8,
    pub memory: MemoryEffects,
    pub may_exit: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffectKind {
    /// ReadWrite performs the whole RMW permission check before reading and
    /// retains a proof for its following store. Its faults have W/R=1.
    Load { address: Address, access: Access },
    /// checked_by refers to a preceding ReadWrite load of this instruction.
    /// Capacity, all bytes, alias/SMC and commit remain one precise store.
    Store {
        address: Address,
        value: ValueId,
        checked_by: Option<EffectId>,
    },
    /// Existing precise complex lowering, with explicit inputs/outputs. This
    /// is not a fallback to another CPU or a per-block host dispatcher.
    Opaque {
        instruction: ir::Instruction,
        inputs: CpuValues,
        contract: OpaqueContract,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effect {
    pub block: BlockId,
    pub previous: Option<EffectId>,
    /// A zero-count shift still executes its unguarded ReadWrite load check,
    /// but its store/capacity/dirty effects are guarded by count != 0.
    pub guard: Option<ValueId>,
    pub before: SnapshotId,
    pub after: SnapshotId,
    pub kind: EffectKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// Kept for exact guest retirement, unsupported diagnostics and adapters.
    /// Ordinary scalar lowering consumes the graph, not this opcode list.
    pub source: ir::Instruction,
    pub before: SnapshotId,
    pub after: SnapshotId,
    pub effects: Vec<EffectId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Block(BlockId),
    External(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Terminator {
    Direct {
        target: Target,
        state: SnapshotId,
    },
    Conditional {
        condition: ValueId,
        taken: Target,
        fallthrough: Target,
        state: SnapshotId,
    },
    /// The state-join gate packs the existing internal ABI and performs the
    /// unrestricted PC lookup. It is an optimization boundary, not a guessed
    /// C calling convention or an all-to-all static CFG expansion.
    Indirect {
        target: ValueId,
        state: SnapshotId,
    },
    /// Unsupported/UD2/RDTSC or a terminal REP adapter uses its precise exit.
    Stop {
        state: SnapshotId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub id: BlockId,
    pub start: u32,
    pub parameters: CpuValues,
    pub steps: Vec<Step>,
    pub terminator: Terminator,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Graph {
    pub entry: BlockId,
    /// Every admitted old Region block remains externally/indirectly enterable.
    pub entries: BTreeMap<u32, BlockId>,
    pub blocks: Vec<Block>,
    pub values: Vec<Value>,
    pub flags: Vec<Flag>,
    pub effects: Vec<Effect>,
    pub snapshots: Vec<ExitState>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for Error {}

#[cfg(test)]
mod tests;
