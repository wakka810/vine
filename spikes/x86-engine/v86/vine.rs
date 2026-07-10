use crate::cpu::cpu::{full_clear_tlb, reg128, update_state_flags};
use crate::cpu::fpu::set_control_word;
use crate::cpu::global_pointers::*;
use crate::softfloat::F80;

const MAGIC: u32 = 0x5649_4E45;
const VERSION: u32 = 1;
const WORD_COUNT: usize = 160;

const HEADER_MAGIC: usize = 0;
const HEADER_VERSION: usize = 1;
const HEADER_WORD_COUNT: usize = 2;
const GPR_BASE: usize = 4;
const EIP: usize = 12;
const PREVIOUS_EIP: usize = 13;
const EFLAGS: usize = 14;
const FLAGS_CHANGED: usize = 15;
const LAST_OP_SIZE: usize = 16;
const LAST_OP1: usize = 17;
const LAST_RESULT: usize = 18;
const PREFIXES: usize = 19;
const CPL: usize = 20;
const PROTECTED_MODE: usize = 21;
const IS_32: usize = 22;
const STACK_SIZE_32: usize = 23;
const IN_HLT: usize = 24;
const SYSENTER_CS: usize = 25;
const SYSENTER_ESP: usize = 26;
const SYSENTER_EIP: usize = 27;
const SREG_BASE: usize = 28;
const SEGMENT_OFFSET_BASE: usize = 36;
const SEGMENT_LIMIT_BASE: usize = 44;
const SEGMENT_META_BASE: usize = 52;
const CR_BASE: usize = 60;
const DREG_BASE: usize = 68;
const PDPTE_BASE: usize = 76;
const IDTR_OFFSET: usize = 84;
const IDTR_SIZE: usize = 85;
const GDTR_OFFSET: usize = 86;
const GDTR_SIZE: usize = 87;
const TSS_SIZE_32: usize = 88;
const MXCSR: usize = 89;
const FPU_STACK_EMPTY: usize = 90;
const FPU_STACK_PTR: usize = 91;
const FPU_CONTROL_WORD: usize = 92;
const FPU_STATUS_WORD: usize = 93;
const FPU_OPCODE: usize = 94;
const FPU_IP: usize = 95;
const FPU_IP_SELECTOR: usize = 96;
const FPU_DP: usize = 97;
const FPU_DP_SELECTOR: usize = 98;
const FPU_BASE: usize = 100;
const XMM_BASE: usize = 124;

static mut STATE: [u32; WORD_COUNT] = [0; WORD_COUNT];

unsafe fn state_ptr() -> *mut u32 { (&raw mut STATE).cast::<u32>() }

unsafe fn put(index: usize, value: u32) { *state_ptr().add(index) = value }

unsafe fn get(index: usize) -> u32 { *state_ptr().add(index) }

#[no_mangle]
pub fn vine_cpu_state_version() -> u32 { VERSION }

#[no_mangle]
pub fn vine_cpu_state_size() -> u32 { (WORD_COUNT * size_of::<u32>()) as u32 }

#[no_mangle]
pub unsafe fn vine_cpu_state_ptr() -> u32 { state_ptr() as u32 }

#[no_mangle]
pub unsafe fn vine_cpu_state_save() {
    put(HEADER_MAGIC, MAGIC);
    put(HEADER_VERSION, VERSION);
    put(HEADER_WORD_COUNT, WORD_COUNT as u32);

    for index in 0..8 {
        put(GPR_BASE + index, *reg32.add(index) as u32);
        put(SREG_BASE + index, *sreg.add(index) as u32);
        put(SEGMENT_OFFSET_BASE + index, *segment_offsets.add(index) as u32);
        put(SEGMENT_LIMIT_BASE + index, *segment_limits.add(index));
        put(
            SEGMENT_META_BASE + index,
            *segment_access_bytes.add(index) as u32
                | (*segment_is_null.add(index) as u32) << 8,
        );
        put(CR_BASE + index, *cr.add(index) as u32);
        put(DREG_BASE + index, *dreg.add(index) as u32);
    }

    put(EIP, *instruction_pointer as u32);
    put(PREVIOUS_EIP, *previous_ip as u32);
    put(EFLAGS, *flags as u32);
    put(FLAGS_CHANGED, *flags_changed as u32);
    put(LAST_OP_SIZE, *last_op_size as u32);
    put(LAST_OP1, *last_op1 as u32);
    put(LAST_RESULT, *last_result as u32);
    put(PREFIXES, *prefixes as u32);
    put(CPL, *cpl as u32);
    put(PROTECTED_MODE, *protected_mode as u32);
    put(IS_32, *is_32 as u32);
    put(STACK_SIZE_32, *stack_size_32 as u32);
    put(IN_HLT, *in_hlt as u32);
    put(SYSENTER_CS, *sysenter_cs as u32);
    put(SYSENTER_ESP, *sysenter_esp as u32);
    put(SYSENTER_EIP, *sysenter_eip as u32);

    for index in 0..4 {
        let value = *reg_pdpte.add(index);
        put(PDPTE_BASE + index * 2, value as u32);
        put(PDPTE_BASE + index * 2 + 1, (value >> 32) as u32);
    }

    put(IDTR_OFFSET, *idtr_offset as u32);
    put(IDTR_SIZE, *idtr_size as u32);
    put(GDTR_OFFSET, *gdtr_offset as u32);
    put(GDTR_SIZE, *gdtr_size as u32);
    put(TSS_SIZE_32, *tss_size_32 as u32);
    put(MXCSR, *mxcsr as u32);
    put(FPU_STACK_EMPTY, *fpu_stack_empty as u32);
    put(FPU_STACK_PTR, *fpu_stack_ptr as u32);
    put(FPU_CONTROL_WORD, *fpu_control_word as u32);
    put(FPU_STATUS_WORD, *fpu_status_word as u32);
    put(FPU_OPCODE, *fpu_opcode as u32);
    put(FPU_IP, *fpu_ip as u32);
    put(FPU_IP_SELECTOR, *fpu_ip_selector as u32);
    put(FPU_DP, *fpu_dp as u32);
    put(FPU_DP_SELECTOR, *fpu_dp_selector as u32);

    for index in 0..8 {
        let value = *fpu_st.add(index);
        let base = FPU_BASE + index * 3;
        put(base, value.mantissa as u32);
        put(base + 1, (value.mantissa >> 32) as u32);
        put(base + 2, value.sign_exponent as u32);

        let xmm = *reg_xmm.add(index);
        for lane in 0..4 {
            put(XMM_BASE + index * 4 + lane, xmm.u32[lane]);
        }
    }
}

#[no_mangle]
pub unsafe fn vine_cpu_state_restore() -> u32 {
    if get(HEADER_MAGIC) != MAGIC {
        return 1;
    }
    if get(HEADER_VERSION) != VERSION {
        return 2;
    }
    if get(HEADER_WORD_COUNT) != WORD_COUNT as u32 {
        return 3;
    }

    for index in 0..8 {
        *reg32.add(index) = get(GPR_BASE + index) as i32;
        *sreg.add(index) = get(SREG_BASE + index) as u16;
        *segment_offsets.add(index) = get(SEGMENT_OFFSET_BASE + index) as i32;
        *segment_limits.add(index) = get(SEGMENT_LIMIT_BASE + index);
        let segment_meta = get(SEGMENT_META_BASE + index);
        *segment_access_bytes.add(index) = segment_meta as u8;
        *segment_is_null.add(index) = segment_meta >> 8 & 1 != 0;
        *cr.add(index) = get(CR_BASE + index) as i32;
        *dreg.add(index) = get(DREG_BASE + index) as i32;
    }

    *instruction_pointer = get(EIP) as i32;
    *previous_ip = get(PREVIOUS_EIP) as i32;
    *flags = get(EFLAGS) as i32;
    *flags_changed = get(FLAGS_CHANGED) as i32;
    *last_op_size = get(LAST_OP_SIZE) as i32;
    *last_op1 = get(LAST_OP1) as i32;
    *last_result = get(LAST_RESULT) as i32;
    *prefixes = get(PREFIXES) as u8;
    *cpl = get(CPL) as u8;
    *protected_mode = get(PROTECTED_MODE) != 0;
    *is_32 = get(IS_32) != 0;
    *stack_size_32 = get(STACK_SIZE_32) != 0;
    *in_hlt = get(IN_HLT) != 0;
    *sysenter_cs = get(SYSENTER_CS) as i32;
    *sysenter_esp = get(SYSENTER_ESP) as i32;
    *sysenter_eip = get(SYSENTER_EIP) as i32;

    for index in 0..4 {
        *reg_pdpte.add(index) = get(PDPTE_BASE + index * 2) as u64
            | (get(PDPTE_BASE + index * 2 + 1) as u64) << 32;
    }

    *idtr_offset = get(IDTR_OFFSET) as i32;
    *idtr_size = get(IDTR_SIZE) as i32;
    *gdtr_offset = get(GDTR_OFFSET) as i32;
    *gdtr_size = get(GDTR_SIZE) as i32;
    *tss_size_32 = get(TSS_SIZE_32) != 0;
    *mxcsr = get(MXCSR) as i32;
    *fpu_stack_empty = get(FPU_STACK_EMPTY) as u8;
    *fpu_stack_ptr = get(FPU_STACK_PTR) as u8;
    set_control_word(get(FPU_CONTROL_WORD) as u16);
    *fpu_status_word = get(FPU_STATUS_WORD) as u16;
    *fpu_opcode = get(FPU_OPCODE) as i32;
    *fpu_ip = get(FPU_IP) as i32;
    *fpu_ip_selector = get(FPU_IP_SELECTOR) as i32;
    *fpu_dp = get(FPU_DP) as i32;
    *fpu_dp_selector = get(FPU_DP_SELECTOR) as i32;

    for index in 0..8 {
        let base = FPU_BASE + index * 3;
        *fpu_st.add(index) = F80 {
            mantissa: get(base) as u64 | (get(base + 1) as u64) << 32,
            sign_exponent: get(base + 2) as u16,
        };
        *reg_xmm.add(index) = reg128 {
            u32: [
                get(XMM_BASE + index * 4),
                get(XMM_BASE + index * 4 + 1),
                get(XMM_BASE + index * 4 + 2),
                get(XMM_BASE + index * 4 + 3),
            ],
        };
    }

    *last_virt_eip = -1;
    *eip_phys = 0;
    update_state_flags();
    full_clear_tlb();
    0
}
