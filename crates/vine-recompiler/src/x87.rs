//! Exact x87 slow path shared by native fixtures and generated Wasm. Inputs are
//! guest descriptors/offsets, never unverified host pointers. The export owning
//! the Backend handle calls this module while borrowing that Backend uniquely.
use crate::memory::{MemoryError, MirrorMemory};
use crate::state::{CpuState, ExitState, exit_kind};
use vine_x87::{
    Arithmetic, Ext80, Format, Instruction, Location, Operand, Prepared, StoreValue, Trap,
};

pub const FLOATING_POINT_EXIT: u32 = 14;
pub const NO_SEGMENT: u32 = u32::MAX;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Load = 1,
    Store = 2,
    Add = 3,
    Subtract = 4,
    Multiply = 5,
    Divide = 6,
    Compare = 7,
    Exchange = 8,
    ChangeSign = 9,
    Absolute = 10,
    LoadControl = 11,
    StoreControl = 12,
    StoreStatus = 13,
    ClearExceptions = 14,
    Initialize = 15,
    Wait = 16,
    LoadConstant = 17,
    SquareRoot = 18,
    RoundToInteger = 19,
    Free = 20,
    Log2Multiply = 21,
    ReverseSubtract = 22,
    ReverseDivide = 23,
    Arctangent = 24,
    Cosine = 25,
    Sine = 26,
    ExponentialMinusOne = 27,
    Scale = 28,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperandKind {
    Register = 0,
    Float32 = 1,
    Float64 = 2,
    Float80 = 3,
    Integer16 = 4,
    Integer32 = 5,
    Integer64 = 6,
}

impl OperandKind {
    pub fn bytes(self) -> Option<usize> {
        match self {
            Self::Register => None,
            Self::Float32 | Self::Integer32 => Some(4),
            Self::Float64 | Self::Integer64 => Some(8),
            Self::Float80 => Some(10),
            Self::Integer16 => Some(2),
        }
    }
    fn format(self) -> Option<Format> {
        match self {
            Self::Register => None,
            Self::Float32 => Some(Format::Float32),
            Self::Float64 => Some(Format::Float64),
            Self::Float80 => Some(Format::Float80),
            Self::Integer16 => Some(Format::Integer16),
            Self::Integer32 => Some(Format::Integer32),
            Self::Integer64 => Some(Format::Integer64),
        }
    }
}

/// Bits 0..7 operation, 8..10 ST index, 11..13 operand kind, 14 destination
/// ST(index) instead of ST0, 15..16 pop count, 17 quiet comparison. All other
/// bits and irrelevant combinations are rejected, including invalid register
/// widths rather than truncating them into a different valid instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Descriptor {
    pub operation: Operation,
    pub operand: OperandKind,
    pub register: u8,
    pub destination_register: bool,
    pub pops: u8,
    pub quiet: bool,
}

impl Descriptor {
    pub fn encode(self) -> Result<u32, String> {
        self.validate()?;
        Ok(self.operation as u32
            | u32::from(self.register) << 8
            | (self.operand as u32) << 11
            | u32::from(self.destination_register) << 14
            | u32::from(self.pops) << 15
            | u32::from(self.quiet) << 17)
    }
    pub fn decode(value: u32) -> Result<Self, String> {
        if value & !0x3ffff != 0 {
            return Err("reserved x87 descriptor bits".into());
        }
        let operation = match value & 255 {
            1 => Operation::Load,
            2 => Operation::Store,
            3 => Operation::Add,
            4 => Operation::Subtract,
            5 => Operation::Multiply,
            6 => Operation::Divide,
            7 => Operation::Compare,
            8 => Operation::Exchange,
            9 => Operation::ChangeSign,
            10 => Operation::Absolute,
            11 => Operation::LoadControl,
            12 => Operation::StoreControl,
            13 => Operation::StoreStatus,
            14 => Operation::ClearExceptions,
            15 => Operation::Initialize,
            16 => Operation::Wait,
            17 => Operation::LoadConstant,
            18 => Operation::SquareRoot,
            19 => Operation::RoundToInteger,
            20 => Operation::Free,
            21 => Operation::Log2Multiply,
            22 => Operation::ReverseSubtract,
            23 => Operation::ReverseDivide,
            24 => Operation::Arctangent,
            25 => Operation::Cosine,
            26 => Operation::Sine,
            27 => Operation::ExponentialMinusOne,
            28 => Operation::Scale,
            _ => return Err("invalid x87 descriptor operation".into()),
        };
        let operand = match value >> 11 & 7 {
            0 => OperandKind::Register,
            1 => OperandKind::Float32,
            2 => OperandKind::Float64,
            3 => OperandKind::Float80,
            4 => OperandKind::Integer16,
            5 => OperandKind::Integer32,
            6 => OperandKind::Integer64,
            _ => return Err("invalid x87 descriptor operand".into()),
        };
        let descriptor = Self {
            operation,
            operand,
            register: (value >> 8 & 7) as u8,
            destination_register: value & (1 << 14) != 0,
            pops: (value >> 15 & 3) as u8,
            quiet: value & (1 << 17) != 0,
        };
        descriptor.validate()?;
        Ok(descriptor)
    }
    fn validate(self) -> Result<(), String> {
        use OperandKind as K;
        use Operation as O;
        if self.register > 7 || self.pops > 2 {
            return Err("invalid x87 descriptor register/pop count".into());
        }
        let register = self.operand == K::Register;
        let common = !self.destination_register && !self.quiet;
        let valid = match self.operation {
            O::Load => common && self.pops == 0 && (register || self.register == 0),
            O::Store => {
                common
                    && self.pops <= 1
                    && (register || self.register == 0)
                    && (!matches!(self.operand, K::Float80 | K::Integer64) || self.pops == 1)
            }
            O::Add
            | O::Subtract
            | O::Multiply
            | O::Divide
            | O::ReverseSubtract
            | O::ReverseDivide => {
                !self.quiet
                    && self.pops <= 1
                    && matches!(
                        self.operand,
                        K::Register | K::Float32 | K::Float64 | K::Integer16 | K::Integer32
                    )
                    && (register
                        || self.register == 0 && !self.destination_register && self.pops == 0)
                    && (self.pops == 0 || self.destination_register)
            }
            O::Compare => {
                !self.destination_register
                    && matches!(
                        self.operand,
                        K::Register | K::Float32 | K::Float64 | K::Integer16 | K::Integer32
                    )
                    && (register || self.register == 0 && self.pops <= 1 && !self.quiet)
                    && (self.pops != 2 || register && self.register == 1)
            }
            O::Exchange | O::Free => common && register && self.pops == 0,
            O::Log2Multiply | O::Arctangent => {
                register
                    && self.register == 1
                    && self.destination_register
                    && self.pops == 1
                    && !self.quiet
            }
            O::LoadConstant => {
                common
                    && register
                    && self.pops == 0
                    && vine_x87::Constant::from_id(self.register).is_some()
            }
            O::LoadControl | O::StoreControl | O::StoreStatus => {
                common && self.operand == K::Integer16 && self.pops == 0 && self.register == 0
            }
            _ => common && register && self.pops == 0 && self.register == 0,
        };
        if valid {
            Ok(())
        } else {
            Err("invalid x87 descriptor operand combination".into())
        }
    }
    pub fn waiting(self) -> bool {
        !matches!(
            self.operation,
            Operation::StoreControl
                | Operation::StoreStatus
                | Operation::ClearExceptions
                | Operation::Initialize
        )
    }
    fn writes_memory(self) -> bool {
        self.operand != OperandKind::Register
            && matches!(
                self.operation,
                Operation::Store | Operation::StoreControl | Operation::StoreStatus
            )
    }
}

fn stop(cpu: &mut CpuState, kind: u32, eip: u32, address: u32, error: u32, completed: u32) -> u32 {
    cpu.exit = ExitState {
        kind,
        fault_eip: eip,
        address,
        error,
        completed,
    };
    kind
}

fn memory_stop(cpu: &mut CpuState, error: MemoryError, eip: u32) -> Result<u32, String> {
    match error {
        MemoryError::Fault {
            address,
            error_code,
            ..
        } => Ok(stop(
            cpu,
            exit_kind::PAGE_FAULT,
            eip,
            address,
            error_code,
            0,
        )),
        MemoryError::ResourceLimit => Ok(stop(cpu, exit_kind::RESOURCE_LIMIT, eip, 0, 0, 0)),
        MemoryError::Invalid(message) => Err(format!("x87 execution memory invariant: {message}")),
    }
}

/// Called by an export that resolved `handle` and uniquely borrowed that
/// Backend. On success only x87 state and an optional checked memory write
/// change. EIP, GPR and EFLAGS remain owned by generated locals. A returned
/// CODE_MODIFIED retired this instruction; other nonzero results did not.
pub fn execute(
    cpu: &mut CpuState,
    memory: &mut MirrorMemory,
    descriptor: u32,
    segment: u32,
    offset: u32,
    eip: u32,
    fop: u32,
) -> Result<u32, String> {
    let descriptor = Descriptor::decode(descriptor)?;
    if fop > 0x7ff {
        return Err("invalid x87 opcode field".into());
    }
    if descriptor.waiting()
        && let Err(Trap::PendingException(flags)) = cpu.x87.check_waiting()
    {
        return Ok(stop(cpu, FLOATING_POINT_EXIT, eip, 0, u32::from(flags), 0));
    }
    let mut linear = 0;
    let mut location = Location {
        instruction_pointer: eip,
        instruction_selector: cpu.segments[1].selector as u16,
        opcode: fop as u16,
        data: None,
    };
    let mut operand = Operand::Register(if descriptor.destination_register {
        0
    } else {
        descriptor.register
    });
    let mut memory_bytes = [0u8; 10];
    if let Some(length) = descriptor.operand.bytes() {
        let Some(selected) = cpu.segments.get(segment as usize).copied() else {
            return Err("invalid x87 memory segment".into());
        };
        if segment == 1
            || (segment != 4 && selected.base != 0)
            || !(selected.attributes == 0 || selected.attributes == 0xf2)
        {
            return Ok(stop(cpu, exit_kind::UNSUPPORTED, eip, offset, 0, 0));
        }
        if u64::from(offset) + length as u64 - 1 > u64::from(selected.limit) {
            return Ok(stop(
                cpu,
                if segment == 2 {
                    exit_kind::STACK_FAULT
                } else {
                    exit_kind::GENERAL_PROTECTION
                },
                eip,
                offset,
                0,
                0,
            ));
        }
        linear = selected.base.wrapping_add(offset);
        if u64::from(linear) + length as u64 > 1u64 << 32 {
            return Ok(stop(
                cpu,
                if segment == 4 {
                    exit_kind::UNSUPPORTED
                } else {
                    exit_kind::GENERAL_PROTECTION
                },
                eip,
                linear,
                0,
                0,
            ));
        }
        // Architectural FDP is the logical offset and selector, not the linear
        // FS-adjusted address used to access the execution mirror.
        location.data = Some((offset, selected.selector as u16));
        if descriptor.writes_memory() {
            if let Err(error) = memory.preflight_small_write(linear, length) {
                return memory_stop(cpu, error, eip);
            }
        } else {
            memory_bytes = match memory.read_small(linear, length) {
                Ok(bytes) => bytes,
                Err(error) => return memory_stop(cpu, error, eip),
            };
            let mut bits = [0u8; 8];
            bits[..length.min(8)].copy_from_slice(&memory_bytes[..length.min(8)]);
            operand = if length == 10 {
                Operand::ExtendedMemory(Ext80 {
                    significand: u64::from_le_bytes(bits),
                    sign_exponent: u16::from_le_bytes(
                        memory_bytes[8..10].try_into().expect("checked length"),
                    ),
                })
            } else {
                Operand::Memory {
                    format: descriptor.operand.format().expect("memory format"),
                    bits: u64::from_le_bytes(bits),
                }
            };
        }
    } else if segment != NO_SEGMENT || offset != 0 {
        return Err("register x87 instruction has a memory operand".into());
    }

    use Operation as O;
    let instruction = match descriptor.operation {
        O::Load => Instruction::Load(operand),
        O::Store => {
            if let Some(format) = descriptor.operand.format() {
                Instruction::StoreMemory {
                    format,
                    pop: descriptor.pops == 1,
                }
            } else {
                Instruction::StoreRegister {
                    index: descriptor.register,
                    pop: descriptor.pops == 1,
                }
            }
        }
        O::Add
        | O::Subtract
        | O::Multiply
        | O::Divide
        | O::ReverseSubtract
        | O::ReverseDivide
        | O::SquareRoot
        | O::RoundToInteger => {
            let operation = match descriptor.operation {
                O::Add => Arithmetic::Add,
                O::Subtract | O::ReverseSubtract => Arithmetic::Subtract,
                O::Multiply => Arithmetic::Multiply,
                O::Divide | O::ReverseDivide => Arithmetic::Divide,
                O::SquareRoot => Arithmetic::SquareRoot,
                _ => Arithmetic::RoundToInteger,
            };
            let destination = if descriptor.destination_register {
                descriptor.register
            } else {
                0
            };
            let register = Operand::Register(destination);
            let (left, right) =
                if matches!(descriptor.operation, O::ReverseSubtract | O::ReverseDivide) {
                    (operand, register)
                } else {
                    (register, operand)
                };
            Instruction::Arithmetic {
                operation,
                left,
                right,
                destination,
                pop: descriptor.pops == 1,
            }
        }
        O::Log2Multiply => Instruction::Log2Multiply,
        O::Arctangent => Instruction::Arctangent,
        O::Cosine => Instruction::Cosine,
        O::Sine => Instruction::Sine,
        O::ExponentialMinusOne => Instruction::ExponentialMinusOne,
        O::Scale => Instruction::Scale,
        O::Compare => Instruction::Compare {
            right: operand,
            quiet: descriptor.quiet,
            pops: descriptor.pops,
        },
        O::Exchange => Instruction::Exchange(descriptor.register),
        O::Free => Instruction::Free(descriptor.register),
        O::ChangeSign => Instruction::ChangeSign,
        O::Absolute => Instruction::Absolute,
        O::LoadConstant => Instruction::Load(Operand::Constant(
            vine_x87::Constant::from_id(descriptor.register)
                .expect("validated constant")
                .value(cpu.x87.control as u16),
        )),
        O::LoadControl => Instruction::LoadControl(u16::from_le_bytes(
            memory_bytes[..2]
                .try_into()
                .expect("preflight control word"),
        )),
        O::ClearExceptions => Instruction::ClearExceptions,
        O::Initialize => Instruction::Initialize,
        O::Wait | O::StoreControl | O::StoreStatus => {
            let store = if descriptor.operation == O::Wait {
                None
            } else {
                let word = if descriptor.operation == O::StoreControl {
                    cpu.x87.control
                } else {
                    cpu.x87.status
                } as u16;
                let mut bytes = [0; 10];
                bytes[..2].copy_from_slice(&word.to_le_bytes());
                Some(StoreValue { bytes, length: 2 })
            };
            return commit(
                cpu,
                memory,
                Prepared {
                    state: cpu.x87.clone(),
                    store,
                    deferred_exception: cpu.x87.pending_exceptions() != 0,
                },
                linear,
                eip,
            );
        }
    };
    let prepared = match cpu.x87.prepare(instruction, location) {
        Ok(prepared) => prepared,
        Err(Trap::PendingException(flags)) => {
            return Ok(stop(cpu, FLOATING_POINT_EXIT, eip, 0, u32::from(flags), 0));
        }
        Err(Trap::Unsupported(_)) => return Ok(stop(cpu, exit_kind::UNSUPPORTED, eip, 0, 0, 0)),
    };
    commit(cpu, memory, prepared, linear, eip)
}

fn commit(
    cpu: &mut CpuState,
    memory: &mut MirrorMemory,
    prepared: Prepared,
    address: u32,
    eip: u32,
) -> Result<u32, String> {
    let mut smc = false;
    if let Some(store) = prepared.store {
        smc = match memory.write_small(address, &store.bytes[..store.length]) {
            Ok(code_modified) => code_modified,
            Err(error) => return memory_stop(cpu, error, eip),
        };
    }
    cpu.x87 = prepared.state;
    Ok(stop(
        cpu,
        if smc {
            exit_kind::CODE_MODIFIED
        } else {
            exit_kind::NONE
        },
        eip,
        0,
        0,
        1,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Access, EXECUTE, READ, SEALED, WRITE};

    fn descriptor(operation: Operation, operand: OperandKind, pops: u8) -> u32 {
        Descriptor {
            operation,
            operand,
            register: 0,
            destination_register: false,
            pops,
            quiet: false,
        }
        .encode()
        .unwrap()
    }
    fn memory() -> MirrorMemory {
        MirrorMemory::new(8).unwrap()
    }
    fn cpu() -> CpuState {
        CpuState::thread(0x400000, 0x200000, 0)
    }

    #[test]
    fn invalid_descriptors_fail_closed() {
        assert!(Descriptor::decode(0).is_err());
        assert!(Descriptor::decode(1 << 31 | 1).is_err());
        assert!(
            Descriptor {
                operation: Operation::Load,
                operand: OperandKind::Register,
                register: 8,
                destination_register: false,
                pops: 0,
                quiet: false
            }
            .encode()
            .is_err()
        );
        assert!(Descriptor::decode(1 | 7 << 11).is_err());
    }

    #[test]
    fn real_audio_load_multiply_store_flows_through_checked_mirror() {
        let mut cpu = cpu();
        let mut memory = memory();
        let mut page = [0u8; 4096];
        page[..4].copy_from_slice(&0x3fc00000u32.to_le_bytes());
        page[4..8].copy_from_slice(&0x40000000u32.to_le_bytes());
        memory.map_page(0x1000, 5, READ | WRITE, &page).unwrap();
        for (op, offset, pops) in [
            (Operation::Load, 0x1000, 0),
            (Operation::Multiply, 0x1004, 0),
            (Operation::Store, 0x1008, 1),
        ] {
            assert_eq!(
                execute(
                    &mut cpu,
                    &mut memory,
                    descriptor(op, OperandKind::Float32, pops),
                    3,
                    offset,
                    0x401000,
                    0x100
                )
                .unwrap(),
                0
            );
        }
        assert_eq!(
            memory.read(0x1008, 4, Access::Read).unwrap(),
            0x40400000u32.to_le_bytes()
        );
        assert_eq!(cpu.x87.tag, 0xffff);
        assert_eq!(memory.collect_dirty_pages().unwrap()[0].address, 0x1000);
    }

    #[test]
    fn pending_fault_precedes_access_and_failed_store_does_not_pop() {
        let mut cpu = cpu();
        let mut memory = memory();
        cpu.x87.control = 0x37e;
        cpu.x87.status = 0x80c1;
        let saved = cpu.x87.clone();
        assert_eq!(
            execute(
                &mut cpu,
                &mut memory,
                descriptor(Operation::Store, OperandKind::Float32, 1),
                3,
                0xffff_ffff,
                0x1234,
                0x100
            )
            .unwrap(),
            FLOATING_POINT_EXIT
        );
        assert_eq!(cpu.x87, saved);
        cpu.x87 = vine_x87::State::default();
        memory
            .map_page(0x1000, 1, READ | WRITE, &[0x5a; 4096])
            .unwrap();
        let saved = cpu.x87.clone();
        assert_eq!(
            execute(
                &mut cpu,
                &mut memory,
                descriptor(Operation::Store, OperandKind::Float32, 1),
                3,
                0x1ffe,
                0x1234,
                0x100
            )
            .unwrap(),
            exit_kind::PAGE_FAULT
        );
        assert_eq!(cpu.exit.address, 0x2000);
        assert_eq!(cpu.exit.error, 6);
        assert_eq!(cpu.x87, saved);
        assert_eq!(memory.read(0x1ffe, 2, Access::Read).unwrap(), [0x5a; 2]);
        assert!(memory.collect_dirty_pages().unwrap().is_empty());
    }

    #[test]
    fn code_alias_store_retires_once_and_invalidates_every_alias() {
        let mut cpu = cpu();
        let mut memory = memory();
        memory
            .map_page(0x1000, 1, READ | WRITE, &[0; 4096])
            .unwrap();
        memory
            .map_alias(0x4000, 0x1000, 2, READ | EXECUTE | SEALED)
            .unwrap();
        let old_stamp = memory.code_stamp(0x4000);
        execute(
            &mut cpu,
            &mut memory,
            descriptor(Operation::LoadConstant, OperandKind::Register, 0),
            NO_SEGMENT,
            0,
            0x401000,
            0x1ee,
        )
        .unwrap();
        assert_eq!(
            execute(
                &mut cpu,
                &mut memory,
                descriptor(Operation::Store, OperandKind::Float32, 1),
                3,
                0x1000,
                0x401002,
                0x118
            )
            .unwrap(),
            exit_kind::CODE_MODIFIED
        );
        assert_eq!(cpu.exit.completed, 1);
        assert_eq!(cpu.x87.tag, 0xffff);
        assert_eq!(memory.code_stamp(0x4000), old_stamp);
        assert!(!memory.mapping_stamp_is_current(0x4000, old_stamp));
        assert_eq!(memory.collect_dirty_pages().unwrap().len(), 2);
        assert_ne!(memory.code_stamp(0x4000), old_stamp);
    }

    #[test]
    fn ten_byte_store_fault_and_repair_preserve_raw80_and_stack_progress() {
        let mut cpu = cpu();
        let mut memory = memory();
        memory
            .map_page(0x1000, 1, READ | WRITE, &[0x5a; 4096])
            .unwrap();
        memory.map_page(0x2000, 2, READ, &[0xa5; 4096]).unwrap();
        let value = Ext80 {
            significand: 0x8000_0000_0000_0001,
            sign_exponent: 0xbfff,
        };
        cpu.x87.registers[0] = value.words();
        cpu.x87.tag = 0xfffc;
        let before = cpu.x87.clone();
        let store = descriptor(Operation::Store, OperandKind::Float80, 1);
        assert_eq!(
            execute(&mut cpu, &mut memory, store, 3, 0x1ffb, 0x401000, 0x338).unwrap(),
            exit_kind::PAGE_FAULT
        );
        assert_eq!(cpu.exit.address, 0x2000);
        assert_eq!(cpu.exit.error, 7);
        assert_eq!(cpu.x87, before);
        assert_eq!(
            memory.read_small(0x1ffb, 10).unwrap(),
            [0x5a, 0x5a, 0x5a, 0x5a, 0x5a, 0xa5, 0xa5, 0xa5, 0xa5, 0xa5]
        );
        assert!(!memory.has_pending_writes());
        memory.protect_page(0x2000, 2, READ | WRITE).unwrap();
        let epoch = memory.epoch();
        assert_eq!(
            execute(&mut cpu, &mut memory, store, 3, 0x1ffb, 0x401000, 0x338).unwrap(),
            0
        );
        assert_eq!(cpu.x87.top(), 1);
        assert_eq!(cpu.x87.tag, 0xffff);
        assert_eq!(memory.epoch(), epoch);
        assert_eq!(
            execute(
                &mut cpu,
                &mut memory,
                descriptor(Operation::Load, OperandKind::Float80, 0),
                3,
                0x1ffb,
                0x401002,
                0x328
            )
            .unwrap(),
            0
        );
        assert_eq!(cpu.x87.read(0).unwrap(), Some(value));
        assert_eq!(cpu.x87.top(), 0);
        assert_eq!(memory.collect_dirty_pages().unwrap().len(), 2);
        assert_eq!(memory.epoch(), epoch + 1);
    }

    #[test]
    fn fs_data_pointer_is_logical_and_controls_leave_environment_unchanged() {
        let mut cpu = cpu();
        let mut memory = memory();
        cpu.segments[4].base = 0x1000;
        cpu.segments[4].selector = 0x3b;
        memory
            .map_page(0x1000, 1, READ | WRITE, &[0; 4096])
            .unwrap();
        execute(
            &mut cpu,
            &mut memory,
            descriptor(Operation::Load, OperandKind::Float32, 0),
            4,
            4,
            0x1234,
            0x100,
        )
        .unwrap();
        assert_eq!(cpu.x87.data_pointer, 4);
        assert_eq!(cpu.x87.data_selector, 0x3b);
        let saved = cpu.x87.clone();
        execute(
            &mut cpu,
            &mut memory,
            descriptor(Operation::StoreStatus, OperandKind::Integer16, 0),
            4,
            8,
            0x1236,
            0x17c,
        )
        .unwrap();
        assert_eq!(cpu.x87, saved);
    }
}
