/* Vine adapter, not an alteration of the vendored arithmetic implementation.
 * No native floating point is used here. Rust serializes calls because the
 * upstream rounding/exception variables describe one operation at a time. */
#include <stdint.h>
#include "platform.h"
#include "softfloat.h"

static uint64_t execute(uint32_t op, extFloat80_t a, extFloat80_t b, extFloat80_t *z) {
    switch (op) {
    case 0: *z = extF80_add(a, b); break;
    case 1: *z = extF80_sub(a, b); break;
    case 2: *z = extF80_mul(a, b); break;
    case 3: *z = extF80_div(a, b); break;
    case 4: *z = extF80_sqrt(a); break;
    case 5: *z = extF80_roundToInt(a, softfloat_roundingMode, true); break;
    case 6: { float32_t v = {(uint32_t)a.signif}; *z = f32_to_extF80(v); break; }
    case 7: { float64_t v = {a.signif}; *z = f64_to_extF80(v); break; }
    case 8: *z = i32_to_extF80((int32_t)a.signif); break;
    case 9: *z = i64_to_extF80((int64_t)a.signif); break;
    case 10: return extF80_to_f32(a).v;
    case 11: return extF80_to_f64(a).v;
    case 12: return (uint32_t)extF80_to_i32(a, softfloat_roundingMode, true);
    case 13: return (uint64_t)extF80_to_i64(a, softfloat_roundingMode, true);
    case 14:
    case 15: {
        int less = op == 14 ? extF80_lt_quiet(a,b) : extF80_lt(a,b);
        int equal = op == 14 ? extF80_eq(a,b) : extF80_eq_signaling(a,b);
        int nan = ((a.signExp & 0x7fff) == 0x7fff && (a.signif << 1)) ||
                  ((b.signExp & 0x7fff) == 0x7fff && (b.signif << 1));
        return nan ? 3 : equal ? 0 : less ? 1 : 2;
    }
    }
    return 0;
}

void vine_softfloat(uint32_t op, uint64_t a_sig, uint32_t a_exp,
                   uint64_t b_sig, uint32_t b_exp, uint32_t precision,
                   uint32_t rounding, uint64_t out[4]) {
    static const uint8_t modes[4] = {
        softfloat_round_near_even, softfloat_round_min,
        softfloat_round_max, softfloat_round_minMag
    };
    extFloat80_t a = {a_sig, (uint16_t)a_exp}, b = {b_sig, (uint16_t)b_exp};
    extFloat80_t z = {0,0}, truncated = {0,0};
    softfloat_roundingMode = modes[rounding];
    softfloat_detectTininess = softfloat_tininess_afterRounding;
    extF80_roundingPrecision = precision;
    softfloat_exceptionFlags = 0;
    uint64_t bits = execute(op, a, b, &z);
    uint32_t flags = softfloat_exceptionFlags;
    uint32_t round_up = 0;
    if ((flags & softfloat_flag_inexact) && !(flags & softfloat_flag_invalid) && rounding != 3) {
        /* x87 C1 records a magnitude increment, not the sign of the result.
         * An exact second calculation with truncation determines this without
         * modifying upstream code. Its cost is part of backend execution. */
        softfloat_roundingMode = softfloat_round_minMag;
        softfloat_exceptionFlags = 0;
        uint64_t truncated_bits = execute(op, a, b, &truncated);
        round_up = bits != truncated_bits || z.signif != truncated.signif || z.signExp != truncated.signExp;
    }
    uint32_t x87_flags = ((flags & 16) >> 4) | ((flags & 8) >> 1) |
                         ((flags & 4) << 1) | ((flags & 2) << 3) | ((flags & 1) << 5);
    out[0] = z.signif;
    out[1] = z.signExp;
    out[2] = bits;
    out[3] = x87_flags | (round_up << 16);
}

/* Pure SSE scalar conversion: no x87 control/state and no host casts. The
 * caller handles DAZ before this function and owns MXCSR masks/sticky flags.
 * CVTTSS2SI/CVTTSD2SI raise only invalid and precision, never denormal. */
uint64_t vine_sse_cvtt(uint32_t wide, uint64_t bits) {
    softfloat_exceptionFlags = 0;
    int32_t value;
    if (wide) {
        float64_t input = {bits};
        value = (int32_t)f64_to_i32(input, softfloat_round_minMag, true);
    } else {
        float32_t input = {(uint32_t)bits};
        value = (int32_t)f32_to_i32(input, softfloat_round_minMag, true);
    }
    uint32_t flags = ((softfloat_exceptionFlags & softfloat_flag_invalid) ? 1 : 0) |
                     ((softfloat_exceptionFlags & softfloat_flag_inexact) ? 32 : 0);
    return (uint32_t)value | ((uint64_t)flags << 32);
}

/* CVTSD2SI uses MXCSR.RC, still raising only IE/PE (never DE). */
uint64_t vine_sse_cvt_f64(uint64_t bits, uint32_t rounding) {
    static const uint8_t modes[4] = {
        softfloat_round_near_even, softfloat_round_min,
        softfloat_round_max, softfloat_round_minMag
    };
    softfloat_exceptionFlags = 0;
    float64_t input = {bits};
    int32_t value = (int32_t)f64_to_i32(input, modes[rounding], true);
    uint32_t flags = ((softfloat_exceptionFlags & softfloat_flag_invalid) ? 1 : 0) |
                     ((softfloat_exceptionFlags & softfloat_flag_inexact) ? 32 : 0);
    return (uint32_t)value | ((uint64_t)flags << 32);
}

/* CVTDQ2PS lane. i32_to_extF80 normalizes by integer shifts: all 32 input
 * bits fit exactly in the 64-bit significand, including INT32_MIN. Only the
 * final extF80_to_f32 rounds, so there is no intermediate/double rounding.
 * Neither operation reads extF80_roundingPrecision. No x87 C1 recalculation,
 * DAZ/FTZ, masks, sticky state or architectural destination belongs here. */
uint64_t vine_sse_i32_to_f32(int32_t value, uint32_t rounding) {
    static const uint8_t modes[4] = {
        softfloat_round_near_even, softfloat_round_min,
        softfloat_round_max, softfloat_round_minMag
    };
    softfloat_roundingMode = modes[rounding];
    softfloat_exceptionFlags = 0;
    float32_t result = extF80_to_f32(i32_to_extF80(value));
    uint32_t flags = (softfloat_exceptionFlags & softfloat_flag_inexact) ? 32 : 0;
    return result.v | ((uint64_t)flags << 32);
}
