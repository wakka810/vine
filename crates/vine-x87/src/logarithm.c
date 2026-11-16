/* Vine's bounded logarithm, using unmodified SoftFloat binary128 operations.
 * No host floating point, guest state, or memory authority is used here.
 * The Rust arithmetic lock covers every use of SoftFloat's global rounding.
 *
 * x = 2^e m, 3/4 <= m < 3/2; u = |m-1|/(m+1) <= 1/5.
 * ln(m) = sign(m-1) * 2 sum(u^(2k+1)/(2k+1), k>=0).
 * Forty terms leave a positive remainder <= 2 u^81/(81(1-u^2)).
 * Every arithmetic operation rounds outwards; the tail is an upper bound,
 * not a heuristic epsilon. ln(2)'s adjacent binary128 bounds are certified
 * by the independent rational-series oracle (see README.md).
 *
 * y is an exact normalized significand plus a separate exponent. The exponent
 * is applied only during final ext80 packing, so binary128's exponent range
 * cannot prematurely overflow/underflow the x87 product. Success requires
 * both endpoints to agree on RC and RZ values and range exceptions. */
#include <stdint.h>
#include "platform.h"
#include "internals.h"
#include "softfloat.h"

typedef struct { float128_t lo, hi; } interval;

static float128_t negate(float128_t a) { a.v[1] ^= UINT64_C(0x8000000000000000); return a; }
static int negative(float128_t a) { return (int)(a.v[1] >> 63); }
static interval exact(float128_t a) { interval r = {a,a}; return r; }
static interval neg(interval a) { interval r = {negate(a.hi),negate(a.lo)}; return r; }
static interval add(interval a, interval b) {
    interval r;
    softfloat_roundingMode = softfloat_round_min; r.lo = f128_add(a.lo,b.lo);
    softfloat_roundingMode = softfloat_round_max; r.hi = f128_add(a.hi,b.hi);
    return r;
}
static interval sub(interval a, interval b) { return add(a,neg(b)); }
/* Both operands are nonnegative. */
static interval positive_mul(interval a, interval b) {
    interval r;
    softfloat_roundingMode = softfloat_round_min; r.lo = f128_mul(a.lo,b.lo);
    softfloat_roundingMode = softfloat_round_max; r.hi = f128_mul(a.hi,b.hi);
    return r;
}
/* Denominator is strictly positive; numerator has one sign (or zero). */
static interval divide(interval a, interval b) {
    interval r;
    softfloat_roundingMode = softfloat_round_min;
    r.lo = f128_div(a.lo,negative(a.lo) ? b.lo : b.hi);
    softfloat_roundingMode = softfloat_round_max;
    r.hi = f128_div(a.hi,negative(a.hi) ? b.hi : b.lo);
    return r;
}
/* Multiply an interval by a single exact positive value. */
static interval scale(interval a, float128_t b) {
    interval r;
    softfloat_roundingMode = softfloat_round_min; r.lo = f128_mul(a.lo,b);
    softfloat_roundingMode = softfloat_round_max; r.hi = f128_mul(a.hi,b);
    return r;
}
static int same80(extFloat80_t a, extFloat80_t b) {
    return a.signif == b.signif && a.signExp == b.signExp;
}
/* Endpoints are normal binary128 here: the least logarithm magnitude is near
 * 2^-64 and the greatest is below 2^15; |normalized y| is in [1,2). */
static extFloat80_t pack_scaled(float128_t a, int scale_exponent, uint8_t mode,
                               uint8_t *flags) {
    uint64_t high = a.v[1];
    int32_t exponent = (int32_t)((high >> 48) & 0x7fff) + scale_exponent;
    struct uint128 significand = softfloat_shortShiftLeft128(
        (high & UINT64_C(0x0000ffffffffffff)) | UINT64_C(0x0001000000000000),
        a.v[0],15);
    softfloat_roundingMode = mode;
    softfloat_exceptionFlags = 0;
    extFloat80_t result = softfloat_roundPackToExtF80(
        high >> 63,exponent,significand.v64,significand.v0,80);
    *flags = (uint8_t)softfloat_exceptionFlags;
    return result;
}

/* Inputs: positive finite x, finite nonzero y; x's significand is not a power
 * of two. Special/exact cases are classified by the Rust numeric wrapper.
 * Returns zero when the enclosure cannot certify a unique rounded result.
 * The output buffer is untouched on that failure. */
uint32_t vine_log2_multiply(uint64_t x_sig, uint32_t x_exp,
                           uint64_t y_sig, uint32_t y_exp,
                           uint32_t rounding, uint64_t out[4]) {
    int x_shift = softfloat_countLeadingZeros64(x_sig);
    int y_shift = softfloat_countLeadingZeros64(y_sig);
    int e = (int)(x_exp & 0x7fff);
    int y_scale = (int)(y_exp & 0x7fff);
    e = (e ? e : 1) - 16383 - x_shift;
    y_scale = (y_scale ? y_scale : 1) - 16383 - y_shift;
    x_sig <<= x_shift;
    y_sig <<= y_shift;
    int reduced = x_sig >= UINT64_C(0xc000000000000000);
    extFloat80_t xm = {x_sig,(uint16_t)(0x3fff-reduced)};
    extFloat80_t ym = {y_sig,0x3fff};
    e += reduced;
    interval m = exact(extF80_to_f128(xm));
    interval one = exact(i32_to_f128(1));
    interval difference = sub(m,one);
    if (reduced) difference = neg(difference);
    interval u = divide(difference,add(m,one));
    interval square = positive_mul(u,u);
    interval power = u;
    interval sum = exact(i32_to_f128(0));
    for (int k=0; k<40; ++k) {
        sum = add(sum,divide(power,exact(i32_to_f128(2*k+1))));
        power = positive_mul(power,square);
    }
    interval log_m = scale(sum,i32_to_f128(2));
    interval tail = divide(scale(power,i32_to_f128(2)),
                          scale(sub(one,square),i32_to_f128(81)));
    softfloat_roundingMode = softfloat_round_max;
    log_m.hi = f128_add(log_m.hi,tail.hi);
    if (reduced) log_m = neg(log_m);
    const interval ln2 = {
        {{UINT64_C(0xf35793c7673007e5),UINT64_C(0x3ffe62e42fefa39e)}},
        {{UINT64_C(0xf35793c7673007e6),UINT64_C(0x3ffe62e42fefa39e)}}
    };
    interval logarithm = add(exact(i32_to_f128(e)),divide(log_m,ln2));
    interval product = scale(logarithm,extF80_to_f128(ym));
    if (y_exp & 0x8000) product = neg(product);
    const uint8_t modes[4] = {softfloat_round_near_even,softfloat_round_min,
                             softfloat_round_max,softfloat_round_minMag};
    softfloat_detectTininess = softfloat_tininess_afterRounding;
    uint8_t lo_flags, hi_flags, rz_lo_flags, rz_hi_flags;
    extFloat80_t lo = pack_scaled(product.lo,y_scale,modes[rounding],&lo_flags);
    extFloat80_t hi = pack_scaled(product.hi,y_scale,modes[rounding],&hi_flags);
    extFloat80_t rz_lo = pack_scaled(product.lo,y_scale,softfloat_round_minMag,&rz_lo_flags);
    extFloat80_t rz_hi = pack_scaled(product.hi,y_scale,softfloat_round_minMag,&rz_hi_flags);
    const uint8_t range = softfloat_flag_overflow | softfloat_flag_underflow;
    if (!same80(lo,hi) || !same80(rz_lo,rz_hi) ||
        ((lo_flags ^ hi_flags) & range) || ((rz_lo_flags ^ rz_hi_flags) & range)) return 0;
    /* For non-power-of-two rational x and finite nonzero rational y, the
     * mathematical result is irrational and hence necessarily inexact. */
    uint32_t flags = 32 | ((lo_flags & softfloat_flag_overflow) ? 8 : 0) |
                         ((lo_flags & softfloat_flag_underflow) ? 16 : 0);
    out[0] = lo.signif; out[1] = lo.signExp; out[2] = 0;
    out[3] = flags | ((!same80(lo,rz_lo)) << 16);
    return 1;
}
