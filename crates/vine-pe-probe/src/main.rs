#![forbid(unsafe_code)]

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::Serialize;
use vine_core::{
    GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryErrorClass, ImageMaterialization,
    ImageMemoryLayout, LoaderError, LoaderErrorClass, MemoryLimits, MemoryStats, ModuleIdentity,
    ModuleSnapshot, Pe32Limits, UnsupportedPoint, build_pe32_load_plan, materialize_pe32_image,
};

#[derive(Debug)]
struct Arguments {
    input: PathBuf,
    output: Option<PathBuf>,
    expected_sha256: Option<String>,
    prepare_memory_pages: Option<usize>,
    materialize_image_base: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FailureReport {
    schema: &'static str,
    schema_version: u32,
    error: LoaderError,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryIngressReport {
    schema: &'static str,
    schema_version: u32,
    source: ModuleIdentity,
    loader_schema: &'static str,
    loader_schema_version: u32,
    memory_limits: MemoryLimits,
    image: MemoryImageSummary,
    layout: ImageMemoryLayout,
    stats: MemoryStats,
    stop: UnsupportedPoint,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryImageSummary {
    preferred_image_base: u32,
    size_of_image: u32,
    entry_point_address: u32,
    section_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MaterializationIngressReport {
    schema: &'static str,
    schema_version: u32,
    loader_schema: &'static str,
    loader_schema_version: u32,
    memory_limits: MemoryLimits,
    materialization: ImageMaterialization,
    stats: MemoryStats,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let failure = FailureReport {
                schema: "vine.pe32-loader-failure",
                schema_version: 1,
                error,
            };
            match serde_json::to_string_pretty(&failure) {
                Ok(json) => eprintln!("{json}"),
                Err(_) => eprintln!("PE probe failed before its diagnostic could be serialized"),
            }
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), LoaderError> {
    let arguments = parse_arguments()?;
    let limits = Pe32Limits::default();
    let bytes = read_bounded(&arguments.input, limits.max_file_bytes)?;
    let snapshot = ModuleSnapshot::from_bytes(bytes, &limits)?;
    if let Some(expected) = &arguments.expected_sha256 {
        snapshot.verify_expected_sha256(expected)?;
    }
    let loader_report = build_pe32_load_plan(&snapshot, limits)?;
    let mut json = if let Some(load_base) = arguments.materialize_image_base {
        let max_physical_pages = arguments.prepare_memory_pages.ok_or_else(usage_error)?;
        let memory_limits = MemoryLimits {
            max_physical_pages,
            ..MemoryLimits::default()
        };
        let mut memory = GuestMemory::new(memory_limits).map_err(memory_error)?;
        let materialization = materialize_pe32_image(
            &mut memory,
            &snapshot,
            &loader_report,
            GuestAddress(load_base),
        )?;
        let report = MaterializationIngressReport {
            schema: "vine.pe32-materialization",
            schema_version: 1,
            loader_schema: loader_report.schema,
            loader_schema_version: loader_report.schema_version,
            memory_limits,
            materialization,
            stats: memory.stats(),
        };
        serialize_report(&report)?
    } else if let Some(max_physical_pages) = arguments.prepare_memory_pages {
        let memory_limits = MemoryLimits {
            max_physical_pages,
            ..MemoryLimits::default()
        };
        let mut memory = GuestMemory::new(memory_limits).map_err(memory_error)?;
        let layout = memory
            .prepare_image_layout(&loader_report.source, &loader_report.load_plan)
            .map_err(memory_error)?;
        let report = MemoryIngressReport {
            schema: "vine.guest-memory-ingress",
            schema_version: 1,
            source: loader_report.source.clone(),
            loader_schema: loader_report.schema,
            loader_schema_version: loader_report.schema_version,
            memory_limits,
            image: MemoryImageSummary {
                preferred_image_base: loader_report.load_plan.image.preferred_image_base,
                size_of_image: loader_report.load_plan.image.size_of_image,
                entry_point_address: loader_report.load_plan.image.entry_point_address,
                section_count: loader_report.load_plan.sections.len(),
            },
            layout,
            stats: memory.stats(),
            stop: UnsupportedPoint {
                stage: "image-materialization",
                code: "image-bytes-relocations-import-binding-unimplemented",
                message: "the sparse image region and zeroed physical pages are owned; source copy, zero-fill verification, relocations, IAT binding, owner sealing, and guest execution were not attempted",
            },
        };
        serialize_report(&report)?
    } else {
        serialize_report(&loader_report)?
    };
    json.push(b'\n');

    if let Some(output) = arguments.output {
        write_report(&output, &json)?;
        println!("wrote runtime ingress report to {}", output.display());
    } else {
        io::stdout().write_all(&json).map_err(|error| {
            io_error(
                "probe-output",
                "stdout-write-failed",
                "could not write the runtime ingress report to stdout",
                error,
            )
        })?;
    }
    Ok(())
}

fn serialize_report(report: &impl Serialize) -> Result<Vec<u8>, LoaderError> {
    serde_json::to_vec_pretty(report).map_err(|error| {
        LoaderError::runtime_defect(
            "probe-output",
            "report-serialization-failed",
            format!("could not serialize the deterministic ingress report: {error}"),
        )
    })
}

fn write_report(output: &Path, json: &[u8]) -> Result<(), LoaderError> {
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            io_error(
                "probe-output",
                "output-directory-create-failed",
                "could not create the report output directory",
                error,
            )
        })?;
    }
    let mut output_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| {
            let (code, context) = if error.kind() == io::ErrorKind::AlreadyExists {
                (
                    "report-output-already-exists",
                    "refusing to replace an existing report or source file",
                )
            } else {
                (
                    "report-create-failed",
                    "could not create the ingress report",
                )
            };
            io_error("probe-output", code, context, error)
        })?;
    output_file.write_all(json).map_err(|error| {
        io_error(
            "probe-output",
            "report-write-failed",
            "could not write the ingress report",
            error,
        )
    })
}

fn parse_arguments() -> Result<Arguments, LoaderError> {
    let mut input = None;
    let mut output = None;
    let mut expected_sha256 = None;
    let mut prepare_memory_pages = None;
    let mut materialize_image_base = None;
    let mut arguments = env::args_os().skip(1);

    while let Some(argument) = arguments.next() {
        if argument == "--output" {
            let value = arguments.next().ok_or_else(usage_error)?;
            if output.replace(PathBuf::from(value)).is_some() {
                return Err(usage_error());
            }
        } else if argument == "--expect-sha256" {
            let value = arguments.next().ok_or_else(usage_error)?;
            let value = value.into_string().map_err(|_| usage_error())?;
            if expected_sha256.replace(value).is_some() {
                return Err(usage_error());
            }
        } else if argument == "--prepare-memory-pages" {
            let value = arguments.next().ok_or_else(usage_error)?;
            let value = value.into_string().map_err(|_| usage_error())?;
            let value = value.parse::<usize>().map_err(|_| usage_error())?;
            if value == 0 || prepare_memory_pages.replace(value).is_some() {
                return Err(usage_error());
            }
        } else if argument == "--materialize-image-base" {
            let value = arguments.next().ok_or_else(usage_error)?;
            let value = value.into_string().map_err(|_| usage_error())?;
            let value = parse_u32_argument(&value)?;
            if materialize_image_base.replace(value).is_some() {
                return Err(usage_error());
            }
        } else if argument.to_string_lossy().starts_with('-') || input.is_some() {
            return Err(usage_error());
        } else {
            input = Some(PathBuf::from(argument));
        }
    }

    if materialize_image_base.is_some() && prepare_memory_pages.is_none() {
        return Err(usage_error());
    }

    Ok(Arguments {
        input: input.ok_or_else(usage_error)?,
        output,
        expected_sha256,
        prepare_memory_pages,
        materialize_image_base,
    })
}

fn parse_u32_argument(value: &str) -> Result<u32, LoaderError> {
    let parsed = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .map_or_else(
            || value.parse::<u32>(),
            |hexadecimal| u32::from_str_radix(hexadecimal, 16),
        );
    parsed.map_err(|_| usage_error())
}

fn read_bounded(path: &Path, max_file_bytes: u64) -> Result<Vec<u8>, LoaderError> {
    let file = File::open(path).map_err(|error| {
        io_error(
            "module-snapshot",
            "source-open-failed",
            "could not open the requested PE source",
            error,
        )
    })?;
    let metadata = file.metadata().map_err(|error| {
        io_error(
            "module-snapshot",
            "source-metadata-failed",
            "could not inspect the requested PE source",
            error,
        )
    })?;
    if metadata.len() > max_file_bytes {
        return Err(LoaderError::resource_limit(
            "module-snapshot",
            "source-file-limit-exceeded",
            format!(
                "source has {} bytes; limit is {max_file_bytes} bytes",
                metadata.len()
            ),
        ));
    }

    let capacity = usize::try_from(metadata.len()).map_err(|_| {
        LoaderError::resource_limit(
            "module-snapshot",
            "source-length-not-representable",
            "source length does not fit the host index type",
        )
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(max_file_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            io_error(
                "module-snapshot",
                "source-read-failed",
                "could not read the requested PE source into an owned snapshot",
                error,
            )
        })?;
    if bytes.len() as u64 > max_file_bytes {
        return Err(LoaderError::resource_limit(
            "module-snapshot",
            "source-file-limit-exceeded",
            "source grew beyond the configured limit while it was read",
        ));
    }
    Ok(bytes)
}

fn usage_error() -> LoaderError {
    LoaderError {
        class: LoaderErrorClass::InvalidInput,
        stage: "probe-arguments",
        code: "usage",
        message: "usage: vine-pe-probe [--expect-sha256 HEX] [--prepare-memory-pages COUNT [--materialize-image-base ADDRESS]] [--output FILE] INPUT".to_owned(),
    }
}

fn memory_error(error: GuestMemoryError) -> LoaderError {
    let class = match error.class {
        GuestMemoryErrorClass::ResourceLimit => LoaderErrorClass::ResourceLimit,
        GuestMemoryErrorClass::RuntimeDefect => LoaderErrorClass::RuntimeDefect,
        GuestMemoryErrorClass::InvalidRequest
        | GuestMemoryErrorClass::Conflict
        | GuestMemoryErrorClass::SynchronizationRequired
        | GuestMemoryErrorClass::Fault => LoaderErrorClass::InvalidInput,
    };
    LoaderError {
        class,
        stage: error.stage,
        code: error.code,
        message: error.message,
    }
}

fn io_error(
    stage: &'static str,
    code: &'static str,
    context: &'static str,
    error: io::Error,
) -> LoaderError {
    LoaderError {
        class: LoaderErrorClass::InvalidInput,
        stage,
        code,
        message: format!("{context}: {error}"),
    }
}
