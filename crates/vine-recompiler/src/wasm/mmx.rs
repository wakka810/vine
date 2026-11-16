//! Reached MMX packed-integer operations lowered directly to Wasm SIMD.
//!
//! Architectural effects follow Intel SDM Vol. 3A, section 14.2/14.4:
//! https://cdrdv2-public.intel.com/835754/253668-sdm-vol-3a.pdf
//! Lane semantics follow the corresponding Intel Vol. 2B instruction entries:
//! https://cdrdv2.intel.com/v1/dl/getContent/671241
//! SIMD encodings: https://github.com/WebAssembly/simd/blob/main/proposals/simd/BinarySIMD.md
//! Only the low 64 bits of each local are guest-visible. No x87 value is
//! converted to f64; MMn aliases the low bits of physical x87 slot n.
//! MOVQ2DQ/MOVDQ2Q share the MMX pending-exception and tag/TOP entry effects.
//! Only MOVDQ2Q marks an MMX destination written; MOVQ2DQ leaves its source's
//! complete raw80 payload intact and clears the XMM destination's upper64.
//! MOVNTQ uses the same checked m64 store as MOVQ. Its non-temporal hint
//! permits sequential visibility in the single guest CPU; unlike MOVNTPS,
//! it does not require 16-byte alignment (admission already requires AC=0).
//! https://cdrdv2-public.intel.com/671488/248966-Software-Optimization-Manual-V1-048.pdf
//! PUNPCKLWD MMX has an m32 source, unlike its XMM m128 form. Its memory
//! lowering reads exactly four bytes before the common MMX entry/commit effects.

use super::*;

const STATUS: u32 = 30;
const TAG: u32 = 31;
const WRITTEN: u32 = 32;
const MM: u32 = 33;
const VECTOR: u32 = 41;
const BITS: u32 = 42;

impl Emitter {
    pub(super) fn simd(&mut self, opcode: u32) {
        self.op(0xfd);
        uleb(&mut self.code, opcode);
    }
    pub(super) fn vector_zero(&mut self) {
        self.simd(0x0c);
        self.code.extend([0; 16]);
    }
    fn shuffle(&mut self, lanes: [u8; 16]) {
        self.simd(0x0d);
        self.code.extend(lanes);
    }

    pub(super) fn initialize_mmx(&mut self) {
        // Reloading after an x87 helper must retire the old MMX-write bitmap:
        // those physical slots may now contain full 80-bit x87 results.
        self.num(0);
        self.set(WRITTEN);
        for index in 0..8 {
            self.get(0);
            self.simd(0x5d);
            uleb(&mut self.code, 0);
            uleb(&mut self.code, 140 + index * 12);
            self.set(MM + index);
        }
        self.load(0, 240);
        self.set(STATUS);
        self.load(0, 244);
        self.set(TAG);
    }

    pub(super) fn flush_mmx(&mut self) {
        for index in 0..8 {
            self.get(WRITTEN);
            self.num(1 << index);
            self.op(0x71);
            self.if_();
            self.get(0);
            self.get(MM + index);
            self.simd(0x1d);
            self.op(0);
            self.mem(0x37, 140 + index * 12);
            self.get(0);
            self.num(0xffff);
            self.mem(0x36, 148 + index * 12);
            self.end();
        }
        self.store(0, 240, STATUS);
        self.store(0, 244, TAG);
    }

    fn vector_operand(&mut self, operand: Operand) {
        match operand {
            Operand::Mmx(index) => self.get(MM + u32::from(index)),
            Operand::Mem(memory) => {
                self.effective(memory, true);
                self.set(ADDR);
                self.access(Width::W64, false, memory.segment == Segment::Ss);
                if self.loop_context.is_some() {
                    self.get(PTR);
                    self.mem(0x29, 0);
                    self.simd(0x12);
                    return;
                }
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
                for n in 0..8 {
                    self.byte_pointer(n);
                    self.mem(0x2d, 0);
                    self.op(0xad);
                    self.op(0x42);
                    sleb(&mut self.code, i64::from(n * 8));
                    self.op(0x86);
                    self.get(BITS);
                    self.op(0x84);
                    self.set(BITS);
                }
                self.end();
                self.get(BITS);
                self.simd(0x12);
            }
            _ => unreachable!("validated MMX vector operand"),
        }
    }

    fn vector_assign(&mut self, destination: Operand) {
        match destination {
            Operand::Mmx(index) => {
                self.get(VECTOR);
                self.set(MM + u32::from(index));
                self.get(WRITTEN);
                self.num(1 << index);
                self.op(0x72);
                self.set(WRITTEN);
            }
            Operand::Mem(_) => {
                // Complete permission/range/journal preflight was performed
                // before any store or architectural MMX state modification.
                self.get(VECTOR);
                self.simd(0x1d);
                self.op(0);
                self.set(BITS);
                if self.loop_context.is_some() {
                    self.get(PTR);
                    self.get(BITS);
                    self.mem(0x37, 0);
                    self.mark_dirty();
                    return;
                }
                self.get(ENTRY);
                self.get(ENTRY2);
                self.op(0x46);
                self.if_();
                self.get(PTR);
                self.get(BITS);
                self.mem(0x37, 0);
                self.op(0x05);
                for n in 0..8 {
                    self.byte_pointer(n);
                    self.get(BITS);
                    self.op(0x42);
                    sleb(&mut self.code, i64::from(n * 8));
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
            _ => unreachable!("validated MMX vector destination"),
        }
    }

    pub(super) fn mmx_instruction(&mut self, opcode: MmxOpcode, operands: &[Operand]) {
        // Pending unmasked x87 exceptions must not be erased by MMX state
        // entry. The outer exception/fallback path owns this unreached case.
        if self.loop_context.is_none() {
            self.load(0, 236);
            self.num(u32::MAX);
            self.op(0x73);
            self.get(STATUS);
            self.op(0x71);
            self.num(0x3f);
            self.op(0x71);
            self.tee(ERROR);
            self.if_();
            self.exit(crate::x87::FLOATING_POINT_EXIT);
            self.end();
        }
        match opcode {
            MmxOpcode::Emms => {}
            MmxOpcode::Movq2dq => {
                let [Operand::Xmm(destination), Operand::Mmx(source)] = operands else {
                    unreachable!("validated MOVQ2DQ register banks");
                };
                self.vector_zero();
                self.get(MM + u32::from(*source));
                self.simd(0x1d); // i64x2.extract_lane
                self.op(0);
                self.simd(0x1e); // i64x2.replace_lane
                self.op(0);
                self.set(super::sse::XMM + u32::from(*destination));
            }
            MmxOpcode::Movdq2q => {
                let Operand::Xmm(source) = operands[1] else {
                    unreachable!("validated MOVDQ2Q source bank");
                };
                self.get(super::sse::XMM + u32::from(source));
                self.set(VECTOR);
                self.vector_assign(operands[0]);
            }
            MmxOpcode::Movd => {
                if matches!(operands[0], Operand::Mmx(_)) {
                    self.operand(operands[1]);
                    self.set(R);
                    self.vector_zero();
                    self.get(R);
                    self.simd(0x1c);
                    self.op(0);
                    self.set(VECTOR);
                    self.vector_assign(operands[0]);
                } else {
                    self.vector_operand(operands[1]);
                    self.simd(0x1b);
                    self.op(0);
                    self.set(R);
                    self.prepare_store(operands[0]);
                    self.assign(operands[0]);
                }
            }
            MmxOpcode::Movq | MmxOpcode::Movntq => {
                self.vector_operand(operands[1]);
                self.set(VECTOR);
                self.prepare_store(operands[0]);
                self.vector_assign(operands[0]);
            }
            MmxOpcode::Psraw => {
                self.vector_operand(operands[0]);
                let Operand::Imm(count) = operands[1] else {
                    unreachable!("validated immediate shift");
                };
                self.num(count.min(15));
                self.simd(0x8c);
                self.set(VECTOR);
                self.vector_assign(operands[0]);
            }
            MmxOpcode::Psrlw => {
                if let Operand::Imm(count) = operands[1] {
                    if count > 15 {
                        self.vector_zero();
                    } else {
                        self.vector_operand(operands[0]);
                        self.num(count);
                        self.simd(0x8d); // i16x8.shr_u
                    }
                    self.set(VECTOR);
                } else {
                    // Intel PSRLW uses the entire unsigned 64-bit count.
                    // Check it before Wasm's modulo-16 lane shift. Load the
                    // complete count before changing any destination/FP state,
                    // including when count aliases the destination register.
                    self.vector_operand(operands[1]);
                    self.simd(0x1d);
                    self.op(0);
                    self.set(BITS);
                    self.get(BITS);
                    self.op(0x42);
                    sleb(&mut self.code, 15);
                    self.op(0x56); // i64.gt_u
                    self.if_();
                    self.vector_zero();
                    self.set(VECTOR);
                    self.op(0x05);
                    self.vector_operand(operands[0]);
                    self.get(BITS);
                    self.op(0xa7);
                    self.simd(0x8d);
                    self.set(VECTOR);
                    self.end();
                }
                // Even count zero writes the MMX alias and performs its normal
                // tag/TOP and destination exponent effects; it is not a NOP.
                self.vector_assign(operands[0]);
            }
            op => {
                self.vector_operand(operands[0]);
                if op == MmxOpcode::Punpcklwd && matches!(operands[1], Operand::Mem(_)) {
                    self.operand(operands[1]);
                    self.simd(0x11); // i32x4.splat; only source low32 is used.
                } else {
                    self.vector_operand(operands[1]);
                }
                match op {
                    MmxOpcode::Pxor => self.simd(0x51),
                    MmxOpcode::Punpcklbw => {
                        self.shuffle([0, 16, 1, 17, 2, 18, 3, 19, 0, 16, 1, 17, 2, 18, 3, 19])
                    }
                    MmxOpcode::Punpcklwd => {
                        self.shuffle([0, 1, 16, 17, 2, 3, 18, 19, 0, 1, 16, 17, 2, 3, 18, 19])
                    }
                    // The high half of a guest MMX register begins at byte 4,
                    // not byte 8 of the Wasm v128 local. Its upper64 is scratch.
                    MmxOpcode::Punpckhbw => {
                        self.shuffle([4, 20, 5, 21, 6, 22, 7, 23, 4, 20, 5, 21, 6, 22, 7, 23])
                    }
                    MmxOpcode::Punpckhwd => {
                        self.shuffle([4, 5, 20, 21, 6, 7, 22, 23, 4, 5, 20, 21, 6, 7, 22, 23])
                    }
                    MmxOpcode::Punpckhdq => {
                        self.shuffle([4, 5, 6, 7, 20, 21, 22, 23, 4, 5, 6, 7, 20, 21, 22, 23])
                    }
                    MmxOpcode::Psubw => self.simd(0x91),
                    MmxOpcode::Pmullw => self.simd(0x95),
                    MmxOpcode::Paddw => self.simd(0x8e),
                    MmxOpcode::Packuswb => {
                        // MMX packs four signed words from each operand. Join
                        // exactly those low halves before Wasm narrows 8 lanes.
                        self.shuffle([0, 1, 2, 3, 4, 5, 6, 7, 16, 17, 18, 19, 20, 21, 22, 23]);
                        self.vector_zero();
                        self.simd(0x66);
                    }
                    _ => unreachable!("handled MMX move/shift/state operation"),
                }
                self.set(VECTOR);
                self.vector_assign(operands[0]);
            }
        }
        self.get(STATUS);
        self.num(!0x3800);
        self.op(0x71);
        self.set(STATUS);
        self.num(if opcode == MmxOpcode::Emms { 0xffff } else { 0 });
        self.set(TAG);
    }
}
