//! Architectural FLD constants, rounded from the internal constant by RC.
//! Intel SDM Vol. 1 §8.3.4 specifies full extended precision independent of PC,
//! with neither a precision exception nor a rounding-up C1 indication.
use crate::Ext80;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Constant {
    Zero = 0,
    One = 1,
    Log2Ten = 2,
    Log2E = 3,
    Pi = 4,
    Log10Two = 5,
    LnTwo = 6,
}

impl Constant {
    pub const fn from_id(id: u8) -> Option<Self> {
        Some(match id {
            0 => Self::Zero,
            1 => Self::One,
            2 => Self::Log2Ten,
            3 => Self::Log2E,
            4 => Self::Pi,
            5 => Self::Log10Two,
            6 => Self::LnTwo,
            _ => return None,
        })
    }

    pub const fn value(self, control: u16) -> Ext80 {
        let (lower, exponent, nearest_up) = match self {
            Self::Zero => return Ext80::ZERO,
            Self::One => return Ext80::ONE,
            Self::Log2Ten => (0xd49a_784b_cd1b_8afe, 0x4000, false),
            Self::Log2E => (0xb8aa_3b29_5c17_f0bb, 0x3fff, true),
            Self::Pi => (0xc90f_daa2_2168_c234, 0x4000, true),
            Self::Log10Two => (0x9a20_9a84_fbcf_f798, 0x3ffd, true),
            Self::LnTwo => (0xb172_17f7_d1cf_79ab, 0x3ffe, true),
        };
        let increment = match control >> 10 & 3 {
            0 => nearest_up,
            2 => true,
            _ => false,
        };
        Ext80 {
            significand: lower + increment as u64,
            sign_exponent: exponent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{C1, Instruction, Location, Operand, PRECISION, State};

    #[test]
    fn constant_load_ignores_precision_and_never_introduces_precision_or_rounding_c1() {
        for id in 0..7 {
            let kind = Constant::from_id(id).unwrap();
            for rounding in 0..4 {
                for precision in [0, 2, 3] {
                    for sticky in [0, PRECISION] {
                        // Precision is unmasked only when not already pending.
                        let mut state = State {
                            control: 0x7f | (precision << 8) | (rounding << 10),
                            status: u32::from(sticky | C1),
                            ..State::default()
                        };
                        if sticky == 0 {
                            state.control &= !u32::from(PRECISION);
                        }
                        let value = kind.value(state.control as u16);
                        let prepared = state
                            .prepare(
                                Instruction::Load(Operand::Constant(value)),
                                Location::default(),
                            )
                            .unwrap();
                        assert_eq!(prepared.state.read(0).unwrap(), Some(value));
                        assert_eq!(
                            prepared.state.status & u32::from(PRECISION | C1),
                            u32::from(sticky)
                        );
                        assert_eq!(value, kind.value((rounding << 10) as u16));
                        assert_eq!(prepared.state.control, state.control);
                        assert!(prepared.store.is_none());
                    }
                }
            }
        }
        assert_eq!(Constant::from_id(7), None);
    }
}
