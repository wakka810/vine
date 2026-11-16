use super::*;

#[test]
fn reverse_memory_arithmetic_widens_left_operand_without_losing_signalingness() {
    let three = Ext80 {
        significand: 0xc000_0000_0000_0000,
        sign_exponent: 0x4000,
    };
    let state = push(State::default(), three);
    for (operation, expected) in [
        (
            Arithmetic::Divide,
            Ext80 {
                significand: 0xc000_0000_0000_0000,
                sign_exponent: 0xc000,
            },
        ),
        (
            Arithmetic::Subtract,
            Ext80 {
                significand: 0xc000_0000_0000_0000,
                sign_exponent: 0xc002,
            },
        ),
    ] {
        let prepared = execute(
            state.clone(),
            Instruction::Arithmetic {
                operation,
                left: Operand::Memory {
                    format: Format::Integer16,
                    bits: (-9i16) as u16 as u64,
                },
                right: Operand::Register(0),
                destination: 0,
                pop: false,
            },
        );
        assert_eq!(prepared.state.read(0).unwrap(), Some(expected));
        assert_eq!(prepared.state.status & 0x3f, 0);
        assert_eq!(prepared.state.top(), state.top());
    }
    let prepared = execute(
        state,
        Instruction::Arithmetic {
            operation: Arithmetic::Divide,
            left: Operand::Memory {
                format: Format::Float32,
                bits: 0x7f80_0001,
            },
            right: Operand::Register(0),
            destination: 0,
            pop: false,
        },
    );
    let result = prepared.state.read(0).unwrap().unwrap();
    assert!(result.is_nan());
    assert!(!result.is_signaling_nan());
    assert_eq!(prepared.state.status & 0x3f, u32::from(INVALID));
}

#[test]
fn reverse_empty_source_has_priority_over_numeric_helper_validation() {
    for invalid_masked in [false, true] {
        let mut state = push(State::default(), Ext80::ONE);
        state.control = 0x17e | u32::from(invalid_masked); // reserved PC, empty ST1
        let prepared = execute(
            state.clone(),
            Instruction::Arithmetic {
                operation: Arithmetic::Divide,
                left: Operand::Register(1),
                right: Operand::Register(0),
                destination: 0,
                pop: false,
            },
        );
        assert_eq!(prepared.state.status & 0x241, 0x41);
        if invalid_masked {
            assert_eq!(prepared.state.read(0).unwrap(), Some(Ext80::INDEFINITE));
        } else {
            assert_eq!(prepared.state.registers, state.registers);
            assert_eq!(prepared.state.tag, state.tag);
        }
    }
}

#[test]
fn retains_more_than_binary64_precision() {
    let increment = Ext80 {
        significand: 0x8000_0000_0000_0000,
        sign_exponent: 0x3fc0,
    };
    let value = arithmetic(Arithmetic::Add, Ext80::ONE, increment, 0x37f).unwrap();
    assert_eq!(value.value.significand, 0x8000_0000_0000_0001);
    assert_eq!(value.exceptions, 0);
    let reduced = arithmetic(Arithmetic::Add, Ext80::ONE, increment, 0x27f).unwrap();
    assert_eq!(reduced.value, Ext80::ONE);
    assert_eq!(reduced.exceptions, PRECISION);
}

#[cfg(target_arch = "x86_64")]
unsafe extern "C" {
    fn vine_x87_native(
        op: u32,
        asig: u64,
        aexp: u32,
        bsig: u64,
        bexp: u32,
        control: u32,
        out: *mut u64,
    );
    fn vine_x87_native_stack(kind: u32, control: u32, out: *mut u64);
    fn vine_x87_native_memory(
        op: u32,
        asig: u64,
        aexp: u32,
        bits: u32,
        control: u32,
        out: *mut u64,
    );
    fn vine_x87_native_copies(kind: u32, asig: u64, aexp: u32, out: *mut u64);
}

#[test]
#[cfg(target_arch = "x86_64")]
fn arithmetic_matches_independent_native_x87_all_precision_and_rounding_modes() {
    let values = [
        Ext80::ZERO,
        Ext80::ONE,
        Ext80 {
            significand: 0x8000_0000_0000_0001,
            sign_exponent: 0x3fff,
        },
        Ext80 {
            significand: 0xaaaa_aaaa_aaaa_aaaa,
            sign_exponent: 0xbfff,
        },
        Ext80 {
            significand: 0x8000_0000_0000_0001,
            sign_exponent: 0x3fc0,
        },
        Ext80 {
            significand: 0xffff_ffff_ffff_ffff,
            sign_exponent: 0x7ffe,
        },
        Ext80 {
            significand: 0x8000_0000_0000_0000,
            sign_exponent: 1,
        },
        Ext80 {
            significand: 1,
            sign_exponent: 0,
        },
        Ext80 {
            significand: 0x8000_0000_0000_0000,
            sign_exponent: 0,
        },
        Ext80 {
            significand: 0x8000_0000_0000_0000,
            sign_exponent: 0x7fff,
        },
        Ext80 {
            significand: 0x8000_0000_0000_0001,
            sign_exponent: 0x7fff,
        },
        Ext80::INDEFINITE,
    ];
    for pc in [0, 2, 3] {
        for rc in 0..4 {
            let control = 0x7f | pc << 8 | rc << 10;
            for op in [
                Arithmetic::Add,
                Arithmetic::Subtract,
                Arithmetic::Multiply,
                Arithmetic::Divide,
                Arithmetic::SquareRoot,
                Arithmetic::RoundToInteger,
            ] {
                for a in values {
                    for b in values {
                        let result = arithmetic(op, a, b, control).unwrap();
                        let mut expected = [0u64; 4];
                        // SAFETY: independent fixed-size assembly probe restores host FPU state.
                        unsafe {
                            vine_x87_native(
                                op as u32,
                                a.significand,
                                u32::from(a.sign_exponent),
                                b.significand,
                                u32::from(b.sign_exponent),
                                u32::from(control),
                                expected.as_mut_ptr(),
                            );
                        }
                        let label = format!("{op:?} a={a:?} b={b:?} cw={control:x}");
                        assert_eq!(
                            result.value,
                            Ext80 {
                                significand: expected[0],
                                sign_exponent: expected[1] as u16
                            },
                            "{label}"
                        );
                        assert_eq!(
                            result.exceptions & 0x3f,
                            expected[3] as u16 & 0x3f,
                            "{label}"
                        );
                        assert_eq!(result.round_up, expected[3] as u16 & C1 != 0, "{label}");
                    }
                }
            }
        }
    }
}

#[test]
fn memory_formats_and_exception_flags() {
    let one = load(Format::Float32, 0x3f80_0000, 0x37f).unwrap();
    assert_eq!(one.value, Ext80::ONE);
    let subnormal = load(Format::Float32, 1, 0x37f).unwrap();
    assert_eq!(subnormal.exceptions, DENORMAL);
    assert_eq!(
        store(Format::Float32, subnormal.value, 0x37f).unwrap().bits,
        1
    );
    assert_eq!(
        load(Format::Float32, 0x7f80_0001, 0x37f)
            .unwrap()
            .exceptions,
        INVALID
    );
    assert_eq!(
        store(Format::Integer16, Ext80::INDEFINITE, 0x37f)
            .unwrap()
            .bits,
        0x8000
    );
    assert_eq!(
        arithmetic(Arithmetic::Divide, Ext80::ONE, Ext80::ZERO, 0x37f)
            .unwrap()
            .exceptions,
        ZERO_DIVIDE
    );
    assert_eq!(compare(Ext80::INDEFINITE, Ext80::ONE, true).exceptions, 0);
    assert_eq!(
        compare(Ext80::INDEFINITE, Ext80::ONE, false).exceptions,
        INVALID
    );
}

fn execute(state: State, instruction: Instruction) -> Prepared {
    state
        .prepare(
            instruction,
            Location {
                instruction_pointer: 0x1234,
                instruction_selector: 0x1b,
                opcode: 0x1c0,
                data: Some((0x10000, 0x23)),
            },
        )
        .unwrap()
}

fn push(state: State, value: Ext80) -> State {
    execute(state, Instruction::Load(Operand::ExtendedMemory(value))).state
}

#[test]
fn store_preparation_is_atomic_and_keeps_physical_register_aliases() {
    assert_eq!(std::mem::size_of::<State>(), 128);
    let state = execute(
        State::default(),
        Instruction::Load(Operand::Memory {
            format: Format::Float32,
            bits: 0x3fc0_0000,
        }),
    )
    .state;
    let state = execute(
        state,
        Instruction::Arithmetic {
            operation: Arithmetic::Multiply,
            left: Operand::Register(0),
            right: Operand::Memory {
                format: Format::Float32,
                bits: 0x4000_0000,
            },
            destination: 0,
            pop: false,
        },
    )
    .state;
    let saved = state.clone();
    let prepared = state
        .prepare(
            Instruction::StoreMemory {
                format: Format::Float32,
                pop: true,
            },
            Location::default(),
        )
        .unwrap();
    assert_eq!(state, saved); // Guest write fault may discard the prepared result.
    assert_eq!(
        &prepared.store.unwrap().bytes[..4],
        &0x4040_0000u32.to_le_bytes()
    );
    assert_eq!(prepared.state.tag, 0xffff);
    assert_eq!(prepared.state.top(), 0);
    assert_eq!(prepared.state.registers[7], saved.registers[7]); // MMX alias survives pop.
    let initialized = execute(saved.clone(), Instruction::Initialize).state;
    assert_eq!(initialized.registers, saved.registers);
    assert_eq!(initialized.tag, 0xffff);
}

#[test]
fn unmasked_invalid_is_deferred_and_clear_does_not_replay() {
    let state = State {
        control: 0x37e,
        ..State::default()
    };
    let prepared = execute(state.clone(), Instruction::Load(Operand::Register(0)));
    assert!(prepared.deferred_exception);
    assert_eq!(prepared.state.top(), 0);
    assert_eq!(prepared.state.tag, 0xffff);
    assert_eq!(prepared.state.registers, state.registers);
    assert_eq!(
        prepared.state.status as u16,
        INVALID | STACK_FAULT | EXCEPTION_SUMMARY | BUSY
    );
    assert_eq!(
        prepared
            .state
            .prepare(Instruction::ChangeSign, Location::default())
            .unwrap_err(),
        Trap::PendingException(INVALID)
    );
    let cleared = execute(prepared.state, Instruction::ClearExceptions).state;
    assert_eq!(cleared.status, 0);
    assert_eq!(cleared.instruction_pointer, 0x1234);
    let push = execute(cleared, Instruction::Load(Operand::Constant(Ext80::ONE)));
    assert!(!push.deferred_exception);
    assert_eq!(push.state.read(0).unwrap(), Some(Ext80::ONE));
}

#[test]
fn unsupported_unmasked_range_result_leaves_live_state_exact() {
    let state = push(
        State {
            control: 0x377,
            ..State::default()
        },
        Ext80 {
            significand: u64::MAX,
            sign_exponent: 0x7ffe,
        },
    );
    let saved = state.clone();
    let result = state.prepare(
        Instruction::Arithmetic {
            operation: Arithmetic::Add,
            left: Operand::Register(0),
            right: Operand::Register(0),
            destination: 0,
            pop: false,
        },
        Location::default(),
    );
    assert_eq!(
        result.unwrap_err(),
        Trap::Unsupported(Unsupported::UnmaskedRangeException)
    );
    assert_eq!(state, saved);
}

#[test]
#[cfg(target_arch = "x86_64")]
fn stack_faults_and_deferred_exceptions_match_native() {
    for control in [0x37f, 0x37e, 0x35f, 0x37b] {
        for kind in 0..7 {
            let mut state = State {
                control,
                ..State::default()
            };
            let instruction = match kind {
                0 => Instruction::Load(Operand::Register(0)),
                1 => {
                    for _ in 0..8 {
                        state = push(state, Ext80::ONE);
                    }
                    Instruction::Load(Operand::Constant(Ext80::ONE))
                }
                2 => Instruction::StoreMemory {
                    format: Format::Float32,
                    pop: true,
                },
                3 => {
                    state = push(state, Ext80::ONE);
                    Instruction::Arithmetic {
                        operation: Arithmetic::Add,
                        left: Operand::Register(0),
                        right: Operand::Register(1),
                        destination: 0,
                        pop: false,
                    }
                }
                4 => {
                    state = push(state, Ext80::ONE);
                    Instruction::Exchange(1)
                }
                5 => {
                    state = push(
                        state,
                        Ext80 {
                            significand: 0x8000_0000_0000_0001,
                            sign_exponent: 0x3fff,
                        },
                    );
                    Instruction::StoreMemory {
                        format: Format::Float32,
                        pop: true,
                    }
                }
                _ => {
                    state = push(push(state, Ext80::ONE), Ext80::ZERO);
                    Instruction::Arithmetic {
                        operation: Arithmetic::Divide,
                        left: Operand::Register(1),
                        right: Operand::Register(0),
                        destination: 1,
                        pop: true,
                    }
                }
            };
            let prepared = execute(state, instruction);
            let mut expected = [0u64; 4];
            // SAFETY: fixed-size raw state probe, host FP/SIMD context restored.
            unsafe {
                vine_x87_native_stack(kind, control, expected.as_mut_ptr());
            }
            assert_eq!(
                prepared.state.status as u64 & 0xbaff,
                expected[0] & 0xbaff,
                "kind={kind} cw={control:x}"
            );
            assert_eq!(
                u64::from(prepared.state.tag),
                expected[1],
                "kind={kind} cw={control:x}"
            );
            if matches!(kind, 2 | 5) {
                let bits = prepared.store.map_or(0x12345678, |value| {
                    u32::from_le_bytes(value.bytes[..4].try_into().unwrap())
                });
                assert_eq!(u64::from(bits), expected[2], "kind={kind} cw={control:x}");
            }
        }
    }
}

#[test]
#[cfg(target_arch = "x86_64")]
fn actual_audio_instruction_families_keep_memory_nan_and_denormal_semantics() {
    let operands = [
        Ext80::ZERO,
        Ext80::ONE,
        Ext80 {
            significand: 0x8000_0000_0000_0001,
            sign_exponent: 0x3fff,
        },
        Ext80 {
            significand: 0xc000_0000_0000_0001,
            sign_exponent: 0x7fff,
        },
        Ext80 {
            significand: 0x8000_0000_0000_0001,
            sign_exponent: 0x7fff,
        },
    ];
    for control in [0x37f, 0xf7f, 0x37e, 0x37d, 0x37b] {
        for a in operands {
            for bits in [
                0u32,
                1,
                0x3f80_0000,
                0x3f80_0001,
                0x7f80_0000,
                0x7f80_0001,
                0x7fbf_ffff,
                0x7fc0_0001,
            ] {
                for op in 0..7 {
                    let operand = Operand::Memory {
                        format: Format::Float32,
                        bits: u64::from(bits),
                    };
                    let state = push(
                        State {
                            control,
                            ..State::default()
                        },
                        a,
                    );
                    let instruction = match op {
                        0..=3 => Instruction::Arithmetic {
                            operation: [
                                Arithmetic::Add,
                                Arithmetic::Subtract,
                                Arithmetic::Multiply,
                                Arithmetic::Divide,
                            ][op as usize],
                            left: Operand::Register(0),
                            right: operand,
                            destination: 0,
                            pop: false,
                        },
                        4 | 5 => Instruction::Compare {
                            right: operand,
                            quiet: false,
                            pops: u8::from(op == 5),
                        },
                        _ => Instruction::Load(operand),
                    };
                    let prepared = execute(state, instruction);
                    let mut expected = [0u64; 4];
                    // SAFETY: fixed-size native operation with restored host environment.
                    unsafe {
                        vine_x87_native_memory(
                            op,
                            a.significand,
                            u32::from(a.sign_exponent),
                            bits,
                            control,
                            expected.as_mut_ptr(),
                        );
                    }
                    let label = format!("op={op} a={a:?} bits={bits:x} cw={control:x}");
                    if op != 5 {
                        assert_eq!(
                            prepared.state.read(0).unwrap(),
                            Some(Ext80 {
                                significand: expected[0],
                                sign_exponent: expected[1] as u16
                            }),
                            "{label}"
                        );
                    }
                    // Intel specifies no condition-code update on unmasked
                    // invalid. This AMD host sets unordered; retain the Intel
                    // rule and compare the remaining native state explicitly.
                    if matches!(op, 4 | 5) && prepared.deferred_exception {
                        assert_eq!(prepared.state.status & 0x4500, 0, "{label}");
                    }
                    let mask = if matches!(op, 4 | 5) && !prepared.deferred_exception {
                        0xffff
                    } else {
                        0xbaff
                    };
                    assert_eq!(
                        u64::from(prepared.state.status) & mask,
                        expected[3] & mask,
                        "{label}"
                    );
                }
            }
        }
    }
}

#[test]
fn unary_instructions_do_not_read_unused_operands_and_pending_precedes_memory() {
    let state = push(State::default(), Ext80::ONE);
    let result = execute(
        state,
        Instruction::Arithmetic {
            operation: Arithmetic::SquareRoot,
            left: Operand::Register(0),
            right: Operand::Register(255),
            destination: 0,
            pop: false,
        },
    );
    assert_eq!(result.state.read(0).unwrap(), Some(Ext80::ONE));
    let pending = State {
        control: 0x37e,
        status: 0x80c1,
        ..State::default()
    };
    assert_eq!(
        pending.check_waiting(),
        Err(Trap::PendingException(INVALID))
    );
    assert_eq!(State::default().check_waiting(), Ok(()));
}

#[test]
#[cfg(target_arch = "x86_64")]
fn raw_register_transfers_and_empty_exchange_match_native() {
    for value in [
        Ext80::ONE,
        Ext80::INDEFINITE,
        Ext80 {
            significand: 0x8000_0000_0000_0001,
            sign_exponent: 0x7fff,
        },
        Ext80 {
            significand: 0x4000_0000_0000_0000,
            sign_exponent: 0x3fff,
        },
        Ext80 {
            significand: 0x8000_0000_0000_0000,
            sign_exponent: 0,
        },
    ] {
        for kind in 0..4 {
            let state = push(State::default(), value);
            let instruction = match kind {
                0 => Instruction::Load(Operand::Register(0)),
                1 => Instruction::StoreRegister {
                    index: 1,
                    pop: false,
                },
                2 => Instruction::StoreRegister {
                    index: 1,
                    pop: true,
                },
                _ => Instruction::Exchange(1),
            };
            let prepared = execute(state, instruction);
            let mut expected = [0u64; 6];
            // SAFETY: raw-transfer probe saves/restores host state and fills six words.
            unsafe {
                vine_x87_native_copies(
                    kind,
                    value.significand,
                    u32::from(value.sign_exponent),
                    expected.as_mut_ptr(),
                );
            }
            let label = format!("kind={kind} value={value:?}");
            assert_eq!(
                u64::from(prepared.state.status) & 0xbaff,
                expected[0] & 0xbaff,
                "{label}"
            );
            assert_eq!(u64::from(prepared.state.tag), expected[1], "{label}");
            for index in 0..2 {
                if let Some(actual) = prepared.state.read(index).unwrap() {
                    assert_eq!(
                        actual,
                        Ext80 {
                            significand: expected[2 + usize::from(index) * 2],
                            sign_exponent: expected[3 + usize::from(index) * 2] as u16
                        },
                        "{label} st{index}"
                    );
                }
            }
        }
    }
}
