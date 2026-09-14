use crate::cpu::cpu::{
    invlpg, safe_read128s, safe_read32s, safe_read8, safe_write32, transition_fpu_to_mmx,
    translate_address, translate_address_read_no_side_effects,
    translate_address_write_and_can_skip_dirty, write_mmx_reg64,
};
use crate::cpu::global_pointers::{
    cpl, cr, eip_phys, flags, in_hlt, instruction_counter, instruction_pointer, last_op1,
    last_virt_eip, previous_ip, reg32, reg_xmm, sreg, state_flags,
};
use crate::cpu::{memory, vine};
use crate::page::Page;
use crate::regs::{EAX, EBP, EBX, ECX, EDI, EDX, ESI, ESP};

const STOP_BUDGET: u32 = 1;
const STOP_PENDING_DISPATCH: u32 = 4;
pub(crate) const STOP_ISLAND_ERROR: u32 = 6;
const STOP_INVALID_OPCODE: u32 = 0x106;
const PAGE_BYTES: usize = 4096;
// Null page, directory, all 1024 page tables, and v86's 128 KiB VGA hole.
const DIRECT_TABLE_BYTES: usize = (1 + 1 + 1024 + 32) * PAGE_BYTES;
const PAGE_MASK: u32 = (PAGE_BYTES as u32) - 1;
const PTE_DIRTY: u32 = 0x40;
const MAX_FAST_APIS_PER_SLICE: u32 = 256;
const FAST_RECORD_WORDS: u32 = 36;
const FAST_RECORD_VERSION: u32 = 2;
const GET_TICK_COUNT_FAST_CODE: u32 = 20;
const MEMORY_DELTA_CONTROL_HEADER_BYTES: usize = 56;
const MEMORY_DELTA_MAPPING_DESCRIPTOR_BYTES: usize = 40;
const MEMORY_DELTA_PAGE_DESCRIPTOR_BYTES: usize = 16;
const SIMD_COPY_SIGNATURE: u32 = 0x0080_180f;
const BYTE_COPY_SIGNATURE: u32 = 0xfcf8_8589;
const FORWARD_COPY_SIGNATURE: u32 = 0x8a08_7d8b;
const SORT_SIGNATURE: u32 = 0xdf3b_f38b;
const HUFFMAN_SIGNATURE: u32 = 0xd285_c933;
const BLEND_SIGNATURE: u32 = 0x0104_6f0f;
const BLEND_LOOP: &[u8] = &[
    0x0f, 0x6f, 0x04, 0x01, 0xf3, 0x0f, 0xd6, 0xc0, 0x0f, 0x6f, 0x00, 0xf3, 0x0f, 0xd6, 0xe0, 0x66,
    0x0f, 0x60, 0xe1, 0x66, 0x0f, 0x60, 0xc1, 0x66, 0x0f, 0xf9, 0xe0, 0x66, 0x0f, 0xd5, 0xe2, 0x66,
    0x0f, 0x6f, 0xd8, 0x66, 0x0f, 0x71, 0xe4, 0x07, 0x66, 0x0f, 0xfd, 0xdc, 0x66, 0x0f, 0x67, 0xd9,
    0xf2, 0x0f, 0xd6, 0xc3, 0x0f, 0x7f, 0x00, 0x83, 0xc0, 0x08, 0x4e, 0x75, 0xc3,
];
const SIMD_COPY_LOOP: &[u8] = &[
    0x0f, 0x18, 0x80, 0x00, 0x02, 0x00, 0x00, 0x0f, 0x28, 0x00, 0x0f, 0x2b, 0x04, 0x02, 0x83, 0xc0,
    0x10, 0x49, 0x75, 0xec,
];
const BYTE_COPY_LOOP: &[u8] = &[
    0x89, 0x85, 0xf8, 0xfc, 0xff, 0xff, 0x3b, 0xc1, 0x73, 0x09, 0x8a, 0x14, 0x30, 0x88, 0x14, 0x38,
    0x40, 0xeb, 0xed,
];
const FORWARD_COPY_LOOP: &[u8] = &[
    0x8b, 0x7d, 0x08, 0x8a, 0x06, 0x88, 0x07, 0x47, 0x46, 0x49, 0x89, 0x7d, 0x08, 0x75, 0xf1,
];
const SORT_LOOP: &[u8] = &[
    0x8b, 0xf3, 0x3b, 0xdf, 0x7d, 0x21, 0x8d, 0x9b, 0x00, 0x00, 0x00, 0x00, 0x8b, 0x84, 0xb5, 0xf8,
    0xf7, 0xff, 0xff, 0x8b, 0x0a, 0x3b, 0xc8, 0x76, 0x09, 0x89, 0x02, 0x89, 0x8c, 0xb5, 0xf8, 0xf7,
    0xff, 0xff, 0x46, 0x3b, 0xf7, 0x7c, 0xe5, 0x43, 0x83, 0xc2, 0x04, 0xff, 0x8d, 0xf0, 0xe7, 0xff,
    0xff, 0x75, 0xcd, 0x33, 0xc9,
];
const HUFFMAN_LOOP: &[u8] = &[
    0x33, 0xc9, 0x85, 0xd2, 0x75, 0x08, 0x8a, 0x1f, 0x47, 0xba, 0x08, 0x00, 0x00, 0x00, 0x0f, 0xb6,
    0xf3, 0xc1, 0xee, 0x07, 0x8d, 0x0c, 0x8e, 0x8b, 0x4c, 0x88, 0x08, 0x8b, 0xf1, 0x03, 0xf6, 0x02,
    0xdb, 0x4a, 0x83, 0x3c, 0xf0, 0x00, 0x75, 0xda,
];

static mut FAST_API_COUNT: u32 = 0;
static mut HOT_LOOP_ACTIVE: bool = false;
static mut WRITEBACK_BATCHES: u32 = 0;
static mut WRITEBACK_PAGES: u32 = 0;
static mut MIRRORED_DELTA_PAGES: u32 = 0;
static mut GENERATION: u64 = 0;
static mut DIRECT_MEMORY: bool = false;
const NATIVE_HEAP_PAGE_CAPACITY: usize = 128;
// ponytail: bounded receipts reuse installed tables; overflow/new tables fall back
// to the existing adapter, which can be widened if profiling shows a remaining cost.
static mut NATIVE_HEAP_PAGES: [[u32; 6]; NATIVE_HEAP_PAGE_CAPACITY] = [[0; 6]; NATIVE_HEAP_PAGE_CAPACITY];
static mut NATIVE_HEAP_PAGE_COUNT: usize = 0;

#[no_mangle]
pub unsafe fn vine_execution_island_heap_pages_ptr() -> u32 {
    std::ptr::addr_of!(NATIVE_HEAP_PAGES) as u32
}

#[no_mangle]
pub unsafe fn vine_execution_island_heap_pages_count() -> u32 { NATIVE_HEAP_PAGE_COUNT as u32 }

unsafe fn materialize_heap_fault() -> Result<bool, String> {
    if NATIVE_HEAP_PAGE_COUNT == NATIVE_HEAP_PAGE_CAPACITY { return Ok(false); }
    let address = *cr.add(2) as u32;
    let error = crate::cpu::cpu::vine_get_stop_error_code() as u32;
    if error & 1 != 0 { return Ok(false); }
    // Only reuse an already installed page table. Creating a table still uses
    // the existing TS adapter allocator; no second page-table allocator here.
    let directory = *cr.add(3) as u32 & !PAGE_MASK;
    if directory as usize + PAGE_BYTES > DIRECT_TABLE_BYTES { return Ok(false); }
    let pde = memory::read32s(directory + (address >> 22) * 4) as u32;
    let table = pde & !PAGE_MASK;
    if pde & 0x87 != 7 || table < PAGE_BYTES as u32 || table as usize + PAGE_BYTES > DIRECT_TABLE_BYTES
        || memory::in_mapped_range(table) { return Ok(false); }
    let pte = table + ((address >> 12) & 1023) * 4;
    if memory::read32s(pte) != 0 { return Ok(false); }
    if !reconcile_dirty_pages()? { return Ok(false); }
    let page = match vine_runtime_wasm::execution_island_materialize_heap_page(address,error)? {
        Some(page) => page,
        None => return Ok(false),
    };
    if (page.physical_offset as usize) < DIRECT_TABLE_BYTES || page.physical_offset & PAGE_MASK != 0
        || page.physical_offset as u64 + PAGE_BYTES as u64 > *crate::cpu::global_pointers::memory_size as u64 {
        return Err("native heap page escaped the authoritative arena".to_owned());
    }
    if vine::vine_set_page_executable(page.address, 0) != 0 {
        return Err("native heap page has an invalid execute-permission address".to_owned());
    }
    memory::write32_no_mmap_or_dirty_check(pte,(page.physical_offset | 7) as i32);
    invlpg(page.address as i32);
    NATIVE_HEAP_PAGES[NATIVE_HEAP_PAGE_COUNT] = [page.address,page.owner,page.allocation as u32,
        (page.allocation>>32)as u32,page.physical_offset,pte];
    NATIVE_HEAP_PAGE_COUNT += 1;
    GENERATION = page.generation;
    Ok(true)
}

#[no_mangle]
pub unsafe fn vine_execution_island_enable_direct_memory(pool_bytes: u32) -> u32 {
    if DIRECT_MEMORY || !memory::mem8.is_null() {
        vine_runtime_wasm::execution_island_set_error("execution memory is already installed".to_owned());
        return 0;
    }
    match vine_runtime_wasm::execution_island_enable_direct_memory(pool_bytes as usize, DIRECT_TABLE_BYTES) {
        Ok(base) => {
            memory::mem8 = base as *mut u8;
            DIRECT_MEMORY = true;
            base as u32
        }
        Err(error) => {
            vine_runtime_wasm::execution_island_set_error(error);
            0
        }
    }
}

#[no_mangle]
pub fn vine_execution_island_page_offset(address: u32, owner: u32, protection: u32) -> u32 {
    match vine_runtime_wasm::execution_island_page_offset(address, owner, protection) {
        Ok(offset) => offset as u32,
        Err(error) => {
            vine_runtime_wasm::execution_island_set_error(error);
            0
        }
    }
}

enum FastDispatch {
    NotHandled,
    Continued,
    Pending,
}

struct HostClock {
    base_elapsed_time_100ns: u64,
    base_monotonic_milliseconds: f64,
}

impl HostClock {
    unsafe fn new(base_elapsed_time_100ns: u64) -> Result<Self, String> {
        let base_monotonic_milliseconds = crate::cpu::cpu::js::microtick();
        if !base_monotonic_milliseconds.is_finite() || base_monotonic_milliseconds < 0.0 {
            return Err("execution island received an invalid host monotonic clock".to_owned());
        }
        Ok(Self {
            base_elapsed_time_100ns,
            base_monotonic_milliseconds,
        })
    }

    unsafe fn elapsed_time_100ns(&self, refresh: bool) -> Result<u64, String> {
        if !refresh {
            return Ok(self.base_elapsed_time_100ns);
        }
        let elapsed_milliseconds =
            crate::cpu::cpu::js::microtick() - self.base_monotonic_milliseconds;
        if !elapsed_milliseconds.is_finite() || elapsed_milliseconds < 0.0 {
            return Err("execution island host monotonic clock regressed".to_owned());
        }
        let elapsed_100ns = (elapsed_milliseconds * 10_000.0).floor();
        if elapsed_100ns > u64::MAX as f64 {
            return Err("execution island host monotonic clock overflowed".to_owned());
        }
        self.base_elapsed_time_100ns
            .checked_add(elapsed_100ns as u64)
            .ok_or_else(|| "execution island host elapsed clock overflowed".to_owned())
    }
}

#[inline(always)]
pub(crate) unsafe fn is_hot_loop_start(address: u32, physical: u32) -> bool {
    address & PAGE_MASK <= PAGE_MASK - 3
        && matches!(
            memory::read32_no_mmap_check(physical) as u32,
            SIMD_COPY_SIGNATURE
                | BYTE_COPY_SIGNATURE
                | FORWARD_COPY_SIGNATURE
                | SORT_SIGNATURE
                | HUFFMAN_SIGNATURE
                | BLEND_SIGNATURE
        )
}

unsafe fn code_matches(address: u32, physical: u32, expected: &[u8]) -> bool {
    if expected.is_empty() || address & PAGE_MASK > PAGE_MASK - expected.len() as u32 {
        return false;
    }
    std::slice::from_raw_parts(memory::mem8.add(physical as usize), expected.len()) == expected
}

pub(crate) unsafe fn owns_hot_loop(physical: u32) -> bool {
    HOT_LOOP_ACTIVE
        && (*state_flags).is_32()
        && (*state_flags).has_flat_segmentation()
        && is_hot_loop_start(physical, physical)
        && [
            BLEND_LOOP,
            SIMD_COPY_LOOP,
            BYTE_COPY_LOOP,
            FORWARD_COPY_LOOP,
            SORT_LOOP,
            HUFFMAN_LOOP,
        ]
        .iter()
        .any(|code| code_matches(physical, physical, code))
}

fn ranges_overlap(left: u32, left_length: u32, right: u32, right_length: u32) -> bool {
    let left_end = left as u64 + left_length as u64;
    let right_end = right as u64 + right_length as u64;
    (left as u64) < right_end && (right as u64) < left_end
}

unsafe fn preflight_guest_range(address: u32, length: u32, write: bool) -> bool {
    let end = address as u64 + length as u64;
    if end > 0x1_0000_0000 {
        return false;
    }
    // The interpreter has already translated the current fetch before entering a hot loop.
    let code_page = Page::page_of((*eip_phys ^ *instruction_pointer) as u32);
    let mut cursor = address as u64;
    while cursor < end {
        match translate_address(cursor as u32 as i32, write, *cpl == 3, false, false) {
            Ok(physical)
                if !memory::in_mapped_range(physical)
                    && (!write || Page::page_of(physical) != code_page) => {}
            _ => return false,
        }
        cursor = (cursor + PAGE_BYTES as u64 - (cursor & PAGE_MASK as u64)).min(end);
    }
    true
}

unsafe fn copy_guest_bytes(
    source: u32,
    destination: u32,
    length: u32,
    allow_forward_overlap: bool,
) -> Result<Option<u8>, String> {
    if length == 0 {
        return Ok(Some(0));
    }
    if !allow_forward_overlap && ranges_overlap(source, length, destination, length) {
        return Ok(None);
    }
    if !preflight_guest_range(source, length, false)
        || !preflight_guest_range(destination, length, true)
    {
        return Ok(None);
    }
    let mut source_cursor = source;
    let mut destination_cursor = destination;
    let mut remaining = length;
    let mut last = 0;
    while remaining > 0 {
        let source_physical = translate_address_read_no_side_effects(source_cursor as i32)
            .map_err(|()| "hot copy lost a preflighted source page".to_owned())?;
        let (destination_physical, can_skip_dirty) =
            translate_address_write_and_can_skip_dirty(destination_cursor as i32)
                .map_err(|()| "hot copy lost a preflighted destination page".to_owned())?;
        if memory::in_mapped_range(source_physical) || memory::in_mapped_range(destination_physical)
        {
            return Err("hot copy reached mapped device memory after preflight".to_owned());
        }
        if !can_skip_dirty {
            crate::jit::jit_dirty_page(Page::page_of(destination_physical));
        }
        let chunk = remaining
            .min(PAGE_BYTES as u32 - (source_cursor & PAGE_MASK))
            .min(PAGE_BYTES as u32 - (destination_cursor & PAGE_MASK));
        let source_pointer = memory::mem8.add(source_physical as usize);
        let destination_pointer = memory::mem8.add(destination_physical as usize);
        if allow_forward_overlap {
            for offset in 0..chunk as usize {
                last = *source_pointer.add(offset);
                *destination_pointer.add(offset) = last;
            }
        } else {
            last = *source_pointer.add(chunk as usize - 1);
            std::ptr::copy_nonoverlapping(source_pointer, destination_pointer, chunk as usize);
        }
        source_cursor = source_cursor.wrapping_add(chunk);
        destination_cursor = destination_cursor.wrapping_add(chunk);
        remaining -= chunk;
    }
    Ok(Some(last))
}

unsafe fn set_hot_instruction_pointer(address: u32, previous_address: u32) {
    *instruction_pointer = address as i32;
    *previous_ip = previous_address as i32;
    *last_virt_eip = -1;
    *eip_phys = 0;
}

#[target_feature(enable = "simd128")]
unsafe fn blend_guest_bytes(source: u32, destination: u32, length: u32) -> Result<(), String> {
    use std::arch::wasm32::*;

    let multiplier = v128_load(reg_xmm.add(2).cast());
    let zero = u64x2(0, 0);
    let mut final_source = zero;
    let mut final_delta = zero;
    let mut final_packed = zero;
    let mut offset = 0;
    while offset < length {
        let source_address = source + offset;
        let destination_address = destination + offset;
        let source_physical = translate_address_read_no_side_effects(source_address as i32)
            .map_err(|()| "hot blend lost its preflighted source page".to_owned())?;
        let (destination_physical, can_skip_dirty) =
            translate_address_write_and_can_skip_dirty(destination_address as i32)
                .map_err(|()| "hot blend lost its preflighted destination page".to_owned())?;
        if !can_skip_dirty {
            crate::jit::jit_dirty_page(Page::page_of(destination_physical));
        }
        let chunk = (length - offset)
            .min(PAGE_BYTES as u32 - (source_address & PAGE_MASK))
            .min(PAGE_BYTES as u32 - (destination_address & PAGE_MASK));
        let source_pointer = memory::mem8.add(source_physical as usize);
        let destination_pointer = memory::mem8.add(destination_physical as usize);
        for index in (0..chunk as usize).step_by(8) {
            let source = source_pointer.add(index).cast::<u64>().read_unaligned();
            let destination = destination_pointer
                .add(index)
                .cast::<u64>()
                .read_unaligned();
            final_source = u16x8_extend_low_u8x16(u64x2(source, 0));
            let destination = u16x8_extend_low_u8x16(u64x2(destination, 0));
            final_delta = i16x8_shr(
                i16x8_mul(i16x8_sub(destination, final_source), multiplier),
                7,
            );
            final_packed = u8x16_narrow_i16x8(i16x8_add(final_source, final_delta), zero);
            destination_pointer
                .add(index)
                .cast::<u64>()
                .write_unaligned(u64x2_extract_lane::<0>(final_packed));
        }
        offset += chunk;
    }
    v128_store(reg_xmm.cast(), final_source);
    v128_store(reg_xmm.add(3).cast(), final_packed);
    v128_store(reg_xmm.add(4).cast(), final_delta);
    write_mmx_reg64(0, u64x2_extract_lane::<0>(final_packed));
    transition_fpu_to_mmx();
    Ok(())
}

pub(crate) unsafe fn try_hot_loop(max_instructions: u32, physical: u32) -> Result<bool, String> {
    if !HOT_LOOP_ACTIVE || !(*state_flags).is_32() || !(*state_flags).has_flat_segmentation() {
        return Ok(false);
    }
    let address = *instruction_pointer as u32;
    if code_matches(address, physical, BLEND_LOOP) {
        let count = *reg32.add(ESI as usize) as u32;
        let iterations = count.min(max_instructions / 17);
        let destination = *reg32.add(EAX as usize) as u32;
        let source = destination.wrapping_add(*reg32.add(ECX as usize) as u32);
        let Some(length) = iterations.checked_mul(8) else {
            return Ok(false);
        };
        if iterations == 0
            || *cr & 0x0c != 0
            || *cr.add(4) & 0x200 == 0
            || source & 7 != 0
            || destination & 7 != 0
            || (*reg_xmm.add(1)).u64 != [0, 0]
            || !preflight_guest_range(source, length, false)
            || !preflight_guest_range(destination, length, true)
        {
            return Ok(false);
        }
        blend_guest_bytes(source, destination, length)?;
        let remaining = count - iterations;
        *reg32.add(EAX as usize) =
            crate::cpu::arith::add32(destination.wrapping_add(length).wrapping_sub(8) as i32, 8);
        *reg32.add(ESI as usize) = remaining as i32;
        crate::cpu::arith::dec32(remaining.wrapping_add(1) as i32);
        set_hot_instruction_pointer(
            if remaining == 0 {
                address + BLEND_LOOP.len() as u32
            } else {
                address
            },
            address + BLEND_LOOP.len() as u32 - 2,
        );
        *instruction_counter = instruction_counter.read().wrapping_add(iterations * 17);
        return Ok(true);
    }
    if code_matches(address, physical, SIMD_COPY_LOOP) {
        let count = *reg32.add(ECX as usize) as u32;
        let iterations = count.min(max_instructions / 6);
        if iterations == 0 {
            return Ok(false);
        }
        let source = *reg32.add(EAX as usize) as u32;
        let destination = source.wrapping_add(*reg32.add(EDX as usize) as u32);
        let length = iterations
            .checked_mul(16)
            .ok_or_else(|| "hot SIMD copy length overflowed".to_owned())?;
        let source_end = source as u64 + length as u64;
        let destination_end = destination as u64 + length as u64;
        if *cr & 0x0c != 0
            || *cr.add(4) & 0x200 == 0
            || source & 15 != 0
            || destination & 15 != 0
            || source_end > 0x1_0000_0000
            || destination_end > 0x1_0000_0000
            || ranges_overlap(source, length, destination, length)
            || copy_guest_bytes(source, destination, length, false)?.is_none()
        {
            return Ok(false);
        }
        *reg_xmm = safe_read128s(source.wrapping_add(length).wrapping_sub(16) as i32)
            .map_err(|()| "hot SIMD copy lost its preflighted final source block".to_owned())?;
        let remaining = count - iterations;
        *reg32.add(EAX as usize) =
            crate::cpu::arith::add32(source.wrapping_add(length).wrapping_sub(16) as i32, 16);
        *reg32.add(ECX as usize) = remaining as i32;
        crate::cpu::arith::dec32(remaining.wrapping_add(1) as i32);
        set_hot_instruction_pointer(
            if remaining == 0 {
                address + SIMD_COPY_LOOP.len() as u32
            } else {
                address
            },
            address + SIMD_COPY_LOOP.len() as u32 - 2,
        );
        *instruction_counter = instruction_counter.read().wrapping_add(iterations * 6);
        return Ok(true);
    }

    if code_matches(address, physical, BYTE_COPY_LOOP) {
        let offset = *reg32.add(EAX as usize) as u32;
        let limit = *reg32.add(ECX as usize) as u32;
        let iterations = limit.saturating_sub(offset);
        let required = iterations
            .checked_mul(7)
            .and_then(|instructions| instructions.checked_add(3))
            .ok_or_else(|| "hot byte copy instruction count overflowed".to_owned())?;
        if (offset < limit && required > max_instructions) || max_instructions < 3 {
            return Ok(false);
        }
        let local = (*reg32.add(EBP as usize) as u32).wrapping_sub(0x308);
        if !preflight_guest_range(local, 4, true) {
            return Ok(false);
        }
        let last = if iterations == 0 {
            None
        } else {
            let source = (*reg32.add(ESI as usize) as u32).wrapping_add(offset);
            let destination = (*reg32.add(EDI as usize) as u32).wrapping_add(offset);
            if ranges_overlap(local, 4, source, iterations)
                || ranges_overlap(local, 4, destination, iterations)
            {
                return Ok(false);
            }
            match copy_guest_bytes(source, destination, iterations, false)? {
                Some(last) => Some(last),
                None => return Ok(false),
            }
        };
        let final_offset = offset.wrapping_add(iterations);
        safe_write32(local as i32, final_offset as i32)
            .map_err(|()| "hot byte copy lost its preflighted local".to_owned())?;
        if let Some(last) = last {
            let edx = *reg32.add(EDX as usize) as u32;
            *reg32.add(EDX as usize) = (edx & !0xff | last as u32) as i32;
            *flags |= 1;
        }
        *reg32.add(EAX as usize) = final_offset as i32;
        crate::cpu::arith::cmp32(final_offset as i32, limit as i32);
        set_hot_instruction_pointer(address + BYTE_COPY_LOOP.len() as u32, address + 8);
        *instruction_counter = instruction_counter.read().wrapping_add(required);
        return Ok(true);
    }

    if code_matches(address, physical, FORWARD_COPY_LOOP) {
        let count = *reg32.add(ECX as usize) as u32;
        let iterations = count.min(max_instructions / 8);
        if iterations == 0 {
            return Ok(false);
        }
        let local = (*reg32.add(EBP as usize) as u32).wrapping_add(8);
        if !preflight_guest_range(local, 4, true) {
            return Ok(false);
        }
        let destination = safe_read32s(local as i32)
            .map_err(|()| "hot forward copy could not read its preflighted local".to_owned())?
            as u32;
        let source = *reg32.add(ESI as usize) as u32;
        if ranges_overlap(local, 4, source, iterations)
            || ranges_overlap(local, 4, destination, iterations)
        {
            return Ok(false);
        }
        let last = match copy_guest_bytes(source, destination, iterations, true)? {
            Some(last) => last,
            None => return Ok(false),
        };
        let final_source = source.wrapping_add(iterations);
        let final_destination = destination.wrapping_add(iterations);
        safe_write32(local as i32, final_destination as i32)
            .map_err(|()| "hot forward copy lost its preflighted local".to_owned())?;
        let eax = *reg32.add(EAX as usize) as u32;
        *reg32.add(EAX as usize) = (eax & !0xff | last as u32) as i32;
        *reg32.add(EDI as usize) = final_destination as i32;
        *reg32.add(ESI as usize) = final_source as i32;
        let remaining = count - iterations;
        *reg32.add(ECX as usize) = remaining as i32;
        crate::cpu::arith::dec32(remaining.wrapping_add(1) as i32);
        set_hot_instruction_pointer(
            if remaining == 0 {
                address + FORWARD_COPY_LOOP.len() as u32
            } else {
                address
            },
            address + FORWARD_COPY_LOOP.len() as u32 - 2,
        );
        *instruction_counter = instruction_counter.read().wrapping_add(iterations * 8);
        return Ok(true);
    }

    if code_matches(address, physical, SORT_LOOP) {
        let total = *reg32.add(EDI as usize);
        let outer = *reg32.add(EBX as usize);
        if total <= 1 || total > 512 || outer < 1 || outer >= total {
            return Ok(false);
        }
        let base = (*reg32.add(EBP as usize) as u32).wrapping_sub(0x808);
        let remaining_local = (*reg32.add(EBP as usize) as u32).wrapping_sub(0x1810);
        let remaining = total - outer;
        let values_length = total as u32 * 4;
        if *reg32.add(EDX as usize) as u32 != base.wrapping_add((outer as u32 - 1).wrapping_mul(4))
            || ranges_overlap(base, values_length, remaining_local, 4)
            || !preflight_guest_range(base, values_length, false)
            || !preflight_guest_range(remaining_local, 4, true)
            || safe_read32s(remaining_local as i32)
                .map_err(|()| "hot sort lost its preflighted loop counter".to_owned())?
                != remaining
        {
            return Ok(false);
        }
        let mut values = [0_u32; 512];
        for index in 0..total as u32 {
            values[index as usize] = safe_read32s(base.wrapping_add(index * 4) as i32)
                .map_err(|()| "hot sort lost its preflighted input".to_owned())?
                as u32;
        }
        let mut touched = [false; 512];
        let mut swaps = 0_u32;
        let mut final_eax = 0_u32;
        // The guest instruction count depends on every conditional swap, so mirror this bounded loop.
        for current in outer as usize..total as usize {
            for index in current..total as usize {
                final_eax = values[index];
                if values[current - 1] > final_eax {
                    values.swap(current - 1, index);
                    touched[current - 1] = true;
                    touched[index] = true;
                    swaps += 1;
                }
            }
        }
        let inner_iterations = remaining as u32 * (remaining as u32 + 1) / 2;
        let required = 1 + remaining as u32 * 8 + inner_iterations * 7 + swaps * 2;
        if required > max_instructions {
            return Ok(false);
        }
        if touched[..total as usize].iter().any(|&value| value)
            && !preflight_guest_range(base, values_length, true)
        {
            return Ok(false);
        }
        for (index, (&value, &write)) in values[..total as usize].iter().zip(&touched).enumerate() {
            if write {
                safe_write32(base.wrapping_add(index as u32 * 4) as i32, value as i32)
                    .map_err(|()| "hot sort lost its preflighted output".to_owned())?;
            }
        }
        safe_write32(remaining_local as i32, 0)
            .map_err(|()| "hot sort lost its preflighted loop counter".to_owned())?;
        *reg32.add(EAX as usize) = final_eax as i32;
        *reg32.add(ECX as usize) = crate::cpu::arith::xor32(0, 0);
        *reg32.add(EDX as usize) = base.wrapping_add((total as u32 - 1) * 4) as i32;
        *reg32.add(EBX as usize) = total;
        *reg32.add(ESI as usize) = total;
        *last_op1 = 1;
        set_hot_instruction_pointer(
            address + SORT_LOOP.len() as u32,
            address + SORT_LOOP.len() as u32 - 2,
        );
        *instruction_counter = instruction_counter.read().wrapping_add(required);
        return Ok(true);
    }

    if code_matches(address, physical, HUFFMAN_LOOP) {
        let tree = *reg32.add(EAX as usize) as u32;
        let mut input = *reg32.add(EDI as usize) as u32;
        let mut bit_count = *reg32.add(EDX as usize) as u32;
        let mut bit_buffer = *reg32.add(EBX as usize) as u32;
        if bit_count > 8 {
            return Ok(false);
        }
        let mut node = 0_u32;
        let mut doubled_node = 0_u32;
        let mut final_flag = 1_i32;
        let mut final_bit = 0_u32;
        let mut required = 1_u32;
        for _ in 0..512 {
            required += 12;
            if bit_count == 0 {
                let byte = safe_read8(input as i32)
                    .map_err(|()| "hot Huffman decode could not read its input".to_owned())?
                    as u32
                    & 0xff;
                bit_buffer = bit_buffer & !0xff | byte;
                input = input.wrapping_add(1);
                bit_count = 8;
                required += 3;
            }
            final_bit = bit_buffer >> 7 & 1;
            let child = tree
                .wrapping_add(node.wrapping_mul(16))
                .wrapping_add(8 + final_bit * 4);
            node = safe_read32s(child as i32)
                .map_err(|()| "hot Huffman decode could not read its child".to_owned())?
                as u32;
            doubled_node = node.wrapping_mul(2);
            bit_buffer = bit_buffer & !0xff | ((bit_buffer & 0xff) << 1 & 0xff);
            bit_count -= 1;
            final_flag = safe_read32s(tree.wrapping_add(doubled_node * 8) as i32)
                .map_err(|()| "hot Huffman decode could not read its node".to_owned())?;
            if final_flag == 0 {
                break;
            }
        }
        if final_flag != 0 || required > max_instructions {
            return Ok(false);
        }
        crate::cpu::arith::xor32(0, 0);
        *flags = *flags & !1 | final_bit as i32;
        crate::cpu::arith::cmp32(final_flag, 0);
        *reg32.add(ECX as usize) = node as i32;
        *reg32.add(ESI as usize) = doubled_node as i32;
        *reg32.add(EDI as usize) = input as i32;
        *reg32.add(EDX as usize) = bit_count as i32;
        *reg32.add(EBX as usize) = bit_buffer as i32;
        set_hot_instruction_pointer(
            address + HUFFMAN_LOOP.len() as u32,
            address + HUFFMAN_LOOP.len() as u32 - 2,
        );
        *instruction_counter = instruction_counter.read().wrapping_add(required);
        return Ok(true);
    }

    Ok(false)
}

unsafe fn stack_is_dirty_and_iat_is_clean(stack_pointer: u32, iat_page: u32) -> bool {
    if vine::vine_dirty_pages_overflowed() != 0 || stack_pointer & PAGE_MASK > PAGE_MASK - 3 {
        return false;
    }
    let pages = std::slice::from_raw_parts(
        vine::vine_dirty_pages_ptr() as *const u32,
        vine::vine_dirty_page_count() as usize,
    );
    let stack_page = stack_pointer & !PAGE_MASK;
    pages.contains(&stack_page) && !pages.contains(&iat_page)
}

unsafe fn clear_page_dirty(address: u32) -> bool {
    let directory = *crate::cpu::global_pointers::cr.add(3) as u32 & !PAGE_MASK;
    let directory_entry = memory::read32s(directory + (address >> 22) * 4) as u32;
    if directory_entry & 1 == 0 || directory_entry & 0x80 != 0 {
        return false;
    }
    let table = directory_entry & !PAGE_MASK;
    let entry_address = table + (address >> 12 & 0x3ff) * 4;
    let entry = memory::read32s(entry_address) as u32;
    if entry & 1 == 0 {
        return false;
    }
    memory::write32_no_mmap_or_dirty_check(entry_address, (entry & !PTE_DIRTY) as i32);
    invlpg(address as i32);
    true
}

unsafe fn reconcile_dirty_pages() -> Result<bool, String> {
    let overflow = vine::vine_dirty_pages_overflowed() != 0;
    if overflow && !DIRECT_MEMORY {
        return Ok(false);
    }
    let count = vine::vine_dirty_page_count() as usize;
    if count == 0 && !overflow {
        return Ok(true);
    }
    let mut scanned = Vec::new();
    let addresses = if overflow {
        let directory = *cr.add(3) as u32 & !PAGE_MASK;
        if directory as usize + PAGE_BYTES > DIRECT_TABLE_BYTES {
            return Err("direct execution page directory escaped its reserved prefix".to_owned());
        }
        for directory_index in 0..1024 {
            let entry = memory::read32s(directory + directory_index * 4) as u32;
            if entry & 1 == 0 { continue; }
            let table = entry & !PAGE_MASK;
            if entry & 0x80 != 0 || table as usize + PAGE_BYTES > DIRECT_TABLE_BYTES {
                return Err("direct execution page table escaped its reserved prefix".to_owned());
            }
            for index in 0..1024 {
                let pte = memory::read32s(table + index * 4) as u32;
                if pte & (1 | PTE_DIRTY) == (1 | PTE_DIRTY) {
                    scanned.push(directory_index << 22 | index << 12);
                }
            }
        }
        scanned.as_slice()
    } else {
        std::slice::from_raw_parts(vine::vine_dirty_pages_ptr() as *const u32, count)
    };
    if DIRECT_MEMORY {
        let mut pages = Vec::with_capacity(addresses.len());
        for &address in addresses {
            let physical = translate_address_read_no_side_effects(address as i32)
                .map_err(|()| "direct execution lost a dirty page mapping".to_owned())? & !PAGE_MASK;
            if memory::in_mapped_range(physical) {
                return Err("direct dirty page reached device memory".to_owned());
            }
            pages.push((address, memory::mem8.add(physical as usize) as usize));
        }
        GENERATION = vine_runtime_wasm::execution_island_acknowledge_direct_writes(GENERATION, &pages)?;
        for &address in addresses {
            if !clear_page_dirty(address) {
                return Err("direct execution lost an acknowledged dirty page".to_owned());
            }
        }
        vine::vine_reset_dirty_pages();
        WRITEBACK_BATCHES += 1;
        WRITEBACK_PAGES += pages.len() as u32;
        return Ok(true);
    }
    let mut pages = Vec::with_capacity(count);
    for &address in addresses {
        let physical = match translate_address_read_no_side_effects(address as i32) {
            Ok(physical) => physical & !PAGE_MASK,
            Err(()) => return Ok(false),
        };
        let bytes = std::slice::from_raw_parts(memory::mem8.add(physical as usize), PAGE_BYTES);
        pages.push((address, bytes));
    }
    let receipt = vine_runtime_wasm::execution_island_reconcile_cpu_pages(&pages)?;
    for &(address, _) in &pages {
        if !clear_page_dirty(address) {
            return Err("execution island lost a dirty guest page mapping".to_owned());
        }
    }
    vine::vine_reset_dirty_pages();
    WRITEBACK_BATCHES += 1;
    WRITEBACK_PAGES = WRITEBACK_PAGES
        .checked_add(receipt.touched_pages as u32)
        .ok_or_else(|| "execution island writeback page count overflowed".to_owned())?;
    GENERATION = receipt.generation;
    Ok(true)
}

unsafe fn synchronize_clock(retired: u64, host_elapsed_time_100ns: u64) -> Result<(), String> {
    let status = vine_runtime_wasm::vine_bridge_synchronize_forward_clock_v1(
        retired as u32,
        (retired >> 32) as u32,
        host_elapsed_time_100ns as u32,
        (host_elapsed_time_100ns >> 32) as u32,
    );
    if status == 0 {
        Ok(())
    } else {
        Err("execution island clock synchronization failed".to_owned())
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| "execution island received a truncated memory delta".to_owned())?;
    Ok(u32::from_le_bytes(
        value.try_into().expect("four checked bytes"),
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let value = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| "execution island received a truncated memory delta".to_owned())?;
    Ok(u64::from_le_bytes(
        value.try_into().expect("eight checked bytes"),
    ))
}

unsafe fn apply_inline_delta(words: &[u32; FAST_RECORD_WORDS as usize]) -> Result<bool, String> {
    let control_length = vine_runtime_wasm::vine_bridge_control_len() as usize;
    let payload_length = vine_runtime_wasm::vine_bridge_payload_len() as usize;
    if control_length == 0 || payload_length == 0 {
        return Ok(control_length == 0 && payload_length == 0 && words[26] == 0);
    }
    if words[26] > 1 || control_length < MEMORY_DELTA_CONTROL_HEADER_BYTES {
        return Ok(false);
    }
    let control = std::slice::from_raw_parts(
        vine_runtime_wasm::vine_bridge_control_ptr() as *const u8,
        control_length,
    );
    if control.get(..8) != Some(b"VINEDL01")
        || read_u32(control, 8)? != 2
        || read_u32(control, 12)? != PAGE_BYTES as u32
        || read_u32(control, 32)? != 0
        || read_u32(control, 36)? != MEMORY_DELTA_MAPPING_DESCRIPTOR_BYTES as u32
        || read_u32(control, 44)? != MEMORY_DELTA_PAGE_DESCRIPTOR_BYTES as u32
        || read_u32(control, 48)? != 0
        || read_u32(control, 52)? != 0
    {
        return Ok(false);
    }
    let page_count = read_u32(control, 40)? as usize;
    let expected_control_length = MEMORY_DELTA_CONTROL_HEADER_BYTES
        .checked_add(
            page_count
                .checked_mul(MEMORY_DELTA_PAGE_DESCRIPTOR_BYTES)
                .ok_or_else(|| "execution island delta page count overflowed".to_owned())?,
        )
        .ok_or_else(|| "execution island delta control length overflowed".to_owned())?;
    let expected_payload_length = page_count
        .checked_mul(PAGE_BYTES)
        .ok_or_else(|| "execution island delta payload length overflowed".to_owned())?;
    if page_count == 0
        || control_length != expected_control_length
        || payload_length != expected_payload_length
    {
        return Ok(false);
    }
    let from_generation = read_u64(control, 16)?;
    let generation = read_u64(control, 24)?;
    if from_generation != GENERATION || generation <= from_generation {
        return Ok(false);
    }
    let payload = std::slice::from_raw_parts(
        vine_runtime_wasm::vine_bridge_payload_ptr() as *const u8,
        payload_length,
    );
    let inline_write = match words[26] {
        0 if words[27] == 0 && words[28] == 0 => None,
        1 => {
            let length = words[28] as usize;
            let end = words[27]
                .checked_add(words[28])
                .ok_or_else(|| "execution island inline write wrapped x86 memory".to_owned())?;
            if length == 0 || length > 28 || !length.is_multiple_of(4) {
                return Ok(false);
            }
            Some((words[27], end, length))
        }
        _ => return Ok(false),
    };
    let mut inline_write_matched = inline_write.is_none();
    for page_index in 0..page_count {
        let descriptor =
            MEMORY_DELTA_CONTROL_HEADER_BYTES + page_index * MEMORY_DELTA_PAGE_DESCRIPTOR_BYTES;
        let page_address = read_u32(control, descriptor)?;
        let page_flags = read_u32(control, descriptor + 8)?;
        let page_end = page_address
            .checked_add(PAGE_BYTES as u32)
            .ok_or_else(|| "execution island delta page wrapped x86 memory".to_owned())?;
        if page_address & PAGE_MASK != 0
            || page_flags & 2 == 0
            || page_flags & 4 != 0
            || read_u32(control, descriptor + 12)? != 0
            || vine::execute_fetch_allowed(page_address)
        {
            return Ok(false);
        }
        // ponytail: dirty deltas are small; use a set if large batches reach this hot path.
        for previous_index in 0..page_index {
            let previous_descriptor = MEMORY_DELTA_CONTROL_HEADER_BYTES
                + previous_index * MEMORY_DELTA_PAGE_DESCRIPTOR_BYTES;
            if read_u32(control, previous_descriptor)? == page_address {
                return Ok(false);
            }
        }
        let physical = translate_address(page_address as i32, true, *cpl == 3, false, false)
            .map_err(|()| "execution island delta targets an unmapped mirror page".to_owned())?;
        if memory::in_mapped_range(physical) {
            return Ok(false);
        }
        if let Some((write_address, write_end, write_length)) = inline_write {
            if write_address >= page_address && write_end <= page_end {
                if inline_write_matched {
                    return Ok(false);
                }
                let write_offset =
                    page_index * PAGE_BYTES + (write_address - page_address) as usize;
                for (index, chunk) in payload[write_offset..write_offset + write_length]
                    .chunks_exact(4)
                    .enumerate()
                {
                    if u32::from_le_bytes(chunk.try_into().expect("four inline bytes"))
                        != words[29 + index]
                    {
                        return Err(
                            "execution island inline write disagrees with its memory delta"
                                .to_owned(),
                        );
                    }
                }
                inline_write_matched = true;
            }
        }
    }
    if !inline_write_matched {
        return Ok(false);
    }
    for page_index in 0..page_count {
        let descriptor =
            MEMORY_DELTA_CONTROL_HEADER_BYTES + page_index * MEMORY_DELTA_PAGE_DESCRIPTOR_BYTES;
        let page_address = read_u32(control, descriptor)?;
        let physical = translate_address(page_address as i32, true, *cpl == 3, false, false)
            .map_err(|()| "execution island delta lost a preflighted mirror page".to_owned())?;
        if !DIRECT_MEMORY {
            std::ptr::copy(
                payload.as_ptr().add(page_index * PAGE_BYTES),
                memory::mem8.add(physical as usize),
                PAGE_BYTES,
            );
        }
    }
    if vine_runtime_wasm::vine_bridge_acknowledge_fast_v1(
        generation as u32,
        (generation >> 32) as u32,
    ) != 0
    {
        return Err("execution island could not acknowledge its inline memory delta".to_owned());
    }
    GENERATION = generation;
    if !DIRECT_MEMORY {
        MIRRORED_DELTA_PAGES = MIRRORED_DELTA_PAGES
            .checked_add(page_count as u32)
            .ok_or_else(|| "execution island mirrored-page count overflowed".to_owned())?;
    }
    Ok(true)
}

unsafe fn apply_register(action: u32, value: u32, register: *mut i32) -> bool {
    match action {
        0 => true,
        1 => {
            *register = value as i32;
            true
        }
        _ => false,
    }
}

unsafe fn dispatch_fast(
    retired: u64,
    host_clock: &HostClock,
    prevalidated_endpoint: &mut Option<(u32, u32)>,
) -> Result<FastDispatch, String> {
    let endpoint = *instruction_pointer as u32;
    if !vine_runtime_wasm::execution_island_forward_endpoint_owned(endpoint) {
        return Ok(FastDispatch::NotHandled);
    }
    let stack_pointer = *reg32.add(ESP as usize) as u32;
    let frame_prevalidated = prevalidated_endpoint.is_some_and(|(address, iat_page)| {
        address == endpoint && stack_is_dirty_and_iat_is_clean(stack_pointer, iat_page)
    });
    if !frame_prevalidated && !reconcile_dirty_pages()? {
        return Ok(FastDispatch::NotHandled);
    }
    synchronize_clock(retired, host_clock.elapsed_time_100ns(frame_prevalidated)?)?;
    let return_address = safe_read32s(stack_pointer as i32)
        .map_err(|()| "execution island could not read the API return address".to_owned())?
        as u32;
    let status = vine_runtime_wasm::vine_bridge_dispatch_fast_v4(
        endpoint,
        stack_pointer,
        return_address,
        retired as u32,
        (retired >> 32) as u32,
        *reg32.add(EAX as usize) as u32,
        *reg32.add(ECX as usize) as u32,
        *reg32.add(EDX as usize) as u32,
        *reg32.add(EBX as usize) as u32,
        stack_pointer,
        *reg32.add(EBP as usize) as u32,
        *reg32.add(ESI as usize) as u32,
        *reg32.add(EDI as usize) as u32,
        endpoint,
        *flags as u32,
        *sreg as u32,
        *sreg.add(1) as u32,
        *sreg.add(2) as u32,
        *sreg.add(3) as u32,
        *sreg.add(4) as u32,
        *sreg.add(5) as u32,
        frame_prevalidated as u32,
    );
    if status != 0 {
        return Err("execution island API dispatch failed".to_owned());
    }
    if vine_runtime_wasm::vine_bridge_fast_dispatch_len_v3() != FAST_RECORD_WORDS {
        return Ok(FastDispatch::Pending);
    }
    let live_words = std::slice::from_raw_parts(
        vine_runtime_wasm::vine_bridge_fast_dispatch_ptr_v3() as *const u32,
        FAST_RECORD_WORDS as usize,
    );
    let mut words = [0; FAST_RECORD_WORDS as usize];
    words.copy_from_slice(live_words);
    if words[0] != FAST_RECORD_VERSION || words[3] != endpoint || words[5] != stack_pointer {
        return Err("execution island received an invalid fast dispatch record".to_owned());
    }
    if words[14] != 0 || words[15] != 0 || !apply_inline_delta(&words)? {
        return Ok(FastDispatch::Pending);
    }
    if !apply_register(words[8], words[9], reg32.add(EAX as usize))
        || !apply_register(words[10], words[11], reg32.add(EDX as usize))
        || !apply_register(words[12], words[13], flags)
    {
        return Err("execution island received an invalid register update".to_owned());
    }
    *reg32.add(ESP as usize) = words[6] as i32;
    *instruction_pointer = words[4] as i32;
    *previous_ip = words[4] as i32;
    *in_hlt = false;
    *last_virt_eip = -1;
    *eip_phys = 0;
    if words[1] == GET_TICK_COUNT_FAST_CODE && words[19] == 0 {
        if !frame_prevalidated {
            *prevalidated_endpoint =
                vine_runtime_wasm::execution_island_forward_endpoint_iat_page(endpoint)
                    .map(|iat_page| (endpoint, iat_page));
        }
    } else {
        *prevalidated_endpoint = None;
    }
    FAST_API_COUNT += 1;
    Ok(FastDispatch::Continued)
}

#[no_mangle]
pub fn vine_execution_island_abi_version() -> u32 {
    1
}

unsafe fn execute_budget(
    max_instructions: u32,
    retired_instructions_low: u32,
    retired_instructions_high: u32,
    host_elapsed_time_100ns_low: u32,
    host_elapsed_time_100ns_high: u32,
) -> u32 {
    FAST_API_COUNT = 0;
    NATIVE_HEAP_PAGE_COUNT = 0;
    HOT_LOOP_ACTIVE = false;
    WRITEBACK_BATCHES = 0;
    WRITEBACK_PAGES = 0;
    MIRRORED_DELTA_PAGES = 0;
    GENERATION = match vine_runtime_wasm::execution_island_memory_generation() {
        Ok(generation) => generation,
        Err(error) => {
            vine_runtime_wasm::execution_island_set_error(error);
            return STOP_ISLAND_ERROR;
        }
    };
    let retired_base = retired_instructions_low as u64 | (retired_instructions_high as u64) << 32;
    let host_elapsed_time_100ns =
        host_elapsed_time_100ns_low as u64 | (host_elapsed_time_100ns_high as u64) << 32;
    let host_clock = match HostClock::new(host_elapsed_time_100ns) {
        Ok(clock) => clock,
        Err(error) => {
            vine_runtime_wasm::execution_island_set_error(error);
            return STOP_ISLAND_ERROR;
        }
    };
    let initial_counter = *instruction_counter;
    let mut prevalidated_endpoint = None;
    loop {
        let executed = (*instruction_counter).wrapping_sub(initial_counter);
        if executed >= max_instructions || FAST_API_COUNT >= MAX_FAST_APIS_PER_SLICE {
            return STOP_BUDGET;
        }
        HOT_LOOP_ACTIVE = true;
        let reason = if DIRECT_MEMORY {
            match vine_runtime_wasm::execution_island_run_direct_cpu(|base, length| {
                if base != memory::mem8 || length != *crate::cpu::global_pointers::memory_size as usize {
                    return STOP_ISLAND_ERROR;
                }
                // Reborrow from GuestMemory for this run; no runtime/byte borrow crosses dispatch.
                memory::mem8 = base;
                crate::cpu::cpu::vine_execute_budget(max_instructions - executed)
            }) {
                Ok(reason) => reason,
                Err(error) => {
                    vine_runtime_wasm::execution_island_set_error(error);
                    HOT_LOOP_ACTIVE = false;
                    return STOP_ISLAND_ERROR;
                }
            }
        } else {
            crate::cpu::cpu::vine_execute_budget(max_instructions - executed)
        };
        HOT_LOOP_ACTIVE = false;
        if DIRECT_MEMORY && reason == 0x10e {
            match materialize_heap_fault() {
                Ok(true) => continue,
                Ok(false) => {},
                Err(error) => {
                    vine_runtime_wasm::execution_island_set_error(error);
                    return STOP_ISLAND_ERROR;
                }
            }
        }
        if reason != STOP_INVALID_OPCODE {
            return reason;
        }
        // Publish new mappings before any API can protect/free/reuse their frames.
        if NATIVE_HEAP_PAGE_COUNT != 0 { return reason; }
        let retired = retired_base + (*instruction_counter).wrapping_sub(initial_counter) as u64;
        match dispatch_fast(retired, &host_clock, &mut prevalidated_endpoint) {
            Ok(FastDispatch::Continued) => {}
            Ok(FastDispatch::Pending) => return STOP_PENDING_DISPATCH,
            Ok(FastDispatch::NotHandled) => return reason,
            Err(error) => {
                vine_runtime_wasm::execution_island_set_error(error);
                return STOP_ISLAND_ERROR;
            }
        }
    }
}

#[no_mangle]
pub unsafe fn vine_execution_island_execute_budget(
    max_instructions: u32,
    retired_instructions_low: u32,
    retired_instructions_high: u32,
    host_elapsed_time_100ns_low: u32,
    host_elapsed_time_100ns_high: u32,
) -> u32 {
    let reason = execute_budget(
        max_instructions, retired_instructions_low, retired_instructions_high,
        host_elapsed_time_100ns_low, host_elapsed_time_100ns_high,
    );
    if DIRECT_MEMORY && reason != STOP_ISLAND_ERROR {
        if let Err(error) = reconcile_dirty_pages() {
            vine_runtime_wasm::execution_island_set_error(error);
            return STOP_ISLAND_ERROR;
        }
    }
    reason
}

#[no_mangle]
pub unsafe fn vine_execution_island_fast_api_count() -> u32 {
    FAST_API_COUNT
}

#[no_mangle]
pub unsafe fn vine_execution_island_writeback_batches() -> u32 {
    WRITEBACK_BATCHES
}

#[no_mangle]
pub unsafe fn vine_execution_island_writeback_pages() -> u32 {
    WRITEBACK_PAGES
}

#[no_mangle]
pub unsafe fn vine_execution_island_mirrored_delta_pages() -> u32 {
    MIRRORED_DELTA_PAGES
}

#[no_mangle]
pub unsafe fn vine_execution_island_generation_low() -> u32 {
    GENERATION as u32
}

#[no_mangle]
pub unsafe fn vine_execution_island_generation_high() -> u32 {
    (GENERATION >> 32) as u32
}
