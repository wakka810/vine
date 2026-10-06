#![forbid(unsafe_code)]

use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use vine_core::{ModuleSnapshot, Pe32Limits, build_pe32_load_plan};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os().skip(1);
    let path = arguments.next().ok_or("usage: vine-pe-probe PE32_FILE")?;
    if arguments.next().is_some() {
        return Err("usage: vine-pe-probe PE32_FILE".into());
    }
    let limits = Pe32Limits::default();
    let snapshot = ModuleSnapshot::from_bytes(
        read_bounded(Path::new(&path), limits.max_file_bytes)?,
        &limits,
    )?;
    let report = build_pe32_load_plan(&snapshot, limits)?;
    let identity = snapshot.identity();
    let image = &report.image;

    println!("{}", Path::new(&path).display());
    println!("SHA-256: {}", identity.sha256);
    println!("Size: {} bytes", identity.bytes);
    println!("Image: {:?} PE32 / i386", image.kind);
    println!("Image base: {:#010x}", image.preferred_image_base);
    println!("Entry point: {:#010x}", image.entry_point_address);
    println!("Image size: {} bytes", image.size_of_image);
    println!("Sections: {}", report.sections.len());
    for section in &report.sections {
        println!(
            "  {} at RVA {:#010x} ({} bytes)",
            section.name, section.destination_rva.0, section.virtual_size
        );
    }
    println!("Imports: {} modules", report.imports.len());
    for module in &report.imports {
        println!("  {}: {} symbols", module.name, module.symbols.len());
    }
    Ok(())
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let size = file.metadata()?.len();
    if size > limit {
        return Err(format!("source exceeds PE32 file limit of {limit} bytes").into());
    }
    // The metadata check alone is insufficient: the file may grow while being read.
    let mut bytes = Vec::with_capacity(usize::try_from(size)?);
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(format!("source grew beyond PE32 file limit of {limit} bytes").into());
    }
    Ok(bytes)
}
