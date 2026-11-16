/* F2XM1: a bounded enclosure of 2^x-1, using SoftFloat binary128 only.
 * For |x|<=1, w=|x|ln2<1 and exp(w)-1=sum(w^n/n!,n>=1).
 * Forty terms plus w^41/41!/(1-w/42) enclose the positive tail.
 * exp(-w)-1=-expm1(w)/(1+expm1(w)), without cancellation.
 * Tiny x uses a normalized significand and an external exponent: for
 * |x|<2^-128, |expm1(x ln2)/x-ln2|<2^-127. Thus intermediate binary128
 * underflow cannot erase ext80 subnormals or an unmasked UE's biased result.
 * Every operation rounds outwards. Both RC and RZ endpoints must agree;
 * otherwise no result is published. The caller holds the SoftFloat lock. */
#include <stdint.h>
#include "platform.h"
#include "internals.h"
#include "softfloat.h"

typedef struct { float128_t lo, hi; } interval;
static float128_t neg(float128_t a) { a.v[1]^=UINT64_C(0x8000000000000000); return a; }
static interval negative(interval a) { interval r={neg(a.hi),neg(a.lo)}; return r; }
static interval exact(float128_t a) { interval r={a,a}; return r; }
static interval add(interval a, interval b) {
    interval r;
    softfloat_roundingMode=softfloat_round_min; r.lo=f128_add(a.lo,b.lo);
    softfloat_roundingMode=softfloat_round_max; r.hi=f128_add(a.hi,b.hi);
    return r;
}
static interval mul(interval a, interval b) {
    interval r;
    softfloat_roundingMode=softfloat_round_min; r.lo=f128_mul(a.lo,b.lo);
    softfloat_roundingMode=softfloat_round_max; r.hi=f128_mul(a.hi,b.hi);
    return r;
}
static interval div_positive(interval a, interval b) {
    interval r;
    softfloat_roundingMode=softfloat_round_min; r.lo=f128_div(a.lo,b.hi);
    softfloat_roundingMode=softfloat_round_max; r.hi=f128_div(a.hi,b.lo);
    return r;
}
static int same(extFloat80_t a, extFloat80_t b) {
    return a.signif==b.signif && a.signExp==b.signExp;
}
typedef struct { extFloat80_t value; uint8_t flags; } packed;
static packed pack(float128_t a, int scale, uint8_t mode) {
    uint64_t h=a.v[1];
    struct uint128 s=softfloat_shortShiftLeft128(
        (h&UINT64_C(0x0000ffffffffffff))|UINT64_C(0x0001000000000000),a.v[0],15);
    softfloat_roundingMode=mode; softfloat_exceptionFlags=0;
    packed r;
    r.value=softfloat_roundPackToExtF80(h>>63,(int)((h>>48)&0x7fff)+scale,s.v64,s.v0,80);
    r.flags=softfloat_exceptionFlags;
    return r;
}

/* Rust handles zero, +/-1, NaNs, infinities, and unsupported/domain cases. */
uint32_t vine_exponential_minus_one(uint64_t xs, uint32_t xe, uint32_t rounding,
                                   uint32_t underflow_unmasked, uint64_t out[4]) {
    const interval ln2={
        {{UINT64_C(0xf35793c7673007e5),UINT64_C(0x3ffe62e42fefa39e)}},
        {{UINT64_C(0xf35793c7673007e6),UINT64_C(0x3ffe62e42fefa39e)}}
    };
    const uint8_t modes[4]={softfloat_round_near_even,softfloat_round_min,
                            softfloat_round_max,softfloat_round_minMag};
    int e=(int)(xe&0x7fff), shift=softfloat_countLeadingZeros64(xs), scale=0;
    e=(e ? e : 1)-16383-shift; xs<<=shift;
    interval value;
    if (e < -128) {
        float128_t epsilon={{0,UINT64_C(0x3f80000000000000)}}; /* 2^-127 */
        interval ratio=ln2;
        if (xe&0x8000) {
            softfloat_roundingMode=softfloat_round_min;
            ratio.lo=f128_sub(ratio.lo,epsilon);
        } else {
            softfloat_roundingMode=softfloat_round_max;
            ratio.hi=f128_add(ratio.hi,epsilon);
        }
        extFloat80_t normalized={xs,0x3fff};
        value=mul(exact(extF80_to_f128(normalized)),ratio);
        scale=e;
    } else {
        extFloat80_t x={xs,(uint16_t)(e+16383)};
        interval w=mul(exact(extF80_to_f128(x)),ln2);
        interval one=exact(i32_to_f128(1)), term=w, sum=w;
        for (int n=2;n<=40;++n) {
            term=div_positive(mul(term,w),exact(i32_to_f128(n)));
            sum=add(sum,term);
        }
        term=div_positive(mul(term,w),exact(i32_to_f128(41)));
        interval denominator=add(one,negative(div_positive(w,exact(i32_to_f128(42)))));
        interval tail=div_positive(term,denominator);
        softfloat_roundingMode=softfloat_round_max;
        sum.hi=f128_add(sum.hi,tail.hi);
        value=(xe&0x8000) ? div_positive(sum,add(one,sum)) : sum;
    }
    if (xe&0x8000) value=negative(value);
    softfloat_detectTininess=softfloat_tininess_afterRounding;
    packed lo=pack(value.lo,scale,modes[rounding]), hi=pack(value.hi,scale,modes[rounding]);
    if (!same(lo.value,hi.value) || ((lo.flags^hi.flags)&softfloat_flag_underflow)) return 0;
    int tiny=(lo.flags&softfloat_flag_underflow)!=0;
    if (tiny && underflow_unmasked) {
        scale+=24576;
        lo=pack(value.lo,scale,modes[rounding]); hi=pack(value.hi,scale,modes[rounding]);
        if (!same(lo.value,hi.value)) return 0;
    }
    packed rzlo=pack(value.lo,scale,softfloat_round_minMag), rzhi=pack(value.hi,scale,softfloat_round_minMag);
    if (!same(rzlo.value,rzhi.value)) return 0;
    out[0]=lo.value.signif; out[1]=lo.value.signExp; out[2]=0;
    out[3]=32|(tiny ? 16:0)|((!same(lo.value,rzlo.value))<<16);
    return 1;
}
