//! Page-bounded loop lowering. A miss reaches ordinary precise code without
//! guest mutation. Successful batches contain no faulting helper/host call.
use super::*;
use crate::loop_ir::Loop;
use std::rc::Rc;

#[derive(Clone)]
pub(super) struct Context {
    pub plan: Rc<Loop>,
    pub base: u32,
}
impl Context {
    fn entry(&self, range: usize) -> u32 {
        self.base + 1 + range as u32 * 3
    }
    fn host(&self, range: usize) -> u32 {
        self.entry(range) + 1
    }
    fn written(&self, range: usize) -> u32 {
        self.entry(range) + 2
    }
    fn proof(&self) -> String {
        format!("lm-proof{}", self.plan.header)
    }
    fn run(&self) -> String {
        format!("lm-run{}", self.plan.header)
    }
    fn miss(&self) -> String {
        format!("lm-fallback{}", self.plan.header)
    }
}
impl Emitter {
    fn loop_miss_if(&mut self, context: &Context) {
        self.if_();
        self.branch(&context.miss());
        self.end();
    }
    fn loop_proof(&mut self, context: &Context) {
        self.get(LEFT);
        self.num(context.plan.maximum_instructions);
        self.op(0x6e);
        self.set(context.base);
        self.get(context.base);
        self.num(2);
        self.op(0x49);
        self.loop_miss_if(context);
        if context.plan.has_mmx {
            // MMX never adds an unmasked pending exception. Check the initial
            // condition once; misses preserve the ordinary instruction ordering.
            self.load(0, 236);
            self.num(u32::MAX);
            self.op(0x73);
            self.get(30);
            self.op(0x71);
            self.num(0x3f);
            self.op(0x71);
            self.loop_miss_if(context);
        }
        for (index, range) in context.plan.ranges.iter().enumerate() {
            self.num(range.minimum as u32);
            for (register, coefficient) in range.coefficients.iter().copied().enumerate() {
                if coefficient == 0 {
                    continue;
                }
                self.get(3 + register as u32);
                if coefficient != 1 {
                    self.num(coefficient as u32);
                    self.op(0x6c);
                }
                self.op(0x6a);
            }
            self.set(ADDR);
            let maximum_offset = 4095 - (range.maximum - range.minimum) as u32;
            self.get(ADDR);
            self.num(4095);
            self.op(0x71);
            self.num(maximum_offset);
            self.op(0x4b);
            self.loop_miss_if(context);
            self.get(1);
            self.get(ADDR);
            self.num(12);
            self.op(0x76);
            self.num(4);
            self.op(0x74);
            self.op(0x6a);
            self.set(context.entry(index));
            let need = crate::memory::MAPPED | range.permissions;
            self.load(context.entry(index), 4);
            self.num(need);
            self.op(0x71);
            self.num(need);
            self.op(0x47);
            self.loop_miss_if(context);
            if range.permissions & crate::memory::WRITE != 0 {
                self.load(context.entry(index), 4);
                self.num(crate::memory::EXECUTE | crate::memory::CODE_ALIAS);
                self.op(0x71);
                self.loop_miss_if(context);
            }
            self.load(context.entry(index), 0);
            self.set(context.host(index));
            self.num(0);
            self.set(context.written(index));
            if range.stride != 0 {
                if range.stride > 0 {
                    self.num(maximum_offset);
                    self.get(ADDR);
                    self.num(4095);
                    self.op(0x71);
                    self.op(0x6b);
                } else {
                    self.get(ADDR);
                    self.num(4095);
                    self.op(0x71);
                }
                self.num(range.stride.unsigned_abs());
                self.op(0x6e);
                self.num(1);
                self.op(0x6a);
                self.tee(TMP);
                self.get(context.base);
                self.op(0x49);
                self.if_();
                self.get(TMP);
                self.set(context.base);
                self.end();
            }
        }
        self.get(context.base);
        self.num(2);
        self.op(0x49);
        self.loop_miss_if(context);
        // This reserves no entries and publishes no dirty pages. A conservative
        // duplicate-page count may miss the fast path, never fabricate a fault.
        if context
            .plan
            .ranges
            .iter()
            .any(|range| range.permissions & 2 != 0)
        {
            self.get(1);
            self.num(16);
            self.op(0x6b);
            self.set(PTR);
            self.load(PTR, 4);
            self.load(PTR, 8);
            self.op(0x4b);
            self.loop_miss_if(context);
            self.num(0);
            for (index, range) in context.plan.ranges.iter().enumerate() {
                if range.permissions & 2 == 0 {
                    continue;
                }
                self.load(context.entry(index), 8);
                self.num(1);
                self.op(0x47);
                self.op(0x6a);
            }
            self.load(PTR, 8);
            self.load(PTR, 4);
            self.op(0x6b);
            self.op(0x4b);
            self.loop_miss_if(context);
        }
    }
    fn loop_commit(&mut self, context: &Context) {
        self.get(1);
        self.num(16);
        self.op(0x6b);
        self.set(TMP);
        for (index, range) in context.plan.ranges.iter().enumerate() {
            if range.permissions & 2 == 0 {
                continue;
            }
            self.get(context.written(index));
            self.if_();
            self.mark_dirty_one(context.entry(index));
            self.end();
        }
    }
    pub(super) fn loop_jump(&mut self, target: u32) -> bool {
        let Some(context) = self.loop_context.clone() else {
            return false;
        };
        self.num(target);
        self.set(PC);
        if target == context.plan.header {
            self.get(context.base);
            self.num(1);
            self.op(0x6b);
            self.tee(context.base);
            self.if_();
            self.branch(&context.run());
            self.end();
            self.loop_commit(&context);
            self.branch(&context.proof());
        } else if context.plan.body.contains(&target) {
            self.branch(&format!("lm-block{target}"));
        } else {
            assert_eq!(target, context.plan.exit, "loop proof must own every exit");
            self.loop_commit(&context);
            self.loop_context = None;
            self.jump(target);
            self.loop_context = Some(context);
        }
        true
    }
    pub(super) fn loop_access(&mut self, width: Width, write: bool) -> bool {
        let Some(context) = self.loop_context.clone() else {
            return false;
        };
        let access = &context.plan.accesses[&self.loop_instruction];
        assert_eq!((access.width, access.write), (width, write));
        self.get(context.entry(access.range));
        self.tee(ENTRY);
        self.set(ENTRY2);
        self.get(context.host(access.range));
        self.tee(PTR2);
        self.get(ADDR);
        self.num(4095);
        self.op(0x71);
        self.op(0x6a);
        self.set(PTR);
        true
    }
    pub(super) fn loop_mark_dirty(&mut self) -> bool {
        let Some(context) = self.loop_context.clone() else {
            return false;
        };
        let access = &context.plan.accesses[&self.loop_instruction];
        assert!(access.write);
        self.num(1);
        self.set(context.written(access.range));
        true
    }
    pub(super) fn loop_memory(&mut self, region: &Region, plan: Rc<Loop>) {
        let context = Context {
            plan,
            base: self.loop_base,
        };
        self.open(0x02, context.miss());
        self.open(0x03, context.proof());
        self.loop_proof(&context);
        self.loop_context = Some(context.clone());
        self.open(0x03, context.run());
        for &pc in context.plan.body.iter().skip(1).rev() {
            self.open(0x02, format!("lm-block{pc}"));
        }
        for (index, &pc) in context.plan.body.iter().enumerate() {
            if index > 0 {
                self.end();
            }
            let block = &region.blocks[&pc];
            for (instruction, flags) in block
                .instructions
                .iter()
                .zip(crate::optimize::flags_needed_after(block))
            {
                self.instruction(instruction, Some(flags));
            }
            if !block
                .instructions
                .last()
                .is_some_and(|i| matches!(i.opcode, Opcode::Jcc | Opcode::Jmp))
            {
                self.block_fallthrough(block);
            }
        }
        self.end();
        self.end();
        self.end();
        self.loop_context = None;
        self.block_body(&region.blocks[&context.plan.header]);
    }
}
