//! Experimental cross-block affine memory proofs. No guest operation is
//! skipped or moved: a broad proof miss executes the original precise access.
use super::*;
use crate::optimize::AffineRange;

#[derive(Clone)]
pub(super) struct Certificate {
    pub range: AffineRange,
    /// Three i32 locals: valid, host pointer corresponding to GPR, page entry.
    pub local: u32,
}

impl Emitter {
    fn certificate(&self, m: MemoryOperand, write: bool) -> Option<Certificate> {
        if m.index.is_some() || m.segment == Segment::Fs || m.width.bytes() > 4 {
            return None;
        }
        self.affine
            .iter()
            .find(|c| {
                m.base == Some(c.range.base)
                    && i64::from(m.displacement as i32) >= i64::from(c.range.minimum)
                    && i64::from(m.displacement as i32) + i64::from(m.width.bytes()) - 1
                        <= i64::from(c.range.maximum)
                    && c.range.permissions & if write { 2 } else { 1 } != 0
            })
            .cloned()
    }

    fn certify(&mut self, c: &Certificate) {
        // A single unsigned interval test proves every included displacement
        // remains in this page. It also proves no linear u32 wrap, including
        // the very first and last pages. Failure does not raise a fault.
        self.get(3 + u32::from(c.range.base));
        self.num(4095);
        self.op(0x71);
        self.tee(TMP);
        self.num(c.range.minimum.unsigned_abs());
        self.op(0x6b);
        self.num((4095 - c.range.maximum + c.range.minimum) as u32);
        self.op(0x4d); // le_u
        self.if_();
        self.get(1);
        self.get(3 + u32::from(c.range.base));
        self.num(12);
        self.op(0x76);
        self.num(4);
        self.op(0x74);
        self.op(0x6a);
        self.set(ENTRY);
        let need = 8 | c.range.permissions;
        self.load(ENTRY, 4);
        self.num(need);
        self.op(0x71);
        self.num(need);
        self.op(0x46);
        self.if_();
        self.get(ENTRY);
        self.set(c.local + 2);
        self.load(ENTRY, 0);
        self.get(TMP);
        self.op(0x6a);
        self.set(c.local + 1);
        self.num(1);
        self.set(c.local);
        self.end();
        self.end();
    }

    fn certified_pointer(&mut self, c: &Certificate, displacement: u32) {
        self.get(c.local + 1);
        self.num(displacement);
        self.op(0x6a);
    }

    fn certified_read(&mut self, c: &Certificate, m: MemoryOperand) {
        self.certified_pointer(c, m.displacement);
        self.mem(
            match m.width {
                Width::W8 => 0x2d,
                Width::W16 => 0x2f,
                Width::W32 => 0x28,
                _ => unreachable!(),
            },
            0,
        );
        self.set(R);
    }

    pub(super) fn affine_read(&mut self, m: MemoryOperand) -> bool {
        let Some(c) = self.certificate(m, false) else {
            return false;
        };
        // The hot hit needs one validity branch and one cached-base load.
        // The small load is duplicated only on the cold certification path.
        self.get(c.local);
        self.if_();
        self.certified_read(&c, m);
        self.op(0x05);
        self.certify(&c);
        self.get(c.local);
        self.if_();
        self.certified_read(&c, m);
        self.op(0x05);
        self.effective(m, true);
        self.set(ADDR);
        self.read_memory(m.width, m.segment == Segment::Ss);
        self.set(R);
        self.end();
        self.end();
        self.get(R);
        true
    }

    pub(super) fn affine_store(&mut self, m: MemoryOperand) -> bool {
        let Some(c) = self.certificate(m, true) else {
            return false;
        };
        self.get(c.local);
        self.op(0x45);
        self.if_();
        self.certify(&c);
        self.end();
        self.get(c.local);
        self.if_();
        // Precise address remains available for journal capacity exceptions.
        self.effective(m, true);
        self.set(ADDR);
        self.get(c.local + 2);
        self.tee(ENTRY);
        self.set(ENTRY2);
        self.dirty_preflight();
        self.certified_pointer(&c, m.displacement);
        self.set(PTR);
        self.op(0x05);
        self.effective(m, true);
        self.set(ADDR);
        self.access(m.width, true, m.segment == Segment::Ss);
        self.end();
        true
    }

    pub(super) fn affine_rmw_read(&mut self, m: MemoryOperand) -> bool {
        let Some(c) = self
            .certificate(m, true)
            .filter(|c| c.range.permissions & 3 == 3)
        else {
            return false;
        };
        self.get(c.local);
        self.op(0x45);
        self.if_();
        self.certify(&c);
        self.end();
        self.get(c.local);
        self.if_();
        // Keep the exact checked destination entries until the following
        // arithmetic/shift decides whether it actually commits a store.
        self.effective(m, true);
        self.set(ADDR);
        self.get(c.local + 2);
        self.tee(ENTRY);
        self.set(ENTRY2);
        self.certified_read(&c, m);
        self.op(0x05);
        self.effective(m, true);
        self.set(ADDR);
        self.access_checked(m.width, true, m.segment == Segment::Ss, true);
        self.read_prepared_memory(m.width);
        self.set(R);
        self.end();
        self.get(R);
        true
    }

    pub(super) fn invalidate_affine(&mut self, writes: u8) {
        let locals: Vec<_> = self
            .affine
            .iter()
            .filter(|c| writes & (1 << c.range.base) != 0)
            .map(|c| c.local)
            .collect();
        for local in locals {
            self.num(0);
            self.set(local);
        }
    }
}
