//! Versioned backend-private context envelope. The architectural projection
//! remains exactly 420 bytes; Runtime transports this whole envelope opaquely.
use crate::{
    continuation::ComparisonContinuation,
    state::{CpuState, STATE_BYTES},
};
pub const SNAPSHOT_BYTES: usize = 512;
pub const SNAPSHOT_VERSION: u32 = 2;
const MAGIC: u32 = 0x32534e56; // VNS2

pub(crate) fn encode(
    state: &CpuState,
    scas: Option<&ComparisonContinuation>,
) -> Result<[u8; SNAPSHOT_BYTES], String> {
    if let Some(scas) = scas {
        scas.validate(state)?;
    }
    let mut bytes = [0; SNAPSHOT_BYTES];
    bytes[..STATE_BYTES].copy_from_slice(&state.snapshot());
    let mut put =
        |offset: usize, word: u32| bytes[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
    put(420, MAGIC);
    put(424, SNAPSHOT_VERSION);
    put(428, SNAPSHOT_BYTES as u32);
    if let Some(scas) = scas {
        put(432, 1);
        put(436, scas.eip);
        put(440, scas.original_flags);
        put(444, scas.original_defined);
        put(448, scas.length);
        for (index, value) in scas.expected.into_iter().enumerate() {
            put(468 + index * 4, value);
        }
        put(488, scas.descriptor);
        bytes[452..468].copy_from_slice(&scas.encoding);
    }
    Ok(bytes)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<(CpuState, Option<ComparisonContinuation>), String> {
    if bytes.len() != SNAPSHOT_BYTES {
        return Err("invalid backend snapshot length".into());
    }
    let word = |offset: usize| {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("four bytes"))
    };
    if word(420) != MAGIC
        || word(424) != SNAPSHOT_VERSION
        || word(428) != SNAPSHOT_BYTES as u32
        || word(432) > 1
        || bytes[492..].iter().any(|&byte| byte != 0)
    {
        return Err("invalid backend snapshot envelope".into());
    }
    let state = CpuState::from_snapshot(&bytes[..STATE_BYTES])?;
    let scas = if word(432) == 0 {
        if bytes[436..].iter().any(|&byte| byte != 0) {
            return Err("inactive snapshot contains continuation payload".into());
        }
        None
    } else {
        let value = ComparisonContinuation {
            eip: word(436),
            original_flags: word(440),
            original_defined: word(444),
            length: word(448),
            encoding: bytes[452..468].try_into().expect("sixteen bytes"),
            descriptor: word(488),
            expected: std::array::from_fn(|n| word(468 + n * 4)),
            in_flight: false,
        };
        value.validate(&state)?;
        Some(value)
    };
    Ok((state, scas))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_architecture_roundtrip_and_strict_envelope() {
        let mut state = CpuState::thread(0x1234, 0x9876, 0xabcd);
        state.x87.registers[7] = [0x1234, 0xfedc_ba98, 0xffff];
        state.xmm[3] = [1, 2, 3, 4];
        state.exit.kind = 16;
        let saved = encode(&state, None).unwrap();
        assert_eq!(&saved[..420], &state.snapshot());
        assert_eq!(decode(&saved).unwrap(), (state, None));
        for offset in [420, 424, 428, 432, 436, 492, 511] {
            let mut bad = saved;
            bad[offset] ^= 0x80;
            assert!(decode(&bad).is_err(), "offset {offset}");
        }
        assert!(decode(&saved[..420]).is_err());
    }
}
