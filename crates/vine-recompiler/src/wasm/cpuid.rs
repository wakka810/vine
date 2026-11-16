//! Ordered CPUID evaluation from the shared immutable guest profile.
use super::*;

impl Emitter {
    pub(super) fn cpuid_instruction(&mut self) {
        // ECX is both an input and an output. Preserve both selectors before
        // assigning any architectural output, including the default zeros.
        self.get(3);
        self.set(A);
        self.get(4);
        self.set(B);
        for local in [3, 6, 4, 5] {
            self.num(0);
            self.set(local);
        }
        for row in vine_cpu_profile::CPUID_LEAVES {
            self.get(A);
            self.num(row.leaf);
            self.op(0x46);
            if let Some(subleaf) = row.subleaf {
                self.get(B);
                self.num(subleaf);
                self.op(0x46);
                self.op(0x71);
            }
            self.if_();
            for (local, value) in [3, 6, 4, 5].into_iter().zip(row.result) {
                self.num(value);
                self.set(local);
            }
            self.end();
        }
        // The synchronous, single-worker generated body already completes all
        // preceding architectural effects before entering here. No instruction
        // or memory motion is permitted across this serializing IR operation.
        // FLAGS and DEFINED deliberately remain unchanged.
    }
}
