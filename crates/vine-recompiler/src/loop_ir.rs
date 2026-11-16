//! Loop-scoped memory effects. Expressions describe wrapping entry-register
//! combinations plus conservative integer intervals, never guest host pointers.
//! A forward conditional DAG returns through one latch. Its memory proof is
//! usable only for bounded, fault-free iterations; the ordinary CFG owns misses.
use crate::ir::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Value {
    coefficients: [i64; 8],
    minimum: i64,
    maximum: i64,
}
impl Value {
    fn register(index: usize) -> Self {
        let mut coefficients = [0; 8];
        coefficients[index] = 1;
        Self {
            coefficients,
            minimum: 0,
            maximum: 0,
        }
    }
    fn bounded(maximum: u32) -> Self {
        Self {
            coefficients: [0; 8],
            minimum: 0,
            maximum: i64::from(maximum),
        }
    }
    fn unknown() -> Self {
        Self::bounded(u32::MAX)
    }
    fn constant(value: u32) -> Self {
        Self {
            coefficients: [0; 8],
            minimum: i64::from(value as i32),
            maximum: i64::from(value as i32),
        }
    }
    fn add(&self, rhs: &Self, scale: i64) -> Option<Self> {
        if scale < 0 {
            return None;
        }
        let mut coefficients = self.coefficients;
        for (left, right) in coefficients.iter_mut().zip(rhs.coefficients) {
            *left = left.checked_add(right.checked_mul(scale)?)?;
        }
        Some(Self {
            coefficients,
            minimum: self.minimum.checked_add(rhs.minimum.checked_mul(scale)?)?,
            maximum: self.maximum.checked_add(rhs.maximum.checked_mul(scale)?)?,
        })
    }
    fn merge(&self, rhs: &Self) -> Self {
        if self.coefficients != rhs.coefficients {
            return Self::unknown();
        }
        Self {
            coefficients: self.coefficients,
            minimum: self.minimum.min(rhs.minimum),
            maximum: self.maximum.max(rhs.maximum),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryRange {
    pub coefficients: [i64; 8],
    /// Inclusive byte offsets relative to the wrapping entry-register sum.
    pub minimum: i64,
    pub maximum: i64,
    pub stride: i32,
    pub permissions: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryUse {
    pub range: usize,
    pub width: Width,
    pub write: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loop {
    pub header: u32,
    pub latch: u32,
    pub exit: u32,
    pub body: Vec<u32>,
    /// Bounds every path, including the final branch. Actual retirement stays
    /// per-instruction, so a skipped arm never consumes imaginary work.
    pub maximum_instructions: u32,
    pub ranges: Vec<MemoryRange>,
    pub accesses: BTreeMap<u32, MemoryUse>,
    pub has_mmx: bool,
}

fn address(memory: MemoryOperand, values: &[Value; 8]) -> Option<Value> {
    let mut result = Value::constant(memory.displacement);
    if let Some(base) = memory.base {
        result = result.add(&values[base as usize], 1)?;
    }
    if let Some(index) = memory.index {
        result = result.add(&values[index as usize], i64::from(memory.scale))?;
    }
    Some(result)
}
fn operand(value: Operand, values: &[Value; 8]) -> Value {
    match value {
        Operand::Imm(value) => Value::constant(value),
        Operand::Reg(r) if r.width == Width::W32 && !r.high8 => values[r.index as usize].clone(),
        Operand::Reg(r) => Value::bounded(match r.width {
            Width::W8 => 255,
            Width::W16 => 65535,
            _ => u32::MAX,
        }),
        Operand::Mem(m) => Value::bounded(match m.width {
            Width::W8 => 255,
            Width::W16 => 65535,
            _ => u32::MAX,
        }),
        _ => Value::unknown(),
    }
}
fn update(i: &Instruction, values: &mut [Value; 8]) {
    let Some(Operand::Reg(destination)) = i.operands.first().copied() else {
        return;
    };
    if crate::optimize::written_gprs(i) & (1 << destination.index) == 0 {
        return;
    }
    let old = values[destination.index as usize].clone();
    let next = if destination.width != Width::W32 || destination.high8 {
        Value::unknown()
    } else {
        match i.opcode {
            Opcode::Mov | Opcode::Movzx => operand(i.operands[1], values),
            Opcode::Lea => match i.operands[1] {
                Operand::Mem(m) => address(m, values).unwrap_or_else(Value::unknown),
                _ => Value::unknown(),
            },
            Opcode::Inc => old
                .add(&Value::constant(1), 1)
                .unwrap_or_else(Value::unknown),
            Opcode::Dec => old
                .add(&Value::constant(u32::MAX), 1)
                .unwrap_or_else(Value::unknown),
            Opcode::Add | Opcode::Sub => match i.operands[1] {
                Operand::Imm(n) => old
                    .add(
                        &Value::constant(if i.opcode == Opcode::Sub {
                            n.wrapping_neg()
                        } else {
                            n
                        }),
                        1,
                    )
                    .unwrap_or_else(Value::unknown),
                _ => Value::unknown(),
            },
            Opcode::Shr => match i.operands[1] {
                Operand::Imm(n) if n & 31 != 0 => Value::bounded(u32::MAX >> (n & 31)),
                Operand::Imm(_) => old,
                _ => Value::unknown(),
            },
            Opcode::And => match i.operands[1] {
                Operand::Imm(n) => Value::bounded(n),
                _ => Value::unknown(),
            },
            _ => Value::unknown(),
        }
    };
    values[destination.index as usize] = next;
}
fn allowed(i: &Instruction) -> bool {
    // This describes which existing operations are fault-free after memory and
    // pending-MMX preflight. Division, x87/SSE numerical exceptions, implicit
    // stack accesses, Runtime calls and unknown control flow keep ordinary code.
    matches!(
        i.opcode,
        Opcode::Nop
            | Opcode::Mov
            | Opcode::Movzx
            | Opcode::Movsx
            | Opcode::Lea
            | Opcode::Add
            | Opcode::Sub
            | Opcode::Adc
            | Opcode::Sbb
            | Opcode::Inc
            | Opcode::Dec
            | Opcode::Cmp
            | Opcode::Test
            | Opcode::And
            | Opcode::Or
            | Opcode::Xor
            | Opcode::Neg
            | Opcode::Not
            | Opcode::Shl
            | Opcode::Shr
            | Opcode::Sar
            | Opcode::Imul
            | Opcode::Jcc
            | Opcode::Jmp
            | Opcode::Mmx(_)
    ) && i.unsupported.is_none()
}

fn analyze(region: &Region, header: u32, latch: u32) -> Option<Loop> {
    let last = region.blocks.get(&latch)?.instructions.last()?;
    if last.opcode != Opcode::Jcc || last.operands.first() != Some(&Operand::Imm(header)) {
        return None;
    }
    let exit = last.next_eip();
    if exit <= latch {
        return None;
    }
    let mut pending = vec![header];
    let mut body = BTreeSet::new();
    while let Some(pc) = pending.pop() {
        if !body.insert(pc) {
            continue;
        }
        let block = region.blocks.get(&pc)?;
        if block.instructions.is_empty() || block.instructions.iter().any(|i| !allowed(i)) {
            return None;
        }
        for &next in &block.successors {
            if pc == latch && (next == header || next == exit) {
                continue;
            }
            if next <= pc || next > latch {
                return None;
            }
            pending.push(next);
        }
        if pc != latch && block.successors.is_empty() {
            return None;
        }
    }
    if !body.contains(&latch) {
        return None;
    }
    let maximum_instructions: usize = body
        .iter()
        .map(|pc| region.blocks[pc].instructions.len())
        .sum();
    if !(2..=256).contains(&maximum_instructions) {
        return None;
    }
    let mut incoming = BTreeMap::from([(header, std::array::from_fn(Value::register))]);
    let mut uses = Vec::new();
    let mut final_values = None;
    let mut has_mmx = false;
    for &pc in &body {
        let mut values = incoming.remove(&pc)?;
        for i in &region.blocks[&pc].instructions {
            has_mmx |= matches!(i.opcode, Opcode::Mmx(_));
            if i.opcode != Opcode::Lea {
                for (index, op) in i.operands.iter().enumerate() {
                    let Operand::Mem(m) = *op else {
                        continue;
                    };
                    if m.segment == Segment::Fs || m.width.bytes() > 8 {
                        return None;
                    }
                    // Conditional stores are permitted; RMW memory destinations
                    // are outside this first proof because their exception and
                    // conditional commit effects need a distinct memory effect.
                    let write = index == 0 && !matches!(i.opcode, Opcode::Cmp | Opcode::Test);
                    if write
                        && !matches!(
                            i.opcode,
                            Opcode::Mov
                                | Opcode::Mmx(
                                    MmxOpcode::Movd | MmxOpcode::Movq | MmxOpcode::Movntq
                                )
                        )
                    {
                        return None;
                    }
                    let mut expression = address(m, &values)?;
                    expression.maximum = expression
                        .maximum
                        .checked_add(i64::from(m.width.bytes()) - 1)?;
                    uses.push((i.eip, expression, m.width, write));
                }
            }
            update(i, &mut values);
        }
        if pc == latch {
            final_values = Some(values.clone());
        }
        for &next in &region.blocks[&pc].successors {
            if next == header || next == exit {
                continue;
            }
            incoming
                .entry(next)
                .and_modify(|old| {
                    for (left, right) in old.iter_mut().zip(&values) {
                        *left = left.merge(right);
                    }
                })
                .or_insert_with(|| values.clone());
        }
    }
    let final_values = final_values?;
    let mut ranges: Vec<MemoryRange> = Vec::new();
    let mut accesses = BTreeMap::new();
    for (eip, expression, width, write) in uses {
        let mut stride = 0_i64;
        for (index, coefficient) in expression.coefficients.iter().copied().enumerate() {
            if coefficient == 0 {
                continue;
            }
            let value = &final_values[index];
            if value.coefficients != Value::register(index).coefficients
                || value.minimum != value.maximum
            {
                return None;
            }
            stride = stride.checked_add(coefficient.checked_mul(value.minimum)?)?;
        }
        let stride = i32::try_from(stride).ok()?;
        if stride.unsigned_abs() > 4096 {
            return None;
        }
        let range = ranges
            .iter()
            .position(|r| r.coefficients == expression.coefficients)
            .unwrap_or_else(|| {
                ranges.push(MemoryRange {
                    coefficients: expression.coefficients,
                    minimum: expression.minimum,
                    maximum: expression.maximum,
                    stride,
                    permissions: 0,
                });
                ranges.len() - 1
            });
        let r = &mut ranges[range];
        r.minimum = r.minimum.min(expression.minimum);
        r.maximum = r.maximum.max(expression.maximum);
        r.permissions |= if write { 2 } else { 1 };
        if r.maximum.checked_sub(r.minimum)? >= 4096 || r.stride != stride {
            return None;
        }
        if accesses
            .insert(
                eip,
                MemoryUse {
                    range,
                    width,
                    write,
                },
            )
            .is_some()
        {
            return None;
        }
    }
    if ranges.is_empty() || ranges.len() > 8 {
        return None;
    }
    Some(Loop {
        header,
        latch,
        exit,
        body: body.into_iter().collect(),
        maximum_instructions: maximum_instructions as u32,
        ranges,
        accesses,
        has_mmx,
    })
}

pub fn discover(region: &Region) -> BTreeMap<u32, Loop> {
    let mut latches = BTreeMap::<u32, Vec<u32>>::new();
    for (&pc, block) in &region.blocks {
        if let Some(last) = block.instructions.last()
            && last.opcode == Opcode::Jcc
            && let Some(&Operand::Imm(header)) = last.operands.first()
            && header <= pc
            && region.blocks.contains_key(&header)
        {
            latches.entry(header).or_default().push(pc);
        }
    }
    latches
        .into_iter()
        .filter_map(|(header, ends)| {
            (ends.len() == 1)
                .then(|| analyze(region, header, ends[0]))
                .flatten()
                .map(|p| (header, p))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Code(Vec<u8>);
    impl crate::decode::CodeSource for Code {
        fn read_code(&self, address: u32, max_len: usize) -> Vec<u8> {
            self.0
                .get(address.wrapping_sub(0x1000) as usize..)
                .unwrap_or_default()
                .iter()
                .copied()
                .take(max_len)
                .collect()
        }
    }
    fn plan(mut body: Vec<u8>) -> BTreeMap<u32, Loop> {
        body.extend([0x49, 0x75, (-(body.len() as i32 + 3)) as u8, 0xcc]);
        discover(&crate::decode::discover(&Code(body), 0x1000, 128))
    }
    #[test]
    fn tracks_decreasing_pointer_and_exact_byte_span() {
        let loops = plan(vec![0x8b, 0x06, 0x89, 0x07, 0x83, 0xee, 4, 0x83, 0xef, 4]);
        let ranges = &loops[&0x1000].ranges;
        assert_eq!(ranges.len(), 2);
        for range in ranges {
            assert_eq!((range.minimum, range.maximum, range.stride), (0, 3, -4));
        }
    }
    #[test]
    fn widened_or_partial_indices_cannot_certify_a_narrow_page_range() {
        // Load arbitrary EAX; using it as an index cannot be proved page-local.
        assert!(plan(vec![0x8b, 0x06, 0x8b, 0x04, 0x87]).is_empty());
        // A later high-byte write destroys the earlier SHR-derived bound.
        assert!(
            plan(vec![
                0x8b, 0x06, 0xc1, 0xe8, 25, 0xb4, 0xff, 0x8b, 0x04, 0x87
            ])
            .is_empty()
        );
        assert!(!plan(vec![0x8b, 0x06, 0xc1, 0xe8, 25, 0x8b, 0x04, 0x87]).is_empty());
    }
    #[test]
    fn conditional_pointer_progress_does_not_claim_a_single_stride() {
        // mov eax,[esi]; test eax,eax; jz join; add esi,4; join: dec ecx; jnz
        assert!(plan(vec![0x8b, 0x06, 0x85, 0xc0, 0x74, 3, 0x83, 0xc6, 4]).is_empty());
    }
    #[test]
    fn unproved_faults_and_effects_remain_in_ordinary_code() {
        for code in [
            vec![0xff, 0x06],
            vec![0x64, 0x8b, 0x06],
            vec![0x8b, 0x06, 0xf7, 0xf3],
            vec![0xd9, 0x06],
            vec![0xa5],
            vec![0x8b, 0x06, 0x50],
            vec![0x8b, 0x06, 0x0f, 0x31],
        ] {
            assert!(plan(code).is_empty());
        }
    }
}
