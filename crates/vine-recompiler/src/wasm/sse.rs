//! Reached SSE bitwise, vector transfers and precise pure numeric conversions.
//! Intel SDM Vol.2A Table2-21 distinguishes legacy XORPS aligned memory from
//! MOVUPS, which permits unaligned memory. Legacy PXOR/POR have the same Type4
//! alignment rule; MOVDQA/MOVAPS require aligned loads/stores, MOVNTPS an
//! aligned store. MOVNTPS's non-temporal cache hint uses the ordinary checked
//! store: the single guest CPU's sequential visibility is permitted by WC's
//! weaker ordering, and GuestMemory has no guest-visible cache or MMIO state.
//! Raw vector operations preserve MXCSR/x87 and flags. The control transfers below
//! read/write only MXCSR and m32; they do not deliver pending x87/SIMD exceptions.
//! SSE2 MOVD likewise only transfers raw bits: r/m32 loads clear XMM[127:32],
//! stores read XMM[31:0], and m32 uses the ordinary unaligned scalar checks.
//! Legacy MOVSD transfers low64 without FP conversion: register copies preserve
//! destination upper64, whereas memory loads clear it. Both m64 directions
//! permit unaligned addresses and preflight the complete span before mutation.
//! SSE2 MOVQ uses the same raw m64 transfer, but both register encodings also
//! clear destination upper64. It never enters the MMX/x87 register bank.
//! MOVSS has the same register/memory distinction for low32 and upper96.
//! PUNPCKLWD may fetch the full m128 source even though it uses only low64;
//! Intel requires the normal segment checks and 16-byte linear alignment.
//! This implementation uses the existing aligned128 read. With 4KiB mappings,
//! a valid aligned span cannot cross a page. No extra source byte is observable.
//! PUNPCKLBW follows the same rule. PMULLW and packs use all 128 source bits;
//! PSRLW/PSRAW/PSRAD/PSLLD load m128 but take their unsigned count from low64 only. These
//! legacy SSE2 memory forms require 16-byte alignment and preserve FP state.
//! CVTDQ2PS reads the full aligned source before pure integer-to-binary32
//! conversion under MXCSR.RC. New PE from all four lanes is accumulated before
//! a synchronous #XM decision; no destination lane commits on an unmasked PE.
//! MULPS likewise stages all four results, but reports only pre-computation
//! IE/DE if any pre-computation exception is unmasked. Old sticky flags remain.
//! SHUFPS selects raw32 lanes from the old destination and full aligned source;
//! it neither raises FP exceptions nor changes NaN payloads or MXCSR.
//! PSHUFD selects all four lanes from one full aligned source, preserving the
//! same raw-bit and fault contract even when the destination aliases it.
//! PINSRW copies low16 from r32 or an exact unaligned m16 read into imm8[2:0];
//! all other destination words and all FP/control state remain unchanged.
//! https://cdrdv2-public.intel.com/812383/253666-sdm-vol-2a.pdf
//! https://cdrdv2-public.intel.com/671241/253667-sdm-vol-2b.pdf
//! https://cdrdv2-public.intel.com/843827/253665-sdm-vol-1-dec-24.pdf

use super::*;
mod packed;

const VECTOR: u32 = 41;
const BITS: u32 = 42;
pub(super) const XMM: u32 = 43;

impl Emitter {
    fn simd_memory(&mut self, opcode: u32, offset: u32) {
        self.simd(opcode);
        uleb(&mut self.code, 0);
        uleb(&mut self.code, offset);
    }

    pub(super) fn initialize_sse(&mut self) {
        for index in 0..8 {
            self.get(0);
            self.simd_memory(0x00, 268 + index * 16);
            self.set(XMM + index);
        }
    }

    pub(super) fn flush_sse(&mut self) {
        for index in 0..8 {
            self.get(0);
            self.get(XMM + index);
            self.simd_memory(0x0b, 268 + index * 16);
        }
    }

    fn sse_operand(&mut self, operand: Operand, aligned: bool) {
        match operand {
            Operand::Xmm(index) => self.get(XMM + u32::from(index)),
            Operand::Mem(memory) => {
                self.effective(memory, true);
                self.set(ADDR);
                if aligned {
                    self.get(ADDR);
                    self.num(15);
                    self.op(0x71);
                    self.check(6, ADDR, 0);
                }
                self.access(Width::W128, false, memory.segment == Segment::Ss);
                self.get(ENTRY);
                self.get(ENTRY2);
                self.op(0x46);
                self.if_();
                self.get(PTR);
                self.simd_memory(0x00, 0);
                self.set(VECTOR);
                self.op(0x05);
                self.vector_zero();
                self.set(VECTOR);
                for n in 0..16 {
                    self.get(VECTOR);
                    self.byte_pointer(n);
                    self.mem(0x2d, 0);
                    self.simd(0x17);
                    self.op(n as u8);
                    self.set(VECTOR);
                }
                self.end();
                self.get(VECTOR);
            }
            _ => unreachable!("validated SSE operand"),
        }
    }

    fn sse_assign(&mut self, destination: Operand) {
        match destination {
            Operand::Xmm(index) => {
                self.get(VECTOR);
                self.set(XMM + u32::from(index));
            }
            Operand::Mem(_) => {
                // Both page permissions and dirty capacity have been checked.
                // No guest byte or architectural vector changes on failure.
                self.get(ENTRY);
                self.get(ENTRY2);
                self.op(0x46);
                self.if_();
                self.get(PTR);
                self.get(VECTOR);
                self.simd_memory(0x0b, 0);
                self.op(0x05);
                for n in 0..16 {
                    self.byte_pointer(n);
                    self.get(VECTOR);
                    self.simd(0x16);
                    self.op(n as u8);
                    self.mem(0x3a, 0);
                }
                self.end();
                self.mark_dirty();
                self.load(ENTRY, 4);
                self.load(ENTRY2, 4);
                self.op(0x72);
                self.num(4 | 32);
                self.op(0x71);
                self.set(SMC);
            }
            _ => unreachable!("validated SSE destination"),
        }
    }

    fn sse_truncate_integer(&mut self, opcode: SseOpcode, operands: &[Operand]) {
        let double = opcode != SseOpcode::Cvttss2si;
        match operands[1] {
            Operand::Xmm(index) => {
                self.get(XMM + u32::from(index));
                self.simd(0x1b); // i32x4.extract_lane
                self.op(0);
                self.set(A);
                if double {
                    self.get(XMM + u32::from(index));
                    self.simd(0x1b);
                    self.op(1);
                } else {
                    self.num(0);
                }
                self.set(B);
            }
            Operand::Mem(memory) if double => {
                // m64 is unaligned and can span two mappings. Check the whole
                // operand before reading any byte or invoking numeric code.
                self.effective(memory, true);
                self.set(ADDR);
                self.access(Width::W64, false, memory.segment == Segment::Ss);
                self.get(ENTRY);
                self.get(ENTRY2);
                self.op(0x46);
                self.if_();
                self.get(PTR);
                self.mem(0x29, 0);
                self.set(BITS);
                self.op(0x05);
                self.op(0x42);
                sleb(&mut self.code, 0);
                self.set(BITS);
                for byte in 0..8 {
                    self.byte_pointer(byte);
                    self.mem(0x2d, 0);
                    self.op(0xad);
                    self.op(0x42);
                    sleb(&mut self.code, i64::from(byte * 8));
                    self.op(0x86);
                    self.get(BITS);
                    self.op(0x84);
                    self.set(BITS);
                }
                self.end();
                self.get(BITS);
                self.op(0xa7);
                self.set(A);
                self.get(BITS);
                self.op(0x42);
                sleb(&mut self.code, 32);
                self.op(0x88);
                self.op(0xa7);
                self.set(B);
            }
            Operand::Mem(_) => {
                self.operand(operands[1]);
                self.set(A);
                self.num(0);
                self.set(B);
            }
            _ => unreachable!("validated scalar SSE conversion source"),
        }
        self.load(0, 396);
        self.get(A);
        self.get(B);
        self.op(0x10);
        uleb(
            &mut self.code,
            self.sse_numeric[if opcode == SseOpcode::Cvtsd2si {
                6
            } else {
                usize::from(double)
            }],
        );
        self.set(BITS);
        self.get(BITS);
        self.op(0x42);
        sleb(&mut self.code, 32);
        self.op(0x88);
        self.op(0xa7);
        self.set(B);
        self.sse_apply_exceptions(0);
        self.get(BITS);
        self.op(0xa7);
        self.set(R);
        self.assign(operands[0]);
    }

    // B contains only this instruction's new SIMD exceptions, including the
    // pure-kernel invalid-input sentinel. Leave BITS/VECTOR results untouched.
    fn sse_apply_exceptions(&mut self, pre_mask: u32) {
        self.get(B);
        self.num(0x8000_0000);
        self.op(0x71);
        self.if_();
        self.num(0);
        self.set(FAULT);
        self.num(4); // malformed pure-helper input; no guest state committed
        self.set(ERROR);
        self.exit(crate::state::exit_kind::UNSUPPORTED);
        self.end();
        if pre_mask != 0 {
            // Aggregate every lane before selecting the exception phase.
            // An unmasked pre-computation exception suppresses this attempt's
            // post-computation flags, including flags from different lanes.
            self.load(0, 396);
            self.num(7);
            self.op(0x76);
            self.num(u32::MAX);
            self.op(0x73);
            self.get(B);
            self.op(0x71);
            self.num(pre_mask);
            self.op(0x71);
            self.if_();
            self.get(B);
            self.num(pre_mask);
            self.op(0x71);
            self.set(B);
            self.end();
        }
        // SIMD exceptions are synchronous. Only this operation's new flags
        // participate in #XM; pre-existing sticky bits do not retrigger it.
        self.load(0, 396);
        self.tee(A);
        self.get(B);
        self.op(0x72);
        self.set(R);
        self.store(0, 396, R);
        self.get(A);
        self.num(7);
        self.op(0x76);
        self.num(u32::MAX);
        self.op(0x73);
        self.get(B);
        self.op(0x71);
        self.num(0x3f);
        self.op(0x71);
        self.if_();
        self.num(0);
        self.set(FAULT);
        self.num(0);
        self.set(ERROR);
        self.exit(crate::state::exit_kind::SIMD_FLOATING_POINT);
        self.end();
    }

    fn sse_packed_numeric(&mut self, opcode: SseOpcode, operands: &[Operand]) {
        let Operand::Xmm(destination) = operands[0] else {
            unreachable!("validated packed SSE numeric destination");
        };
        let (numeric_index, binary, pre_mask) = match opcode {
            SseOpcode::Cvttps2dq => (0, false, 1),
            SseOpcode::Cvtdq2ps => (2, false, 0),
            SseOpcode::Mulps => (3, true, 7),
            SseOpcode::Addps => (4, true, 7),
            SseOpcode::Subps => (5, true, 7),
            _ => unreachable!("validated packed SSE numeric operation"),
        };
        // Complete the source access before any numeric call or sticky-bit
        // update. VECTOR is private scratch: replacing one lane leaves all
        // later raw input lanes intact, including source/destination aliases.
        self.sse_operand(operands[1], true);
        self.set(VECTOR);
        let native = opcode != SseOpcode::Mulps;
        if native {
            self.sse_native_packed_guard(opcode, destination);
            self.if_();
            self.sse_native_packed(opcode, destination);
            self.op(0x05);
        }
        self.num(0);
        self.set(B);
        for lane in 0..4 {
            self.load(0, 396);
            if binary {
                // Legacy binary SSE's first operand is the old destination. This
                // operand order also determines which NaN payload wins.
                self.get(XMM + u32::from(destination));
                self.simd(0x1b);
                self.op(lane);
            }
            self.get(VECTOR);
            self.simd(0x1b);
            self.op(lane);
            if !binary {
                self.num(0);
            }
            self.op(0x10);
            uleb(&mut self.code, self.sse_numeric[numeric_index]);
            self.set(BITS);
            self.get(BITS);
            self.op(0x42);
            sleb(&mut self.code, 32);
            self.op(0x88);
            self.op(0xa7);
            self.get(B);
            self.op(0x72);
            self.set(B);
            self.get(VECTOR);
            self.get(BITS);
            self.op(0xa7);
            self.simd(0x1c);
            self.op(lane);
            self.set(VECTOR);
        }
        if native {
            self.end();
        }
        self.sse_apply_exceptions(pre_mask);
        self.get(VECTOR);
        self.set(XMM + u32::from(destination));
    }

    fn sse_double_numeric(&mut self, opcode: SseOpcode, operands: &[Operand]) {
        let Operand::Xmm(destination) = operands[0] else {
            unreachable!("validated SSE2 destination")
        };
        let scalar = matches!(
            opcode,
            SseOpcode::Addsd | SseOpcode::Subsd | SseOpcode::Mulsd
        );
        let numeric = match opcode {
            SseOpcode::Addsd | SseOpcode::Addpd => 7,
            SseOpcode::Subsd | SseOpcode::Subpd => 8,
            SseOpcode::Mulsd | SseOpcode::Mulpd => 9,
            _ => unreachable!("validated SSE2 operation"),
        };
        if scalar {
            self.sse_read_low64(operands[1]);
            self.get(XMM + u32::from(destination));
            self.get(BITS);
            self.simd(0x1e); // i64x2.replace_lane, retain old upper64
            self.op(0);
        } else {
            self.sse_operand(operands[1], true);
        }
        self.set(VECTOR);
        self.num(0);
        self.set(B);
        for lane in 0..if scalar { 1 } else { 2 } {
            self.load(0, 396);
            self.get(XMM + u32::from(destination));
            self.simd(0x1d); // i64x2.extract_lane
            self.op(lane);
            self.get(VECTOR);
            self.simd(0x1d);
            self.op(lane);
            self.op(0x10);
            uleb(&mut self.code, self.sse_numeric[numeric]);
            self.tee(PTR);
            self.op(0x45);
            self.if_();
            self.num(0);
            self.set(FAULT);
            self.num(4);
            self.set(ERROR);
            self.exit(crate::state::exit_kind::UNSUPPORTED);
            self.end();
            // Copy all scratch output before another numeric call or exit.
            self.get(PTR);
            self.mem(0x29, 0);
            self.set(BITS);
            self.load(PTR, 8);
            self.get(B);
            self.op(0x72);
            self.set(B);
            self.get(VECTOR);
            self.get(BITS);
            self.simd(0x1e);
            self.op(lane);
            self.set(VECTOR);
        }
        self.sse_apply_exceptions(7);
        self.get(VECTOR);
        self.set(XMM + u32::from(destination));
    }

    fn sse_read_low64(&mut self, source: Operand) {
        match source {
            Operand::Xmm(index) => {
                self.get(XMM + u32::from(index));
                self.simd(0x1d); // i64x2.extract_lane
                self.op(0);
                self.set(BITS);
            }
            Operand::Mem(memory) => {
                self.effective(memory, true);
                self.set(ADDR);
                self.access(Width::W64, false, memory.segment == Segment::Ss);
                self.get(ENTRY);
                self.get(ENTRY2);
                self.op(0x46);
                self.if_();
                self.get(PTR);
                self.mem(0x29, 0);
                self.set(BITS);
                self.op(0x05);
                self.op(0x42);
                sleb(&mut self.code, 0);
                self.set(BITS);
                for byte in 0..8 {
                    self.byte_pointer(byte);
                    self.mem(0x2d, 0);
                    self.op(0xad);
                    self.op(0x42);
                    sleb(&mut self.code, i64::from(byte * 8));
                    self.op(0x86);
                    self.get(BITS);
                    self.op(0x84);
                    self.set(BITS);
                }
                self.end();
            }
            _ => unreachable!("validated SSE low64 source"),
        }
    }

    fn sse_move_low64(&mut self, operands: &[Operand], opcode: SseOpcode) {
        self.sse_read_low64(operands[1]);
        match operands[0] {
            Operand::Xmm(index) => {
                if opcode == SseOpcode::Movq
                    || (opcode == SseOpcode::Movsd && matches!(operands[1], Operand::Mem(_)))
                {
                    self.vector_zero();
                } else {
                    self.get(XMM + u32::from(index));
                }
                self.get(BITS);
                self.simd(0x1e); // i64x2.replace_lane
                self.op(0);
                self.set(XMM + u32::from(index));
            }
            Operand::Mem(_) => {
                // Full permissions and dirty capacity precede the first byte;
                // the source XMM and all other architectural state are intact.
                self.prepare_store(operands[0]);
                self.get(ENTRY);
                self.get(ENTRY2);
                self.op(0x46);
                self.if_();
                self.get(PTR);
                self.get(BITS);
                self.mem(0x37, 0);
                self.op(0x05);
                for byte in 0..8 {
                    self.byte_pointer(byte);
                    self.get(BITS);
                    self.op(0x42);
                    sleb(&mut self.code, i64::from(byte * 8));
                    self.op(0x88);
                    self.op(0xa7);
                    self.mem(0x3a, 0);
                }
                self.end();
                self.mark_dirty();
                self.load(ENTRY, 4);
                self.load(ENTRY2, 4);
                self.op(0x72);
                self.num(4 | 32);
                self.op(0x71);
                self.set(SMC);
            }
            _ => unreachable!("validated SSE low64 destination"),
        }
    }

    pub(super) fn sse_instruction(&mut self, opcode: SseOpcode, operands: &[Operand]) {
        if matches!(opcode, SseOpcode::Pextrw | SseOpcode::Pmovmskb) {
            let Operand::Xmm(source) = operands[1] else {
                unreachable!("validated SSE extraction")
            };
            self.get(XMM + u32::from(source));
            if opcode == SseOpcode::Pextrw {
                let Operand::Imm(lane) = operands[2] else {
                    unreachable!("validated word selector")
                };
                self.simd(0x19); // i16x8.extract_lane_u
                self.op((lane & 7) as u8);
            } else {
                self.simd(0x64); // i8x16.bitmask
            }
            self.set(R);
            self.assign(operands[0]);
            return;
        }
        if opcode == SseOpcode::Cvtdq2pd {
            let Operand::Xmm(destination) = operands[0] else {
                unreachable!("validated SSE destination")
            };
            self.sse_read_low64(operands[1]);
            self.vector_zero();
            self.get(BITS);
            self.simd(0x1e); // i64x2.replace_lane
            self.op(0);
            self.simd(0xfe); // f64x2.convert_low_i32x4_s, exact for every i32
            self.set(XMM + u32::from(destination));
            return;
        }
        if opcode == SseOpcode::Maskmovdqu {
            self.sse_masked_store(operands);
            return;
        }
        if opcode == SseOpcode::Pshufd {
            let (Operand::Xmm(destination), Operand::Imm(selectors)) = (operands[0], operands[2])
            else {
                unreachable!("validated PSHUFD operands");
            };
            self.sse_operand(operands[1], true);
            self.set(VECTOR);
            self.get(VECTOR);
            self.get(VECTOR);
            self.simd(0x0d); // i8x16.shuffle; every selected byte is from source.
            for lane in 0..4 {
                let selected = (selectors >> (lane * 2)) & 3;
                self.code
                    .extend((0..4).map(|byte| (selected * 4 + byte) as u8));
            }
            self.set(XMM + u32::from(destination));
            return;
        }
        if opcode == SseOpcode::Shufps {
            let (Operand::Xmm(destination), Operand::Imm(selectors)) = (operands[0], operands[2])
            else {
                unreachable!("validated SHUFPS operands");
            };
            self.get(XMM + u32::from(destination));
            self.sse_operand(operands[1], true);
            self.simd(0x0d); // i8x16.shuffle: first input old destination, second source
            for lane in 0..4 {
                let selected = ((selectors >> (lane * 2)) & 3) + if lane >= 2 { 4 } else { 0 };
                self.code
                    .extend((0..4).map(|byte| (selected * 4 + byte) as u8));
            }
            self.set(XMM + u32::from(destination));
            return;
        }
        if matches!(
            opcode,
            SseOpcode::Cvtdq2ps
                | SseOpcode::Cvttps2dq
                | SseOpcode::Mulps
                | SseOpcode::Addps
                | SseOpcode::Subps
        ) {
            self.sse_packed_numeric(opcode, operands);
            return;
        }
        if matches!(
            opcode,
            SseOpcode::Punpcklbw
                | SseOpcode::Punpcklwd
                | SseOpcode::Punpckhwd
                | SseOpcode::Paddw
                | SseOpcode::Psubw
                | SseOpcode::Pmullw
                | SseOpcode::Packuswb
                | SseOpcode::Packsswb
                | SseOpcode::Packssdw
                | SseOpcode::Psubq
                | SseOpcode::Psubd
                | SseOpcode::Pcmpeqd
                | SseOpcode::Pmaxsw
                | SseOpcode::Unpcklpd
        ) {
            let Operand::Xmm(destination) = operands[0] else {
                unreachable!("validated packed SSE destination");
            };
            self.get(XMM + u32::from(destination));
            self.sse_operand(operands[1], true);
            match opcode {
                SseOpcode::Punpcklbw => {
                    self.simd(0x0d);
                    self.code
                        .extend([0, 16, 1, 17, 2, 18, 3, 19, 4, 20, 5, 21, 6, 22, 7, 23]);
                }
                SseOpcode::Punpcklwd => {
                    self.simd(0x0d);
                    self.code
                        .extend([0, 1, 16, 17, 2, 3, 18, 19, 4, 5, 20, 21, 6, 7, 22, 23]);
                }
                SseOpcode::Punpckhwd => {
                    self.simd(0x0d);
                    self.code
                        .extend([8, 9, 24, 25, 10, 11, 26, 27, 12, 13, 28, 29, 14, 15, 30, 31]);
                }
                SseOpcode::Paddw => self.simd(0x8e), // i16x8.add, modulo 2^16 per lane
                SseOpcode::Psubw => self.simd(0x91), // i16x8.sub, modulo 2^16 per lane
                SseOpcode::Pmullw => self.simd(0x95), // i16x8.mul, low16 per lane
                SseOpcode::Packuswb => self.simd(0x66), // signed i16 -> saturated u8
                SseOpcode::Packsswb => self.simd(0x65), // signed i16 -> saturated i8
                SseOpcode::Packssdw => self.simd(0x85), // signed i32 -> saturated i16
                SseOpcode::Psubq => self.simd(0xd1), // i64x2.sub
                SseOpcode::Psubd => self.simd(0xb1), // i32x4.sub
                SseOpcode::Pcmpeqd => self.simd(0x37), // i32x4.eq
                SseOpcode::Pmaxsw => self.simd(0x98), // i16x8.max_s
                SseOpcode::Unpcklpd => {
                    self.simd(0x0d);
                    self.code
                        .extend([0, 1, 2, 3, 4, 5, 6, 7, 16, 17, 18, 19, 20, 21, 22, 23]);
                }
                _ => unreachable!(),
            }
            self.set(XMM + u32::from(destination));
            return;
        }
        if matches!(
            opcode,
            SseOpcode::Psrlw
                | SseOpcode::Psrad
                | SseOpcode::Psraw
                | SseOpcode::Pslld
                | SseOpcode::Psrlq
                | SseOpcode::Psllq
        ) {
            let Operand::Xmm(destination) = operands[0] else {
                unreachable!("validated SSE packed shift destination");
            };
            let (arithmetic, maximum, shift) = match opcode {
                SseOpcode::Psrlw => (false, 15, 0x8d), // i16x8.shr_u
                SseOpcode::Psraw => (true, 15, 0x8c),  // i16x8.shr_s
                SseOpcode::Psrad => (true, 31, 0xac),  // i32x4.shr_s
                SseOpcode::Pslld => (false, 31, 0xab), // i32x4.shl
                SseOpcode::Psrlq => (false, 63, 0xcd), // i64x2.shr_u
                SseOpcode::Psllq => (false, 63, 0xcb), // i64x2.shl
                _ => unreachable!("validated SSE packed shift"),
            };
            if let Operand::Imm(count) = operands[1] {
                if !arithmetic && count > maximum {
                    self.vector_zero();
                } else {
                    self.get(XMM + u32::from(destination));
                    // Width-1 yields sign fill for every x86 count >= width,
                    // without Wasm's modulo-lane-width shift behavior.
                    self.num(count.min(maximum));
                    self.simd(shift);
                }
                self.set(XMM + u32::from(destination));
            } else {
                // Complete the count read before mutation, including aliases.
                // Guard the full low64 count before Wasm masks shift counts.
                self.sse_operand(operands[1], true);
                self.simd(0x1d);
                self.op(0);
                self.set(BITS);
                self.get(BITS);
                self.op(0x42);
                sleb(&mut self.code, i64::from(maximum));
                self.op(0x56);
                self.if_();
                if arithmetic {
                    self.get(XMM + u32::from(destination));
                    self.num(maximum);
                    self.simd(shift);
                } else {
                    self.vector_zero();
                }
                self.set(XMM + u32::from(destination));
                self.op(0x05);
                self.get(XMM + u32::from(destination));
                self.get(BITS);
                self.op(0xa7);
                self.simd(shift);
                self.set(XMM + u32::from(destination));
                self.end();
            }
            return;
        }
        if opcode == SseOpcode::Pinsrw {
            let (Operand::Xmm(destination), Operand::Imm(lane)) = (operands[0], operands[2]) else {
                unreachable!("validated SSE word insertion");
            };
            // Read the complete scalar source before touching the destination.
            // replace_lane takes low16, including r32's intentionally ignored
            // high16; the immediate selects one of all eight XMM words.
            self.operand(operands[1]);
            self.set(R);
            self.get(XMM + u32::from(destination));
            self.get(R);
            self.simd(0x1a); // i16x8.replace_lane
            self.op((lane & 7) as u8);
            self.set(XMM + u32::from(destination));
            return;
        }
        if matches!(
            opcode,
            SseOpcode::Movsd | SseOpcode::Movq | SseOpcode::Movlpd
        ) {
            self.sse_move_low64(operands, opcode);
            return;
        }
        if matches!(opcode, SseOpcode::Movd | SseOpcode::Movss) {
            if let Operand::Xmm(index) = operands[0] {
                // The scalar read completes before any XMM mutation. Unlike
                // legacy MMX MOVD, this instruction has no x87 entry effects.
                if let Operand::Xmm(source) = operands[1] {
                    self.get(XMM + u32::from(source));
                    self.simd(0x1b);
                    self.op(0);
                } else {
                    self.operand(operands[1]);
                }
                self.set(R);
                if opcode == SseOpcode::Movss && matches!(operands[1], Operand::Xmm(_)) {
                    self.get(XMM + u32::from(index));
                } else {
                    self.vector_zero();
                }
                self.get(R);
                self.simd(0x1c); // i32x4.replace_lane
                self.op(0);
                self.set(XMM + u32::from(index));
            } else {
                let Operand::Xmm(index) = operands[1] else {
                    unreachable!("validated SSE low32 move source");
                };
                self.get(XMM + u32::from(index));
                self.simd(0x1b); // i32x4.extract_lane
                self.op(0);
                self.set(R);
                self.prepare_store(operands[0]);
                self.assign(operands[0]);
            }
            return;
        }
        if matches!(
            opcode,
            SseOpcode::Cvttss2si | SseOpcode::Cvttsd2si | SseOpcode::Cvtsd2si
        ) {
            self.sse_truncate_integer(opcode, operands);
            return;
        }
        if matches!(
            opcode,
            SseOpcode::Addsd
                | SseOpcode::Subsd
                | SseOpcode::Mulsd
                | SseOpcode::Addpd
                | SseOpcode::Subpd
                | SseOpcode::Mulpd
        ) {
            self.sse_double_numeric(opcode, operands);
            return;
        }
        if opcode == SseOpcode::Ldmxcsr {
            // Read the complete m32 operand before testing its contents. A page
            // fault therefore precedes reserved-bit #GP, and neither commits
            // MXCSR. No alignment restriction applies in admitted AC=0 state.
            self.operand(operands[0]);
            self.set(R);
            self.get(R);
            self.num(!vine_cpu_profile::MXCSR_MASK);
            self.op(0x71);
            self.if_();
            self.num(0);
            self.set(FAULT);
            self.num(0);
            self.set(ERROR);
            self.exit(crate::state::exit_kind::GENERAL_PROTECTION);
            self.end();
            self.store(0, 396, R);
            return;
        }
        if opcode == SseOpcode::Stmxcsr {
            // Scalar lowering preflights every byte and dirty journal slot,
            // then preserves the ordinary alias/SMC exit contract.
            self.prepare_store(operands[0]);
            self.load(0, 396);
            self.num(vine_cpu_profile::MXCSR_MASK);
            self.op(0x71);
            self.set(R);
            self.assign(operands[0]);
            return;
        }
        let bitwise = matches!(
            opcode,
            SseOpcode::Xorps | SseOpcode::Pxor | SseOpcode::Por | SseOpcode::And
        );
        let aligned = matches!(
            opcode,
            SseOpcode::Movdqa | SseOpcode::Movaps | SseOpcode::Movntps
        );
        if bitwise {
            self.sse_operand(operands[0], false);
        }
        self.sse_operand(operands[1], bitwise || aligned);
        if bitwise {
            self.simd(match opcode {
                SseOpcode::Por => 0x50,
                SseOpcode::And => 0x4e,
                _ => 0x51,
            });
        }
        self.set(VECTOR);
        if aligned && let Operand::Mem(memory) = operands[0] {
            self.effective(memory, true);
            self.set(ADDR);
            self.get(ADDR);
            self.num(15);
            self.op(0x71);
            self.check(6, ADDR, 0);
        }
        self.prepare_store(operands[0]);
        self.sse_assign(operands[0]);
    }
}
