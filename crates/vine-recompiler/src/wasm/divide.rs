//! Exact integer division; Wasm traps must never replace a guest #DE.
use super::*;

impl Emitter {
    fn i64_const(&mut self, value: i64) {
        self.op(0x42);
        sleb(&mut self.code, value);
    }

    fn dividend(&mut self, width: Width, signed: bool) {
        match width {
            Width::W8 => {
                self.get(3);
                self.truncate(Width::W16);
                if signed {
                    self.sign_extend(Width::W16);
                }
                self.op(if signed { 0xac } else { 0xad });
            }
            Width::W16 => {
                self.get(5);
                self.num(16);
                self.op(0x74);
                self.get(3);
                self.truncate(Width::W16);
                self.op(0x72);
                self.op(if signed { 0xac } else { 0xad });
            }
            Width::W32 => {
                self.get(5);
                self.op(0xad);
                self.i64_const(32);
                self.op(0x86);
                self.get(3);
                self.op(0xad);
                self.op(0x84);
            }
            _ => unreachable!("integer division width"),
        }
    }

    fn divisor(&mut self, width: Width, signed: bool) {
        self.get(B);
        if signed {
            self.sign_extend(width);
        }
        self.op(if signed { 0xac } else { 0xad });
    }

    pub(super) fn divide(&mut self, instruction: &Instruction) {
        let width = instruction.width;
        let signed = instruction.opcode == Opcode::Idiv;
        self.operand(instruction.operands[0]);
        self.set(B);
        self.get(B);
        self.op(0x45);
        self.if_();
        self.exit(11);
        self.end();
        if signed && width == Width::W32 {
            // i64::MIN / -1 would trap in Wasm before x86 quotient validation.
            self.dividend(width, true);
            self.i64_const(i64::MIN);
            self.op(0x51);
            self.get(B);
            self.num(u32::MAX);
            self.op(0x46);
            self.op(0x71);
            self.if_();
            self.exit(11);
            self.end();
        }
        self.dividend(width, signed);
        self.divisor(width, signed);
        self.op(if signed { 0x7f } else { 0x80 });
        self.set(self.division_scratch);
        self.get(self.division_scratch);
        if signed {
            self.i64_const((1i64 << (width.bits() - 1)) - 1);
            self.op(0x55);
            self.get(self.division_scratch);
            self.i64_const(-(1i64 << (width.bits() - 1)));
            self.op(0x53);
            self.op(0x72);
        } else {
            self.i64_const((1i64 << width.bits()) - 1);
            self.op(0x56);
        }
        self.if_();
        self.exit(11);
        self.end();
        self.dividend(width, signed);
        self.divisor(width, signed);
        self.op(if signed { 0x81 } else { 0x82 });
        self.op(0xa7);
        self.set(A);
        self.get(self.division_scratch);
        self.op(0xa7);
        self.set(R);
        if width == Width::W8 {
            self.get(A);
            self.truncate(Width::W8);
            self.num(8);
            self.op(0x74);
            self.get(R);
            self.truncate(Width::W8);
            self.op(0x72);
            self.set(R);
            self.assign(Operand::Reg(Register {
                index: 0,
                width: Width::W16,
                high8: false,
            }));
        } else {
            self.assign(Operand::Reg(Register {
                index: 0,
                width,
                high8: false,
            }));
            self.get(A);
            self.set(R);
            self.assign(Operand::Reg(Register {
                index: 2,
                width,
                high8: false,
            }));
        }
    }
}
