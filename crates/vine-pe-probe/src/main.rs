#![forbid(unsafe_code)]

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::{Deserialize, Serialize};
use vine_core::{
    CallbackReturnSentinel, GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryErrorClass,
    ImageMaterialization, ImageMemoryLayout, ImportBinding, ImportThunk, InitialProcessEnvironment,
    InitialThreadStack, LoaderError, LoaderErrorClass, LoaderIngressReport, MemoryInvalidation,
    MemoryLimits, MemoryReflection, MemoryStats, ModuleIdentity, ModuleSnapshot, Pe32Limits,
    ProcessEntryReturnSentinel, RuntimeExport, RuntimeExportCatalog, UnsupportedPoint,
    bind_pe32_imports, bind_pe32_imports_with_unsupported_traps, build_pe32_load_plan,
    install_callback_return_sentinel, install_controlled_initial_stack,
    install_controlled_process_environment, install_controlled_process_environment_with_parameters,
    install_runtime_return_sentinels, materialize_pe32_image,
};

#[derive(Debug)]
struct Arguments {
    input: PathBuf,
    output: Option<PathBuf>,
    expected_sha256: Option<String>,
    prepare_memory_pages: Option<usize>,
    materialize_image_base: Option<u32>,
    runtime_exports: Vec<RuntimeExport>,
    trap_unsupported_imports: bool,
    v86_reflection_session: bool,
    install_callback_return_sentinel: bool,
    install_process_entry_return_sentinel: bool,
    install_controlled_initial_stack: bool,
    install_controlled_process_environment: bool,
    install_controlled_process_parameters: bool,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportBindingIngressReport {
    schema: &'static str,
    schema_version: u32,
    loader_schema: &'static str,
    loader_schema_version: u32,
    memory_limits: MemoryLimits,
    runtime_exports: Vec<RuntimeExport>,
    #[serde(skip_serializing_if = "is_false")]
    trap_unsupported_imports: bool,
    materialization: ImageMaterialization,
    binding: ImportBinding,
    stats: MemoryStats,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86ReflectionOffer {
    schema: &'static str,
    schema_version: u32,
    source: ModuleIdentity,
    image_load_base: GuestAddress,
    entry_point_address: GuestAddress,
    image_owner: vine_core::MemoryOwnerId,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_thunk: Option<ImportThunk>,
    thunks: Vec<ImportThunk>,
    #[serde(skip_serializing_if = "Option::is_none")]
    callback_return_sentinel: Option<CallbackReturnSentinel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    process_entry_return_sentinel: Option<ProcessEntryReturnSentinel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    initial_thread_stack: Option<InitialThreadStack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    initial_process_environment: Option<InitialProcessEnvironment>,
    reflection: MemoryReflection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86ReflectionAck {
    schema: String,
    schema_version: u32,
    generation: u64,
    data_tlb: bool,
    instruction_fetch: bool,
    generated_code: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86ReflectionReceipt {
    schema: &'static str,
    schema_version: u32,
    acknowledged_generation: u64,
    stats: MemoryStats,
    #[serde(skip_serializing_if = "Option::is_none")]
    iat_value: Option<u32>,
    entry_bytes: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thunk_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    callback_return_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    process_entry_return_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    initial_stack_top_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    teb_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    peb_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    process_parameters_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command_line_bytes: Option<Vec<u8>>,
    stop: UnsupportedPoint,
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
    if arguments.v86_reflection_session {
        return run_v86_reflection_session(arguments, &snapshot, &loader_report);
    }
    let mut json = if !arguments.runtime_exports.is_empty() || arguments.trap_unsupported_imports {
        let load_base = arguments.materialize_image_base.ok_or_else(usage_error)?;
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
        let catalog = RuntimeExportCatalog::new(arguments.runtime_exports.clone())?;
        let binding = if arguments.trap_unsupported_imports {
            bind_pe32_imports_with_unsupported_traps(
                &mut memory,
                &loader_report,
                &materialization,
                &catalog,
            )?
        } else {
            bind_pe32_imports(&mut memory, &loader_report, &materialization, &catalog)?
        };
        let report = ImportBindingIngressReport {
            schema: "vine.pe32-import-binding",
            schema_version: 1,
            loader_schema: loader_report.schema,
            loader_schema_version: loader_report.schema_version,
            memory_limits,
            runtime_exports: arguments.runtime_exports,
            trap_unsupported_imports: arguments.trap_unsupported_imports,
            materialization,
            binding,
            stats: memory.stats(),
        };
        serialize_report(&report)?
    } else if let Some(load_base) = arguments.materialize_image_base {
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

fn run_v86_reflection_session(
    arguments: Arguments,
    snapshot: &ModuleSnapshot,
    loader_report: &LoaderIngressReport,
) -> Result<(), LoaderError> {
    const MAX_REFLECTION_PAGES: usize = 16;
    let load_base = arguments.materialize_image_base.ok_or_else(usage_error)?;
    let max_physical_pages = arguments.prepare_memory_pages.ok_or_else(usage_error)?;
    if max_physical_pages > MAX_REFLECTION_PAGES {
        return Err(LoaderError::resource_limit(
            "memory-reflection",
            "reflection-page-limit-exceeded",
            "the version 1 reflection session is limited to 16 physical pages",
        ));
    }
    let memory_limits = MemoryLimits {
        max_physical_pages,
        ..MemoryLimits::default()
    };
    let mut memory = GuestMemory::new(memory_limits).map_err(memory_error)?;
    let materialization = materialize_pe32_image(
        &mut memory,
        snapshot,
        loader_report,
        GuestAddress(load_base),
    )?;
    let trap_unsupported_imports = arguments.trap_unsupported_imports;
    let catalog = RuntimeExportCatalog::new(arguments.runtime_exports)?;
    let binding = if trap_unsupported_imports {
        bind_pe32_imports_with_unsupported_traps(
            &mut memory,
            loader_report,
            &materialization,
            &catalog,
        )?
    } else {
        bind_pe32_imports(&mut memory, loader_report, &materialization, &catalog)?
    };
    if !binding.complete || !binding.image_owner_sealed {
        return Err(LoaderError::invalid_input(
            "memory-reflection",
            "image-import-binding-incomplete",
            "v86 reflection requires an image whose imports and owner are fully bound and sealed",
        ));
    }
    let first_thunk = binding.thunks.first().cloned();
    if first_thunk.is_none() && binding.total_imports != 0 {
        return Err(LoaderError::runtime_defect(
            "memory-reflection",
            "complete-import-image-has-no-owned-thunk",
            "a fully bound image with planned imports has no runtime-owned thunk",
        ));
    }
    let (callback_return_sentinel, process_entry_return_sentinel) =
        if arguments.install_process_entry_return_sentinel {
            let sentinels = install_runtime_return_sentinels(&mut memory).map_err(memory_error)?;
            (
                Some(sentinels.callback_return),
                Some(sentinels.process_entry_return),
            )
        } else {
            (
                arguments
                    .install_callback_return_sentinel
                    .then(|| install_callback_return_sentinel(&mut memory).map_err(memory_error))
                    .transpose()?,
                None,
            )
        };
    let initial_thread_stack = if arguments.install_controlled_initial_stack {
        let entry_return = process_entry_return_sentinel.as_ref().ok_or_else(|| {
            LoaderError::runtime_defect(
                "process-bootstrap",
                "process-entry-return-sentinel-missing",
                "controlled initial stack requires process-entry return metadata",
            )
        })?;
        Some(install_controlled_initial_stack(&mut memory, entry_return).map_err(memory_error)?)
    } else {
        None
    };
    let initial_process_environment = if arguments.install_controlled_process_parameters {
        Some(
            install_controlled_process_environment_with_parameters(&mut memory)
                .map_err(memory_error)?,
        )
    } else {
        arguments
            .install_controlled_process_environment
            .then(|| install_controlled_process_environment(&mut memory).map_err(memory_error))
            .transpose()?
    };
    let expected_invalidation = initial_process_environment
        .as_ref()
        .map(|environment| environment.invalidation)
        .or_else(|| {
            initial_thread_stack
                .as_ref()
                .map(|stack| stack.invalidation)
        })
        .or_else(|| {
            process_entry_return_sentinel
                .as_ref()
                .map(|sentinel| sentinel.invalidation)
        })
        .or_else(|| {
            callback_return_sentinel
                .as_ref()
                .map(|sentinel| sentinel.invalidation)
        })
        .unwrap_or(binding.invalidation);
    let reflection = memory.pending_reflection().map_err(memory_error)?;
    if reflection.invalidation != expected_invalidation {
        return Err(LoaderError::runtime_defect(
            "memory-reflection",
            "runtime-reflection-generation-mismatch",
            "reflected memory does not carry the completed runtime-control generation",
        ));
    }
    let offer = V86ReflectionOffer {
        schema: "vine.v86-memory-reflection",
        schema_version: 1,
        source: loader_report.source.clone(),
        image_load_base: materialization.load_base,
        entry_point_address: materialization.entry_point_address,
        image_owner: binding.image_owner,
        first_thunk: first_thunk.clone(),
        thunks: binding.thunks.clone(),
        callback_return_sentinel: callback_return_sentinel.clone(),
        process_entry_return_sentinel: process_entry_return_sentinel.clone(),
        initial_thread_stack: initial_thread_stack.clone(),
        initial_process_environment: initial_process_environment.clone(),
        reflection,
    };
    write_protocol_message(&offer)?;

    let ack = read_v86_reflection_ack()?;
    validate_v86_reflection_ack(&ack, offer.reflection.invalidation)?;
    memory
        .acknowledge_invalidation(ack.generation)
        .map_err(memory_error)?;
    let iat_value = first_thunk
        .as_ref()
        .map(|thunk| {
            let iat_bytes = memory.read(thunk.iat_address, 4).map_err(memory_error)?;
            let iat_value = u32::from_le_bytes(iat_bytes.try_into().map_err(|_| {
                LoaderError::runtime_defect(
                    "memory-reflection",
                    "acknowledged-iat-width-invalid",
                    "acknowledged IAT read did not return four bytes",
                )
            })?);
            if iat_value != thunk.address.0 {
                return Err(LoaderError::runtime_defect(
                    "memory-reflection",
                    "acknowledged-iat-value-mismatch",
                    "acknowledged IAT no longer points to the first owned thunk",
                ));
            }
            Ok(iat_value)
        })
        .transpose()?;
    let entry_bytes = memory
        .fetch(materialization.entry_point_address, 16)
        .map_err(memory_error)?;
    let thunk_bytes = first_thunk
        .as_ref()
        .map(|thunk| memory.fetch(thunk.address, 16).map_err(memory_error))
        .transpose()?;
    let callback_return_bytes = callback_return_sentinel
        .as_ref()
        .map(|sentinel| memory.fetch(sentinel.address, 16).map_err(memory_error))
        .transpose()?;
    let process_entry_return_bytes = process_entry_return_sentinel
        .as_ref()
        .map(|sentinel| memory.fetch(sentinel.address, 16).map_err(memory_error))
        .transpose()?;
    let initial_stack_top_bytes = initial_thread_stack
        .as_ref()
        .map(|stack| {
            let address = GuestAddress(stack.stack_top.0.checked_sub(16).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "process-bootstrap",
                    "initial-stack-top-underflow",
                    "controlled initial stack cannot expose its top bytes",
                )
            })?);
            memory.read(address, 16).map_err(memory_error)
        })
        .transpose()?;
    let teb_bytes = initial_process_environment
        .as_ref()
        .map(|environment| {
            memory
                .read(environment.teb_address, 64)
                .map_err(memory_error)
        })
        .transpose()?;
    let peb_bytes = initial_process_environment
        .as_ref()
        .map(|environment| {
            memory
                .read(environment.peb_address, 64)
                .map_err(memory_error)
        })
        .transpose()?;
    let process_parameters_bytes = initial_process_environment
        .as_ref()
        .and_then(|environment| environment.process_parameters.as_ref())
        .map(|parameters| {
            memory
                .read(parameters.process_parameters_address, 0x48)
                .map_err(memory_error)
        })
        .transpose()?;
    let command_line_bytes = initial_process_environment
        .as_ref()
        .and_then(|environment| environment.process_parameters.as_ref())
        .map(|parameters| {
            memory
                .read(
                    parameters.command_line_buffer_address,
                    usize::from(parameters.command_line_maximum_length_bytes),
                )
                .map_err(memory_error)
        })
        .transpose()?;
    let process_parameters_installed = initial_process_environment
        .as_ref()
        .is_some_and(|environment| environment.process_parameters.is_some());
    let (reflection_stop_code, reflection_stop_message) = if first_thunk.is_none()
        && process_parameters_installed
    {
        (
            "entry-execution-pending",
            "the sealed import-free image, control page, initial stack, TEB, PEB, process parameters, and command-line pages are synchronized for adapter execution; guest entry execution has not yet been attempted",
        )
    } else if first_thunk.is_none() && initial_process_environment.is_some() {
        (
            "entry-execution-pending",
            "the sealed import-free image, control page, initial stack, TEB, and PEB are synchronized for adapter execution; guest entry execution has not yet been attempted",
        )
    } else if first_thunk.is_none() && initial_thread_stack.is_some() {
        (
            "entry-execution-pending",
            "the sealed import-free image, control page, and initial stack are synchronized for adapter execution; guest entry execution has not yet been attempted",
        )
    } else if first_thunk.is_none() {
        (
            "entry-execution-pending",
            "the sealed import-free image is synchronized for adapter execution; process control, initial stack setup, and guest entry execution have not yet been attempted",
        )
    } else if initial_thread_stack.is_some() {
        (
            "entry-to-first-owned-thunk-pending",
            "the production-owned image, thunk, control, and initial stack pages are synchronized for adapter execution; guest entry execution has not yet been attempted",
        )
    } else {
        (
            "entry-to-first-owned-thunk-pending",
            "the production-owned image and thunk pages are synchronized for adapter execution; initial stack setup and guest entry execution have not yet been attempted",
        )
    };
    let receipt = V86ReflectionReceipt {
        schema: "vine.v86-memory-reflection-receipt",
        schema_version: 1,
        acknowledged_generation: ack.generation,
        stats: memory.stats(),
        iat_value,
        entry_bytes,
        thunk_bytes,
        callback_return_bytes,
        process_entry_return_bytes,
        initial_stack_top_bytes,
        teb_bytes,
        peb_bytes,
        process_parameters_bytes,
        command_line_bytes,
        stop: UnsupportedPoint {
            stage: "adapter-execution",
            code: reflection_stop_code,
            message: reflection_stop_message,
        },
    };
    write_protocol_message(&receipt)
}

fn read_v86_reflection_ack() -> Result<V86ReflectionAck, LoaderError> {
    const MAX_ACK_BYTES: u64 = 4_096;
    let mut bytes = Vec::new();
    io::stdin()
        .take(MAX_ACK_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            io_error(
                "memory-reflection",
                "reflection-ack-read-failed",
                "could not read the adapter invalidation acknowledgement",
                error,
            )
        })?;
    if bytes.len() as u64 > MAX_ACK_BYTES {
        return Err(LoaderError::resource_limit(
            "memory-reflection",
            "reflection-ack-size-exceeded",
            "adapter invalidation acknowledgement exceeds 4 KiB",
        ));
    }
    serde_json::from_slice(&bytes).map_err(|error| {
        LoaderError::invalid_input(
            "memory-reflection",
            "reflection-ack-invalid",
            format!("adapter invalidation acknowledgement is not valid JSON: {error}"),
        )
    })
}

fn validate_v86_reflection_ack(
    ack: &V86ReflectionAck,
    pending: MemoryInvalidation,
) -> Result<(), LoaderError> {
    if ack.schema != "vine.v86-memory-reflection-ack" || ack.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "memory-reflection",
            "reflection-ack-schema-unsupported",
            "adapter acknowledgement must use vine.v86-memory-reflection-ack version 1",
        ));
    }
    if ack.generation != pending.generation {
        return Err(LoaderError::invalid_input(
            "memory-reflection",
            "reflection-ack-generation-mismatch",
            "adapter acknowledgement does not match the pending memory generation",
        ));
    }
    if (pending.data_tlb && !ack.data_tlb)
        || (pending.instruction_fetch && !ack.instruction_fetch)
        || (pending.generated_code && !ack.generated_code)
    {
        return Err(LoaderError::invalid_input(
            "memory-reflection",
            "reflection-ack-incomplete",
            "adapter acknowledgement did not complete every required invalidation class",
        ));
    }
    Ok(())
}

fn write_protocol_message(message: &impl Serialize) -> Result<(), LoaderError> {
    let mut stdout = io::stdout().lock();
    serde_json::to_writer(&mut stdout, message).map_err(|error| {
        LoaderError::runtime_defect(
            "memory-reflection",
            "reflection-message-serialization-failed",
            format!("could not serialize the reflection protocol message: {error}"),
        )
    })?;
    stdout.write_all(b"\n").map_err(|error| {
        io_error(
            "memory-reflection",
            "reflection-message-write-failed",
            "could not write the reflection protocol message",
            error,
        )
    })?;
    stdout.flush().map_err(|error| {
        io_error(
            "memory-reflection",
            "reflection-message-flush-failed",
            "could not flush the reflection protocol message",
            error,
        )
    })
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
    let mut runtime_exports = Vec::new();
    let mut trap_unsupported_imports = false;
    let mut v86_reflection_session = false;
    let mut install_callback_return_sentinel = false;
    let mut install_process_entry_return_sentinel = false;
    let mut install_controlled_initial_stack = false;
    let mut install_controlled_process_environment = false;
    let mut install_controlled_process_parameters = false;
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
        } else if argument == "--bind-name" {
            let module = arguments.next().ok_or_else(usage_error)?;
            let module = module.into_string().map_err(|_| usage_error())?;
            let name = arguments.next().ok_or_else(usage_error)?;
            let name = name.into_string().map_err(|_| usage_error())?;
            runtime_exports.push(RuntimeExport::named(module, name));
        } else if argument == "--bind-ordinal" {
            let module = arguments.next().ok_or_else(usage_error)?;
            let module = module.into_string().map_err(|_| usage_error())?;
            let ordinal = arguments.next().ok_or_else(usage_error)?;
            let ordinal = ordinal.into_string().map_err(|_| usage_error())?;
            let ordinal = ordinal.parse::<u16>().map_err(|_| usage_error())?;
            runtime_exports.push(RuntimeExport::ordinal(module, ordinal));
        } else if argument == "--trap-unsupported-imports" {
            if trap_unsupported_imports {
                return Err(usage_error());
            }
            trap_unsupported_imports = true;
        } else if argument == "--v86-reflection-session" {
            if v86_reflection_session {
                return Err(usage_error());
            }
            v86_reflection_session = true;
        } else if argument == "--install-callback-return-sentinel" {
            if install_callback_return_sentinel {
                return Err(usage_error());
            }
            install_callback_return_sentinel = true;
        } else if argument == "--install-process-entry-return-sentinel" {
            if install_process_entry_return_sentinel {
                return Err(usage_error());
            }
            install_process_entry_return_sentinel = true;
        } else if argument == "--install-controlled-initial-stack" {
            if install_controlled_initial_stack {
                return Err(usage_error());
            }
            install_controlled_initial_stack = true;
        } else if argument == "--install-controlled-process-environment" {
            if install_controlled_process_environment {
                return Err(usage_error());
            }
            install_controlled_process_environment = true;
        } else if argument == "--install-controlled-process-parameters" {
            if install_controlled_process_parameters {
                return Err(usage_error());
            }
            install_controlled_process_parameters = true;
        } else if argument.to_string_lossy().starts_with('-') || input.is_some() {
            return Err(usage_error());
        } else {
            input = Some(PathBuf::from(argument));
        }
    }

    if materialize_image_base.is_some() && prepare_memory_pages.is_none() {
        return Err(usage_error());
    }
    if !runtime_exports.is_empty() && materialize_image_base.is_none() {
        return Err(usage_error());
    }
    if trap_unsupported_imports && materialize_image_base.is_none() {
        return Err(usage_error());
    }
    if v86_reflection_session
        && (output.is_some() || materialize_image_base.is_none() || prepare_memory_pages.is_none())
    {
        return Err(usage_error());
    }
    if install_callback_return_sentinel && !v86_reflection_session {
        return Err(usage_error());
    }
    if install_process_entry_return_sentinel
        && (!v86_reflection_session || !install_callback_return_sentinel)
    {
        return Err(usage_error());
    }
    if install_controlled_initial_stack
        && (!v86_reflection_session || !install_process_entry_return_sentinel)
    {
        return Err(usage_error());
    }
    if install_controlled_process_environment && !install_controlled_initial_stack {
        return Err(usage_error());
    }
    if install_controlled_process_parameters && !install_controlled_process_environment {
        return Err(usage_error());
    }

    Ok(Arguments {
        input: input.ok_or_else(usage_error)?,
        output,
        expected_sha256,
        prepare_memory_pages,
        materialize_image_base,
        runtime_exports,
        trap_unsupported_imports,
        v86_reflection_session,
        install_callback_return_sentinel,
        install_process_entry_return_sentinel,
        install_controlled_initial_stack,
        install_controlled_process_environment,
        install_controlled_process_parameters,
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
        message: "usage: vine-pe-probe [--expect-sha256 HEX] [--prepare-memory-pages COUNT [--materialize-image-base ADDRESS [--bind-name MODULE SYMBOL | --bind-ordinal MODULE ORDINAL | --trap-unsupported-imports]...]] [--v86-reflection-session [--install-callback-return-sentinel [--install-process-entry-return-sentinel [--install-controlled-initial-stack [--install-controlled-process-environment [--install-controlled-process-parameters]]]]]] | --output FILE] INPUT".to_owned(),
    }
}

fn is_false(value: &bool) -> bool {
    !*value
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
