//! Deliberate numerical policies, separate from x87 stack/commit semantics.
//! Approximate modes use host binary64/binary32 for ordinary finite, masked,
//! round-to-nearest calculations. Other controls, exceptional values and range
//! boundaries use the existing precision path. Masked PE is conservative and
//! C1 is cleared on approximate calculations; these modes are not bit exact.
use std::cell::Cell;
use vine_x87::{Ext80, NumericResult, PRECISION};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u32)]
pub enum NumericMode {
    #[default]
    Strict = 0,
    Balanced = 1,
    Fast = 2,
}
impl TryFrom<u32> for NumericMode {
    type Error = String;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Strict),
            1 => Ok(Self::Balanced),
            2 => Ok(Self::Fast),
            _ => Err("numeric mode must be strict, balanced or fast".into()),
        }
    }
}
impl NumericMode {
    pub fn import_prefix(self, operation: u32) -> &'static str {
        if !matches!(operation, 0..=4 | 20..=24) {
            return "fp_kernel";
        }
        match self {
            Self::Strict => "fp_kernel",
            Self::Balanced => "fp_balanced",
            Self::Fast => "fp_fast",
        }
    }
}

thread_local! {
    // Diagnostic only, per compiler instance; no CPU or memory authority.
    #[cfg_attr(all(target_os = "windows", target_env = "gnu"),
        allow(clippy::missing_const_for_thread_local,
            reason = "https://github.com/rust-lang/rust-clippy/issues/13422"))]
    static COUNTS: Cell<[u32; 6]> = const { Cell::new([0; 6]) };
}
#[unsafe(no_mangle)]
pub extern "C" fn backend_numeric_count(mode: u32, fallback: u32) -> u32 {
    if mode > 2 || fallback > 1 {
        return 0;
    }
    COUNTS.with(|c| c.get()[(mode * 2 + fallback) as usize])
}

/// Round a finite normalized ext80 operand to binary64 without SoftFloat or
/// an x87 state round-trip. Unsupported encodings and exponent edges decline.
fn binary64(value: Ext80) -> Option<f64> {
    let sign = u64::from(value.sign_exponent >> 15) << 63;
    if value.significand == 0 && value.exponent() == 0 {
        return Some(f64::from_bits(sign));
    }
    if value.is_unsupported() || !(15361..=17406).contains(&value.exponent()) {
        return None;
    }
    let mut significand = value.significand >> 11;
    let remainder = value.significand & 2047;
    if remainder > 1024 || remainder == 1024 && significand & 1 != 0 {
        significand += 1;
    }
    let mut exponent = u64::from(value.exponent() - 15360);
    if significand == 1 << 53 {
        significand >>= 1;
        exponent += 1;
    }
    if exponent >= 2047 {
        return None;
    }
    Some(f64::from_bits(
        sign | exponent << 52 | (significand & ((1 << 52) - 1)),
    ))
}

fn extended(value: f64) -> Option<Ext80> {
    let bits = value.to_bits();
    let sign = ((bits >> 63) as u16) << 15;
    if value == 0.0 {
        return Some(Ext80 {
            significand: 0,
            sign_exponent: sign,
        });
    }
    let exponent = ((bits >> 52) & 2047) as u16;
    if exponent == 0 || exponent == 2047 {
        return None;
    }
    Some(Ext80 {
        significand: ((bits & ((1 << 52) - 1)) | (1 << 52)) << 11,
        sign_exponent: sign | (exponent + 15360),
    })
}

#[inline(always)]
fn compute(
    mode: NumericMode,
    operation: u32,
    control: u16,
    a: Ext80,
    b: Ext80,
) -> Option<NumericResult> {
    // An unmasked exception must retain the exact instruction's trap/commit
    // decision. Directed rounding and reserved precision keep that path too.
    if mode == NumericMode::Strict
        || control & 0x3f != 0x3f
        || control & 0xc00 != 0
        || control & 0x300 == 0x100
    {
        return None;
    }
    if !matches!(operation, 0..=4 | 20..=24) {
        return None;
    }
    let x = binary64(a)?;
    let y = if matches!(operation, 0..=3 | 20 | 21) {
        binary64(b)?
    } else {
        0.0
    };
    if operation == 3 && y == 0.0
        || operation == 4 && x < 0.0
        || operation == 20 && x <= 0.0
        || operation == 24 && x.abs() > 1.0
    {
        return None;
    }
    let short = mode == NumericMode::Fast || control & 0x300 == 0;
    let trig_limit = if short { 4096.0 } else { 1048576.0 };
    if matches!(operation, 22 | 23) && x.abs() > trig_limit {
        return None;
    }
    let value = if short {
        let x = x as f32;
        let y = y as f32;
        if !x.is_finite()
            || !y.is_finite()
            || x.is_subnormal()
            || y.is_subnormal()
            || x == 0.0 && a.significand != 0
            || y == 0.0 && matches!(operation, 0..=3 | 20 | 21) && b.significand != 0
        {
            return None;
        }
        let value = match operation {
            0 => x + y,
            1 => x - y,
            2 => x * y,
            3 => x / y,
            4 => x.sqrt(),
            20 => y * x.log2(),
            21 => y.atan2(x),
            22 => x.cos(),
            23 => x.sin(),
            24 => (x * std::f32::consts::LN_2).exp_m1(),
            _ => unreachable!(),
        };
        if value.is_subnormal() {
            return None;
        }
        f64::from(value)
    } else {
        match operation {
            0 => x + y,
            1 => x - y,
            2 => x * y,
            3 => x / y,
            4 => x.sqrt(),
            20 => y * x.log2(),
            21 => y.atan2(x),
            22 => x.cos(),
            23 => x.sin(),
            24 => (x * std::f64::consts::LN_2).exp_m1(),
            _ => unreachable!(),
        }
    };
    // Do not turn arithmetic range loss into an apparently ordinary zero.
    if value == 0.0 && matches!(operation, 2..=4) && x != 0.0 && (operation == 4 || y != 0.0) {
        return None;
    }
    Some(NumericResult {
        value: extended(value)?,
        exceptions: PRECISION,
        ..NumericResult::default()
    })
}

#[inline(always)]
pub fn approximate(
    mode: NumericMode,
    operation: u32,
    control: u16,
    a: Ext80,
    b: Ext80,
) -> Option<NumericResult> {
    let result = compute(mode, operation, control, a, b);
    COUNTS.with(|c| {
        let mut counts = c.get();
        let index = mode as usize * 2 + usize::from(result.is_none());
        counts[index] = counts[index].saturating_add(1);
        c.set(counts);
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn approximation_retains_range_and_exception_boundaries() {
        let a = Ext80::ONE;
        for mode in [NumericMode::Balanced, NumericMode::Fast] {
            assert_eq!(
                approximate(mode, 0, 0x37f, a, a)
                    .unwrap()
                    .value
                    .sign_exponent,
                0x4000
            );
            assert!(approximate(mode, 3, 0x37f, a, Ext80::ZERO).is_none());
            assert!(approximate(mode, 0, 0x35f, a, a).is_none());
            assert!(approximate(mode, 0, 0x77f, a, a).is_none());
            assert!(approximate(mode, 0, 0x17f, a, a).is_none());
            assert!(approximate(mode, 0, 0x37f, Ext80::INDEFINITE, a).is_none());
        }
        let close = Ext80 {
            significand: a.significand + (1 << 33),
            ..a
        };
        assert_ne!(
            approximate(NumericMode::Balanced, 2, 0x37f, close, a)
                .unwrap()
                .value,
            approximate(NumericMode::Fast, 2, 0x37f, close, a)
                .unwrap()
                .value
        );
        for value in [0.0, -0.0, 1.0, -7.25, f64::MIN_POSITIVE, f64::MAX] {
            assert_eq!(
                binary64(extended(value).unwrap()).unwrap().to_bits(),
                value.to_bits()
            );
        }
    }
}
