#![forbid(unsafe_code)]

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use vine_core::{
    CallbackReturnSentinel, GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryErrorClass,
    GuestPageReconciliation, GuestWriteResult, ImageMaterialization, ImageMemoryLayout,
    ImportBinding, ImportTarget, ImportThunk, ImportThunkDisposition, InitialProcessEnvironment,
    InitialThreadStack, LoaderError, LoaderErrorClass, LoaderIngressReport, MemoryDelta,
    MemoryInvalidation, MemoryLimits, MemoryOwnerId, MemoryReflection, MemoryStats, ModuleIdentity,
    ModuleSnapshot, Pe32Limits, ProcessEntryReturnSentinel, RuntimeExport, RuntimeExportCatalog,
    UnsupportedPoint, bind_pe32_imports, bind_pe32_imports_with_unsupported_traps,
    build_pe32_load_plan, install_callback_return_sentinel, install_controlled_initial_stack,
    install_controlled_process_environment,
    install_controlled_process_environment_with_command_line,
    install_controlled_process_environment_with_parameters, install_runtime_return_sentinels,
    materialize_pe32_image,
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
    system_time_delta_session: bool,
    process_id_return_session: bool,
    thread_id_return_session: bool,
    tick_count_return_session: bool,
    large_image_reflection: bool,
    install_callback_return_sentinel: bool,
    install_process_entry_return_sentinel: bool,
    install_controlled_initial_stack: bool,
    install_controlled_process_environment: bool,
    install_controlled_process_parameters: bool,
    controlled_command_line: Option<String>,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86SystemTimeWriteRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    output_address: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    clock_owner: String,
    clock_sample_id: String,
    clock_utc_iso: String,
    file_time_low: u32,
    file_time_high: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86SystemTimeDeltaOffer {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    output_address: GuestAddress,
    clock_owner: String,
    clock_sample_id: String,
    clock_utc_iso: String,
    file_time_ticks: String,
    file_time_low: u32,
    file_time_high: u32,
    stack_reconciliation: GuestPageReconciliation,
    write: GuestWriteResult,
    delta: MemoryDelta,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86MemoryDeltaAck {
    schema: String,
    schema_version: u32,
    generation: u64,
    data_tlb: bool,
    instruction_fetch: bool,
    generated_code: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86SystemTimeDeltaReceipt {
    schema: &'static str,
    schema_version: u32,
    acknowledged_generation: u64,
    output_address: GuestAddress,
    output_bytes: Vec<u8>,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86ProcessIdReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    process_identity_owner: String,
    process_id: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86ProcessIdReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    process_identity_owner: String,
    process_id: u32,
    stack_reconciliation: GuestPageReconciliation,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86ThreadIdReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    thread_identity_owner: String,
    thread_id: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86ThreadIdReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    thread_identity_owner: String,
    thread_id: u32,
    stack_reconciliation: GuestPageReconciliation,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86TickCountReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    monotonic_clock_owner: String,
    sample_id: String,
    paired_wall_clock_sample_id: String,
    tick_count: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86TickCountReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    monotonic_clock_owner: String,
    sample_id: String,
    paired_wall_clock_sample_id: String,
    tick_count: u32,
    stack_reconciliation: GuestPageReconciliation,
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
    const MAX_LARGE_IMAGE_REFLECTION_PAGES: usize = 400;
    const MAX_REFLECTION_MESSAGE_BYTES: usize = 512 * 1024;
    const MAX_LARGE_IMAGE_REFLECTION_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
    let load_base = arguments.materialize_image_base.ok_or_else(usage_error)?;
    let max_physical_pages = arguments.prepare_memory_pages.ok_or_else(usage_error)?;
    let reflection_page_limit = if arguments.large_image_reflection {
        MAX_LARGE_IMAGE_REFLECTION_PAGES
    } else {
        MAX_REFLECTION_PAGES
    };
    if max_physical_pages > reflection_page_limit {
        let message = if arguments.large_image_reflection {
            "the version 1 large-image reflection session is limited to 400 physical pages"
        } else {
            "the version 1 reflection session is limited to 16 physical pages"
        };
        return Err(LoaderError::resource_limit(
            "memory-reflection",
            "reflection-page-limit-exceeded",
            message,
        ));
    }
    let reflection_message_limit = if arguments.large_image_reflection {
        MAX_LARGE_IMAGE_REFLECTION_MESSAGE_BYTES
    } else {
        MAX_REFLECTION_MESSAGE_BYTES
    };
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
            if let Some(command_line) = arguments.controlled_command_line.as_deref() {
                install_controlled_process_environment_with_command_line(&mut memory, command_line)
                    .map_err(memory_error)?
            } else {
                install_controlled_process_environment_with_parameters(&mut memory)
                    .map_err(memory_error)?
            },
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
    write_protocol_message(&offer, reflection_message_limit)?;

    let stdin = io::stdin();
    let mut protocol_input = stdin.lock();
    let ack = read_v86_reflection_ack(&mut protocol_input)?;
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
    write_protocol_message(&receipt, reflection_message_limit)?;
    if arguments.system_time_delta_session {
        run_v86_system_time_delta_session(
            &mut protocol_input,
            &mut memory,
            &binding,
            reflection_message_limit,
            arguments.process_id_return_session,
            arguments.thread_id_return_session,
            arguments.tick_count_return_session,
        )?;
    }
    Ok(())
}

fn read_v86_reflection_ack(input: &mut impl BufRead) -> Result<V86ReflectionAck, LoaderError> {
    read_protocol_message(
        input,
        4 * 1024,
        "memory-reflection",
        "reflection-ack",
        "adapter invalidation acknowledgement",
    )
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

fn run_v86_system_time_delta_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    binding: &ImportBinding,
    message_limit: usize,
    process_id_return_session: bool,
    thread_id_return_session: bool,
    tick_count_return_session: bool,
) -> Result<(), LoaderError> {
    let request: V86SystemTimeWriteRequest = read_protocol_message(
        input,
        64 * 1024,
        "win32-system-time",
        "system-time-write-request",
        "controlled system-time write request",
    )?;
    if request.schema != "vine.v86-system-time-write-request" || request.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "win32-system-time",
            "system-time-write-request-schema-unsupported",
            "system-time write request must use vine.v86-system-time-write-request version 1",
        ));
    }
    if request.argument_bytes != 4 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "system-time-argument-width-mismatch",
            "GetSystemTimeAsFileTime requires one four-byte stdcall argument",
        ));
    }
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "system-time-thunk-id-unowned",
                "system-time write request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "GetSystemTimeAsFileTime"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "system-time-thunk-identity-mismatch",
            "system-time write request does not match the declared GetSystemTimeAsFileTime thunk",
        ));
    }

    let file_time_ticks =
        (u64::from(request.file_time_high) << 32) | u64::from(request.file_time_low);
    let file_time_bytes = file_time_ticks.to_le_bytes();
    let (stack_reconciliation, write, delta) = {
        let mut transaction = memory.begin_transaction();
        let stack_reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;

        validate_stdcall_iat_frame(
            &transaction,
            &thunk,
            request.stack_pointer,
            request.return_address,
            &[request.output_address],
        )?;

        let write = transaction
            .write(GuestAddress(request.output_address), &file_time_bytes)
            .map_err(memory_error)?;
        let delta = transaction.pending_delta().map_err(memory_error)?;
        if delta.from_generation != request.reflected_generation
            || delta.dirty_pages.len() != 1
            || delta.dirty_pages[0].address.0 != request.stack_page_address
            || delta.dirty_pages[0].owner.0 != request.stack_page_owner
            || !delta.mapping_operations.is_empty()
            || delta.cache_effects.flush_tlb
            || delta.cache_effects.invalidate_instruction_fetch
            || delta.cache_effects.invalidate_generated_code
        {
            return Err(LoaderError::runtime_defect(
                "memory-delta",
                "system-time-delta-shape-invalid",
                "checked FILETIME write did not produce one data-only stack-page delta",
            ));
        }
        transaction.commit_transaction();
        (stack_reconciliation, write, delta)
    };

    let delta_generation = delta.generation;
    write_protocol_message(
        &V86SystemTimeDeltaOffer {
            schema: "vine.v86-system-time-delta",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            output_address: GuestAddress(request.output_address),
            clock_owner: request.clock_owner,
            clock_sample_id: request.clock_sample_id,
            clock_utc_iso: request.clock_utc_iso,
            file_time_ticks: file_time_ticks.to_string(),
            file_time_low: request.file_time_low,
            file_time_high: request.file_time_high,
            stack_reconciliation,
            write,
            delta,
        },
        message_limit,
    )?;

    let ack: V86MemoryDeltaAck = read_protocol_message(
        input,
        4 * 1024,
        "memory-delta",
        "system-time-delta-ack",
        "system-time memory-delta acknowledgement",
    )?;
    if ack.schema != "vine.v86-memory-delta-ack"
        || ack.schema_version != 1
        || ack.generation != delta_generation
        || ack.data_tlb
        || ack.instruction_fetch
        || ack.generated_code
    {
        return Err(LoaderError::invalid_input(
            "memory-delta",
            "system-time-delta-ack-invalid",
            "system-time delta acknowledgement does not match its generation and cache effects",
        ));
    }
    memory
        .acknowledge_invalidation(ack.generation)
        .map_err(memory_error)?;
    let output_bytes = memory
        .read(GuestAddress(request.output_address), file_time_bytes.len())
        .map_err(memory_error)?;
    write_protocol_message(
        &V86SystemTimeDeltaReceipt {
            schema: "vine.v86-system-time-delta-receipt",
            schema_version: 1,
            acknowledged_generation: ack.generation,
            output_address: GuestAddress(request.output_address),
            output_bytes,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if process_id_return_session {
        run_v86_process_id_return_session(
            input,
            memory,
            binding,
            message_limit,
            thread_id_return_session,
            tick_count_return_session,
        )?;
    }
    Ok(())
}

fn run_v86_process_id_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    binding: &ImportBinding,
    message_limit: usize,
    thread_id_return_session: bool,
    tick_count_return_session: bool,
) -> Result<(), LoaderError> {
    let request: V86ProcessIdReturnRequest = read_protocol_message(
        input,
        64 * 1024,
        "win32-process-identity",
        "process-id-return-request",
        "controlled process-ID return request",
    )?;
    if request.schema != "vine.v86-process-id-return-request" || request.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "win32-process-identity",
            "process-id-return-request-schema-unsupported",
            "process-ID request must use vine.v86-process-id-return-request version 1",
        ));
    }
    if request.argument_bytes != 0 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "process-id-argument-width-mismatch",
            "GetCurrentProcessId requires a zero-argument stdcall frame",
        ));
    }
    if request.process_identity_owner.is_empty() || request.process_id == 0 {
        return Err(LoaderError::invalid_input(
            "win32-process-identity",
            "controlled-process-identity-invalid",
            "controlled process identity requires a nonempty owner and nonzero process ID",
        ));
    }
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "process-id-thunk-id-unowned",
                "process-ID request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "GetCurrentProcessId"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "process-id-thunk-identity-mismatch",
            "process-ID request does not match the declared GetCurrentProcessId thunk",
        ));
    }

    let stack_reconciliation = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_stdcall_iat_frame(
            &transaction,
            &thunk,
            request.stack_pointer,
            request.return_address,
            &[],
        )?;
        transaction.commit_transaction();
        reconciliation
    };

    write_protocol_message(
        &V86ProcessIdReturnReceipt {
            schema: "vine.v86-process-id-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            process_identity_owner: request.process_identity_owner,
            process_id: request.process_id,
            stack_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if thread_id_return_session {
        run_v86_thread_id_return_session(
            input,
            memory,
            binding,
            message_limit,
            tick_count_return_session,
        )?;
    }
    Ok(())
}

fn run_v86_thread_id_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    binding: &ImportBinding,
    message_limit: usize,
    tick_count_return_session: bool,
) -> Result<(), LoaderError> {
    let request: V86ThreadIdReturnRequest = read_protocol_message(
        input,
        64 * 1024,
        "win32-thread-identity",
        "thread-id-return-request",
        "controlled thread-ID return request",
    )?;
    if request.schema != "vine.v86-thread-id-return-request" || request.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "win32-thread-identity",
            "thread-id-return-request-schema-unsupported",
            "thread-ID request must use vine.v86-thread-id-return-request version 1",
        ));
    }
    if request.argument_bytes != 0 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "thread-id-argument-width-mismatch",
            "GetCurrentThreadId requires a zero-argument stdcall frame",
        ));
    }
    if request.thread_identity_owner.is_empty() || request.thread_id == 0 {
        return Err(LoaderError::invalid_input(
            "win32-thread-identity",
            "controlled-thread-identity-invalid",
            "controlled thread identity requires a nonempty owner and nonzero thread ID",
        ));
    }
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "thread-id-thunk-id-unowned",
                "thread-ID request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "GetCurrentThreadId"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "thread-id-thunk-identity-mismatch",
            "thread-ID request does not match the declared GetCurrentThreadId thunk",
        ));
    }

    let stack_reconciliation = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_stdcall_iat_frame(
            &transaction,
            &thunk,
            request.stack_pointer,
            request.return_address,
            &[],
        )?;
        transaction.commit_transaction();
        reconciliation
    };

    write_protocol_message(
        &V86ThreadIdReturnReceipt {
            schema: "vine.v86-thread-id-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            thread_identity_owner: request.thread_identity_owner,
            thread_id: request.thread_id,
            stack_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if tick_count_return_session {
        run_v86_tick_count_return_session(input, memory, binding, message_limit)?;
    }
    Ok(())
}

fn run_v86_tick_count_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    binding: &ImportBinding,
    message_limit: usize,
) -> Result<(), LoaderError> {
    let request: V86TickCountReturnRequest = read_protocol_message(
        input,
        64 * 1024,
        "win32-monotonic-clock",
        "tick-count-return-request",
        "controlled tick-count return request",
    )?;
    if request.schema != "vine.v86-tick-count-return-request" || request.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "win32-monotonic-clock",
            "tick-count-return-request-schema-unsupported",
            "tick-count request must use vine.v86-tick-count-return-request version 1",
        ));
    }
    if request.argument_bytes != 0 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "tick-count-argument-width-mismatch",
            "GetTickCount requires a zero-argument stdcall frame",
        ));
    }
    if request.monotonic_clock_owner.is_empty()
        || request.sample_id.is_empty()
        || request.paired_wall_clock_sample_id.is_empty()
    {
        return Err(LoaderError::invalid_input(
            "win32-monotonic-clock",
            "controlled-tick-count-sample-invalid",
            "controlled tick-count sample requires nonempty clock ownership and sample identities",
        ));
    }
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "tick-count-thunk-id-unowned",
                "tick-count request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "GetTickCount"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "tick-count-thunk-identity-mismatch",
            "tick-count request does not match the declared GetTickCount thunk",
        ));
    }

    let stack_reconciliation = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_stdcall_iat_frame(
            &transaction,
            &thunk,
            request.stack_pointer,
            request.return_address,
            &[],
        )?;
        transaction.commit_transaction();
        reconciliation
    };

    write_protocol_message(
        &V86TickCountReturnReceipt {
            schema: "vine.v86-tick-count-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            monotonic_clock_owner: request.monotonic_clock_owner,
            sample_id: request.sample_id,
            paired_wall_clock_sample_id: request.paired_wall_clock_sample_id,
            tick_count: request.tick_count,
            stack_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )
}

fn validate_stdcall_iat_frame(
    memory: &GuestMemory,
    thunk: &ImportThunk,
    stack_pointer: u32,
    return_address: u32,
    expected_arguments: &[u32],
) -> Result<(), LoaderError> {
    let frame_words = expected_arguments.len().checked_add(1).ok_or_else(|| {
        LoaderError::resource_limit(
            "abi-gateway",
            "stdcall-frame-word-count-overflow",
            "stdcall frame word count overflowed host accounting",
        )
    })?;
    let frame_bytes = frame_words.checked_mul(4).ok_or_else(|| {
        LoaderError::resource_limit(
            "abi-gateway",
            "stdcall-frame-byte-count-overflow",
            "stdcall frame byte count overflowed host accounting",
        )
    })?;
    let frame = memory
        .read(GuestAddress(stack_pointer), frame_bytes)
        .map_err(memory_error)?;
    let observed_words = frame
        .chunks_exact(4)
        .map(|word| {
            let word = word.try_into().map_err(|_| {
                LoaderError::runtime_defect(
                    "abi-gateway",
                    "stdcall-frame-word-width-invalid",
                    "checked stdcall frame contained a partial word",
                )
            })?;
            Ok(u32::from_le_bytes(word))
        })
        .collect::<Result<Vec<_>, LoaderError>>()?;
    if observed_words.first().copied() != Some(return_address)
        || observed_words.get(1..) != Some(expected_arguments)
    {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "stdcall-frame-mismatch",
            "reconciled stdcall frame does not match the adapter-observed return and arguments",
        ));
    }

    let call_site = return_address.checked_sub(6).ok_or_else(|| {
        LoaderError::invalid_input(
            "abi-gateway",
            "stdcall-return-address-underflow",
            "stdcall return address cannot identify a six-byte indirect call",
        )
    })?;
    let call = memory
        .fetch(GuestAddress(call_site), 6)
        .map_err(memory_error)?;
    let iat_operand = u32::from_le_bytes(call[2..6].try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "abi-gateway",
            "stdcall-call-width-invalid",
            "checked import call did not contain a four-byte IAT operand",
        )
    })?);
    if call[0..2] != [0xff, 0x15] || iat_operand != thunk.iat_address.0 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "stdcall-callsite-mismatch",
            "stdcall return address does not follow its expected indirect IAT call",
        ));
    }
    let iat = memory.read(thunk.iat_address, 4).map_err(memory_error)?;
    let iat_value = u32::from_le_bytes(iat.try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "abi-gateway",
            "stdcall-iat-width-invalid",
            "checked stdcall IAT entry did not contain four bytes",
        )
    })?);
    if iat_value != thunk.address.0 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "stdcall-iat-value-mismatch",
            "stdcall IAT entry no longer points to its owned thunk",
        ));
    }
    Ok(())
}

fn read_protocol_message<T: DeserializeOwned>(
    input: &mut impl BufRead,
    maximum_bytes: usize,
    stage: &'static str,
    code: &'static str,
    label: &'static str,
) -> Result<T, LoaderError> {
    let mut bytes = Vec::new();
    input
        .take((maximum_bytes + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .map_err(|error| io_error(stage, "protocol-message-read-failed", label, error))?;
    if bytes.is_empty() {
        return Err(LoaderError::invalid_input(
            stage,
            "protocol-message-missing",
            format!("{label} was not provided"),
        ));
    }
    if bytes.len() > maximum_bytes {
        return Err(LoaderError::resource_limit(
            stage,
            "protocol-message-size-exceeded",
            format!("{label} exceeds its {maximum_bytes}-byte limit"),
        ));
    }
    serde_json::from_slice(&bytes).map_err(|error| {
        LoaderError::invalid_input(stage, code, format!("{label} is not valid JSON: {error}"))
    })
}

fn write_protocol_message(
    message: &impl Serialize,
    maximum_bytes: usize,
) -> Result<(), LoaderError> {
    let bytes = serde_json::to_vec(message).map_err(|error| {
        LoaderError::runtime_defect(
            "memory-reflection",
            "reflection-message-serialization-failed",
            format!("could not serialize the reflection protocol message: {error}"),
        )
    })?;
    if bytes.len() > maximum_bytes {
        return Err(LoaderError::resource_limit(
            "memory-reflection",
            "reflection-message-size-exceeded",
            format!(
                "reflection protocol message has {} bytes; limit is {maximum_bytes} bytes",
                bytes.len()
            ),
        ));
    }
    let mut stdout = io::stdout().lock();
    stdout.write_all(&bytes).map_err(|error| {
        io_error(
            "memory-reflection",
            "reflection-message-write-failed",
            "could not write the reflection protocol message",
            error,
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
    let mut system_time_delta_session = false;
    let mut process_id_return_session = false;
    let mut thread_id_return_session = false;
    let mut tick_count_return_session = false;
    let mut large_image_reflection = false;
    let mut install_callback_return_sentinel = false;
    let mut install_process_entry_return_sentinel = false;
    let mut install_controlled_initial_stack = false;
    let mut install_controlled_process_environment = false;
    let mut install_controlled_process_parameters = false;
    let mut controlled_command_line = None;
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
        } else if argument == "--system-time-delta-session" {
            if system_time_delta_session {
                return Err(usage_error());
            }
            system_time_delta_session = true;
        } else if argument == "--process-id-return-session" {
            if process_id_return_session {
                return Err(usage_error());
            }
            process_id_return_session = true;
        } else if argument == "--thread-id-return-session" {
            if thread_id_return_session {
                return Err(usage_error());
            }
            thread_id_return_session = true;
        } else if argument == "--tick-count-return-session" {
            if tick_count_return_session {
                return Err(usage_error());
            }
            tick_count_return_session = true;
        } else if argument == "--large-image-reflection" {
            if large_image_reflection {
                return Err(usage_error());
            }
            large_image_reflection = true;
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
        } else if argument == "--controlled-command-line" {
            let value = arguments.next().ok_or_else(usage_error)?;
            let value = value.into_string().map_err(|_| usage_error())?;
            if controlled_command_line.replace(value).is_some() {
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
    if large_image_reflection && !v86_reflection_session {
        return Err(usage_error());
    }
    if system_time_delta_session && !v86_reflection_session {
        return Err(usage_error());
    }
    if process_id_return_session && !system_time_delta_session {
        return Err(usage_error());
    }
    if thread_id_return_session && !process_id_return_session {
        return Err(usage_error());
    }
    if tick_count_return_session && !thread_id_return_session {
        return Err(usage_error());
    }
    if controlled_command_line.is_some() && !install_controlled_process_parameters {
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
        system_time_delta_session,
        process_id_return_session,
        thread_id_return_session,
        tick_count_return_session,
        large_image_reflection,
        install_callback_return_sentinel,
        install_process_entry_return_sentinel,
        install_controlled_initial_stack,
        install_controlled_process_environment,
        install_controlled_process_parameters,
        controlled_command_line,
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
        message: "usage: vine-pe-probe [--expect-sha256 HEX] [--prepare-memory-pages COUNT [--materialize-image-base ADDRESS [--bind-name MODULE SYMBOL | --bind-ordinal MODULE ORDINAL | --trap-unsupported-imports]...]] [--v86-reflection-session [--system-time-delta-session [--process-id-return-session [--thread-id-return-session [--tick-count-return-session]]]] [--large-image-reflection] [--install-callback-return-sentinel [--install-process-entry-return-sentinel [--install-controlled-initial-stack [--install-controlled-process-environment [--install-controlled-process-parameters [--controlled-command-line TEXT]]]]]]] | --output FILE] INPUT".to_owned(),
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
