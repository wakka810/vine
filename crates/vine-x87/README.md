This crate supplies exact extended-precision arithmetic and an x87 architectural
stack for Vine's independent PE32 backend. It has no v86 dependency. The state is
128 bytes with physical raw 80-bit registers, full tags, TOP, control/status,
instruction/data pointers and selectors. MMX aliases the significands; popping or
initializing the FPU does not erase the register contents.

The arithmetic source is Berkeley SoftFloat Release 3e, upstream commit
`07ebd29db0cfb55723a79fbbffe99103ef38e305`, obtained directly from
[ucb-bar/berkeley-softfloat-3](https://github.com/ucb-bar/berkeley-softfloat-3/tree/07ebd29db0cfb55723a79fbbffe99103ef38e305).
`vendor/sources.txt` lists the 80 unmodified C source files included from that
commit, with its `8086` NaN specialization. The headers and BSD 3-clause license
are retained in `vendor`; distribute `vendor/COPYING.txt` with the binary. Vine's
platform configuration and C adapter are separate from upstream source. No
native floating-point operation is used by the arithmetic implementation.

The initial operations cover the real audio capture's FLD/FSTP, FADD/FADDP,
FSUB/FSUBP, FMUL, FCHS, FXCH, FCOM/FCOMP and status state. The numeric layer also
offers division, square root, integral rounding and integer conversions. Numeric
basic operations respect all four rounding directions and 24/53/64-bit significand
precision with the x87 exponent range. Conversions are not affected by precision
control. Pseudo-denormals normalize only at arithmetic input, and unsupported
80-bit encodings produce the architectural invalid response. Values are never
approximated with binary64.

Reverse subtraction and division use the same numeric operations with explicit
left and right operands. Register, real-memory and integer-memory forms retain
their distinct destination and pop effects; memory widening preserves signaling
NaNs on either side. Empty-stack faults suppress arithmetic, and unmasked
pre-computation exceptions suppress both destination changes and pop.

FYL2X uses a separate combined-operation kernel. It classifies both raw inputs
before computing, preserves invalid/zero-divide/denormal priority, and ignores
the defined PC settings while respecting RC. The general finite path computes
outward binary128 intervals for `x = 2^e m`, `3/4 <= m < 3/2`, using forty terms
of `2 atanh((m-1)/(m+1))` and a geometric remainder bound. The adjacent binary128
ln(2) bounds are certified independently by an exact rational series. The
normalized y significand and separate exponent avoid premature binary128 range
exceptions. Final RC and toward-zero ext80 conversions of both endpoints must
agree, including range flags, before a value and C1 are returned. Otherwise the
kernel returns explicit `UncertainRounding` without publishing a result. This is
a bounded implementation, not a claim to handle every possible rounding case.
No intermediate precision flag leaks from the interval calculation.

FPATAN likewise computes the combined atan2 operation, including signed-zero
and infinity quadrants, without dividing those special inputs. Its finite path
reduces the magnitude ratio to at most one half and encloses a hundred-term
alternating arctangent series by its next term. Independently certified adjacent
binary128 bounds supply pi. Tiny ratios retain a separate exponent until ext80
packing; a proven relative correction bound prevents premature underflow.
Nonzero angles are irrational, so final conversion uses the inward limits of
the rational bounds. Very small unrotated ratios use an exact integer quotient
and remainder: a lower bound on their distance from rounding boundaries proves
that the arctangent correction cannot cross a boundary, with exact coincidences
approached from below. This resolves cases beyond binary128 quotient precision.
Both RC and toward-zero endpoint results, C1 and range
flags must agree, or the kernel returns `UncertainRounding`. Defined PC settings
retain full precision. Zero results do not raise PE; nonzero angles do, and raw
denormal inputs retain DE. Native approximation differences are checked against
an independent rational oracle rather than hidden by a tolerance.

FCOS follows Intel's documented `cos(x*pi/p)` numerical target, where
`p = 0xC90FDAA22168C234C * 2^-66` is the internal finite pi; replacing it with
mathematical `cos(x)` would change large-argument behavior. Exact integer
reduction by `p/2` keeps the quadrant, then the residual is multiplied by an
outward interval for `pi/p`. A bounded alternating series on at most `pi/4`
and its remainder certify both the full ext80 RC result and C1. Near one,
`1-cos(x)` is retained separately through integer final packing to avoid losing
small corrections at rounding boundaries. Ambiguous results stop explicitly.
Finite `|x| >= 2^63` leaves ST0 unchanged and sets C2 without a new exception;
ordinary completion clears C2, preserves TOP, and records FIP/FOP without
changing FDP. Zero produces exact +1 without PE; nonzero finite completion
reports PE, with raw denormal DE and pre-computation suppression retained.
Defined PC values do not shorten the result. C0/C3 are preserved and undefined
range-failure C1 is set to zero. Native approximation differences are recorded
separately from the certified numerical target. See Intel's
[x87 trigonometric accuracy explanation](https://www.intel.com/content/www/us/en/developer/articles/technical/the-difference-between-x87-instructions-and-mathematical-functions.html)
and the FCOS entry in its Software Developer's Manual.

FSIN uses the corresponding `sin(x*pi/p)` target and the same exact reduction
and interval arithmetic. Signed zero is retained without new PE. Tiny inputs
keep a separate exponent through normalized interval evaluation and final
packing. Underflow is decided after full64 rounding with an unbounded exponent;
the physical result can round to the minimum normal value while UE remains set.
With UM clear, the original interval is packed with an exponent bias of +24576,
retaining all significand bits and the biased result's C1. This result commits
and records a deferred exception. IE/DE priority, pending #MF, raw tags, TOP and
environment publication retain the same architectural contract as FCOS.

F2XM1 encloses mathematical `2^x-1` with outward binary128 arithmetic and a
bounded exponential series. Negative inputs use a cancellation-free identity;
tiny inputs retain a separate exponent through final packing. RC and toward-zero
endpoints must certify the same full64 values, including C1 and underflow, or
execution stops explicitly. Signed zero is exact; ordinary nonzero completion
reports PE. Defined PC settings do not shorten the result. This accuracy target
satisfies Intel's transcendental accuracy contract; native approximations need
not have identical bits. Finite inputs outside [-1,1] have architecturally
undefined results and return an explicit domain diagnostic, not a fabricated
invalid-operation exception. Independent integer intervals check the result.

FSCALE truncates ST1 toward zero independently of RC, normalizes ST0 with integer
operations and packs the full64 result. It preserves ST1 and TOP. NaN, denormal,
range and pre-computation exception priority are explicit. Unmasked range results
are computed from the original significand with the +/-24576 exponent bias;
adjusted exponents still outside the normal range remain explicitly unsupported.
Both instructions preserve undefined C0/C2/C3, publish the original FIP/FOP,
and defer unmasked exceptions to the next waiting instruction. Numeric kernels
own no guest state, pointers, mappings or callback authority.

The numeric layer also supplies fixed f32/f64-to-i32 truncation for SSE scalar
conversion. It applies DAZ before direct SoftFloat conversion and returns only
the integer and newly raised IE/PE; these instructions never raise DE. Neither
RC, FTZ nor x87 control affects the numeric conversion. CPU lowering owns
MXCSR sticky bits and synchronous #XM, including destination suppression.

Exact powers of two use the mathematical integer-log identity with full ext80
multiplication. As observed independently with native FYL2X, the ordinary path
reports precision even for an exact product, and underflow for an exact
subnormal result; the x=1 and y=zero identities do not introduce precision.
These flags, stack effects and unmasked-exception behavior are checked apart
from numeric accuracy. Native transcendental results can differ by one ulp from
the certified mathematical rounding, so native raw equality is not used as its
sole numeric oracle. C0/C2/C3 are architecturally undefined for FYL2X; Vine keeps
their previous values. Unmasked register range results still stop explicitly.

`State::prepare` returns a new state and optional output bytes. The caller must
first call `State::check_waiting` for waiting instructions, then preflight the
whole guest memory access before preparing the operation, and must commit the
returned store before publishing the new state. Discarding the result leaves
registers, TOP and memory untouched. Completed pre-exception side effects from
earlier guest instructions remain the backend's responsibility. Invalid,
denormal and zero-divide exceptions can be deferred with suppressed destination
updates; the next waiting instruction reports a pending exception. Non-waiting
clear/init remain available. FLD follows its documented special case: an
unmasked denormal exception still loads and pushes the widened value. Inexact
results retain their completed result,
including memory stores/pop, and record pending precision exceptions. Unmasked
memory overflow/underflow suppress the store. Unmasked register overflow or
underflow explicitly returns `Unsupported` except the prepared biased results
of FSIN, F2XM1 and the supported FSCALE exponent range; other wrapped-exponent
results are not implemented yet.
Reserved precision control also stops explicitly.

C1 is calculated by repeating an inexact operation with round-toward-zero and
checking whether the selected rounding increased the magnitude. This cost is
included in backend timings. No performance benefit is assumed from reusing
SoftFloat. Each operation installs its own control and collects its own flags;
a Rust mutex isolates the upstream operation globals across native threads.

Native tests execute independent host x87 instructions, preserving the host
FP/SIMD environment, and compare raw extended results, exceptions and C1 across
all precision and rounding settings. Stack tests include masked/unmasked stack
faults, deferred exceptions, precision stores and divide-by-zero. The native
oracle is not built for Wasm. Intel-prescribed condition codes are retained on
unmasked FCOM invalid exceptions; the observed AMD host instead sets unordered.
Tests explicitly check the Intel rule and compare the remaining native state
for that case. Native comparison is evidence, not the sole semantic oracle.
These checks supplement the
[Intel SDM](https://cdrdv2-public.intel.com/874240/325462-090-sdm-vol-1-2abcd-3abcd-4.pdf)
and [SoftFloat interface specification](https://www.jhauser.us/arithmetic/SoftFloat-3/doc/SoftFloat.html).

Building requires clang and llvm-ar, selectable with `VINE_X87_CC` and
`VINE_X87_AR`. The compiler receives Cargo's exact target; wasm32 builds require
a clang distribution with its WebAssembly target. On Windows the standard LLVM
installation is preferred for Wasm because some MinGW distributions omit it.
The crate is not a complete x87 emulator: remaining transcendental/remainder operations,
environment save/restore instructions, unmasked register range-exception
results and eventual Windows #MF dispatch still need backend integration or
explicit unsupported handling. Runtime authority and guest access checks remain
outside this arithmetic crate.
