#![forbid(unsafe_code)]

use std::error::Error;
use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};
use vine_core::{
    GuestAddress, GuestMemory, MemoryDelta, MemoryLimits, MemoryOwnerKind, MemoryReflection,
    MemoryRegionKind, MemoryStats, PageProtection,
};

const CODE_BASE: GuestAddress = GuestAddress(0x1000_0000);
const DATA_BASE: GuestAddress = GuestAddress(0x2000_0000);
const CROSS_PAGE_BASE: GuestAddress = GuestAddress(0x3000_0000);
const CROSS_PAGE_INSTRUCTION: GuestAddress = GuestAddress(0x3000_0ffe);
const EXPECTED_VALUE: u64 = 0x8877_6655_4433_2211;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitialOffer {
    schema: &'static str,
    schema_version: u32,
    entry_point: GuestAddress,
    data_address: GuestAddress,
    expected_low: u32,
    expected_high: u32,
    cross_page_instruction: GuestAddress,
    reflection: MemoryReflection,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeltaOffer {
    schema: &'static str,
    schema_version: u32,
    delta: MemoryDelta,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransferAck {
    schema: String,
    schema_version: u32,
    generation: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionReceipt {
    schema: &'static str,
    schema_version: u32,
    acknowledged_generation: u64,
    value_bytes: Vec<u8>,
    stats: MemoryStats,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 4,
        max_regions: 3,
        max_owners: 3,
        max_access_bytes: 0x1000,
    })?;

    let code_owner = memory.create_owner(MemoryOwnerKind::Runtime)?;
    let code_range =
        memory.reserve_fixed(code_owner, MemoryRegionKind::Runtime, CODE_BASE, 0x1000)?;
    memory.commit(
        code_owner,
        code_range,
        PageProtection {
            read: true,
            write: false,
            execute: true,
        },
    )?;
    let code = [
        0xa1, 0x00, 0x00, 0x00, 0x20, // mov eax, [0x20000000]
        0x8b, 0x15, 0x04, 0x00, 0x00, 0x20, // mov edx, [0x20000004]
        0x0f, 0x0b, // ud2
    ];
    memory.write_initialization(code_owner, CODE_BASE, &code)?;
    memory.seal_owner(code_owner)?;

    let data_owner = memory.create_owner(MemoryOwnerKind::Anonymous)?;
    let data_range =
        memory.reserve_fixed(data_owner, MemoryRegionKind::Anonymous, DATA_BASE, 0x1000)?;
    memory.commit(
        data_owner,
        data_range,
        PageProtection {
            read: true,
            write: true,
            execute: false,
        },
    )?;
    memory.seal_owner(data_owner)?;

    let cross_page_owner = memory.create_owner(MemoryOwnerKind::Runtime)?;
    memory.reserve_fixed(
        cross_page_owner,
        MemoryRegionKind::Runtime,
        CROSS_PAGE_BASE,
        0x2000,
    )?;
    let first_cross_page = vine_core::GuestRange::new(CROSS_PAGE_BASE, 0x1000)?;
    let second_cross_page =
        vine_core::GuestRange::new(GuestAddress(CROSS_PAGE_BASE.0 + 0x1000), 0x1000)?;
    memory.commit(
        cross_page_owner,
        first_cross_page,
        PageProtection {
            read: true,
            write: false,
            execute: true,
        },
    )?;
    memory.commit(
        cross_page_owner,
        second_cross_page,
        PageProtection {
            read: true,
            write: false,
            execute: false,
        },
    )?;
    memory.write_initialization(
        cross_page_owner,
        CROSS_PAGE_INSTRUCTION,
        &[0xb8, 0x78, 0x56, 0x34, 0x12, 0x0f, 0x0b],
    )?;
    memory.seal_owner(cross_page_owner)?;

    let reflection = memory.pending_reflection()?;
    let initial_generation = reflection.invalidation.generation;
    write_message(&InitialOffer {
        schema: "vine.memory-delta-initial",
        schema_version: 1,
        entry_point: CODE_BASE,
        data_address: DATA_BASE,
        expected_low: EXPECTED_VALUE as u32,
        expected_high: (EXPECTED_VALUE >> 32) as u32,
        cross_page_instruction: CROSS_PAGE_INSTRUCTION,
        reflection,
    })?;
    let initial_ack = read_ack()?;
    validate_ack(&initial_ack, initial_generation)?;
    memory.acknowledge_delta(initial_ack.generation)?;

    memory.write(DATA_BASE, &EXPECTED_VALUE.to_le_bytes())?;
    let delta = memory.pending_delta()?;
    let delta_generation = delta.generation;
    write_message(&DeltaOffer {
        schema: "vine.memory-delta-update",
        schema_version: 1,
        delta,
    })?;
    let delta_ack = read_ack()?;
    validate_ack(&delta_ack, delta_generation)?;
    memory.acknowledge_delta(delta_ack.generation)?;

    let value_bytes = memory.read(DATA_BASE, 8)?;
    write_message(&SessionReceipt {
        schema: "vine.memory-delta-receipt",
        schema_version: 1,
        acknowledged_generation: delta_generation,
        value_bytes,
        stats: memory.stats(),
    })?;
    Ok(())
}

fn write_message(value: &impl Serialize) -> Result<(), Box<dyn Error>> {
    let mut stdout = io::stdout().lock();
    serde_json::to_writer(&mut stdout, value)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

fn read_ack() -> Result<TransferAck, Box<dyn Error>> {
    let mut line = String::new();
    let read = io::stdin().lock().read_line(&mut line)?;
    if read == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "memory transfer acknowledgement was not provided",
        )
        .into());
    }
    Ok(serde_json::from_str(&line)?)
}

fn validate_ack(ack: &TransferAck, generation: u64) -> Result<(), Box<dyn Error>> {
    if ack.schema != "vine.memory-transfer-ack"
        || ack.schema_version != 1
        || ack.generation != generation
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "invalid memory transfer acknowledgement: schema {} version {}, generation {}; expected generation {generation}",
                ack.schema, ack.schema_version, ack.generation
            ),
        )
        .into());
    }
    Ok(())
}
