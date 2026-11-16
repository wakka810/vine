//! Exact x87 slow path through the independent arithmetic module. GPRs and
//! flags remain local across calls; guest memory and x87 state stay Rust-owned.
use super::*;

impl Emitter {
    pub(super) fn x87_instruction(
        &mut self,
        descriptor: u32,
        fop: u16,
        segment: u32,
        source: Option<Operand>,
        eip: u32,
    ) {
        if self.has_mmx {
            self.flush_mmx();
        }
        // Initial exact helper stores may retire the generated dirty journal.
        // Do not carry a speculative memory certificate through this call.
        self.invalidate_affine(u8::MAX);
        self.op(0x23); // global.get: immutable backend handle, never a JS call.
        uleb(&mut self.code, 0);
        self.num(descriptor);
        self.num(segment);
        if let Some(Operand::Mem(memory)) = source {
            // The helper checks pending #MF before segment and page faults.
            self.effective(memory, false);
        } else {
            self.num(0);
        }
        self.num(eip);
        self.num(u32::from(fop));
        self.op(0x10); // call imported backend_x87 Wasm function directly.
        uleb(&mut self.code, 1); // resolver occupies function import zero
        self.set(KIND);
        if self.has_mmx {
            self.initialize_mmx();
        }
        self.get(KIND);
        self.num(7);
        self.op(0x46);
        self.if_();
        self.num(1);
        self.set(SMC);
        self.op(0x05);
        self.get(KIND);
        self.if_();
        self.load(0, 408);
        self.set(FAULT);
        self.load(0, 412);
        self.set(ERROR);
        self.branch("exit");
        self.end();
        self.end();
        self.num(0);
        self.set(KIND);
    }

    pub(super) fn x87_status_ax(&mut self) {
        if self.has_mmx {
            self.flush_mmx();
        }
        self.load(0, 240);
        self.set(R);
        self.assign(Operand::Reg(Register {
            index: 0,
            width: Width::W16,
            high8: false,
        }));
    }
}
