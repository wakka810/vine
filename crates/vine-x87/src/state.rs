use crate::{
    Arithmetic, BUSY, C1, DENORMAL, EXCEPTION_SUMMARY, Ext80, Format, INVALID, NumericResult,
    OVERFLOW, PRECISION, STACK_FAULT, UNDERFLOW, Unsupported, ZERO_DIVIDE, arithmetic, compare,
    load, store,
};

/// Physical registers and full architectural environment. Layout matches the
/// backend interchange ABI; no lazy flags or compiler temporary is included.
#[repr(C)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State {
    pub registers: [[u32; 3]; 8],
    pub control: u32,
    pub status: u32,
    pub tag: u32,
    pub opcode: u32,
    pub instruction_pointer: u32,
    pub instruction_selector: u32,
    pub data_pointer: u32,
    pub data_selector: u32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            registers: [[0; 3]; 8],
            control: 0x37f,
            status: 0,
            tag: 0xffff,
            opcode: 0,
            instruction_pointer: 0,
            instruction_selector: 0,
            data_pointer: 0,
            data_selector: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Location {
    pub instruction_pointer: u32,
    pub instruction_selector: u16,
    pub opcode: u16,
    pub data: Option<(u32, u16)>,
}

#[derive(Clone, Copy, Debug)]
pub enum Operand {
    Register(u8),
    /// Memory has already passed the backend's complete read preflight.
    Memory {
        format: Format,
        bits: u64,
    },
    /// FLD m80 is an exact ten-byte copy, including unsupported encodings.
    ExtendedMemory(Ext80),
    Constant(Ext80),
}

#[derive(Clone, Copy, Debug)]
pub enum Instruction {
    Load(Operand),
    StoreMemory {
        format: Format,
        pop: bool,
    },
    StoreRegister {
        index: u8,
        pop: bool,
    },
    Arithmetic {
        operation: Arithmetic,
        left: Operand,
        right: Operand,
        destination: u8,
        pop: bool,
    },
    /// One combined operation: ST(1) * log2(ST(0)), then pop on completion.
    Log2Multiply,
    /// Quadrant-aware atan2(ST(1), ST(0)), then pop on completion.
    Arctangent,
    /// FCOS replaces ST0, does not pop, and reports finite range via C2.
    Cosine,
    /// FSIN additionally supports exponent-biased unmasked underflow results.
    Sine,
    ExponentialMinusOne,
    Scale,
    Compare {
        right: Operand,
        quiet: bool,
        pops: u8,
    },
    Exchange(u8),
    ChangeSign,
    Absolute,
    Free(u8),
    LoadControl(u16),
    /// FNCLEX and FNINIT are explicitly non-waiting forms.
    ClearExceptions,
    Initialize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Trap {
    PendingException(u16),
    Unsupported(Unsupported),
}

impl From<Unsupported> for Trap {
    fn from(value: Unsupported) -> Self {
        Self::Unsupported(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoreValue {
    pub bytes: [u8; 10],
    pub length: usize,
}

/// A prepared instruction does not mutate live state or guest memory. Admit a
/// waiting instruction with `check_waiting`, then preflight the complete memory
/// access, prepare the operation, write these bytes and replace live state. A
/// fault discards this object; no stack pop occurred.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub state: State,
    pub store: Option<StoreValue>,
    pub deferred_exception: bool,
}

impl State {
    pub fn top(&self) -> usize {
        (self.status as usize >> 11) & 7
    }
    pub fn physical_index(&self, logical: u8) -> Result<usize, Unsupported> {
        if logical < 8 {
            Ok((self.top() + usize::from(logical)) & 7)
        } else {
            Err(Unsupported::InvalidRegister)
        }
    }
    pub fn read(&self, logical: u8) -> Result<Option<Ext80>, Unsupported> {
        let index = self.physical_index(logical)?;
        Ok(if self.tag >> (index * 2) & 3 == 3 {
            None
        } else {
            Some(Ext80::from_words(self.registers[index]))
        })
    }
    fn write(&mut self, logical: u8, value: Ext80) -> Result<(), Unsupported> {
        let index = self.physical_index(logical)?;
        self.registers[index] = value.words();
        self.tag = self.tag & !(3 << (index * 2)) | u32::from(value.tag()) << (index * 2);
        Ok(())
    }
    fn set_top(&mut self, top: usize) {
        self.status = self.status & !(7 << 11) | ((top as u32 & 7) << 11);
    }
    fn pop(&mut self) {
        self.tag |= 3 << (self.top() * 2);
        self.set_top((self.top() + 1) & 7);
    }
    pub fn pending_exceptions(&self) -> u16 {
        (self.status & !self.control & 0x3f) as u16
    }
    /// Admission for a waiting x87 instruction, before its memory operand is
    /// accessed. The backend must perform this check before segment or page
    /// preflight; non-waiting FN instructions bypass it.
    pub fn check_waiting(&self) -> Result<(), Trap> {
        let pending = self.pending_exceptions();
        if pending == 0 {
            Ok(())
        } else {
            Err(Trap::PendingException(pending))
        }
    }
    fn set_summary(&mut self) {
        let mask = u32::from(EXCEPTION_SUMMARY | BUSY);
        self.status = if self.pending_exceptions() != 0 {
            self.status | mask
        } else {
            self.status & !mask
        };
    }
    fn location(&mut self, location: Location) {
        self.instruction_pointer = location.instruction_pointer;
        self.instruction_selector = u32::from(location.instruction_selector);
        self.opcode = u32::from(location.opcode & 0x7ff);
        if let Some((pointer, selector)) = location.data {
            self.data_pointer = pointer;
            self.data_selector = u32::from(selector);
        }
    }
    fn operand(&self, operand: Operand) -> Result<(NumericResult, bool), Unsupported> {
        Ok(match operand {
            Operand::Register(index) => match self.read(index)? {
                Some(value) => (
                    NumericResult {
                        value,
                        ..NumericResult::default()
                    },
                    false,
                ),
                None => (
                    NumericResult {
                        value: Ext80::INDEFINITE,
                        exceptions: INVALID | STACK_FAULT,
                        ..NumericResult::default()
                    },
                    true,
                ),
            },
            Operand::Memory { format, bits } => (load(format, bits, self.control as u16)?, false),
            Operand::ExtendedMemory(value) | Operand::Constant(value) => (
                NumericResult {
                    value,
                    ..NumericResult::default()
                },
                false,
            ),
        })
    }
    fn arithmetic_operand(&self, operand: Operand) -> Result<(NumericResult, bool), Unsupported> {
        let (mut result, empty) = self.operand(operand)?;
        // A memory arithmetic operand is widened exactly, not first processed
        // as a separate FLD. Retain signalingness for x87 NaN selection.
        if let Operand::Memory { format, bits } = operand {
            let signaling = match format {
                Format::Float32 => {
                    bits as u32 & 0x7f80_0000 == 0x7f80_0000
                        && bits as u32 & 0x007f_ffff != 0
                        && bits as u32 & 0x0040_0000 == 0
                }
                Format::Float64 => {
                    bits & 0x7ff0_0000_0000_0000 == 0x7ff0_0000_0000_0000
                        && bits & 0x000f_ffff_ffff_ffff != 0
                        && bits & 0x0008_0000_0000_0000 == 0
                }
                _ => false,
            };
            if signaling {
                result.value.significand &= !(1 << 62);
            }
        }
        Ok((result, empty))
    }
    /// Returns whether the destination/stack may commit. Unsupported unmasked
    /// OE/UE register results require wrapped exponents and stop explicitly.
    fn apply_exceptions(
        &mut self,
        result: NumericResult,
        stack_overflow: bool,
        memory_store: bool,
    ) -> Result<bool, Unsupported> {
        self.apply_exceptions_with_adjusted_range(result, stack_overflow, memory_store, 0)
    }

    /// Only a numeric operation that has already prepared the exponent-biased
    /// register value may admit those range exceptions. Other operations retain their
    /// explicit unsupported boundary; memory stores keep suppressing writes.
    fn apply_exceptions_with_adjusted_range(
        &mut self,
        result: NumericResult,
        stack_overflow: bool,
        memory_store: bool,
        adjusted_range: u16,
    ) -> Result<bool, Unsupported> {
        let mut flags = result.exceptions;
        let unmasked = flags & !(self.control as u16) & 0x3f;
        // Pre-computation exceptions suppress subsequent result exceptions.
        if unmasked & INVALID != 0 {
            flags &= INVALID | STACK_FAULT;
        } else if unmasked & DENORMAL != 0 {
            flags &= DENORMAL;
        } else if unmasked & ZERO_DIVIDE != 0 {
            flags &= ZERO_DIVIDE;
        }
        let unmasked = flags & !(self.control as u16) & 0x3f;
        let unsupported_range = (OVERFLOW | UNDERFLOW) & !adjusted_range;
        if unmasked & unsupported_range != 0 && !memory_store {
            return Err(Unsupported::UnmaskedRangeException);
        }
        if memory_store && unmasked & (OVERFLOW | UNDERFLOW) != 0 {
            flags &= !PRECISION;
        }
        self.status = self.status & !u32::from(C1) | u32::from(flags);
        if flags & STACK_FAULT != 0 && stack_overflow
            || result.round_up
                && unmasked & (INVALID | DENORMAL | ZERO_DIVIDE | unsupported_range) == 0
        {
            self.status |= u32::from(C1);
        }
        self.set_summary();
        Ok(unmasked & (INVALID | DENORMAL | ZERO_DIVIDE) == 0
            && !(memory_store && unmasked & (OVERFLOW | UNDERFLOW) != 0))
    }

    pub fn prepare(&self, instruction: Instruction, location: Location) -> Result<Prepared, Trap> {
        let non_waiting = matches!(
            instruction,
            Instruction::ClearExceptions | Instruction::Initialize
        );
        if !non_waiting {
            self.check_waiting()?;
        }
        let mut next = self.clone();
        let mut output = None;
        if !matches!(
            instruction,
            Instruction::LoadControl(_) | Instruction::ClearExceptions | Instruction::Initialize
        ) {
            next.location(location);
        }
        match instruction {
            Instruction::Load(operand) => {
                let (mut result, _) = self.operand(operand)?;
                let push_index = (self.top() + 7) & 7;
                let overflow = self.tag >> (push_index * 2) & 3 != 3;
                if overflow {
                    result = NumericResult {
                        value: Ext80::INDEFINITE,
                        exceptions: INVALID | STACK_FAULT,
                        ..NumericResult::default()
                    };
                }
                // Register and m80 loads retain all raw bits; f32/f64 load
                // conversion reports signaling NaN and denormal exceptions.
                let commit = next.apply_exceptions(result, overflow, false)?;
                // FLD's documented exception to the usual pre-computation
                // rule: even unmasked DE loads and pushes the converted value.
                if commit || result.exceptions == DENORMAL {
                    next.set_top(push_index);
                    next.write(0, result.value)?;
                }
            }
            Instruction::StoreMemory { format, pop } => {
                let (operand, empty) = self.operand(Operand::Register(0))?;
                let mut result = store(format, operand.value, self.control as u16)?;
                if empty {
                    result.exceptions = INVALID | STACK_FAULT;
                    result.round_up = false;
                }
                if next.apply_exceptions(result, false, true)? {
                    let mut bytes = [0u8; 10];
                    let length = match format {
                        Format::Float32 | Format::Integer32 => 4,
                        Format::Float64 | Format::Integer64 => 8,
                        Format::Integer16 => 2,
                        Format::Float80 => 10,
                    };
                    if format == Format::Float80 {
                        bytes[..8].copy_from_slice(&operand.value.significand.to_le_bytes());
                        bytes[8..].copy_from_slice(&operand.value.sign_exponent.to_le_bytes());
                    } else {
                        bytes[..length].copy_from_slice(&result.bits.to_le_bytes()[..length]);
                    }
                    output = Some(StoreValue { bytes, length });
                    if pop {
                        next.pop();
                    }
                }
            }
            Instruction::StoreRegister { index, pop } => {
                self.physical_index(index)?;
                let (result, _) = self.operand(Operand::Register(0))?;
                if next.apply_exceptions(result, false, false)? {
                    next.write(index, result.value)?;
                    if pop {
                        next.pop();
                    }
                }
            }
            Instruction::Arithmetic {
                operation,
                left,
                right,
                destination,
                pop,
            } => {
                self.physical_index(destination)?;
                let (a, empty_a) = self.arithmetic_operand(left)?;
                let (b, empty_b) = if matches!(
                    operation,
                    Arithmetic::SquareRoot | Arithmetic::RoundToInteger
                ) {
                    (NumericResult::default(), false)
                } else {
                    self.arithmetic_operand(right)?
                };
                let mut result = if empty_a || empty_b {
                    NumericResult {
                        value: Ext80::INDEFINITE,
                        exceptions: INVALID | STACK_FAULT,
                        ..NumericResult::default()
                    }
                } else {
                    arithmetic(operation, a.value, b.value, self.control as u16)?
                };
                if !empty_a && !empty_b && (a.exceptions | b.exceptions) & INVALID != 0 {
                    result.exceptions = INVALID;
                    result.round_up = false;
                } else if !empty_a
                    && !empty_b
                    && result.exceptions & (INVALID | ZERO_DIVIDE) == 0
                    && !a.value.is_nan()
                    && !b.value.is_nan()
                {
                    result.exceptions |= a.exceptions | b.exceptions;
                }
                if next.apply_exceptions(result, false, false)? {
                    next.write(destination, result.value)?;
                    if pop {
                        next.pop();
                    }
                }
            }
            Instruction::Log2Multiply | Instruction::Arctangent => {
                let (x, empty_x) = self.operand(Operand::Register(0))?;
                let (y, empty_y) = self.operand(Operand::Register(1))?;
                // Stack faults have priority over numeric classification and
                // any unsupported rounding case in the transcendental core.
                let result = if empty_x || empty_y {
                    NumericResult {
                        value: Ext80::INDEFINITE,
                        exceptions: INVALID | STACK_FAULT,
                        ..NumericResult::default()
                    }
                } else if matches!(instruction, Instruction::Arctangent) {
                    crate::arctangent(x.value, y.value, self.control as u16)?
                } else {
                    crate::log2_multiply(x.value, y.value, self.control as u16)?
                };
                if next.apply_exceptions(result, false, false)? {
                    next.write(1, result.value)?;
                    next.pop();
                }
            }
            Instruction::Cosine | Instruction::Sine => {
                let (operand, empty) = self.operand(Operand::Register(0))?;
                let result = if empty {
                    NumericResult {
                        value: Ext80::INDEFINITE,
                        exceptions: INVALID | STACK_FAULT,
                        ..NumericResult::default()
                    }
                } else if matches!(instruction, Instruction::Sine) {
                    crate::sine(operand.value, self.control as u16)?
                } else {
                    crate::cosine(operand.value, self.control as u16)?
                };
                if result.bits == 1 {
                    // C1 is undefined for a range failure; choose zero in
                    // both implementations while preserving C0/C3.
                    next.status = (next.status & !0x200) | 0x400;
                } else {
                    next.status &= !0x400;
                    if next.apply_exceptions_with_adjusted_range(
                        result,
                        false,
                        false,
                        if matches!(instruction, Instruction::Sine) {
                            UNDERFLOW
                        } else {
                            0
                        },
                    )? {
                        next.write(0, result.value)?;
                    }
                }
            }
            Instruction::ExponentialMinusOne | Instruction::Scale => {
                let (x, empty_x) = self.operand(Operand::Register(0))?;
                let is_scale = matches!(instruction, Instruction::Scale);
                let (y, empty_y) = if is_scale {
                    self.operand(Operand::Register(1))?
                } else {
                    (NumericResult::default(), false)
                };
                let result = if empty_x || empty_y {
                    NumericResult {
                        value: Ext80::INDEFINITE,
                        exceptions: INVALID | STACK_FAULT,
                        ..NumericResult::default()
                    }
                } else if is_scale {
                    crate::scale(x.value, y.value, self.control as u16)?
                } else {
                    crate::exponential_minus_one(x.value, self.control as u16)?
                };
                if next.apply_exceptions_with_adjusted_range(
                    result,
                    false,
                    false,
                    if is_scale {
                        OVERFLOW | UNDERFLOW
                    } else {
                        UNDERFLOW
                    },
                )? {
                    next.write(0, result.value)?;
                }
            }
            Instruction::Compare { right, quiet, pops } => {
                if pops > 2 {
                    return Err(Unsupported::InvalidRegister.into());
                }
                let (a, empty_a) = self.operand(Operand::Register(0))?;
                let (b, empty_b) = self.arithmetic_operand(right)?;
                let mut result = compare(a.value, b.value, quiet);
                if empty_a || empty_b {
                    result.bits = 3;
                    result.exceptions = INVALID | STACK_FAULT;
                } else if (a.exceptions | b.exceptions) & INVALID != 0 {
                    result.exceptions = INVALID;
                } else if result.exceptions & INVALID == 0 && !a.value.is_nan() && !b.value.is_nan()
                {
                    result.exceptions |= a.exceptions | b.exceptions;
                }
                if next.apply_exceptions(result, false, false)? {
                    let conditions = match result.bits {
                        0 => 0x4000,
                        1 => 0x100,
                        2 => 0,
                        _ => 0x4500,
                    };
                    next.status = next.status & !0x4500 | conditions;
                    for _ in 0..pops {
                        next.pop();
                    }
                }
            }
            Instruction::Exchange(index) => {
                let (a, empty_a) = self.operand(Operand::Register(0))?;
                let (b, empty_b) = self.operand(Operand::Register(index))?;
                let result = NumericResult {
                    exceptions: if empty_a || empty_b {
                        INVALID | STACK_FAULT
                    } else {
                        0
                    },
                    ..NumericResult::default()
                };
                if next.apply_exceptions(result, false, false)? {
                    next.write(0, b.value)?;
                    next.write(index, a.value)?;
                }
            }
            Instruction::ChangeSign | Instruction::Absolute => {
                let (mut result, empty) = self.operand(Operand::Register(0))?;
                if !empty {
                    result.value.sign_exponent = if matches!(instruction, Instruction::Absolute) {
                        result.value.sign_exponent & 0x7fff
                    } else {
                        result.value.sign_exponent ^ 0x8000
                    };
                }
                if next.apply_exceptions(result, false, false)? {
                    next.write(0, result.value)?;
                }
            }
            Instruction::Free(index) => {
                let index = self.physical_index(index)?;
                next.tag |= 3 << (index * 2);
            }
            Instruction::LoadControl(control) => {
                next.control = u32::from(control);
                next.set_summary();
            }
            Instruction::ClearExceptions => {
                next.status &= !0x80ff;
            }
            Instruction::Initialize => {
                next = Self {
                    registers: self.registers,
                    ..Self::default()
                };
            }
        }
        Ok(Prepared {
            deferred_exception: next.pending_exceptions() != 0,
            state: next,
            store: output,
        })
    }
}

#[cfg(test)]
mod log2_multiply_tests {
    use super::*;

    #[test]
    fn empty_operand_precedes_numeric_work_and_pop_obeys_invalid_mask() {
        for (instruction, opcode) in [
            (Instruction::Log2Multiply, 0x1f1),
            (Instruction::Arctangent, 0x1f3),
        ] {
            for top in 0..8 {
                for missing in 0..2 {
                    for masked in [false, true] {
                        let mut state = State {
                            control: if masked { 0x37f } else { 0x37e },
                            status: (top << 11) | u32::from(C1),
                            data_pointer: 0x1234,
                            data_selector: 0x2b,
                            ..State::default()
                        };
                        state.write(0, Ext80::ONE).unwrap();
                        state.write(1, Ext80::ONE).unwrap();
                        let missing_index = (top + missing) & 7;
                        state.tag |= 3 << (missing_index * 2);
                        let before = state.clone();
                        let prepared = state
                            .prepare(
                                instruction,
                                Location {
                                    instruction_pointer: 0x4321,
                                    instruction_selector: 0x23,
                                    opcode,
                                    data: None,
                                },
                            )
                            .unwrap();
                        assert_eq!(state, before);
                        assert_eq!(prepared.state.status & 0x23f, u32::from(INVALID));
                        assert_eq!(
                            prepared.state.status & u32::from(STACK_FAULT),
                            u32::from(STACK_FAULT)
                        );
                        assert_eq!(prepared.state.instruction_pointer, 0x4321);
                        assert_eq!(prepared.state.opcode, u32::from(opcode));
                        assert_eq!(prepared.state.data_pointer, 0x1234);
                        assert_eq!(prepared.state.data_selector, 0x2b);
                        assert_eq!(prepared.deferred_exception, !masked);
                        assert!(prepared.store.is_none());
                        if masked {
                            assert_eq!(prepared.state.top(), ((top + 1) & 7) as usize);
                            assert_eq!(prepared.state.read(0).unwrap(), Some(Ext80::INDEFINITE));
                            assert_eq!(prepared.state.tag >> (top * 2) & 3, 3);
                            for index in 0..8 {
                                if index != ((top + 1) & 7) as usize {
                                    assert_eq!(
                                        prepared.state.registers[index],
                                        before.registers[index]
                                    );
                                }
                            }
                        } else {
                            assert_eq!(prepared.state.top(), top as usize);
                            assert_eq!(prepared.state.tag, before.tag);
                            assert_eq!(prepared.state.registers, before.registers);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn pending_exception_does_not_publish_location_or_pop() {
        for instruction in [Instruction::Log2Multiply, Instruction::Arctangent] {
            let state = State {
                control: 0x35f,
                status: u32::from(PRECISION | EXCEPTION_SUMMARY | BUSY),
                instruction_pointer: 0x1234,
                ..State::default()
            };
            let before = state.clone();
            assert!(matches!(
                state.prepare(
                    instruction,
                    Location {
                        instruction_pointer: 0x5678,
                        opcode: 0x1f1,
                        ..Location::default()
                    }
                ),
                Err(Trap::PendingException(PRECISION))
            ));
            assert_eq!(state, before);
        }
    }
}

#[cfg(test)]
mod cosine_tests {
    use super::*;

    fn location() -> Location {
        Location {
            instruction_pointer: 0x1234,
            instruction_selector: 0x23,
            opcode: 0x1ff,
            data: None,
        }
    }

    #[test]
    fn cosine_stack_range_and_deferred_precision_commit_precisely() {
        for top in 0..8 {
            for masked in [false, true] {
                let state = State {
                    control: if masked { 0x37f } else { 0x37e },
                    status: (top << 11) | 0x4700,
                    data_pointer: 0x5678,
                    data_selector: 0x2b,
                    ..State::default()
                };
                let before = state.clone();
                let result = state.prepare(Instruction::Cosine, location()).unwrap();
                assert_eq!(state, before);
                assert_eq!(result.state.top(), top as usize);
                assert_eq!(result.state.status & 0x640, 0x40);
                assert_eq!(result.state.status & 0x4100, 0x4100);
                assert_eq!(result.deferred_exception, !masked);
                assert_eq!(
                    result.state.read(0).unwrap(),
                    if masked {
                        Some(Ext80::INDEFINITE)
                    } else {
                        None
                    }
                );
                assert_eq!(result.state.data_pointer, 0x5678);
                assert_eq!(result.state.data_selector, 0x2b);
                assert_eq!(result.state.instruction_pointer, 0x1234);
                assert_eq!(result.state.opcode, 0x1ff);
            }
            let mut state = State {
                control: 0x35f,
                status: (top << 11) | 0x4500,
                ..State::default()
            };
            state
                .write(
                    0,
                    Ext80 {
                        significand: 1 << 63,
                        sign_exponent: 0x3fdf,
                    },
                )
                .unwrap();
            let result = state.prepare(Instruction::Cosine, location()).unwrap();
            assert!(result.deferred_exception);
            assert_eq!(result.state.top(), top as usize);
            assert_eq!(result.state.status & 0x420, u32::from(PRECISION));
            assert!(matches!(
                result.state.prepare(Instruction::Cosine, location()),
                Err(Trap::PendingException(PRECISION))
            ));

            let huge = Ext80 {
                significand: 1 << 63,
                sign_exponent: 0xc03e,
            };
            state.write(0, huge).unwrap();
            state.status = (top << 11) | 0x4300;
            let before = state.clone();
            let result = state.prepare(Instruction::Cosine, location()).unwrap();
            assert_eq!(result.state.registers, before.registers);
            assert_eq!(result.state.tag, before.tag);
            assert_eq!(result.state.status, (top << 11) | 0x4500);
            assert!(!result.deferred_exception);
        }
    }

    #[test]
    fn cosine_pending_and_denormal_mask_preserve_prior_state() {
        let mut state = State {
            control: 0x37d,
            status: 0x4500,
            ..State::default()
        };
        state
            .write(
                0,
                Ext80 {
                    significand: 1,
                    sign_exponent: 0,
                },
            )
            .unwrap();
        let before = state.clone();
        let result = state.prepare(Instruction::Cosine, location()).unwrap();
        assert_eq!(result.state.registers, before.registers);
        assert_eq!(result.state.tag, before.tag);
        assert_eq!(result.state.status & 0x63f, u32::from(DENORMAL));
        assert!(result.deferred_exception);
        let pending = result.state.clone();
        assert!(matches!(
            pending.prepare(Instruction::Cosine, location()),
            Err(Trap::PendingException(DENORMAL))
        ));
        assert_eq!(pending, result.state);
    }
}

#[cfg(test)]
mod sine_tests {
    use super::*;

    fn location() -> Location {
        Location {
            instruction_pointer: 0x1234,
            instruction_selector: 0x23,
            opcode: 0x1fe,
            data: None,
        }
    }

    fn seeded(top: u32, control: u32) -> State {
        let mut state = State {
            control,
            status: top << 11 | 0x4700,
            data_pointer: 0x5678,
            data_selector: 0x2b,
            instruction_pointer: 0xabcd,
            opcode: 0x345,
            ..State::default()
        };
        for (index, reg) in state.registers.iter_mut().enumerate() {
            *reg = [0x1234_5678 + index as u32, 0x8765_4321, 0xc001];
        }
        state
    }

    #[test]
    fn biased_underflow_commits_one_register_and_defers_the_next_waiting_fault() {
        for top in 0..8 {
            for rc in 0..4 {
                let mut state = seeded(top, 0x36f | rc << 10);
                state
                    .write(
                        0,
                        Ext80 {
                            significand: (1 << 63) - 1,
                            sign_exponent: 0,
                        },
                    )
                    .unwrap();
                let before = state.clone();
                let prepared = state.prepare(Instruction::Sine, location()).unwrap();
                assert_eq!(state, before);
                let next = prepared.state;
                assert!(prepared.deferred_exception);
                assert!(prepared.store.is_none());
                assert_eq!(next.top(), top as usize);
                assert_eq!(
                    next.read(0).unwrap().unwrap(),
                    Ext80 {
                        significand: u64::MAX - u64::from(rc != 2),
                        sign_exponent: 0x6000
                    }
                );
                for register in 0..8 {
                    if register != top as usize {
                        assert_eq!(next.registers[register], before.registers[register]);
                    }
                }
                assert_eq!(next.tag & !(3 << (top * 2)), before.tag & !(3 << (top * 2)));
                assert_eq!(next.tag >> (top * 2) & 3, 0);
                assert_eq!(
                    next.status & 0x3f,
                    u32::from(DENORMAL | UNDERFLOW | PRECISION)
                );
                assert_eq!(next.status & 0x8080, 0x8080);
                assert_eq!(next.status & 0x600, if rc == 2 { 0x200 } else { 0 });
                assert_eq!(
                    (
                        next.instruction_pointer,
                        next.opcode,
                        next.data_pointer,
                        next.data_selector
                    ),
                    (0x1234, 0x1fe, 0x5678, 0x2b)
                );
                assert_eq!(next.control, before.control);
                assert!(matches!(
                    next.prepare(Instruction::Sine, location()),
                    Err(Trap::PendingException(UNDERFLOW))
                ));
                let cleared = next
                    .prepare(Instruction::ClearExceptions, Location::default())
                    .unwrap()
                    .state;
                let stored = cleared
                    .prepare(
                        Instruction::StoreMemory {
                            format: Format::Float80,
                            pop: true,
                        },
                        location(),
                    )
                    .unwrap();
                let bytes = stored.store.unwrap().bytes;
                assert_eq!(
                    u64::from_le_bytes(bytes[..8].try_into().unwrap()),
                    u64::MAX - u64::from(rc != 2)
                );
                assert_eq!(u16::from_le_bytes(bytes[8..10].try_into().unwrap()), 0x6000);
                assert_eq!(stored.state.top(), (top as usize + 1) & 7);
            }
        }
    }

    #[test]
    fn empty_invalid_denormal_and_pending_preserve_the_suppressed_result() {
        for top in 0..8 {
            for (value, mask, reason) in [
                (None, INVALID, INVALID | STACK_FAULT),
                (
                    Some(Ext80 {
                        significand: 1 << 63,
                        sign_exponent: 0x7fff,
                    }),
                    INVALID,
                    INVALID,
                ),
                (
                    Some(Ext80 {
                        significand: 1,
                        sign_exponent: 0,
                    }),
                    DENORMAL,
                    DENORMAL,
                ),
            ] {
                let mut state = seeded(top, u32::from(0x37f & !mask));
                if let Some(value) = value {
                    state.write(0, value).unwrap();
                }
                let before = state.clone();
                let prepared = state.prepare(Instruction::Sine, location()).unwrap();
                assert_eq!(prepared.state.registers, before.registers);
                assert_eq!(prepared.state.tag, before.tag);
                assert_eq!(prepared.state.status & 0x67f, u32::from(reason));
                assert_eq!(prepared.state.instruction_pointer, 0x1234);
                assert!(prepared.deferred_exception);
                let next = prepared.state;
                assert!(
                    matches!(next.prepare(Instruction::Sine, location()),Err(Trap::PendingException(bits)) if bits == mask)
                );
                assert_eq!(state, before);
            }
        }
    }

    #[test]
    fn range_and_signed_zero_retire_without_changing_stack_progress() {
        for top in 0..8 {
            for sign in [0, 0x8000] {
                for large in [false, true] {
                    let value = Ext80 {
                        significand: if large { 1 << 63 } else { 0 },
                        sign_exponent: sign | if large { 0x403e } else { 0 },
                    };
                    let mut state = seeded(top, 0x37f);
                    state.status |= u32::from(PRECISION);
                    state.write(0, value).unwrap();
                    let before = state.clone();
                    let prepared = state.prepare(Instruction::Sine, location()).unwrap();
                    assert_eq!(prepared.state.registers, before.registers);
                    assert_eq!(prepared.state.tag, before.tag);
                    assert_eq!(prepared.state.status & 0x600, if large { 0x400 } else { 0 });
                    assert_eq!(prepared.state.status & 0x3f, u32::from(PRECISION));
                    assert!(!prepared.deferred_exception);
                }
            }
        }
        let mut state = seeded(3, 0x17f);
        state.write(0, Ext80::ONE).unwrap();
        let before = state.clone();
        assert!(matches!(
            state.prepare(Instruction::Sine, location()),
            Err(Trap::Unsupported(Unsupported::ReservedPrecision))
        ));
        assert_eq!(state, before);
    }
}
