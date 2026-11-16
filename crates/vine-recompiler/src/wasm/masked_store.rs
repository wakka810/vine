//! Legacy MASKMOVDQU, with a full-span write-fault policy.
//!
//! Intel permits address/page faults with an all-zero mask; AMD explicitly
//! permits faults for unselected bytes. Check the entire unaligned 16-byte
//! destination before any store, as the native i386 page-boundary probe does.
//! No destination read is required. Only selected bytes, and pages containing
//! them, are written/marked dirty. A zero mask has no journal or SMC effect.
//! The WC hint permits the ordinary visibility of our single guest CPU.
//! https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-software-developer-vol-2b-manual.pdf
//! AMD64 APM Vol.4, MASKMOVDQU (publication 26568).

use super::*;

impl Emitter {
    pub(super) fn sse_masked_store(&mut self, operands: &[Operand]) {
        let [Operand::Mem(memory), Operand::Xmm(data), Operand::Xmm(mask)] = operands else {
            unreachable!("validated MASKMOVDQU operands");
        };
        self.effective(*memory, true);
        self.set(ADDR);
        // Check write permission even for unselected bytes, but do not reserve
        // journal slots for pages this instruction will not write.
        self.access_span(
            Width::W128,
            true,
            memory.segment == Segment::Ss,
            false,
            false,
        );
        self.get(sse::XMM + u32::from(*mask));
        self.simd(0x64); // i8x16.bitmask: exactly the 16 byte sign bits.
        self.set(R);
        self.get(ENTRY);
        self.get(ENTRY2);
        self.op(0x46);
        self.if_();
        self.get(R);
        self.num(0);
        self.op(0x47);
        self.set(A);
        self.num(0);
        self.set(B);
        self.op(0x05);
        // A crossing span has 1..15 bytes before the second page.
        self.num(4096);
        self.get(ADDR);
        self.num(4095);
        self.op(0x71);
        self.op(0x6b);
        self.set(TMP);
        self.get(R);
        self.num(1);
        self.get(TMP);
        self.op(0x74);
        self.num(1);
        self.op(0x6b);
        self.op(0x71);
        self.num(0);
        self.op(0x47);
        self.set(A);
        self.get(R);
        self.get(TMP);
        self.op(0x76);
        self.num(0);
        self.op(0x47);
        self.set(B);
        self.end();

        self.load(ENTRY, 8);
        self.num(1);
        self.op(0x47);
        self.get(A);
        self.op(0x71);
        self.load(ENTRY2, 8);
        self.num(1);
        self.op(0x47);
        self.get(B);
        self.op(0x71);
        self.op(0x6a);
        self.set(TMP);
        self.dirty_capacity();
        for lane in 0..16 {
            self.get(R);
            self.num(1 << lane);
            self.op(0x71);
            self.if_();
            self.byte_pointer(lane);
            self.get(sse::XMM + u32::from(*data));
            self.simd(0x16); // i8x16.extract_lane_u
            self.op(lane as u8);
            self.mem(0x3a, 0);
            self.end();
        }
        self.get(1);
        self.num(16);
        self.op(0x6b);
        self.set(TMP);
        self.num(0);
        self.set(SMC);
        for (entry, selected) in [(ENTRY, A), (ENTRY2, B)] {
            self.get(selected);
            self.if_();
            self.mark_dirty_one(entry);
            self.load(entry, 4);
            self.num(4 | 32);
            self.op(0x71);
            self.get(SMC);
            self.op(0x72);
            self.set(SMC);
            self.end();
        }
    }
}
