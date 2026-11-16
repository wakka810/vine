//! One-operand MUL/IMUL keep the architectural double-width product.
//! Signed CF/OF compare that product with the sign-extended low half.
//! https://cdrdv2-public.intel.com/868137/325462-089-sdm-vol-1-2abcd-3abcd-4.pdf
use super::*;

impl Emitter {
    pub(super) fn wide_multiply(&mut self, instruction: &Instruction) {
        let width = instruction.width;
        let signed = instruction.opcode == Opcode::ImulWide;
        // Read the explicit source before either implicit destination changes.
        // A memory fault therefore retains EAX/EDX and the preceding flags.
        self.operand(instruction.operands[0]);
        self.set(B);
        self.get(3);
        self.truncate(width);
        if signed {
            self.sign_extend(width);
        }
        self.op(if signed { 0xac } else { 0xad });
        self.get(B);
        if signed {
            self.sign_extend(width);
        }
        self.op(if signed { 0xac } else { 0xad });
        self.op(0x7e); // i64.mul, wrapping exactly at 64 bits
        self.set(self.division_scratch);
        self.get(self.division_scratch);
        self.op(0xa7);
        self.set(R);
        if signed {
            // No signed product of two <=32-bit operands overflows i64.
            // The full product must fit the signed low half to clear CF/OF.
            self.get(R);
            self.sign_extend(width);
            self.op(0xac);
            self.get(self.division_scratch);
            self.op(0x52); // i64.ne
            self.set(CARRY);
        }
        self.assign(Operand::Reg(Register {
            index: 0,
            width: if width == Width::W8 {
                Width::W16
            } else {
                width
            },
            high8: false,
        }));
        self.get(self.division_scratch);
        self.op(0x42);
        sleb(&mut self.code, i64::from(width.bits()));
        self.op(0x88); // i64.shr_u
        self.op(0xa7);
        self.set(B);
        if width != Width::W8 {
            self.get(B);
            self.set(R);
            self.assign(Operand::Reg(Register {
                index: 2,
                width,
                high8: false,
            }));
        }
        self.get(FLAGS);
        self.num(!(CF | OF));
        self.op(0x71);
        self.set(FLAGS);
        if !signed {
            self.get(B);
            self.num(0);
            self.op(0x47); // unsigned high half is nonzero
            self.set(CARRY);
        }
        self.get(CARRY);
        self.add_flag(CF);
        self.get(CARRY);
        self.add_flag(OF);
    }
}
