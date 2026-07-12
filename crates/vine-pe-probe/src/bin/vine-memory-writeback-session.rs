#![forbid(unsafe_code)]

use std::error::Error;
use std::io::{self, Read, Write};

use vine_core::{
    CpuWritebackReceipt, GUEST_PAGE_SIZE, GuestAddress, GuestMemory, MemoryLimits, MemoryOwnerKind,
    MemoryRegionKind, PageProtection,
};

const FRAME_MAGIC: &[u8; 8] = b"VINEFR01";
const FRAME_VERSION: u32 = 1;
const FRAME_HEADER_BYTES: usize = 24;
const FRAME_INITIAL_REFLECTION: u32 = 1;
const FRAME_ACKNOWLEDGEMENT: u32 = 2;
const FRAME_CPU_WRITEBACK: u32 = 3;
const FRAME_WRITEBACK_RECEIPT: u32 = 4;
const FRAME_HOST_DELTA: u32 = 5;
const MAX_CONTROL_BYTES: usize = 1024 * 1024;
const MAX_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;

const ENTRY_POINT: GuestAddress = GuestAddress(0x1000_0000);
const SECOND_ENTRY_POINT: GuestAddress = GuestAddress(0x1000_0100);
const STACK_BASE: GuestAddress = GuestAddress(0x2000_0000);
const HEAP_BASE: GuestAddress = GuestAddress(0x3000_0000);
const SURFACE_BASE: GuestAddress = GuestAddress(0x4000_0000);
const CODE_TARGET_BASE: GuestAddress = GuestAddress(0x5000_0000);
const SCRATCH_BASE: GuestAddress = GuestAddress(0x6000_0000);

fn main() -> Result<(), Box<dyn Error>> {
    let mut memory = build_memory()?;
    let reflection = memory.pending_reflection()?;
    let reflection_generation = reflection.invalidation.generation;
    let reflection_buffers = reflection.encode_binary()?;

    let stdout = io::stdout();
    let mut writer = stdout.lock();
    write_frame(
        &mut writer,
        FRAME_INITIAL_REFLECTION,
        &reflection_buffers.control,
        &reflection_buffers.page_bytes,
    )?;

    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let acknowledgement = read_frame(&mut reader, FRAME_ACKNOWLEDGEMENT)?;
    if !acknowledgement.payload.is_empty() || acknowledgement.control.len() != 8 {
        return Err(invalid_data("invalid reflection acknowledgement frame").into());
    }
    let acknowledged_generation = u64::from_le_bytes(
        acknowledgement.control[..8]
            .try_into()
            .expect("checked acknowledgement size"),
    );
    if acknowledged_generation != reflection_generation {
        return Err(invalid_data("reflection acknowledgement generation mismatch").into());
    }
    memory.acknowledge_delta(acknowledged_generation)?;

    let writeback = read_frame(&mut reader, FRAME_CPU_WRITEBACK)?;
    let receipt = memory.reconcile_cpu_writeback_binary(&writeback.control, &writeback.payload)?;
    validate_fixture_result(&memory, &receipt)?;
    let receipt_control = encode_receipt(&memory, &receipt)?;
    write_frame(&mut writer, FRAME_WRITEBACK_RECEIPT, &receipt_control, &[])?;

    memory.write(SCRATCH_BASE, &[0xee, 0xdd, 0xcc, 0xbb])?;
    let host_delta = memory.pending_delta()?;
    let host_delta_generation = host_delta.generation;
    let host_delta_buffers = host_delta.encode_binary()?;
    write_frame(
        &mut writer,
        FRAME_HOST_DELTA,
        &host_delta_buffers.control,
        &host_delta_buffers.page_bytes,
    )?;
    let host_delta_acknowledgement = read_frame(&mut reader, FRAME_ACKNOWLEDGEMENT)?;
    if !host_delta_acknowledgement.payload.is_empty()
        || host_delta_acknowledgement.control.len() != 8
    {
        return Err(invalid_data("invalid host delta acknowledgement frame").into());
    }
    let host_delta_acknowledged_generation = u64::from_le_bytes(
        host_delta_acknowledgement.control[..8]
            .try_into()
            .expect("checked acknowledgement size"),
    );
    if host_delta_acknowledged_generation != host_delta_generation {
        return Err(invalid_data("host delta acknowledgement generation mismatch").into());
    }
    memory.acknowledge_delta(host_delta_acknowledged_generation)?;

    let second_writeback = read_frame(&mut reader, FRAME_CPU_WRITEBACK)?;
    let second_receipt = memory
        .reconcile_cpu_writeback_binary(&second_writeback.control, &second_writeback.payload)?;
    validate_second_fixture_result(&memory, &second_receipt)?;
    let second_receipt_control = encode_receipt(&memory, &second_receipt)?;
    write_frame(
        &mut writer,
        FRAME_WRITEBACK_RECEIPT,
        &second_receipt_control,
        &[],
    )?;
    Ok(())
}

fn build_memory() -> Result<GuestMemory, Box<dyn Error>> {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 9,
        max_regions: 6,
        max_owners: 6,
        max_access_bytes: GUEST_PAGE_SIZE as usize,
    })?;

    let code_owner = install_region(
        &mut memory,
        ENTRY_POINT,
        1,
        MemoryRegionKind::Runtime,
        PageProtection {
            read: true,
            write: false,
            execute: true,
        },
    )?;
    memory.write_initialization(code_owner, ENTRY_POINT, guest_program())?;
    memory.write_initialization(code_owner, SECOND_ENTRY_POINT, second_guest_program())?;
    memory.seal_owner(code_owner)?;

    install_writable_region(&mut memory, STACK_BASE, 2, MemoryRegionKind::Stack)?;
    install_writable_region(&mut memory, HEAP_BASE, 2, MemoryRegionKind::Heap)?;
    install_writable_region(&mut memory, SURFACE_BASE, 2, MemoryRegionKind::Surface)?;
    let code_target_owner = install_region(
        &mut memory,
        CODE_TARGET_BASE,
        1,
        MemoryRegionKind::Runtime,
        PageProtection {
            read: true,
            write: true,
            execute: true,
        },
    )?;
    memory.write_initialization(code_target_owner, CODE_TARGET_BASE, &[0x90])?;
    memory.seal_owner(code_target_owner)?;
    install_writable_region(&mut memory, SCRATCH_BASE, 1, MemoryRegionKind::Anonymous)?;
    Ok(memory)
}

fn install_writable_region(
    memory: &mut GuestMemory,
    base: GuestAddress,
    pages: u64,
    kind: MemoryRegionKind,
) -> Result<(), Box<dyn Error>> {
    let owner = install_region(
        memory,
        base,
        pages,
        kind,
        PageProtection {
            read: true,
            write: true,
            execute: false,
        },
    )?;
    memory.seal_owner(owner)?;
    Ok(())
}

fn install_region(
    memory: &mut GuestMemory,
    base: GuestAddress,
    pages: u64,
    kind: MemoryRegionKind,
    protection: PageProtection,
) -> Result<vine_core::MemoryOwnerId, Box<dyn Error>> {
    let owner = memory.create_owner(MemoryOwnerKind::Anonymous)?;
    let range = memory.reserve_fixed(owner, kind, base, pages * GUEST_PAGE_SIZE)?;
    memory.commit(owner, range, protection)?;
    Ok(owner)
}

fn guest_program() -> &'static [u8] {
    &[
        0xbf, 0xf0, 0x0f, 0x00, 0x30, // mov edi, 0x30000ff0
        0xb8, 0x44, 0x33, 0x22, 0x11, // mov eax, 0x11223344
        0xb9, 0x08, 0x00, 0x00, 0x00, // mov ecx, 8
        0xf3, 0xab, // rep stosd
        0xbc, 0x04, 0x10, 0x00, 0x20, // mov esp, 0x20001004
        0x68, 0x88, 0x77, 0x66, 0x55, // push 0x55667788
        0x68, 0xcc, 0xbb, 0xaa, 0x99, // push 0x99aabbcc
        0xc7, 0x05, 0xfc, 0x0f, 0x00, 0x40, 0xdd, 0xcc, 0xbb, 0xaa, 0xc7, 0x05, 0x00, 0x10, 0x00,
        0x40, 0x44, 0x33, 0x22, 0x11, 0xc6, 0x05, 0x00, 0x00, 0x00, 0x50,
        0xcc, // self-modifying target byte
        0x0f, 0x0b, // ud2
    ]
}

fn second_guest_program() -> &'static [u8] {
    &[
        0xc7, 0x05, 0x00, 0x00, 0x00, 0x20, 0x04, 0x03, 0x02, 0x01, 0xc7, 0x05, 0x20, 0x10, 0x00,
        0x30, 0x08, 0x07, 0x06, 0x05, 0xc7, 0x05, 0x10, 0x00, 0x00, 0x40, 0x0c, 0x0b, 0x0a, 0x09,
        0xc6, 0x05, 0x00, 0x00, 0x00, 0x50, 0xcd, 0x0f, 0x0b,
    ]
}

fn validate_fixture_result(
    memory: &GuestMemory,
    receipt: &CpuWritebackReceipt,
) -> Result<(), Box<dyn Error>> {
    if receipt.touched_pages != 7
        || receipt.changed_pages != 7
        || receipt.unchanged_pages != 0
        || receipt.executable_pages != 1
        || receipt.changed_bytes != 49
        || receipt.transferred_bytes != 7 * GUEST_PAGE_SIZE
    {
        return Err(invalid_data("unexpected CPU writeback receipt").into());
    }
    require_bytes(
        memory,
        GuestAddress(STACK_BASE.0 + 0x0ffc),
        &[0xcc, 0xbb, 0xaa, 0x99],
        "first stack page",
    )?;
    require_bytes(
        memory,
        GuestAddress(STACK_BASE.0 + 0x1000),
        &[0x88, 0x77, 0x66, 0x55],
        "second stack page",
    )?;
    let heap = memory.read(GuestAddress(HEAP_BASE.0 + 0x0ff0), 32)?;
    if !heap
        .chunks_exact(4)
        .all(|word| word == [0x44, 0x33, 0x22, 0x11])
    {
        return Err(invalid_data("heap REP writeback mismatch").into());
    }
    require_bytes(
        memory,
        GuestAddress(SURFACE_BASE.0 + 0x0ffc),
        &[0xdd, 0xcc, 0xbb, 0xaa],
        "first surface page",
    )?;
    require_bytes(
        memory,
        GuestAddress(SURFACE_BASE.0 + 0x1000),
        &[0x44, 0x33, 0x22, 0x11],
        "second surface page",
    )?;
    require_bytes(memory, CODE_TARGET_BASE, &[0xcc], "executable target page")?;
    Ok(())
}

fn validate_second_fixture_result(
    memory: &GuestMemory,
    receipt: &CpuWritebackReceipt,
) -> Result<(), Box<dyn Error>> {
    if receipt.touched_pages != 4
        || receipt.changed_pages != 4
        || receipt.unchanged_pages != 0
        || receipt.executable_pages != 1
        || receipt.changed_bytes != 13
        || receipt.transferred_bytes != 4 * GUEST_PAGE_SIZE
    {
        return Err(invalid_data("unexpected second CPU writeback receipt").into());
    }
    require_bytes(
        memory,
        STACK_BASE,
        &[0x04, 0x03, 0x02, 0x01],
        "second-slice stack page",
    )?;
    require_bytes(
        memory,
        GuestAddress(HEAP_BASE.0 + 0x1020),
        &[0x08, 0x07, 0x06, 0x05],
        "second-slice heap page",
    )?;
    require_bytes(
        memory,
        GuestAddress(SURFACE_BASE.0 + 0x10),
        &[0x0c, 0x0b, 0x0a, 0x09],
        "second-slice surface page",
    )?;
    require_bytes(
        memory,
        CODE_TARGET_BASE,
        &[0xcd],
        "second-slice executable target",
    )?;
    require_bytes(
        memory,
        SCRATCH_BASE,
        &[0xee, 0xdd, 0xcc, 0xbb],
        "host delta scratch page",
    )?;
    Ok(())
}

fn require_bytes(
    memory: &GuestMemory,
    address: GuestAddress,
    expected: &[u8],
    label: &str,
) -> Result<(), Box<dyn Error>> {
    if memory.read(address, expected.len())? != expected {
        return Err(invalid_data(&format!("{label} writeback mismatch")).into());
    }
    Ok(())
}

fn encode_receipt(
    memory: &GuestMemory,
    receipt: &CpuWritebackReceipt,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = Vec::with_capacity(120);
    bytes.extend_from_slice(b"VINEWR01");
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&receipt.from_generation.to_le_bytes());
    bytes.extend_from_slice(&receipt.generation.to_le_bytes());
    bytes.extend_from_slice(&(receipt.touched_pages as u32).to_le_bytes());
    bytes.extend_from_slice(&(receipt.changed_pages as u32).to_le_bytes());
    bytes.extend_from_slice(&(receipt.unchanged_pages as u32).to_le_bytes());
    bytes.extend_from_slice(&(receipt.executable_pages as u32).to_le_bytes());
    bytes.extend_from_slice(&receipt.changed_bytes.to_le_bytes());
    bytes.extend_from_slice(&receipt.transferred_bytes.to_le_bytes());
    for index in (0..receipt.content_sha256.len()).step_by(2) {
        bytes.push(u8::from_str_radix(
            &receipt.content_sha256[index..index + 2],
            16,
        )?);
    }
    let stats = memory.stats();
    bytes.extend_from_slice(&stats.generation.to_le_bytes());
    bytes.extend_from_slice(&stats.acknowledged_generation.to_le_bytes());
    bytes.extend_from_slice(&(stats.dirty_pages as u32).to_le_bytes());
    bytes.extend_from_slice(&u32::from(stats.pending_invalidation.is_some()).to_le_bytes());
    if bytes.len() != 120 {
        return Err(invalid_data("writeback receipt binary size mismatch").into());
    }
    Ok(bytes)
}

struct Frame {
    control: Vec<u8>,
    payload: Vec<u8>,
}

fn write_frame(
    writer: &mut impl Write,
    kind: u32,
    control: &[u8],
    payload: &[u8],
) -> Result<(), Box<dyn Error>> {
    let control_bytes = u32::try_from(control.len())?;
    let payload_bytes = u32::try_from(payload.len())?;
    let mut header = Vec::with_capacity(FRAME_HEADER_BYTES);
    header.extend_from_slice(FRAME_MAGIC);
    header.extend_from_slice(&FRAME_VERSION.to_le_bytes());
    header.extend_from_slice(&kind.to_le_bytes());
    header.extend_from_slice(&control_bytes.to_le_bytes());
    header.extend_from_slice(&payload_bytes.to_le_bytes());
    writer.write_all(&header)?;
    writer.write_all(control)?;
    writer.write_all(payload)?;
    writer.flush()?;
    Ok(())
}

fn read_frame(reader: &mut impl Read, expected_kind: u32) -> Result<Frame, Box<dyn Error>> {
    let mut header = [0u8; FRAME_HEADER_BYTES];
    reader.read_exact(&mut header)?;
    if header[..8] != *FRAME_MAGIC {
        return Err(invalid_data("memory transport frame magic mismatch").into());
    }
    let version = u32::from_le_bytes(header[8..12].try_into().unwrap());
    let kind = u32::from_le_bytes(header[12..16].try_into().unwrap());
    let control_bytes = u32::from_le_bytes(header[16..20].try_into().unwrap()) as usize;
    let payload_bytes = u32::from_le_bytes(header[20..24].try_into().unwrap()) as usize;
    if version != FRAME_VERSION || kind != expected_kind {
        return Err(invalid_data("memory transport frame type mismatch").into());
    }
    if control_bytes > MAX_CONTROL_BYTES || payload_bytes > MAX_PAYLOAD_BYTES {
        return Err(invalid_data("memory transport frame exceeds bounded size").into());
    }
    let mut control = vec![0; control_bytes];
    let mut payload = vec![0; payload_bytes];
    reader.read_exact(&mut control)?;
    reader.read_exact(&mut payload)?;
    Ok(Frame { control, payload })
}

fn invalid_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
