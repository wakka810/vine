/* Vine's bounded atan2 using unmodified SoftFloat binary128 arithmetic.
 * No host FP, guest CPU state, or guest memory. The Rust lock serializes the
 * SoftFloat globals. Every operation encloses the mathematical value.
 *
 * Reduce the positive magnitude ratio to t <= 1, then for t > 1/2 use
 * atan(t) = pi/4 - atan((1-t)/(1+t)). For 0 <= u <= 1/2, the alternating
 * series with 100 terms is below atan(u); its next term bounds the remainder.
 * The adjacent binary128 pi bounds are independently certified with rational
 * Machin series. Intermediate inexact flags are never guest exceptions.
 *
 * If t = q * 2^e with e < -64, q*(1-2^-128) < atan(t)/2^e < q.
 * Keeping e separate until final packing covers the full ext80 ratio range.
 * All nonzero angles are irrational, so rational enclosure endpoints are
 * strict. Final conversion uses one-sided limits, not closed endpoints that
 * would incorrectly reject a tiny correction to a representable ratio. */
#include <stdint.h>
#include "platform.h"
#include "internals.h"
#include "softfloat.h"

typedef struct { float128_t lo, hi; } interval;
static float128_t minus(float128_t a) { a.v[1] ^= UINT64_C(0x8000000000000000); return a; }
static interval exact(float128_t a) { interval r = {a,a}; return r; }
static interval negate(interval a) { interval r = {minus(a.hi),minus(a.lo)}; return r; }
static interval add(interval a, interval b) {
    interval r;
    softfloat_roundingMode=softfloat_round_min; r.lo=f128_add(a.lo,b.lo);
    softfloat_roundingMode=softfloat_round_max; r.hi=f128_add(a.hi,b.hi);
    return r;
}
static interval sub(interval a, interval b) { return add(a,negate(b)); }
/* Nonnegative operands; division denominator is positive. */
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
static float128_t power2(int e) {
    float128_t r={{0,(uint64_t)(e+16383)<<48}}; return r;
}
static const interval pi={
    {{UINT64_C(0x8469898cc51701b8),UINT64_C(0x4000921fb54442d1)}},
    {{UINT64_C(0x8469898cc51701b9),UINT64_C(0x4000921fb54442d1)}}
};
static interval pi_part(int power) { return mul(pi,exact(power2(power))); }
static int same80(extFloat80_t a, extFloat80_t b) {
    return a.signif==b.signif && a.signExp==b.signExp;
}
/* Near zero with e < -80, atan(q*2^e)/2^e differs from q by less
 * than 2^-159 (q < 2). A normalized quotient of two 64-bit integers is
 * either exactly on an ext80 rounding boundary or at distance >= 2^-129:
 * such boundaries have at most 65 significant bits, including a midpoint.
 * Subnormal output boundaries are coarser still. The correction therefore
 * cannot cross a boundary unless the quotient equals it.
 *
 * Generate 128 quotient bits and a remainder without a binary128 rounding.
 * Nonzero remainder is jammed; an exact quotient is approached from below.
 * This evaluates the correctly rounded atan, not a rounded division used as
 * an approximation to it. No value guard or executable address is involved. */
static uint32_t tiny_ratio(uint64_t numerator, uint64_t denominator, int e,
                           int sign, uint32_t rounding, uint64_t out[4]) {
    uint64_t remainder;
    if (numerator >= denominator) remainder=numerator-denominator;
    else { remainder=(numerator<<1)-denominator; --e; }
    uint64_t high=UINT64_C(0x8000000000000000), extra=0;
    for (int bit=126;bit>=0;--bit) {
        int carry=(int)(remainder>>63);
        remainder<<=1;
        int digit=carry || remainder>=denominator;
        if (digit) remainder-=denominator;
        if (bit>=64) high|=(uint64_t)digit<<(bit-64);
        else extra|=(uint64_t)digit<<bit;
    }
    if (remainder) extra|=1;
    else {
        if (extra--==0) --high;
        if (!(high>>63)) {
            high=(high<<1)|(extra>>63); extra<<=1; --e;
        }
    }
    const uint8_t modes[4]={softfloat_round_near_even,softfloat_round_min,
                            softfloat_round_max,softfloat_round_minMag};
    softfloat_detectTininess=softfloat_tininess_afterRounding;
    softfloat_exceptionFlags=0;
    softfloat_roundingMode=modes[rounding];
    extFloat80_t result=softfloat_roundPackToExtF80(sign,e+16383,high,extra,80);
    softfloat_exceptionFlags=0;
    softfloat_roundingMode=softfloat_round_minMag;
    extFloat80_t truncated=softfloat_roundPackToExtF80(sign,e+16383,high,extra,80);
    out[0]=result.signif; out[1]=result.signExp; out[2]=0;
    /* Nonzero FPATAN is inexact; UE follows the rounded result's tininess.
     * Do not inherit the packer's pre-shift internal underflow predicate. */
    out[3]=32|(!(result.signExp&0x7fff) ? 16 : 0)|((!same80(result,truncated))<<16);
    return 1;
}
/* Endpoints are normal, nonzero binary128. Extra contains 15 trailing zero
 * bits, so +/-1 evaluates the infinitesimal inward rounding limit without
 * crossing a different binary128 or ext80 rounding boundary. */
static extFloat80_t pack_limit(float128_t a, int scale, int lower,
                              uint8_t mode) {
    uint64_t high=a.v[1];
    int sign=(int)(high>>63);
    int exponent=(int)((high>>48)&0x7fff)+scale;
    struct uint128 s=softfloat_shortShiftLeft128(
        (high&UINT64_C(0x0000ffffffffffff))|UINT64_C(0x0001000000000000),a.v[0],15);
    if (lower != sign) {
        if (++s.v0==0) ++s.v64;
    } else {
        if (s.v0--==0) --s.v64;
        if (!(s.v64>>63)) { s=softfloat_shortShiftLeft128(s.v64,s.v0,1); --exponent; }
    }
    softfloat_roundingMode=mode;
    softfloat_exceptionFlags=0;
    extFloat80_t r=softfloat_roundPackToExtF80(sign,exponent,s.v64,s.v0,80);
    return r;
}
static interval atan_series(interval t) {
    interval one=exact(i32_to_f128(1));
    /* These bit comparisons are ordered for positive normal binary128. */
    int reduce=t.hi.v[1]>UINT64_C(0x3ffe000000000000) ||
        (t.hi.v[1]==UINT64_C(0x3ffe000000000000) && t.hi.v[0]);
    interval u=reduce ? divide(sub(one,t),add(one,t)) : t;
    interval square=mul(u,u), power=u, sum=exact(i32_to_f128(0));
    for (int k=0;k<100;++k) {
        interval term=divide(power,exact(i32_to_f128(2*k+1)));
        sum=(k&1) ? sub(sum,term) : add(sum,term);
        power=mul(power,square);
    }
    interval tail=divide(power,exact(i32_to_f128(201)));
    softfloat_roundingMode=softfloat_round_max;
    sum.hi=f128_add(sum.hi,tail.hi);
    /* Outward summation can obscure atan(t) < t when its cubic correction
     * is smaller than one binary128 ulp. Keep this independent strict bound. */
    if (!reduce && (sum.hi.v[1]>t.hi.v[1] ||
        (sum.hi.v[1]==t.hi.v[1] && sum.hi.v[0]>t.hi.v[0]))) sum.hi=t.hi;
    return reduce ? sub(pi_part(-2),sum) : sum;
}

/* Rust rejects unsupported encodings/NaNs and handles signed-zero outputs.
 * Nonzero quadrant constants and finite inputs are handled here uniformly.
 * A failed certification never modifies out. */
uint32_t vine_arctangent(uint64_t xs, uint32_t xe, uint64_t ys, uint32_t ye,
                        uint32_t rounding, uint64_t out[4]) {
    int xn=(xe&0x8000)!=0, yn=(ye&0x8000)!=0;
    int xi=(xe&0x7fff)==0x7fff, yi=(ye&0x7fff)==0x7fff;
    interval angle;
    int output_scale=0;
    if (!ys || (xi && !yi)) {
        angle=pi; /* nonzero branch necessarily has negative x */
    } else if (!xs || (yi && !xi)) {
        angle=pi_part(-1);
    } else if (xi && yi) {
        angle=pi_part(-2);
        if (xn) angle=sub(pi,angle);
    } else {
        int xshift=softfloat_countLeadingZeros64(xs), yshift=softfloat_countLeadingZeros64(ys);
        int ex=(int)(xe&0x7fff), ey=(int)(ye&0x7fff);
        ex=(ex ? ex : 1)-16383-xshift;
        ey=(ey ? ey : 1)-16383-yshift;
        xs<<=xshift; ys<<=yshift;
        int inverted=ey>ex || (ey==ex && ys>xs);
        extFloat80_t small={inverted ? xs : ys,0x3fff};
        extFloat80_t large={inverted ? ys : xs,0x3fff};
        int e=inverted ? ex-ey : ey-ex;
        if (!inverted && !xn && e < -80) return tiny_ratio(ys,xs,e,yn,rounding,out);
        interval ratio=divide(exact(extF80_to_f128(small)),exact(extF80_to_f128(large)));
        if (e < -64) {
            interval correction=sub(exact(i32_to_f128(1)),exact(power2(-128)));
            interval small_angle=mul(ratio,correction);
            small_angle.hi=ratio.hi;
            if (!inverted && !xn) {
                angle=small_angle;
                output_scale=e;
            } else {
                if (e >= -128) small_angle=mul(small_angle,exact(power2(e)));
                else { small_angle.lo=i32_to_f128(0); small_angle.hi=power2(-128); }
                angle=inverted ? sub(pi_part(-1),small_angle) : small_angle;
                if (xn) angle=sub(pi,angle);
            }
        } else {
            angle=atan_series(mul(ratio,exact(power2(e))));
            if (inverted) angle=sub(pi_part(-1),angle);
            if (xn) angle=sub(pi,angle);
        }
    }
    if (yn) angle=negate(angle);
    const uint8_t modes[4]={softfloat_round_near_even,softfloat_round_min,
                            softfloat_round_max,softfloat_round_minMag};
    softfloat_detectTininess=softfloat_tininess_afterRounding;
    extFloat80_t lo=pack_limit(angle.lo,output_scale,1,modes[rounding]);
    extFloat80_t hi=pack_limit(angle.hi,output_scale,0,modes[rounding]);
    extFloat80_t rzlo=pack_limit(angle.lo,output_scale,1,softfloat_round_minMag);
    extFloat80_t rzhi=pack_limit(angle.hi,output_scale,0,softfloat_round_minMag);
    if (!same80(lo,hi) || !same80(rzlo,rzhi)) return 0;
    out[0]=lo.signif; out[1]=lo.signExp; out[2]=0;
    out[3]=32|(!(lo.signExp&0x7fff) ? 16 : 0)|((!same80(lo,rzlo))<<16);
    return 1;
}
