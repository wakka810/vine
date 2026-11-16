/* FCOS's Intel-defined cos(x*pi/p), p=C90FDAA22168C234C*2^-66.
 * p/2=P*2^-65 with the odd 66-bit integer P=3243F6A8885A308D3.
 * Reduce the dyadic ext80 input modulo p/2 with exact integer arithmetic,
 * keeping the quadrant. Divide the exact residual by p, THEN multiply by a
 * certified interval for mathematical pi. Using cos(residual) would compute
 * a different function. No host floating point or guest-state mutation.
 *
 * The final argument is <=pi/4. Directed binary128 operations enclose a
 * 32-term alternating sine/cosine series and its next-term remainder. Both
 * RC rounding and the toward-zero value used for C1 must certify to identical
 * ext80 values across the interval. Otherwise return failure without output.
 * Nonzero admitted dyadic inputs cannot give 0, +/-1 or +/-1/2 here: P is odd
 * with 66 significant bits (P/3, if integral, still needs 65), whereas an
 * ext80 input has at most 64. The cosine is therefore irrational; endpoint
 * limits may be taken strictly inward. */
#include <stdint.h>
#include "platform.h"
#include "internals.h"
#include "softfloat.h"

typedef struct { float128_t lo, hi; } interval;
static float128_t negative(float128_t a) { a.v[1]^=UINT64_C(0x8000000000000000); return a; }
static interval exact(float128_t a) { interval r={a,a}; return r; }
static interval negate(interval a) { interval r={negative(a.hi),negative(a.lo)}; return r; }
static interval add(interval a, interval b) {
    interval r;
    softfloat_roundingMode=softfloat_round_min; r.lo=f128_add(a.lo,b.lo);
    softfloat_roundingMode=softfloat_round_max; r.hi=f128_add(a.hi,b.hi);
    return r;
}
static interval sub(interval a, interval b) { return add(a,negate(b)); }
/* All multiplication/division operands below are nonnegative. */
static interval mul(interval a, interval b) {
    interval r;
    softfloat_roundingMode=softfloat_round_min; r.lo=f128_mul(a.lo,b.lo);
    softfloat_roundingMode=softfloat_round_max; r.hi=f128_mul(a.hi,b.hi);
    return r;
}
static interval divide(interval a, interval b) {
    interval r;
    softfloat_roundingMode=softfloat_round_min; r.lo=f128_div(a.lo,b.hi);
    softfloat_roundingMode=softfloat_round_max; r.hi=f128_div(a.hi,b.lo);
    return r;
}
static float128_t power2(int e) { float128_t r={{0,(uint64_t)(e+16383)<<48}}; return r; }
static const interval pi={
    {{UINT64_C(0x8469898cc51701b8),UINT64_C(0x4000921fb54442d1)}},
    {{UINT64_C(0x8469898cc51701b9),UINT64_C(0x4000921fb54442d1)}}
};
static const uint64_t PLOW=UINT64_C(0x243f6a8885a308d3);
/* hi is at most 3; this sum is exactly representable in binary128. */
static interval integer66(uint64_t hi, uint64_t lo) {
    return add(exact(ui64_to_f128(lo)),mul(exact(ui64_to_f128(hi)),exact(power2(64))));
}
static int compare_p(uint64_t hi, uint64_t lo) {
    return hi>3 || (hi==3 && lo>=PLOW);
}
static int same80(extFloat80_t a, extFloat80_t b) { return a.signif==b.signif && a.signExp==b.signExp; }
static extFloat80_t pack_limit(float128_t a, int lower, uint8_t mode) {
    uint64_t high=a.v[1];
    int sign=(int)(high>>63), exponent=(int)((high>>48)&0x7fff);
    struct uint128 s=softfloat_shortShiftLeft128(
        (high&UINT64_C(0x0000ffffffffffff))|UINT64_C(0x0001000000000000),a.v[0],15);
    if (lower!=sign) { if (++s.v0==0) ++s.v64; }
    else {
        if (s.v0--==0) --s.v64;
        if (!(s.v64>>63)) { s=softfloat_shortShiftLeft128(s.v64,s.v0,1); --exponent; }
    }
    softfloat_roundingMode=mode; softfloat_exceptionFlags=0;
    return softfloat_roundPackToExtF80(sign,exponent,s.v64,s.v0,80);
}
static interval series(interval angle, int sine) {
    interval square=mul(angle,angle);
    interval term=sine ? angle : exact(i32_to_f128(1)), sum=term;
    for (int k=1;k<=32;++k) {
        int denominator=sine ? 2*k*(2*k+1) : (2*k-1)*(2*k);
        term=divide(mul(term,square),exact(i32_to_f128(denominator)));
        sum=(k&1) ? sub(sum,term) : add(sum,term);
    }
    int denominator=sine ? 66*67 : 65*66;
    interval tail=divide(mul(term,square),exact(i32_to_f128(denominator)));
    softfloat_roundingMode=softfloat_round_min;
    sum.lo=f128_sub(sum.lo,tail.hi);
    /* cos(a)<1 and sin(a)<a for every positive reduced argument. */
    float128_t bound=sine ? angle.hi : i32_to_f128(1);
    if (sum.hi.v[1]>bound.v[1] || (sum.hi.v[1]==bound.v[1] && sum.hi.v[0]>bound.v[0])) sum.hi=bound;
    return sum;
}
/* Near1 compute d=1-cos(a), not cos(a), so a correction smaller than a
 * binary128 ulp of1 still determines the correct ext80 midpoint side. */
static interval decrement(interval angle) {
    interval square=mul(angle,angle);
    interval term=mul(square,exact(power2(-1))), sum=term;
    for (int k=2;k<=32;++k) {
        term=divide(mul(term,square),exact(i32_to_f128((2*k-1)*(2*k))));
        sum=(k&1) ? add(sum,term) : sub(sum,term);
    }
    interval tail=divide(mul(term,square),exact(i32_to_f128(65*66)));
    softfloat_roundingMode=softfloat_round_max;
    sum.hi=f128_add(sum.hi,tail.hi);
    return sum;
}
/* d is normal with exponent between-67 and-33. Form (1-d)*2^128 as an
 * integer complement, with a sticky bit for its exact fractional tail. For
 * integral endpoints the inward limit is encoded before rounding; no f128
 * subtraction from1, and therefore no cancellation/double rounding occurs. */
static extFloat80_t pack_decrement(float128_t d, int lower_result, uint8_t mode) {
    uint64_t sh=(d.v[1]&UINT64_C(0x0000ffffffffffff))|UINT64_C(0x0001000000000000);
    int exponent=(int)((d.v[1]>>48)&0x7fff)-16383;
    int shift=-exponent-16; /*17..51, strictly between0 and64*/
    uint64_t dh=sh>>shift, dl=(sh<<(64-shift))|(d.v[0]>>shift);
    int fractional=(d.v[0]<<(64-shift))!=0;
    uint64_t yl=0-dl, yh=~dh+(yl==0);
    if (fractional || !lower_result) { if (yl--==0) --yh; }
    yl|=1;
    softfloat_roundingMode=mode; softfloat_exceptionFlags=0;
    return softfloat_roundPackToExtF80(0,16382,yh,yl,80);
}

/* Rust handles zero, nonfinite, unsupported, denormal exception priority and
 * |x|>=2^63. xs is nonzero; xe contains only the positive magnitude exponent. */
uint32_t vine_cosine(uint64_t xs, uint32_t xe, uint32_t rounding, uint64_t out[4]) {
    const uint8_t modes[4]={softfloat_round_near_even,softfloat_round_min,
                            softfloat_round_max,softfloat_round_minMag};
    /* For |x|<2^-33, 0<1-cos(x*pi/p)<2^-66, below half the gap
     * below 1 (2^-65). This also avoids tiny internal products underflowing. */
    if (xe<16383-33) {
        int up=rounding==0 || rounding==2;
        out[0]=up ? UINT64_C(0x8000000000000000) : UINT64_MAX;
        out[1]=up ? 0x3fff : 0x3ffe; out[2]=0; out[3]=32|((uint32_t)up<<16);
        return 1;
    }
    interval p_integer=integer66(3,PLOW), angle;
    int quadrant=0, residual_negative=0;
    int e=(int)xe-16383;
    if (e>=-2) {
        /* N=xs<<(e+2), with at most128 bits, divided bitwise by P.
         * Only quotient mod4 and the exact66-bit remainder are needed. */
        uint64_t rh=0, rl=0;
        int shift=e+2;
        for (int bit=63+shift;bit>=0;--bit) {
            rh=(rh<<1)|(rl>>63);
            rl=(rl<<1)|((bit>=shift) ? ((xs>>(bit-shift))&1) : 0);
            int digit=compare_p(rh,rl);
            if (digit) { uint64_t old=rl; rl-=PLOW; rh-=3+(old<PLOW); }
            quadrant=((quadrant<<1)|digit)&3;
        }
        /* Nearest quadrant gives residual magnitude <=p/4. P is odd, so
         * an exact half remainder cannot occur. */
        if (compare_p((rh<<1)|(rl>>63),rl<<1)) {
            uint64_t old=rl;
            rl=PLOW-rl; rh=3-rh-(PLOW<old);
            residual_negative=1; quadrant=(quadrant+1)&3;
        }
        angle=mul(divide(integer66(rh,rl),p_integer),mul(pi,exact(power2(-1))));
    } else {
        extFloat80_t x={xs,(uint16_t)xe};
        /* x*pi/p, where p=P*2^-64, before any series approximation. */
        angle=mul(divide(exact(extF80_to_f128(x)),mul(p_integer,exact(power2(-64)))),pi);
    }
    int sine=quadrant&1;
    int sign=quadrant==2 || (quadrant==1 && !residual_negative) || (quadrant==3 && residual_negative);
    if (e < -16) {
        interval d=decrement(angle);
        extFloat80_t lo=pack_decrement(d.hi,1,modes[rounding]), hi=pack_decrement(d.lo,0,modes[rounding]);
        extFloat80_t rzlo=pack_decrement(d.hi,1,softfloat_round_minMag), rzhi=pack_decrement(d.lo,0,softfloat_round_minMag);
        if (!same80(lo,hi) || !same80(rzlo,rzhi)) return 0;
        out[0]=lo.signif; out[1]=lo.signExp; out[2]=0;
        out[3]=32|((!same80(lo,rzlo))<<16);
        return 1;
    }
    interval value=series(angle,sine);
    if (sign) value=negate(value);
    softfloat_detectTininess=softfloat_tininess_afterRounding;
    extFloat80_t lo=pack_limit(value.lo,1,modes[rounding]), hi=pack_limit(value.hi,0,modes[rounding]);
    extFloat80_t rzlo=pack_limit(value.lo,1,softfloat_round_minMag), rzhi=pack_limit(value.hi,0,softfloat_round_minMag);
    if (!same80(lo,hi) || !same80(rzlo,rzhi)) return 0;
    out[0]=lo.signif; out[1]=lo.signExp; out[2]=0;
    out[3]=32|((!same80(lo,rzlo))<<16);
    return 1;
}

/* FSIN uses the same exact finite-p reduction and interval arithmetic, but
 * must retain the input sign and can produce tiny results. Keep the FCOS
 * entry and its established packing paths unchanged. */
typedef struct { extFloat80_t value; uint8_t flags; } sine_packed;
static sine_packed sine_pack_limit(float128_t a, int lower, uint8_t mode, int scale) {
    uint64_t high=a.v[1];
    int sign=(int)(high>>63), exponent=(int)((high>>48)&0x7fff)+scale;
    struct uint128 s=softfloat_shortShiftLeft128(
        (high&UINT64_C(0x0000ffffffffffff))|UINT64_C(0x0001000000000000),a.v[0],15);
    if (lower!=sign) { if (++s.v0==0) ++s.v64; }
    else {
        if (s.v0--==0) --s.v64;
        if (!(s.v64>>63)) { s=softfloat_shortShiftLeft128(s.v64,s.v0,1); --exponent; }
    }
    softfloat_detectTininess=softfloat_tininess_afterRounding;
    softfloat_roundingMode=mode; softfloat_exceptionFlags=0;
    sine_packed r;
    r.value=softfloat_roundPackToExtF80(sign,exponent,s.v64,s.v0,80);
    r.flags=softfloat_exceptionFlags;
    return r;
}

/* xe includes sign. xs is nonzero, finite and supported; Rust has already
 * handled denormal exception priority. A separate scale keeps the entire
 * ext80 exponent range out of intermediate binary128 underflow. */
uint32_t vine_sine(uint64_t xs, uint32_t xe, uint32_t rounding,
                   uint32_t underflow_unmasked, uint64_t out[4]) {
    const uint8_t modes[4]={softfloat_round_near_even,softfloat_round_min,
                            softfloat_round_max,softfloat_round_minMag};
    int input_sign=(int)(xe>>15), exponent=(int)(xe&0x7fff);
    int e=(exponent ? exponent : 1)-16383;
    while (!(xs>>63)) { xs<<=1; --e; }
    interval p_integer=integer66(3,PLOW), angle;
    int quadrant=0, residual_negative=0, scale=0;
    if (e>=-2) {
        uint64_t rh=0, rl=0;
        int shift=e+2;
        for (int bit=63+shift;bit>=0;--bit) {
            rh=(rh<<1)|(rl>>63);
            rl=(rl<<1)|((bit>=shift) ? ((xs>>(bit-shift))&1) : 0);
            int digit=compare_p(rh,rl);
            if (digit) { uint64_t old=rl; rl-=PLOW; rh-=3+(old<PLOW); }
            quadrant=((quadrant<<1)|digit)&3;
        }
        if (compare_p((rh<<1)|(rl>>63),rl<<1)) {
            uint64_t old=rl;
            rl=PLOW-rl; rh=3-rh-(PLOW<old);
            residual_negative=1; quadrant=(quadrant+1)&3;
        }
        angle=mul(divide(integer66(rh,rl),p_integer),mul(pi,exact(power2(-1))));
    } else {
        extFloat80_t x={xs,(uint16_t)(e < -64 ? 16383 : e+16383)};
        angle=mul(divide(exact(extF80_to_f128(x)),mul(p_integer,exact(power2(-64)))),pi);
        if (e < -64) scale=e;
    }
    int sine=!(quadrant&1);
    int sign=input_sign ^ (quadrant>=2) ^ (sine && residual_negative);
    interval value;
    if (scale) {
        /* t=x*pi/p / 2^e is in [1,2), and e<=-65. Hence
         * t*(1-2^-112) < sin(t*2^e)/2^e < t. The conservative
         * relative lower bound is exactly representable in binary128;
         * directed operations and endpoint certification retain all RC/C1
         * information without treating the tiny sine as an exact input. */
        float128_t below_one={{UINT64_C(0xfffffffffffffffe),UINT64_C(0x3ffeffffffffffff)}};
        softfloat_roundingMode=softfloat_round_min;
        value.lo=f128_mul(angle.lo,below_one);
        value.hi=angle.hi;
    } else {
        value=series(angle,sine);
    }
    if (sign) value=negate(value);
    sine_packed lo=sine_pack_limit(value.lo,1,modes[rounding],scale);
    sine_packed hi=sine_pack_limit(value.hi,0,modes[rounding],scale);
    if (!same80(lo.value,hi.value) || ((lo.flags^hi.flags)&softfloat_flag_underflow)) return 0;
    int tiny=(lo.flags&softfloat_flag_underflow)!=0;
    if (tiny && underflow_unmasked) {
        /* Repack the ORIGINAL interval. Scaling an already rounded
         * subnormal loses both significand bits and the correct C1. */
        scale+=24576;
        lo=sine_pack_limit(value.lo,1,modes[rounding],scale);
        hi=sine_pack_limit(value.hi,0,modes[rounding],scale);
        if (!same80(lo.value,hi.value)) return 0;
    }
    sine_packed rzlo=sine_pack_limit(value.lo,1,softfloat_round_minMag,scale);
    sine_packed rzhi=sine_pack_limit(value.hi,0,softfloat_round_minMag,scale);
    if (!same80(rzlo.value,rzhi.value)) return 0;
    out[0]=lo.value.signif; out[1]=lo.value.signExp; out[2]=0;
    out[3]=32|(tiny ? 16 : 0)|((!same80(lo.value,rzlo.value))<<16);
    return 1;
}
