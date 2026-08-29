use crate::cpu::cpu::{
    full_clear_tlb, safe_read32s, translate_address, translate_address_read_no_side_effects,
};
use crate::cpu::global_pointers::{
    cpl, eip_phys, flags, in_hlt, instruction_counter, instruction_pointer, last_virt_eip,
    previous_ip, reg32, sreg,
};
use crate::cpu::{memory, vine};
use crate::regs::{EAX, EBP, EBX, ECX, EDI, EDX, ESI, ESP};

const STOP_BUDGET: u32 = 1;
const STOP_PENDING_DISPATCH: u32 = 4;
const STOP_ISLAND_ERROR: u32 = 6;
const STOP_INVALID_OPCODE: u32 = 0x106;
const PAGE_BYTES: usize = 4096;
const PAGE_MASK: u32 = (PAGE_BYTES as u32) - 1;
const PTE_DIRTY: u32 = 0x40;
const MAX_FAST_APIS_PER_SLICE: u32 = 256;
const FAST_RECORD_WORDS: u32 = 36;
const FAST_RECORD_VERSION: u32 = 2;
const MEMORY_DELTA_CONTROL_HEADER_BYTES: usize = 56;
const MEMORY_DELTA_MAPPING_DESCRIPTOR_BYTES: usize = 40;
const MEMORY_DELTA_PAGE_DESCRIPTOR_BYTES: usize = 16;

static mut FAST_API_COUNT: u32 = 0;
static mut WRITEBACK_BATCHES: u32 = 0;
static mut WRITEBACK_PAGES: u32 = 0;
static mut MIRRORED_DELTA_PAGES: u32 = 0;
static mut GENERATION: u64 = 0;

enum FastDispatch {
    NotHandled,
    Continued,
    Pending,
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
    true
}

unsafe fn reconcile_dirty_pages() -> Result<bool, String> {
    if vine::vine_dirty_pages_overflowed() != 0 {
        return Ok(false);
    }
    let count = vine::vine_dirty_page_count() as usize;
    if count == 0 {
        return Ok(true);
    }
    let addresses = std::slice::from_raw_parts(vine::vine_dirty_pages_ptr() as *const u32, count);
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
    full_clear_tlb();
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
        },
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
        std::ptr::copy(
            payload.as_ptr().add(page_index * PAGE_BYTES),
            memory::mem8.add(physical as usize),
            PAGE_BYTES,
        );
    }
    if vine_runtime_wasm::vine_bridge_acknowledge_fast_v1(
        generation as u32,
        (generation >> 32) as u32,
    ) != 0
    {
        return Err("execution island could not acknowledge its inline memory delta".to_owned());
    }
    GENERATION = generation;
    MIRRORED_DELTA_PAGES = MIRRORED_DELTA_PAGES
        .checked_add(page_count as u32)
        .ok_or_else(|| "execution island mirrored-page count overflowed".to_owned())?;
    Ok(true)
}

unsafe fn apply_register(action: u32, value: u32, register: *mut i32) -> bool {
    match action {
        0 => true,
        1 => {
            *register = value as i32;
            true
        },
        _ => false,
    }
}

unsafe fn dispatch_fast(
    retired: u64,
    host_elapsed_time_100ns: u64,
) -> Result<FastDispatch, String> {
    let endpoint = *instruction_pointer as u32;
    if !vine_runtime_wasm::execution_island_forward_endpoint_owned(endpoint) {
        return Ok(FastDispatch::NotHandled);
    }
    if !reconcile_dirty_pages()? {
        return Ok(FastDispatch::NotHandled);
    }
    synchronize_clock(retired, host_elapsed_time_100ns)?;
    let stack_pointer = *reg32.add(ESP as usize) as u32;
    let return_address = safe_read32s(stack_pointer as i32)
        .map_err(|()| "execution island could not read the API return address".to_owned())?
        as u32;
    let status = vine_runtime_wasm::vine_bridge_dispatch_fast_v3(
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
    FAST_API_COUNT += 1;
    Ok(FastDispatch::Continued)
}

#[no_mangle]
pub fn vine_execution_island_abi_version() -> u32 {
    1
}

#[no_mangle]
pub unsafe fn vine_execution_island_execute_budget(
    max_instructions: u32,
    retired_instructions_low: u32,
    retired_instructions_high: u32,
    host_elapsed_time_100ns_low: u32,
    host_elapsed_time_100ns_high: u32,
) -> u32 {
    FAST_API_COUNT = 0;
    WRITEBACK_BATCHES = 0;
    WRITEBACK_PAGES = 0;
    MIRRORED_DELTA_PAGES = 0;
    GENERATION = match vine_runtime_wasm::execution_island_memory_generation() {
        Ok(generation) => generation,
        Err(error) => {
            vine_runtime_wasm::execution_island_set_error(error);
            return STOP_ISLAND_ERROR;
        },
    };
    let retired_base = retired_instructions_low as u64 | (retired_instructions_high as u64) << 32;
    let host_elapsed_time_100ns =
        host_elapsed_time_100ns_low as u64 | (host_elapsed_time_100ns_high as u64) << 32;
    let initial_counter = *instruction_counter;
    loop {
        let executed = (*instruction_counter).wrapping_sub(initial_counter);
        if executed >= max_instructions || FAST_API_COUNT >= MAX_FAST_APIS_PER_SLICE {
            return STOP_BUDGET;
        }
        let reason = crate::cpu::cpu::vine_execute_budget(max_instructions - executed);
        if reason != STOP_INVALID_OPCODE {
            return reason;
        }
        let retired = retired_base + (*instruction_counter).wrapping_sub(initial_counter) as u64;
        match dispatch_fast(retired, host_elapsed_time_100ns) {
            Ok(FastDispatch::Continued) => {},
            Ok(FastDispatch::Pending) => return STOP_PENDING_DISPATCH,
            Ok(FastDispatch::NotHandled) => return reason,
            Err(error) => {
                vine_runtime_wasm::execution_island_set_error(error);
                return STOP_ISLAND_ERROR;
            },
        }
    }
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
