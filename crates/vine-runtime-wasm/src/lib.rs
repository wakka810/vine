#![deny(unsafe_op_in_unsafe_fn)]

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use encoding_rs::SHIFT_JIS;
use serde_json::{Value, json};
pub use vine_core::X86CpuContext;
use vine_core::directshow::{
    ComMediaAudio, ComMediaDeferred, ComMediaFormat, ComMediaFrame, ComMediaOperation,
    ComMediaResponse, ComMediaSource,
};
use vine_core::{
    ApiDeferredCall, ApiDispatcher, ApiEndpointIdentity, ApiFault, ApiGuestCallbackCall,
    ApiImmediate, ApiImmediateCompletion, ApiOwnedTrap, ComAudioDeferred, ComCallTrap,
    ComDispatchOutcome, DispatchOutcome, Gdi32TextRasterHostResponse, GuestAddress, GuestMemory,
    HostCompletion, ImportTarget, MemoryDeltaBuffers, MemoryLimits, MemoryRegionKind,
    ModuleSnapshot, OpfsFileOperation, Pe32Limits, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationRecord, RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE,
    RUNTIME_GDI_TEXT_RASTER_SERVICE, RUNTIME_MESSAGE_BOX_SERVICE, RUNTIME_OBJECT_WAIT_SERVICE,
    RUNTIME_OPFS_READ_SERVICE, RUNTIME_WAITABLE_TIMER_PACING_SERVICE, Runtime, RuntimeBootstrap,
    RuntimeClockState, RuntimeCursorPositionProfile, RuntimeDisplayProfile,
    RuntimeDllLifecycleAction, RuntimeDllLifecyclePhase, RuntimeExport, RuntimeExportCatalog,
    RuntimeGuestImportBinding, RuntimeInputProfile, RuntimeKeyboardSourceEvent,
    RuntimeKeyboardSourceEventKind, RuntimeMessageBoxIcon, RuntimeModule, RuntimePointerButton,
    RuntimePointerButtonSourceEvent, RuntimePointerButtonSourceEventKind,
    RuntimePointerPositionSourceEvent, RuntimeProcessId, RuntimeRect, RuntimeSchedulerEvent,
    RuntimeSourceFileManifest, RuntimeSourceMountManifest, RuntimeSourceReadHostResponse,
    RuntimeSourceSnapshotManifest, RuntimeStartupInfo, RuntimeThreadId, SchedulerThreadState,
    build_pe32_load_plan, complete_gdi32_text_raster, complete_kernel32_audio_notification_wait,
    complete_kernel32_object_wait, complete_kernel32_opfs_open, complete_kernel32_opfs_write,
    complete_kernel32_source_read, complete_kernel32_waitable_timer_pacing,
    complete_opfs_file_operation, complete_user32_message_box, install_initial_thread_stack,
    install_runtime_return_sentinels, materialize_pe32_image,
};

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
// User-requested 1.5 GiB guest RAM allowance for full traversal and save compression.
const MAX_GAME_PHYSICAL_PAGES: usize = 384 * 1024;
const SYSTEM_KERNEL32_HANDLE: u32 = 0x4d01_0001;

thread_local! {
    // Rust 1.97 reports this macro expansion despite the explicit const initializer.
    #[allow(clippy::missing_const_for_thread_local)]
    static BRIDGE: RefCell<Bridge> = const { RefCell::new(Bridge::new()) };
}

struct SourceDll {
    name: String,
    relative_path: String,
    snapshot: ModuleSnapshot,
    loader: vine_core::Pe32LoadPlan,
}

struct MappedSourceDll {
    source: SourceDll,
    image: vine_core::ImageMaterialization,
    source_id: u32,
}

enum DispatchMetadata {
    Json(Value),
    ApiCompletion([u32; 4]),
}

#[derive(Debug)]
enum RuntimeOutputBytes {
    Owned(Vec<u8>),
    Shared(Arc<Vec<u8>>),
}

impl RuntimeOutputBytes {
    const fn new() -> Self {
        Self::Owned(Vec::new())
    }

    fn clear(&mut self) {
        match self {
            Self::Owned(bytes) => bytes.clear(),
            Self::Shared(_) => *self = Self::new(),
        }
    }

    fn into_owned(self) -> Vec<u8> {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Shared(bytes) => Arc::unwrap_or_clone(bytes),
        }
    }

    fn append_snapshot(&mut self, snapshot: &Arc<Vec<u8>>) -> usize {
        let offset = self.len();
        if snapshot.is_empty() {
            return offset;
        }
        match self {
            Self::Owned(bytes) if bytes.is_empty() => {
                // Sharing the Vec owner preserves its existing pixel allocation.
                *self = Self::Shared(Arc::clone(snapshot));
            }
            Self::Owned(bytes) => bytes.extend_from_slice(snapshot),
            Self::Shared(first) => {
                // Multiple payloads retain the existing contiguous transport ABI.
                let mut bytes = Vec::with_capacity(first.len() + snapshot.len());
                bytes.extend_from_slice(first);
                bytes.extend_from_slice(snapshot);
                *self = Self::Owned(bytes);
            }
        }
        offset
    }
}

impl std::ops::Deref for RuntimeOutputBytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Shared(bytes) => bytes,
        }
    }
}

impl From<Vec<u8>> for RuntimeOutputBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Owned(bytes)
    }
}

fn append_presentation_pixels(
    bytes: &mut RuntimeOutputBytes,
    snapshot: &Arc<Vec<u8>>,
    pitch: Option<u32>,
) -> Value {
    if snapshot.is_empty() {
        Value::Null
    } else {
        let offset = bytes.append_snapshot(snapshot);
        json!({ "offset": offset, "length": snapshot.len(),
            "pitch": pitch.expect("captured texture pitch") })
    }
}

struct Bridge {
    input: Vec<u8>,
    source_dlls: Vec<SourceDll>,
    metadata: Vec<u8>,
    api_completion: Option<[u32; 4]>,
    control: Vec<u8>,
    payload: Vec<u8>,
    presentation_bytes: RuntimeOutputBytes,
    audio_bytes: Vec<u8>,
    media_bytes: Vec<u8>,
    stop_context_metadata: Vec<u8>,
    writeback_generation: Option<u64>,
    runtime: Option<BrowserRuntime>,
}

impl Bridge {
    const fn new() -> Self {
        Self {
            input: Vec::new(),
            source_dlls: Vec::new(),
            metadata: Vec::new(),
            api_completion: None,
            control: Vec::new(),
            payload: Vec::new(),
            presentation_bytes: RuntimeOutputBytes::new(),
            audio_bytes: Vec::new(),
            media_bytes: Vec::new(),
            stop_context_metadata: Vec::new(),
            writeback_generation: None,
            runtime: None,
        }
    }

    fn clear_output(&mut self) {
        self.metadata.clear();
        self.api_completion = None;
        self.control.clear();
        self.payload.clear();
        self.presentation_bytes.clear();
        self.audio_bytes.clear();
        self.media_bytes.clear();
        self.writeback_generation = None;
    }

    #[cold]
    fn stop_context(&mut self, instruction_pointer: u32) -> Result<(), String> {
        let context = StopContext::read(self.runtime.as_ref(), instruction_pointer);
        self.stop_context_metadata = serde_json::to_vec(&json!({
            "kind": "stop-context",
            "threadId": context.thread_id,
            "instructionPointer": context.instruction_pointer,
            "lastError": context.last_error,
            "module": context.module,
            "symbol": context.symbol,
        }))
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn set_output(
        &mut self,
        metadata: Value,
        control: Vec<u8>,
        payload: Vec<u8>,
    ) -> Result<(), String> {
        self.api_completion = None;
        self.metadata = serde_json::to_vec(&metadata).map_err(|error| error.to_string())?;
        self.control = control;
        self.payload = payload;
        Ok(())
    }

    fn put_cpu_snapshot(
        &mut self,
        thread_id: u32,
        expected_revision: u64,
        backend_tag: u32,
        abi_version: u32,
        input_length: usize,
    ) -> Result<(), String> {
        if input_length != self.input.len() || input_length > 4096 {
            return Err(
                "CPU snapshot input must exactly match its bounded bridge buffer".to_owned(),
            );
        }
        let next_revision = expected_revision
            .checked_add(1)
            .ok_or_else(|| "CPU snapshot revision space is exhausted".to_owned())?;
        // Serialize the receipt before publishing the new Rust-owned snapshot.
        let metadata = serde_json::to_vec(&json!({
            "kind": "cpu-snapshot-stored", "threadId": thread_id,
            "revisionLow": next_revision as u32, "revisionHigh": (next_revision >> 32) as u32,
            "backendTag": backend_tag, "abiVersion": abi_version, "byteLength": input_length,
        }))
        .map_err(error_string)?;
        let runtime = &mut self
            .runtime
            .as_mut()
            .ok_or_else(|| "runtime is not prepared".to_owned())?
            .runtime;
        let revision = runtime
            .put_thread_cpu_snapshot(
                RuntimeThreadId(thread_id),
                expected_revision,
                backend_tag,
                abi_version,
                &self.input,
            )
            .map_err(error_string)?;
        debug_assert_eq!(revision, next_revision);
        self.metadata = metadata;
        Ok(())
    }

    fn get_cpu_snapshot(&mut self, thread_id: u32) -> Result<(), String> {
        let runtime = &self
            .runtime
            .as_ref()
            .ok_or_else(|| "runtime is not prepared".to_owned())?
            .runtime;
        let (backend_tag, abi_version, revision, bytes) = runtime
            .thread_cpu_snapshot(RuntimeThreadId(thread_id))
            .map_err(error_string)?;
        let metadata = serde_json::to_vec(&json!({
            "kind": "cpu-snapshot", "threadId": thread_id,
            "revisionLow": revision as u32, "revisionHigh": (revision >> 32) as u32,
            "backendTag": backend_tag, "abiVersion": abi_version, "byteLength": bytes.len(),
        }))
        .map_err(error_string)?;
        let mut payload = Vec::new();
        payload
            .try_reserve_exact(bytes.len())
            .map_err(|_| "CPU snapshot output allocation failed".to_owned())?;
        payload.extend_from_slice(bytes);
        self.metadata = metadata;
        self.payload = payload;
        Ok(())
    }

    fn set_error(&mut self, message: String) {
        self.api_completion = None;
        self.control.clear();
        self.payload.clear();
        self.presentation_bytes.clear();
        self.audio_bytes.clear();
        self.media_bytes.clear();
        self.metadata = serde_json::to_vec(&json!({
            "kind": "error",
            "message": message,
        }))
        .unwrap_or_else(|_| b"{\"kind\":\"error\",\"message\":\"serialization failed\"}".to_vec());
    }

    fn set_runtime_output(
        &mut self,
        metadata: Value,
        delta: Option<MemoryDeltaBuffers>,
        bytes: RuntimeOutputBytes,
    ) -> Result<(), String> {
        let (control, payload) = delta
            .map(|buffers| (buffers.control, buffers.page_bytes))
            .unwrap_or_default();
        if metadata["kind"] == "audio-request" {
            self.audio_bytes = self
                .runtime_mut()?
                .pending_com_audio
                .as_ref()
                .ok_or_else(|| "audio request has no pending command".to_owned())?
                .command_bytes()
                .to_vec();
        }
        if metadata["kind"] == "media-data" || metadata["kind"] == "media-request" {
            self.media_bytes = bytes.into_owned();
        } else {
            self.presentation_bytes = bytes;
        }
        self.set_output(metadata, control, payload)
    }

    fn inspect_executable(
        &mut self,
        executable_length: usize,
        name_length: usize,
    ) -> Result<(), String> {
        let limits = Pe32Limits::default();
        if executable_length as u64 > limits.max_file_bytes {
            return Err("executable exceeds the PE32 file limit".to_owned());
        }
        let total_length = executable_length
            .checked_add(name_length)
            .ok_or_else(|| "executable input length overflowed".to_owned())?;
        let input = self
            .input
            .get(..total_length)
            .ok_or_else(|| "executable input exceeds the bridge buffer".to_owned())?;
        let (bytes, name) = input.split_at(executable_length);
        let name = std::str::from_utf8(name)
            .map_err(|_| "selected executable filename is not UTF-8".to_owned())?;
        virtual_executable_path(name)?;
        let snapshot = ModuleSnapshot::from_bytes(bytes.to_vec(), &limits).map_err(error_string)?;
        let plan = build_pe32_load_plan(&snapshot, limits).map_err(error_string)?;
        if plan.image.kind != vine_core::ImageKind::Executable {
            return Err("selected PE32 image is a DLL, not an executable".to_owned());
        }
        self.set_output(
            json!({
                "sha256": snapshot.identity().sha256,
                "imageBase": plan.image.preferred_image_base,
                "entryPoint": plan.image.entry_point_address,
                "importModules": plan.imports.len(),
                "guestImports": guest_import_names(&plan),
            }),
            Vec::new(),
            Vec::new(),
        )
    }

    fn register_source_dll(
        &mut self,
        image_length: usize,
        path_length: usize,
    ) -> Result<(), String> {
        if self.source_dlls.len() >= 32 || image_length > 16 * 1024 * 1024 {
            return Err("source DLL count or byte length exceeds the bounded loader".to_owned());
        }
        let total = image_length
            .checked_add(path_length)
            .ok_or_else(|| "source DLL input length overflowed".to_owned())?;
        let input = self
            .input
            .get(..total)
            .ok_or_else(|| "source DLL input exceeds the bridge buffer".to_owned())?;
        let (bytes, path) = input.split_at(image_length);
        let relative_path =
            std::str::from_utf8(path).map_err(|_| "source DLL path is not UTF-8".to_owned())?;
        virtual_executable_path(relative_path)?;
        let name = relative_path.rsplit(['/', '\\']).next().unwrap_or_default();
        if !name.to_ascii_lowercase().ends_with(".dll")
            || self
                .source_dlls
                .iter()
                .any(|dll| dll.name.eq_ignore_ascii_case(name))
        {
            return Err("source DLL name is invalid or already registered".to_owned());
        }
        let limits = Pe32Limits::default();
        let snapshot = ModuleSnapshot::from_bytes(bytes.to_vec(), &limits).map_err(error_string)?;
        let loader = build_pe32_load_plan(&snapshot, limits).map_err(error_string)?;
        if loader.image.kind != vine_core::ImageKind::DynamicLibrary {
            return Err("selected source DLL is not a PE32 dynamic library".to_owned());
        }
        let source = snapshot.identity();
        let imports = guest_import_names(&loader);
        let source_id = u32::try_from(self.source_dlls.len() + 2)
            .map_err(|_| "source DLL ID overflow".to_owned())?;
        self.source_dlls.push(SourceDll {
            name: name.to_owned(),
            relative_path: relative_path.to_owned(),
            snapshot,
            loader,
        });
        self.set_output(
            json!({"sourceId": source_id, "sha256": source.sha256,
            "guestImports": imports}),
            Vec::new(),
            Vec::new(),
        )
    }

    fn prepare_executable(
        &mut self,
        executable_length: usize,
        path_length: usize,
        encoded_pointer_cookie: u32,
    ) -> Result<(), String> {
        let total_length = executable_length
            .checked_add(path_length)
            .ok_or_else(|| "selected executable input length overflowed".to_owned())?;
        let input = self
            .input
            .get(..total_length)
            .ok_or_else(|| "selected executable exceeds the bridge buffer".to_owned())?;
        let (executable, relative_path) = input.split_at(executable_length);
        let relative_path = std::str::from_utf8(relative_path)
            .map_err(|_| "selected executable path is not UTF-8".to_owned())?;
        let (runtime, metadata, reflection) = BrowserRuntime::prepare_executable(
            executable.to_vec(),
            relative_path,
            std::mem::take(&mut self.source_dlls),
            encoded_pointer_cookie,
        )?;
        self.runtime = Some(runtime);
        self.set_output(metadata, reflection.control, reflection.page_bytes)
    }

    fn complete_source_read(
        &mut self,
        request_id: u64,
        control_length: usize,
        payload_length: usize,
    ) -> Result<(), String> {
        let total_length = control_length
            .checked_add(payload_length)
            .ok_or_else(|| "source read bridge input length overflowed".to_owned())?;
        let input = self
            .input
            .get(..total_length)
            .ok_or_else(|| "source read input lengths exceed the bridge buffer".to_owned())?;
        let (control, payload) = input.split_at(control_length);
        let response = RuntimeSourceReadHostResponse::decode_binary(request_id, control, payload)
            .map_err(error_string)?;
        let (metadata, delta) = self.runtime_mut()?.complete_source_read(response)?;
        let (control, payload) = delta
            .map(|buffers| (buffers.control, buffers.page_bytes))
            .unwrap_or_default();
        self.set_output(metadata, control, payload)
    }

    fn complete_gdi_text_raster(
        &mut self,
        request_id: u64,
        payload_length: usize,
    ) -> Result<(), String> {
        let pixels = self
            .input
            .get(..payload_length)
            .ok_or_else(|| "text raster payload length exceeds the bridge buffer".to_owned())?
            .to_vec();
        let response = Gdi32TextRasterHostResponse { request_id, pixels };
        let (metadata, delta) = self.runtime_mut()?.complete_gdi_text_raster(response)?;
        let (control, payload) = delta
            .map(|buffers| (buffers.control, buffers.page_bytes))
            .unwrap_or_default();
        self.set_output(metadata, control, payload)
    }

    fn runtime_mut(&mut self) -> Result<&mut BrowserRuntime, String> {
        self.runtime
            .as_mut()
            .ok_or_else(|| "browser runtime has not been prepared".to_owned())
    }

    fn ingest_keyboard_event(&mut self, event: RuntimeKeyboardSourceEvent) -> Result<(), String> {
        let metadata = self.runtime_mut()?.ingest_keyboard_event(event)?;
        self.set_output(metadata, Vec::new(), Vec::new())
    }

    fn ingest_pointer_event(
        &mut self,
        sequence: u64,
        kind: u32,
        button: u32,
        normalized_x: u16,
        normalized_y: u16,
    ) -> Result<(), String> {
        let metadata = self.runtime_mut()?.ingest_pointer_event(
            sequence,
            kind,
            button,
            normalized_x,
            normalized_y,
        )?;
        self.set_output(metadata, Vec::new(), Vec::new())
    }

    fn reconcile_writeback_input(
        &mut self,
        control_length: usize,
        payload_length: usize,
    ) -> Result<(), String> {
        let total = control_length
            .checked_add(payload_length)
            .ok_or_else(|| "writeback input length overflowed".to_owned())?;
        let input = self
            .input
            .get(..total)
            .ok_or_else(|| "writeback input exceeds the bridge buffer".to_owned())?;
        let (control, payload) = input.split_at(control_length);
        let generation = self
            .runtime
            .as_mut()
            .ok_or_else(|| "browser runtime has not been prepared".to_owned())?
            .reconcile_writeback(control, payload)?;
        self.writeback_generation = Some(generation);
        Ok(())
    }
}

struct PendingHostCall {
    thread_id: RuntimeThreadId,
    call: ApiDeferredCall,
}

struct PreparedExecutableProcess {
    runtime: Runtime,
    binding: RuntimeGuestImportBinding,
    dll_bindings: Vec<RuntimeGuestImportBinding>,
    module_handles: Vec<u32>,
}

struct PendingSourceSearch {
    pattern: Vec<u8>,
    parent: Vec<u8>,
    selected: Option<Vec<(Vec<u8>, bool)>>,
}

struct BrowserRuntime {
    runtime: Runtime,
    dispatcher: ApiDispatcher,
    callback_sentinel: GuestAddress,
    pending_host: BTreeMap<u64, PendingHostCall>,
    pending_com_audio: Option<ComAudioDeferred>,
    pending_com_media: BTreeMap<u64, ComMediaDeferred>,
    prepared_source_trap: Option<(RuntimeThreadId, GuestAddress, GuestAddress, GuestAddress)>,
    pending_source_lookup: Option<(u32, Vec<u8>)>,
    pending_source_search: Option<PendingSourceSearch>,
}

struct StopContext {
    thread_id: Option<u32>,
    instruction_pointer: u32,
    last_error: Option<u32>,
    module: Option<String>,
    symbol: Option<String>,
}

impl StopContext {
    #[cold]
    fn read(browser: Option<&BrowserRuntime>, instruction_pointer: u32) -> Self {
        let endpoint = browser.and_then(|browser| {
            browser
                .dispatcher
                .endpoint(GuestAddress(instruction_pointer))
        });
        let (module, symbol) = if let Some(endpoint) = endpoint {
            (
                Some(endpoint.module.clone()),
                Some(endpoint.diagnostic_symbol()),
            )
        } else {
            browser
                .map(|browser| {
                    browser
                        .runtime
                        .diagnostic_location(GuestAddress(instruction_pointer))
                })
                .unwrap_or_default()
        };
        Self {
            thread_id: browser.map(|browser| browser.runtime.current_thread_id().0),
            instruction_pointer,
            last_error: browser.and_then(|browser| browser.runtime.current_last_error().ok()),
            module,
            symbol,
        }
    }
}

impl BrowserRuntime {
    fn ingest_keyboard_event(
        &mut self,
        event: RuntimeKeyboardSourceEvent,
    ) -> Result<Value, String> {
        let sequence = event.sequence;
        self.runtime
            .ingest_keyboard_event(event)
            .map_err(error_string)?;
        Ok(json!({
            "kind": "keyboard-event-ingested",
            "sequence": sequence,
        }))
    }

    fn ingest_pointer_event(
        &mut self,
        sequence: u64,
        kind: u32,
        button: u32,
        normalized_x: u16,
        normalized_y: u16,
    ) -> Result<Value, String> {
        if self
            .runtime
            .foreground_window_handle()
            .map_err(error_string)?
            .is_none()
        {
            self.runtime
                .skip_unroutable_pointer_event(sequence)
                .map_err(error_string)?;
            return Ok(json!({"kind": "pointer-event-ignored", "sequence": sequence}));
        }
        let (message, window_handle) = match kind {
            1 if button == 0 => {
                let receipt = self
                    .runtime
                    .ingest_pointer_position_event(RuntimePointerPositionSourceEvent {
                        sequence,
                        normalized_x,
                        normalized_y,
                    })
                    .map_err(error_string)?;
                (receipt.message, receipt.hwnd)
            }
            2 | 3 => {
                let button = match button {
                    1 => RuntimePointerButton::Primary,
                    2 => RuntimePointerButton::Secondary,
                    _ => return Err("pointer button is outside the bridge ABI".to_owned()),
                };
                let receipt = self
                    .runtime
                    .ingest_pointer_button_event(RuntimePointerButtonSourceEvent {
                        sequence,
                        kind: if kind == 2 {
                            RuntimePointerButtonSourceEventKind::ButtonDown
                        } else {
                            RuntimePointerButtonSourceEventKind::ButtonUp
                        },
                        button,
                        normalized_x,
                        normalized_y,
                    })
                    .map_err(error_string)?;
                (receipt.message, receipt.hwnd)
            }
            _ => return Err("pointer event kind is outside the bridge ABI".to_owned()),
        };
        Ok(
            json!({"kind": "pointer-event-ingested", "sequence": sequence,
            "message": message, "windowHandle": window_handle}),
        )
    }

    fn prepare_executable(
        executable: Vec<u8>,
        relative_path: &str,
        source_dlls: Vec<SourceDll>,
        encoded_pointer_cookie: u32,
    ) -> Result<(Self, Value, vine_core::MemoryReflectionBuffers), String> {
        let virtual_image_path = virtual_executable_path(relative_path)?;
        let executable_name = relative_path
            .rsplit(['/', '\\'])
            .next()
            .ok_or_else(|| "selected executable path is empty".to_owned())?;
        let limits = Pe32Limits::default();
        let snapshot = ModuleSnapshot::from_bytes(executable, &limits).map_err(error_string)?;
        let plan = build_pe32_load_plan(&snapshot, limits).map_err(error_string)?;
        if plan.image.kind != vine_core::ImageKind::Executable {
            return Err("selected PE32 image is a DLL, not an executable".to_owned());
        }
        let all_loaders = std::iter::once(&plan)
            .chain(source_dlls.iter().map(|dll| &dll.loader))
            .collect::<Vec<_>>();
        let required = all_loaders
            .iter()
            .flat_map(|loader| guest_import_names(loader))
            .map(|name| name.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        let available = source_dlls
            .iter()
            .map(|dll| dll.name.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        if let Some(missing) = required.difference(&available).next() {
            return Err(format!("imported source DLL {missing} was not supplied"));
        }
        if let Some(unused) = available.difference(&required).next() {
            return Err(format!("unreferenced source DLL {unused} was supplied"));
        }
        #[cfg(all(feature = "shared-guest-memory", target_arch = "wasm32"))]
        let memory_constructor = GuestMemory::new_shared;
        #[cfg(not(all(feature = "shared-guest-memory", target_arch = "wasm32")))]
        let memory_constructor = GuestMemory::new;
        let mut memory = memory_constructor(MemoryLimits {
            max_physical_pages: MAX_GAME_PHYSICAL_PAGES,
            ..MemoryLimits::default()
        })
        .map_err(error_string)?;
        let main_load_base = guest_image_load_base(
            &memory,
            plan.image.preferred_image_base,
            plan.image.size_of_image,
        )?;
        let materialization = materialize_pe32_image(&mut memory, &snapshot, &plan, main_load_base)
            .map_err(error_string)?;
        let catalog = runtime_catalog_for_load_plan(&all_loaders)?;
        drop(all_loaders);
        let mut mapped_dlls = Vec::with_capacity(source_dlls.len());
        for (index, source) in source_dlls.into_iter().enumerate() {
            let base = guest_image_load_base(
                &memory,
                source.loader.image.preferred_image_base,
                source.loader.image.size_of_image,
            )?;
            let image = materialize_pe32_image(&mut memory, &source.snapshot, &source.loader, base)
                .map_err(error_string)?;
            mapped_dlls.push(MappedSourceDll {
                source,
                image,
                source_id: u32::try_from(index + 2).map_err(|_| "source DLL ID overflow")?,
            });
        }
        let mut process = prepare_executable_runtime_process(
            memory,
            &plan,
            &materialization,
            &mapped_dlls,
            &catalog,
            encoded_pointer_cookie,
            executable_name,
            virtual_image_path.clone(),
        )?;
        let source = snapshot.identity();
        process
            .runtime
            .register_source_snapshot(RuntimeSourceSnapshotManifest {
                mount: RuntimeSourceMountManifest {
                    source_id: 1,
                    root_path_a: b"C:\\Vine\\\0".to_vec(),
                },
                files: std::iter::once(RuntimeSourceFileManifest {
                    object_id: 1,
                    relative_path_a: virtual_image_path
                        [b"C:\\Vine\\".len()..virtual_image_path.len() - 1]
                        .to_vec(),
                    byte_length: source.bytes,
                })
                .chain(
                    mapped_dlls
                        .iter()
                        .map(|dll| {
                            let path = virtual_executable_path(&dll.source.relative_path)?;
                            let identity = dll.source.snapshot.identity();
                            Ok(RuntimeSourceFileManifest {
                                object_id: dll.source_id,
                                relative_path_a: path[b"C:\\Vine\\".len()..path.len() - 1].to_vec(),
                                byte_length: identity.bytes,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                )
                .collect(),
            })
            .map_err(error_string)?;
        process
            .runtime
            .configure_input_profile(RuntimeInputProfile {
                initial_mouse_buttons_swapped: false,
                initial_keyboard_source_focused: Some(false),
                cursor_position: Some(RuntimeCursorPositionProfile {
                    initial_screen_x: 960,
                    initial_screen_y: 540,
                }),
            })
            .map_err(error_string)?;
        process
            .runtime
            .configure_display_profile(RuntimeDisplayProfile {
                primary_monitor_rect: RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 1080,
                },
                primary_work_rect: RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 1032,
                },
            })
            .map_err(error_string)?;
        let callback_allocation = control_mapping_allocation(
            &process.runtime,
            ProcessAddressAllocationPurpose::CallbackReturns,
        )?;
        let sentinels =
            install_runtime_return_sentinels(process.runtime.memory_mut(), &callback_allocation)
                .map_err(error_string)?;
        let stack = install_initial_thread_stack(
            process.runtime.memory_mut(),
            &sentinels.process_entry_return,
            plan.image.stack_reserve,
            plan.image.stack_commit,
        )
        .map_err(error_string)?;
        process
            .runtime
            .register_initial_thread_stack(&stack)
            .map_err(error_string)?;
        let command_line = relative_path.replace('/', "\\");
        let command_line = if command_line.contains(' ') {
            format!("\"{command_line}\"")
        } else {
            command_line
        };
        let environment = process
            .runtime
            .install_process_environment(&command_line)
            .map_err(error_string)?;
        let attach =
            finalize_executable_runtime_process(&mut process, &[], sentinels.callback_return)?;
        let dll_attach = match attach {
            RuntimeDllLifecycleAction::Callback {
                target,
                stack_pointer,
            } => Some((target, stack_pointer)),
            RuntimeDllLifecycleAction::Complete { .. } => None,
            RuntimeDllLifecycleAction::AttachFailed { failed_module } => {
                return Err(format!("DLL process attach failed: {}", failed_module));
            }
        };
        let mut endpoint_by_address: BTreeMap<GuestAddress, ApiEndpointIdentity> = BTreeMap::new();
        for import in process.binding.imports.iter().chain(
            process
                .dll_bindings
                .iter()
                .flat_map(|binding| &binding.imports),
        ) {
            if let Some(endpoint) = ApiEndpointIdentity::from_runtime_guest_import(import) {
                if let Some(previous) = endpoint_by_address.get(&endpoint.address) {
                    if previous.id != endpoint.id
                        || previous.module != endpoint.module
                        || previous.symbol != endpoint.symbol
                    {
                        return Err(
                            "shared runtime import address has conflicting module identity"
                                .to_owned(),
                        );
                    }
                } else {
                    endpoint_by_address.insert(endpoint.address, endpoint);
                }
            }
        }
        let endpoints = endpoint_by_address.into_values().collect::<Vec<_>>();
        let dispatcher = ApiDispatcher::from_endpoints(endpoints).map_err(api_fault_string)?;
        let reflection_buffers = process
            .runtime
            .memory_mut()
            .pending_reflection_binary()
            .map_err(error_string)?;
        let code_images = std::iter::once((&plan, &materialization))
            .chain(
                mapped_dlls
                    .iter()
                    .map(|dll| (&dll.source.loader, &dll.image)),
            )
            .collect::<Vec<_>>();
        let code_entries =
            vine_core::code_preparation_entries(process.runtime.memory(), &code_images, 4096)
                .map_err(error_string)?;
        let metadata = json!({
            "kind": "prepared",
            "entryPoint": materialization.entry_point_address.0,
            "codeEntries": code_entries.entries,
            "codeEntriesLimited": code_entries.limited,
            "fsBase": environment.teb_address.0,
            "initialStackPointer": stack.initial_esp.0,
            "entrySentinel": sentinels.process_entry_return.address.0,
            "callbackSentinel": sentinels.callback_return.0,
            "dllAttach": dll_attach.map(|(target, stack)| json!({"target": target.0,
                "stackPointer": stack.0})),
            "maximumPhysicalPages": MAX_GAME_PHYSICAL_PAGES,
            "regions": browser_memory_regions(process.runtime.memory()),
            "runtime": { "threadId": process.runtime.current_thread_id().0 },
        });
        Ok((
            Self {
                runtime: process.runtime,
                dispatcher,
                callback_sentinel: sentinels.callback_return,
                pending_host: BTreeMap::new(),
                pending_com_audio: None,
                pending_com_media: BTreeMap::new(),
                prepared_source_trap: None,
                pending_source_lookup: None,
                pending_source_search: None,
            },
            metadata,
            reflection_buffers,
        ))
    }

    fn acknowledge(&mut self, generation: u64) -> Result<Value, String> {
        self.acknowledge_fast(generation)?;
        Ok(json!({
            "kind": "acknowledged",
            "generation": generation,
            "dirtyPages": 0,
        }))
    }

    fn acknowledge_fast(&mut self, generation: u64) -> Result<(), String> {
        self.runtime
            .memory_mut()
            .acknowledge_delta(generation)
            .map_err(error_string)?;
        debug_assert_eq!(self.runtime.memory_mut().stats().dirty_pages, 0);
        Ok(())
    }

    fn reconcile_writeback(&mut self, control: &[u8], payload: &[u8]) -> Result<u64, String> {
        let generation = self
            .runtime
            .memory_mut()
            .reconcile_cpu_writeback_binary(control, payload)
            .map_err(error_string)?;
        debug_assert_eq!(self.runtime.memory_mut().stats().dirty_pages, 0);
        Ok(generation)
    }

    fn service_page_fault(
        &mut self,
        fault_address: GuestAddress,
        page_fault_error_code: u32,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>), String> {
        self.runtime
            .service_page_fault(fault_address, page_fault_error_code)
            .map_err(error_string)?;
        let buffers = self
            .runtime
            .memory_mut()
            .pending_delta_binary()
            .map_err(error_string)?;
        Ok((json!({"kind": "page-fault-resolved"}), Some(buffers)))
    }

    fn synchronize_forward_clock(
        &mut self,
        retired_instructions: u64,
        host_elapsed_time_100ns: u64,
    ) -> Result<(), String> {
        self.runtime
            .synchronize_scheduled_clock_for_host_execution(
                retired_instructions,
                host_elapsed_time_100ns,
            )
            .map_err(error_string)?;
        Ok(())
    }

    fn advance_clock_for_dispatch(&mut self, retired_instructions: u64) -> Result<(), String> {
        let synchronized = self.runtime.synchronized_instruction_count();
        if synchronized != retired_instructions {
            return Err(
                "forward dispatch reached Rust before its host clock synchronization".to_owned(),
            );
        }
        Ok(())
    }

    fn complete_source_lookup(&mut self, result: Option<Value>) -> Result<Value, String> {
        if let Some(search) = self.pending_source_search.as_mut() {
            if let Some(matches) = search.selected.as_ref() {
                let Some(Value::Array(results)) = result else {
                    return Err("source search results must be an array".to_owned());
                };
                if results.len() != matches.len() {
                    return Err("source search result count changed during verification".to_owned());
                }
                let mut entries = Vec::new();
                for ((path, file), value) in matches.iter().zip(results) {
                    if value.is_null() {
                        continue;
                    }
                    let kind = value.get("kind").and_then(Value::as_str);
                    let info = match (*file, kind) {
                        (true, Some("file")) => {
                            let byte_length = value
                                .get("byteLength")
                                .and_then(Value::as_u64)
                                .ok_or_else(|| "source search file length is missing".to_owned())?;
                            Some(byte_length)
                        }
                        (false, Some("directory")) => None,
                        _ => {
                            return Err(
                                "source search entry changed kind during verification".to_owned()
                            );
                        }
                    };
                    entries.push((path.clone(), info));
                }
                let entries_paths = entries
                    .iter()
                    .map(|(path, _)| path.clone())
                    .collect::<Vec<_>>();
                let registered = self
                    .runtime
                    .register_source_search_batch(&search.parent, entries)
                    .map_err(error_string)?;
                self.pending_source_search = None;
                return Ok(
                    json!({ "kind": "source-search-complete", "registered": registered.into_iter()
                    .map(|(index, object_id)| json!({ "objectId": object_id, "path": entries_paths[index] }))
                    .collect::<Vec<_>>() }),
                );
            }
            let Some(list) = result else {
                self.pending_source_search = None;
                return Ok(json!({ "kind": "source-search-complete", "registered": [] }));
            };
            let segments = list
                .get("pathSegments")
                .and_then(Value::as_array)
                .ok_or_else(|| "source directory spelling is missing".to_owned())?;
            if segments.len() > 256 {
                return Err("source directory exceeds DOS depth".to_owned());
            }
            let segments = segments
                .iter()
                .map(|part| {
                    part.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "source directory component is invalid".to_owned())
                })
                .collect::<Result<Vec<_>, String>>()?;
            let names = list
                .get("entries")
                .and_then(Value::as_array)
                .ok_or_else(|| "source directory listing must be an array".to_owned())?;
            if names.len() > vine_core::RUNTIME_SOURCE_FILE_LIMIT {
                return Err("source directory listing exceeds the bounded source limit".to_owned());
            }
            let names = names
                .iter()
                .map(|entry| {
                    let name = entry
                        .get("name")
                        .and_then(Value::as_str)
                        .ok_or_else(|| "source search name is missing".to_owned())?;
                    let kind = entry.get("kind").and_then(Value::as_str);
                    if !matches!(kind, Some("file" | "directory")) {
                        return Err("source search entry kind is invalid".to_owned());
                    }
                    Ok((name.to_owned(), kind == Some("file")))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let (actual_parent, selected) = self
                .runtime
                .source_search_matches(&search.pattern, &segments, &names)
                .map_err(error_string)?;
            let metadata = json!({ "kind": "source-search-matches", "entries": selected.iter()
                .map(|(path, _)| path).collect::<Vec<_>>() });
            search.parent = actual_parent;
            search.selected = Some(selected);
            return Ok(metadata);
        }
        let (object_id, path) = self
            .pending_source_lookup
            .as_ref()
            .ok_or_else(|| "no source lookup is pending".to_owned())?;
        let object_id = *object_id;
        let found = if let Some(result) = result {
            if result.get("kind").and_then(Value::as_str) == Some("directory") {
                self.runtime
                    .register_source_directory(path)
                    .map_err(error_string)?;
                self.pending_source_lookup = None;
                return Ok(json!({ "kind": "source-lookup-complete", "found": true }));
            }
            if result.get("kind").and_then(Value::as_str) != Some("file") {
                return Err("source lookup returned an unknown entry kind".to_owned());
            }
            let length = result
                .get("byteLength")
                .and_then(Value::as_u64)
                .ok_or_else(|| "source lookup length is missing".to_owned())?;
            self.runtime
                .register_source_file(RuntimeSourceFileManifest {
                    object_id,
                    relative_path_a: path.clone(),
                    byte_length: length,
                })
                .map_err(error_string)?;
            true
        } else {
            false
        };
        self.pending_source_lookup = None;
        Ok(json!({ "kind": "source-lookup-complete", "found": found, "objectId": object_id }))
    }

    fn dispatch(
        &mut self,
        endpoint_address: GuestAddress,
        stack_pointer: GuestAddress,
        return_address: GuestAddress,
        retired_instructions: u64,
        x86_context: X86CpuContext,
    ) -> Result<
        (
            DispatchMetadata,
            Option<MemoryDeltaBuffers>,
            RuntimeOutputBytes,
        ),
        String,
    > {
        if self.pending_com_audio.is_some() {
            return Err("COM audio handoff must finish before guest dispatch".to_owned());
        }
        if self.pending_source_lookup.is_some() || self.pending_source_search.is_some() {
            return Err("source lookup must finish before guest API dispatch".to_owned());
        }
        let identity = (
            self.runtime.current_thread_id(),
            endpoint_address,
            stack_pointer,
            return_address,
        );
        let route = self.dispatcher.endpoint(endpoint_address);
        let endpoint_id = route.map(|endpoint| endpoint.id);
        if let Some(prepared) = self.prepared_source_trap {
            if prepared != identity {
                return Err("source completion belongs to a different guest trap".to_owned());
            }
            self.prepared_source_trap = None;
        } else if let Some(route) = route
            && let Some(lookup) = self
                .runtime
                .prepare_api_source_lookup(route, stack_pointer)
                .map_err(api_fault_string)?
        {
            let metadata = match lookup {
                vine_core::ApiSourceLookup::Search { pattern, parent } => {
                    let metadata = json!({ "kind": "source-search", "relativeDirectoryA": parent });
                    self.pending_source_search = Some(PendingSourceSearch {
                        pattern,
                        parent,
                        selected: None,
                    });
                    metadata
                }
                vine_core::ApiSourceLookup::File {
                    object_id,
                    relative_path_a,
                } => {
                    let metadata = json!({ "kind": "source-lookup", "objectId": object_id, "relativePathA": relative_path_a });
                    self.pending_source_lookup = Some((object_id, relative_path_a));
                    metadata
                }
            };
            self.prepared_source_trap = Some(identity);
            return Ok((
                DispatchMetadata::Json(metadata),
                None,
                RuntimeOutputBytes::new(),
            ));
        }
        let Some(endpoint_id) = endpoint_id else {
            return self
                .dispatch_com(
                    endpoint_address,
                    stack_pointer,
                    return_address,
                    retired_instructions,
                )
                .map(|(metadata, delta, bytes)| (DispatchMetadata::Json(metadata), delta, bytes));
        };
        self.advance_clock_for_dispatch(retired_instructions)?;
        let trap = ApiOwnedTrap {
            endpoint_id,
            endpoint_address,
            stack_pointer,
            return_address,
        };
        let callback_sentinel = self.callback_sentinel;
        let outcome = self.runtime.dispatch_api_call(
            &self.dispatcher,
            trap,
            callback_sentinel,
            Some(x86_context),
        );
        acknowledge_thread_wakes(&mut self.runtime);
        let result: Result<(DispatchMetadata, Option<MemoryDeltaBuffers>), String> = match outcome {
            DispatchOutcome::Deferred(call) => {
                let message_box = if call.request.service == RUNTIME_MESSAGE_BOX_SERVICE {
                    if !call.request.payload.is_empty() {
                        return Err(
                            "message box host request contains unexpected payload".to_owned()
                        );
                    }
                    let dialog = self
                        .runtime
                        .message_box(call.request.request_id)
                        .map_err(error_string)?;
                    Some(json!({
                        "title": dialog.title,
                        "text": dialog.text,
                        "buttons": dialog.buttons.iter().map(|button| json!({
                            "id": button.id,
                            "label": button.label,
                        })).collect::<Vec<_>>(),
                        "defaultButton": dialog.default_button,
                        "cancelButton": dialog.cancel_button,
                        "icon": match dialog.icon {
                            RuntimeMessageBoxIcon::None => "none",
                            RuntimeMessageBoxIcon::Error => "error",
                            RuntimeMessageBoxIcon::Question => "question",
                            RuntimeMessageBoxIcon::Warning => "warning",
                            RuntimeMessageBoxIcon::Information => "information",
                        },
                    }))
                } else {
                    None
                };
                let opfs_write = if call.request.service == "browser.opfs-write" {
                    Some(
                        self.runtime
                            .opfs_write_host_info(call.request.request_id)
                            .map_err(error_string)?,
                    )
                } else {
                    None
                };
                let opfs_read_path = if call.request.service == RUNTIME_OPFS_READ_SERVICE {
                    Some(
                        self.runtime
                            .opfs_read_host_path(call.request.request_id)
                            .map_err(error_string)?,
                    )
                } else {
                    None
                };
                let opfs_path = if matches!(
                    call.request.service.as_str(),
                    "browser.opfs-create-directory"
                        | "browser.opfs-delete-file"
                        | "browser.opfs-stat"
                        | "browser.opfs-open"
                ) {
                    let path = if call.request.service == "browser.opfs-open" {
                        let header = call
                            .request
                            .payload
                            .get(..3)
                            .ok_or_else(|| "OPFS file open header is missing".to_owned())?;
                        let path = call
                            .request
                            .payload
                            .get(3..)
                            .ok_or_else(|| "OPFS file open path is missing".to_owned())?;
                        if !matches!(header[0], 2..=4)
                            || !matches!(header[1], 2 | 3)
                            || usize::from(header[2])
                                > path.iter().filter(|byte| **byte == b'/').count()
                        {
                            return Err("OPFS file open header is invalid".to_owned());
                        }
                        path
                    } else if matches!(
                        call.request.service.as_str(),
                        "browser.opfs-stat"
                            | "browser.opfs-delete-file"
                            | "browser.opfs-create-directory"
                    ) {
                        let (missing, path) =
                            call.request.payload.split_first().ok_or_else(|| {
                                "OPFS source result or path is missing".to_owned()
                            })?;
                        if call.request.service == "browser.opfs-create-directory" {
                            if usize::from(*missing)
                                > path.iter().filter(|byte| **byte == b'/').count()
                            {
                                return Err(
                                    "OPFS source parent count exceeds the path depth".to_owned()
                                );
                            }
                        } else if !matches!(missing, 2 | 3) {
                            return Err("OPFS source result is invalid".to_owned());
                        }
                        path
                    } else {
                        &call.request.payload
                    };
                    Some(
                        String::from_utf8(path.to_vec())
                            .map_err(|_| "OPFS path is not UTF-8".to_owned())?,
                    )
                } else {
                    None
                };
                let arguments = if let Some((_, cursor, transferred)) = &opfs_write {
                    vec![*cursor as u32, (*cursor >> 32) as u32, *transferred]
                } else if call.request.service == "browser.opfs-open" {
                    call.request.payload[..3]
                        .iter()
                        .map(|byte| u32::from(*byte))
                        .collect()
                } else if call.request.service == "browser.opfs-create-directory" {
                    vec![u32::from(call.request.payload[0])]
                } else if opfs_path.is_some() || message_box.is_some() {
                    Vec::new()
                } else {
                    decode_u32_payload(&call.request.payload)
                };
                let thread_id = self.runtime.current_thread_id();
                match self.runtime.scheduler_state().map_err(error_string)? {
                    SchedulerThreadState::WaitingHost { request_id }
                        if request_id == call.request.request_id => {}
                    _ => {
                        return Err(
                            "Rust scheduler did not authorize the emitted host request".to_owned()
                        );
                    }
                }
                let mut metadata = json!({
                    "kind": "host-request",
                    "requestId": call.request.request_id.to_string(),
                    "threadId": thread_id.0,
                    "service": call.request.service,
                    "arguments": arguments,
                    "scheduler": {
                        "state": "waiting-host",
                        "requestId": call.request.request_id.to_string(),
                    },
                });
                if let Some(dialog) = message_box {
                    metadata["messageBox"] = dialog;
                }
                if let Some(path) = opfs_path
                    .as_deref()
                    .or(opfs_read_path.as_deref())
                    .or_else(|| opfs_write.as_ref().map(|(path, _, _)| *path))
                {
                    metadata["relativePath"] = json!(path);
                }
                let request_id = call.request.request_id;
                if self.pending_host.contains_key(&request_id) {
                    return Err(format!("host request ID {request_id} is already pending"));
                }
                let previous = self
                    .pending_host
                    .insert(request_id, PendingHostCall { thread_id, call });
                debug_assert!(previous.is_none());
                let write_bytes =
                    opfs_write.map(|_| self.pending_host[&request_id].call.request.payload.clone());
                Ok((
                    DispatchMetadata::Json(metadata),
                    write_bytes.map(|bytes| MemoryDeltaBuffers {
                        control: Vec::new(),
                        page_bytes: bytes,
                    }),
                ))
            }
            DispatchOutcome::GuestCallback(call) => {
                let (metadata, delta) = self.begin_callback(call)?;
                Ok((DispatchMetadata::Json(metadata), delta))
            }
            DispatchOutcome::Immediate(completion) => {
                let delta = self.pending_delta()?;
                let created_thread = if let Some(thread_id) = completion.created_thread {
                    let bootstrap = self
                        .runtime
                        .thread_state(thread_id)
                        .map_err(error_string)?
                        .guest_bootstrap
                        .as_ref()
                        .ok_or_else(|| "created thread has no CPU bootstrap".to_owned())?;
                    Some(json!({
                        "threadId": bootstrap.thread_id.0,
                        "entryPoint": bootstrap.entry_point.0,
                        "initialStackPointer": bootstrap.initial_stack_pointer.0,
                        "fsBase": bootstrap.fs_base.0,
                        "returnSentinel": bootstrap.return_sentinel.0,
                    }))
                } else {
                    None
                };
                let selection = if completion.execution_boundary
                    == vine_core::RuntimeApiBoundaryDisposition::TerminateCurrentThread
                {
                    None
                } else if completion.execution_boundary
                    == vine_core::RuntimeApiBoundaryDisposition::BlockCurrentThread
                {
                    if !matches!(
                        self.runtime.scheduler_state().map_err(error_string)?,
                        SchedulerThreadState::WaitingCriticalSection { .. }
                            | SchedulerThreadState::WaitingObject { .. }
                    ) {
                        return Err(
                            "blocking API completion has no matching scheduler wait".to_owned()
                        );
                    }
                    self.runtime
                        .select_next_executable_thread()
                        .map_err(error_string)?
                } else {
                    self.runtime
                        .complete_api_execution_boundary_with_disposition(
                            completion.execution_boundary,
                        )
                        .map_err(error_string)?
                };
                let metadata = if completion.execution_boundary
                    == vine_core::RuntimeApiBoundaryDisposition::ContinueQuantum
                    && created_thread.is_none()
                    && selection.is_none()
                {
                    DispatchMetadata::ApiCompletion([
                        completion.instruction_pointer.0,
                        completion.stack_pointer.0,
                        completion.eax.unwrap_or(0),
                        u32::from(completion.eax.is_some()),
                    ])
                } else {
                    let mut metadata = completion_metadata("api-complete", &completion);
                    if let Some(created_thread) = created_thread {
                        metadata["createdThread"] = created_thread;
                    }
                    if let Some(selection) = selection {
                        metadata["threadSelection"] = json!({"threadId": selection.0});
                    }
                    DispatchMetadata::Json(metadata)
                };
                Ok((metadata, delta))
            }
            DispatchOutcome::Fault(fault) => Err(api_fault_string(fault)),
        };
        result.map(|(metadata, delta)| (metadata, delta, RuntimeOutputBytes::new()))
    }

    fn dispatch_com(
        &mut self,
        endpoint_address: GuestAddress,
        stack_pointer: GuestAddress,
        return_address: GuestAddress,
        retired_instructions: u64,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>, RuntimeOutputBytes), String> {
        self.advance_clock_for_dispatch(retired_instructions)?;

        let trap = ComCallTrap {
            endpoint_address,
            stack_pointer,
            return_address,
            call_site: return_address,
        };
        let outcome = self.runtime.dispatch_com_call(trap);
        self.com_outcome(outcome, endpoint_address)
    }

    fn com_outcome(
        &mut self,
        outcome: ComDispatchOutcome,
        endpoint_address: GuestAddress,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>, RuntimeOutputBytes), String> {
        acknowledge_thread_wakes(&mut self.runtime);
        match outcome {
            ComDispatchOutcome::Completed(completion) => {
                let delta = self.pending_delta()?;
                let cpu_resume = json!({
                    "instructionPointer": completion.instruction_pointer.0,
                    "stackPointer": completion.stack_pointer.0,
                    "eax": completion.eax,
                });
                let mut presentation_bytes = RuntimeOutputBytes::new();
                let mut metadata = json!({
                    "kind": "com-complete",
                    "threadId": self.runtime.current_thread_id().0,
                    "completion": cpu_resume,
                });
                if let Some(thread_id) = completion.created_thread {
                    let bootstrap = self
                        .runtime
                        .thread_state(thread_id)
                        .map_err(error_string)?
                        .guest_bootstrap
                        .as_ref()
                        .ok_or_else(|| "created COM thread has no CPU bootstrap".to_owned())?;
                    metadata["createdThread"] = json!({
                        "threadId": bootstrap.thread_id.0,
                        "entryPoint": bootstrap.entry_point.0,
                        "initialStackPointer": bootstrap.initial_stack_pointer.0,
                        "fsBase": bootstrap.fs_base.0,
                        "returnSentinel": bootstrap.return_sentinel.0,
                    });
                }
                if let Some(device_id) = completion.presented_device {
                    let (width, height, frame) = self
                        .runtime
                        .presented_graphics_frame(device_id)
                        .map_err(|error| error.to_string())?;
                    let draws: Vec<Value> = frame
                        .draw_submissions
                        .iter()
                        .map(|draw| {
                            let fills: Vec<Value> = draw
                                .texture_fills
                                .iter()
                                .map(|fill| {
                                    json!({
                                        "left": fill.rect.left, "top": fill.rect.top,
                                        "right": fill.rect.right, "bottom": fill.rect.bottom,
                                        "colorRef": fill.color_ref,
                                    })
                                })
                                .collect();
                            let pixels = append_presentation_pixels(
                                &mut presentation_bytes, &draw.texture_data, draw.texture_pitch,
                            );
                            json!({
                                "vertices": draw.vertex_data,
                                "texture": { "width": draw.texture_width,
                                    "height": draw.texture_height, "fills": fills, "pixels": pixels },
                            })
                        })
                        .collect();
                    metadata["presentation"] = json!({
                        "width": width,
                        "height": height,
                        "colorArgb": frame.clear_color,
                        "draws": draws,
                    });
                }
                Ok((metadata, delta, presentation_bytes))
            }
            ComDispatchOutcome::AudioPending(pending) => {
                if self.pending_com_audio.is_some() {
                    return Err("another COM audio call is pending".to_owned());
                }
                let metadata = json!({ "kind": "audio-request", "requestId": pending.request_id().to_string(),
                    "threadId": self.runtime.current_thread_id().0 });
                self.pending_com_audio = Some(pending);
                Ok((metadata, None, RuntimeOutputBytes::new()))
            }
            ComDispatchOutcome::GuestCallback(prepared) => Ok((
                json!({ "kind": "guest-callback", "threadId": self.runtime.current_thread_id().0,
                    "target": prepared.target.0,
                    "stackPointer": prepared.stack_pointer.0,
                    "returnSentinel": prepared.return_sentinel.0 }),
                self.pending_delta()?,
                RuntimeOutputBytes::new(),
            )),
            ComDispatchOutcome::MediaPending(pending) => {
                if self.pending_com_media.contains_key(&pending.request_id) {
                    return Err("COM media request identity is already pending".to_owned());
                }
                let metadata = media_request_metadata(&pending);
                let audio = pending
                    .audio
                    .as_ref()
                    .map(|audio| audio.pcm_f32le.clone())
                    .unwrap_or_default();
                self.pending_com_media.insert(pending.request_id, pending);
                Ok((metadata, self.pending_delta()?, audio.into()))
            }
            ComDispatchOutcome::MediaData(data) => Ok((
                json!({ "kind": "media-data", "requestId": data.request_id.to_string(),
                    "threadId": self.runtime.current_thread_id().0,
                    "offset": data.offset.to_string(), "byteLength": data.bytes.len() }),
                self.pending_delta()?,
                data.bytes.into(),
            )),
            ComDispatchOutcome::Fault(fault) => {
                let target = fault
                    .target
                    .map(|target| {
                        format!(
                            "{} slot {} ({})",
                            target.interface, target.vtable_slot, target.method
                        )
                    })
                    .unwrap_or_else(|| format!("endpoint {:#010x}", endpoint_address.0));
                Err(format!(
                    "COM {} at {:#010x}: {}",
                    target, fault.call_site.0, fault.message
                ))
            }
        }
    }

    fn complete_com_audio(
        &mut self,
        request_id: u64,
        accepted: bool,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>), String> {
        if self
            .pending_com_audio
            .as_ref()
            .map(ComAudioDeferred::request_id)
            != Some(request_id)
        {
            return Err("audio acknowledgement does not match the pending COM call".to_owned());
        }
        let pending = self
            .pending_com_audio
            .take()
            .expect("checked pending audio");
        match self.runtime.complete_com_audio(pending, accepted) {
            ComDispatchOutcome::Completed(completion) => {
                acknowledge_thread_wakes(&mut self.runtime);
                let metadata = json!({ "kind": "com-complete", "completion": {
                    "instructionPointer": completion.instruction_pointer.0,
                    "stackPointer": completion.stack_pointer.0, "eax": completion.eax,
                }});
                Ok((metadata, self.pending_delta()?))
            }
            ComDispatchOutcome::Fault(fault) => {
                Err(format!("COM audio completion: {}", fault.message))
            }
            ComDispatchOutcome::AudioPending(_) => {
                Err("audio completion deferred again".to_owned())
            }
            ComDispatchOutcome::GuestCallback(_)
            | ComDispatchOutcome::MediaPending(_)
            | ComDispatchOutcome::MediaData(_) => {
                Err("audio completion returned an unrelated COM operation".to_owned())
            }
        }
    }

    fn read_com_media(
        &mut self,
        request_id: u64,
        offset: u64,
        length: u32,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>, RuntimeOutputBytes), String> {
        let pending = self
            .pending_com_media
            .get(&request_id)
            .ok_or_else(|| "encoded read has no pending COM media request".to_owned())?;
        let thread_id = pending.thread_id;
        let previous_thread_id = self.runtime.current_thread_id();
        self.runtime
            .select_thread(thread_id)
            .map_err(error_string)?;
        let outcome = match self.runtime.read_com_media(request_id, offset, length) {
            Ok(outcome) => outcome,
            Err(error) => {
                self.runtime
                    .select_thread(previous_thread_id)
                    .map_err(error_string)?;
                return Err(error_string(error));
            }
        };
        self.com_outcome(outcome, GuestAddress(0))
    }

    fn complete_com_media(
        &mut self,
        request_id: u64,
        response: ComMediaResponse,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>, RuntimeOutputBytes), String> {
        let pending = self
            .pending_com_media
            .get(&request_id)
            .ok_or_else(|| "media completion has no pending COM request".to_owned())?;
        let previous_thread_id = self.runtime.current_thread_id();
        self.runtime
            .select_thread(pending.thread_id)
            .map_err(error_string)?;
        let outcome = match self.runtime.complete_com_media(pending, response) {
            Ok(outcome) => outcome,
            Err(error) => {
                self.runtime
                    .select_thread(previous_thread_id)
                    .map_err(error_string)?;
                return Err(error_string(error));
            }
        };
        self.pending_com_media.remove(&request_id);
        self.com_outcome(outcome, GuestAddress(0))
    }

    fn observe_audio_end(&mut self, resource_id: u64) -> Result<Value, String> {
        if self.pending_com_audio.is_some() {
            return Err("audio end observed during a COM audio handoff".to_owned());
        }
        let ended = self
            .runtime
            .observe_browser_audio_end(resource_id)
            .map_err(error_string)?;
        acknowledge_thread_wakes(&mut self.runtime);
        Ok(json!({ "kind": "audio-end-observed", "ended": ended }))
    }

    fn pending_delta(&mut self) -> Result<Option<MemoryDeltaBuffers>, String> {
        if !self.runtime.memory_mut().has_pending_delta() {
            return Ok(None);
        }
        self.runtime
            .memory_mut()
            .pending_delta_binary()
            .map(Some)
            .map_err(error_string)
    }

    fn begin_callback(
        &mut self,
        call: ApiGuestCallbackCall,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>), String> {
        let prepared = self
            .runtime
            .begin_guest_callback(call)
            .map_err(api_fault_string)?;
        let buffers = self
            .runtime
            .memory_mut()
            .pending_delta_binary()
            .map_err(error_string)?;
        let metadata = json!({
            "kind": "guest-callback",
            "target": prepared.target.0,
            "stackPointer": prepared.stack_pointer.0,
            "returnSentinel": prepared.return_sentinel.0,
        });
        Ok((metadata, Some(buffers)))
    }

    fn complete_opfs_write(
        &mut self,
        request_id: u64,
        observation: u32,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>), String> {
        let pending = self
            .pending_host
            .get(&request_id)
            .ok_or_else(|| "OPFS write completion has no pending call".to_owned())?;
        if pending.call.request.service != "browser.opfs-write"
            || pending.call.request.request_id != request_id
        {
            return Err("OPFS write completion has the wrong host identity".to_owned());
        }
        let previous_thread_id = self.runtime.current_thread_id();
        let owner_thread_id = pending.thread_id;
        if previous_thread_id != owner_thread_id {
            self.runtime
                .select_thread(owner_thread_id)
                .map_err(error_string)?;
        }
        let outcome = complete_kernel32_opfs_write(
            &self.dispatcher,
            &pending.call,
            observation,
            &mut self.runtime,
        );
        let completion = match outcome {
            DispatchOutcome::Immediate(completion) => completion,
            DispatchOutcome::Fault(fault) => {
                restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
                return Err(api_fault_string(fault));
            }
            _ => return Err("OPFS write produced another suspended call".to_owned()),
        };
        match self.runtime.pop_scheduler_event() {
            Some(RuntimeSchedulerEvent::HostCompleted(event))
                if event.request_id == request_id && event.thread_id == owner_thread_id => {}
            _ => return Err("OPFS write scheduler completion mismatch".to_owned()),
        }
        self.pending_host.remove(&request_id);
        if !self
            .runtime
            .scheduler_state_for_thread(owner_thread_id)
            .map_err(error_string)?
            .is_executable()
        {
            return Err("OPFS write did not resume its guest thread".to_owned());
        }
        restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
        let delta = self.pending_delta()?;
        Ok((
            json!({
                "kind": "host-complete", "threadId": owner_thread_id.0,
                "completion": cpu_resume_metadata(&completion),
            }),
            delta,
        ))
    }

    fn complete_opfs_open(
        &mut self,
        request_id: u64,
        observation: u32,
        byte_length: u64,
    ) -> Result<Value, String> {
        let pending = self
            .pending_host
            .get(&request_id)
            .ok_or_else(|| "OPFS open completion has no pending call".to_owned())?;
        if pending.call.request.service != "browser.opfs-open"
            || pending.call.request.request_id != request_id
        {
            return Err("OPFS open completion has the wrong host identity".to_owned());
        }
        let previous_thread_id = self.runtime.current_thread_id();
        let owner_thread_id = pending.thread_id;
        if previous_thread_id != owner_thread_id {
            self.runtime
                .select_thread(owner_thread_id)
                .map_err(error_string)?;
        }
        let outcome = complete_kernel32_opfs_open(
            &self.dispatcher,
            &pending.call,
            observation,
            byte_length,
            &mut self.runtime,
        );
        let completion = match outcome {
            DispatchOutcome::Immediate(completion) => completion,
            DispatchOutcome::Fault(fault) => {
                restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
                return Err(api_fault_string(fault));
            }
            _ => return Err("OPFS file open produced another suspended call".to_owned()),
        };
        match self.runtime.pop_scheduler_event() {
            Some(RuntimeSchedulerEvent::HostCompleted(event))
                if event.request_id == request_id && event.thread_id == owner_thread_id => {}
            _ => return Err("OPFS open scheduler completion mismatch".to_owned()),
        }
        self.pending_host.remove(&request_id);
        if !self
            .runtime
            .scheduler_state_for_thread(owner_thread_id)
            .map_err(error_string)?
            .is_executable()
        {
            return Err("OPFS open did not resume its guest thread".to_owned());
        }
        restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
        Ok(json!({
            "kind": "host-complete",
            "threadId": owner_thread_id.0,
            "completion": cpu_resume_metadata(&completion),
        }))
    }

    fn complete_host(&mut self, request_id: u64, result: u32) -> Result<Value, String> {
        if self
            .pending_host
            .get(&request_id)
            .is_some_and(|pending| pending.call.request.service == RUNTIME_OBJECT_WAIT_SERVICE)
        {
            return self.complete_object_wait(request_id, result);
        }
        if self
            .pending_host
            .get(&request_id)
            .is_some_and(|pending| pending.call.request.service == RUNTIME_MESSAGE_BOX_SERVICE)
        {
            return self.complete_message_box(request_id, result);
        }
        if self.pending_host.get(&request_id).is_some_and(|pending| {
            pending.call.request.service == RUNTIME_WAITABLE_TIMER_PACING_SERVICE
        }) {
            return self.complete_waitable_timer_pacing(request_id, result);
        }
        if self.pending_host.get(&request_id).is_some_and(|pending| {
            pending.call.request.service == RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE
        }) {
            return self.complete_audio_notification_wait(request_id, result);
        }
        // Decode browser outcomes before consuming the pending call or changing
        // scheduler state. These are host observations, not Windows error codes.
        let opfs_result = if let Some(pending) = self.pending_host.get(&request_id) {
            let operation = match pending.call.request.service.as_str() {
                "browser.opfs-create-directory" => Some(OpfsFileOperation::CreateDirectory),
                "browser.opfs-delete-file" => Some(OpfsFileOperation::DeleteFile {
                    source_missing: *pending
                        .call
                        .request
                        .payload
                        .first()
                        .ok_or_else(|| "OPFS delete source result is missing".to_owned())?,
                }),
                "browser.opfs-stat" => Some(OpfsFileOperation::Attributes {
                    source_missing: *pending
                        .call
                        .request
                        .payload
                        .first()
                        .ok_or_else(|| "OPFS stat source result is missing".to_owned())?,
                }),
                _ => None,
            };
            operation
                .map(|operation| {
                    let preserved = self
                        .runtime
                        .thread_state(pending.thread_id)
                        .map_err(error_string)?
                        .last_error;
                    complete_opfs_file_operation(operation, result, preserved)
                        .map_err(api_fault_string)
                })
                .transpose()?
        } else {
            None
        };
        let pending = self
            .pending_host
            .get(&request_id)
            .ok_or_else(|| "host completion has no pending API call".to_owned())?;
        if pending.call.request.request_id != request_id {
            return Err("host completion request identity is corrupt".to_owned());
        }
        let previous_thread_id = self.runtime.current_thread_id();
        let owner_thread_id = pending.thread_id;
        self.dispatcher
            .validate_deferred_completion(&pending.call, request_id, self.runtime.memory())
            .map_err(api_fault_string)?;
        let runtime_completion = self
            .runtime
            .complete_host_request(request_id)
            .map_err(error_string)?;
        match self.runtime.pop_scheduler_event() {
            Some(RuntimeSchedulerEvent::HostCompleted(completion))
                if completion == runtime_completion => {}
            _ => return Err("host completion scheduler event mismatch".to_owned()),
        }
        if self.runtime.current_thread_id() != owner_thread_id {
            self.runtime
                .select_thread(owner_thread_id)
                .map_err(error_string)?;
        }
        let dispatcher = &self.dispatcher;
        let sleep_delay = pending.call.request.service == vine_core::RUNTIME_SLEEP_DELAY_SERVICE;
        let completion = match dispatcher.complete_deferred(
            &pending.call,
            HostCompletion {
                request_id,
                result: if let Some(result) = opfs_result {
                    result
                } else if sleep_delay {
                    ApiImmediate::new(None)
                } else {
                    ApiImmediate::new(Some(result))
                },
            },
            &mut self.runtime,
        ) {
            DispatchOutcome::Immediate(completion) => Ok(completion),
            DispatchOutcome::Fault(fault) => Err(api_fault_string(fault)),
            _ => Err("deferred API completion produced another suspended outcome".to_owned()),
        }?;
        let scheduler = self
            .runtime
            .scheduler_state_for_thread(owner_thread_id)
            .map_err(error_string)?;
        if !scheduler.is_executable() {
            return Err("Rust scheduler did not resume the completed host request".to_owned());
        }
        restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
        self.pending_host.remove(&request_id);
        Ok(json!({
            "kind": "host-complete",
            "threadId": owner_thread_id.0,
            "completion": cpu_resume_metadata(&completion),
        }))
    }

    fn complete_message_box(&mut self, request_id: u64, button_id: u32) -> Result<Value, String> {
        let pending = self
            .pending_host
            .get(&request_id)
            .ok_or_else(|| "message box completion has no pending API call".to_owned())?;
        if pending.call.request.request_id != request_id
            || pending.call.request.service != RUNTIME_MESSAGE_BOX_SERVICE
            || !pending.call.request.payload.is_empty()
        {
            return Err("message box completion request identity is corrupt".to_owned());
        }
        let previous_thread_id = self.runtime.current_thread_id();
        let owner_thread_id = pending.thread_id;
        if previous_thread_id != owner_thread_id {
            self.runtime
                .select_thread(owner_thread_id)
                .map_err(error_string)?;
        }
        let outcome = complete_user32_message_box(
            &self.dispatcher,
            &pending.call,
            button_id,
            &mut self.runtime,
        );
        let completion = match outcome {
            DispatchOutcome::Immediate(completion) => completion,
            DispatchOutcome::Fault(fault) => {
                restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
                return Err(api_fault_string(fault));
            }
            _ => {
                restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
                return Err("message box completion produced another suspended call".to_owned());
            }
        };
        match self.runtime.pop_scheduler_event() {
            Some(RuntimeSchedulerEvent::HostCompleted(event))
                if event.request_id == request_id
                    && event.thread_id == owner_thread_id
                    && event.service == RUNTIME_MESSAGE_BOX_SERVICE => {}
            _ => return Err("message box scheduler completion mismatch".to_owned()),
        }
        if !self
            .runtime
            .scheduler_state_for_thread(owner_thread_id)
            .map_err(error_string)?
            .is_executable()
        {
            return Err("message box completion did not resume its guest thread".to_owned());
        }
        restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
        self.pending_host.remove(&request_id);
        Ok(json!({
            "kind": "host-complete",
            "threadId": owner_thread_id.0,
            "completion": cpu_resume_metadata(&completion),
        }))
    }

    fn complete_waitable_timer_pacing(
        &mut self,
        request_id: u64,
        acknowledgement: u32,
    ) -> Result<Value, String> {
        self.complete_paced_object_or_timer_wait(request_id, acknowledgement, false)
    }

    fn complete_object_wait(
        &mut self,
        request_id: u64,
        acknowledgement: u32,
    ) -> Result<Value, String> {
        self.complete_paced_object_or_timer_wait(request_id, acknowledgement, true)
    }

    fn complete_paced_object_or_timer_wait(
        &mut self,
        request_id: u64,
        acknowledgement: u32,
        object_wait: bool,
    ) -> Result<Value, String> {
        let service = if object_wait {
            RUNTIME_OBJECT_WAIT_SERVICE
        } else {
            RUNTIME_WAITABLE_TIMER_PACING_SERVICE
        };
        let pending = self
            .pending_host
            .get(&request_id)
            .ok_or_else(|| "timer pacing completion has no pending API call".to_owned())?;
        if pending.call.request.request_id != request_id || pending.call.request.service != service
        {
            return Err("timer pacing completion request identity is corrupt".to_owned());
        }
        let previous_thread_id = self.runtime.current_thread_id();
        let owner_thread_id = pending.thread_id;
        if previous_thread_id != owner_thread_id {
            self.runtime
                .select_thread(owner_thread_id)
                .map_err(error_string)?;
        }
        let complete = if object_wait {
            complete_kernel32_object_wait
        } else {
            complete_kernel32_waitable_timer_pacing
        };
        let outcome = complete(
            &self.dispatcher,
            &pending.call,
            acknowledgement,
            &mut self.runtime,
        );
        let completion = match outcome {
            DispatchOutcome::Immediate(completion) => completion,
            DispatchOutcome::Deferred(retry) => {
                if previous_thread_id != owner_thread_id {
                    self.runtime
                        .select_thread(previous_thread_id)
                        .map_err(error_string)?;
                }
                if retry.request.request_id != request_id
                    || retry.request.service != service
                    || retry.request.payload.len() != 4
                    || self
                        .runtime
                        .scheduler_state_for_thread(owner_thread_id)
                        .map_err(error_string)?
                        != (SchedulerThreadState::WaitingHost { request_id })
                {
                    return Err("timer pacing retry request identity is corrupt".to_owned());
                }
                let milliseconds = u32::from_le_bytes(
                    retry
                        .request
                        .payload
                        .as_slice()
                        .try_into()
                        .expect("four-byte timer delay"),
                );
                if milliseconds == 0 || milliseconds > i32::MAX as u32 {
                    return Err("timer pacing retry delay exceeds the browser transport".to_owned());
                }
                // The core retries the same suspended frame. Keep the original
                // call here and replace only its refreshed browser delay.
                self.pending_host
                    .get_mut(&request_id)
                    .expect("validated pending timer request")
                    .call
                    .request
                    .payload = retry.request.payload;
                return Ok(json!({
                    "kind": "host-retry",
                    "threadId": owner_thread_id.0,
                    "requestId": request_id.to_string(),
                    "service": service,
                    "arguments": [milliseconds],
                }));
            }
            DispatchOutcome::Fault(fault) => {
                restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
                return Err(api_fault_string(fault));
            }
            _ => {
                restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
                return Err("timer pacing completion produced a suspended outcome".to_owned());
            }
        };
        match self.runtime.pop_scheduler_event() {
            Some(RuntimeSchedulerEvent::HostCompleted(completion))
                if completion.request_id == request_id
                    && completion.thread_id == owner_thread_id
                    && completion.service == service => {}
            _ => return Err("timer pacing scheduler completion event mismatch".to_owned()),
        }
        self.pending_host.remove(&request_id);
        let scheduler = self
            .runtime
            .scheduler_state_for_thread(owner_thread_id)
            .map_err(error_string)?;
        if !scheduler.is_executable() {
            return Err("Rust scheduler did not resume the timer pacing request".to_owned());
        }
        restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
        Ok(json!({
            "kind": "host-complete",
            "threadId": owner_thread_id.0,
            "completion": cpu_resume_metadata(&completion),
            "memory": browser_memory_sync(self.runtime.memory()),
        }))
    }

    fn complete_audio_notification_wait(
        &mut self,
        request_id: u64,
        acknowledgement: u32,
    ) -> Result<Value, String> {
        let pending = self
            .pending_host
            .get(&request_id)
            .ok_or_else(|| "audio notification completion has no pending API call".to_owned())?;
        if pending.call.request.request_id != request_id
            || pending.call.request.service != RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE
        {
            return Err("audio notification completion request identity is corrupt".to_owned());
        }
        let previous_thread_id = self.runtime.current_thread_id();
        let owner_thread_id = pending.thread_id;
        if previous_thread_id != owner_thread_id {
            self.runtime
                .select_thread(owner_thread_id)
                .map_err(error_string)?;
        }
        let outcome = complete_kernel32_audio_notification_wait(
            &self.dispatcher,
            &pending.call,
            acknowledgement,
            &mut self.runtime,
        );
        let completion = match outcome {
            DispatchOutcome::Immediate(completion) => completion,
            DispatchOutcome::Deferred(retry) => {
                if previous_thread_id != owner_thread_id {
                    self.runtime
                        .select_thread(previous_thread_id)
                        .map_err(error_string)?;
                }
                if retry.request.request_id != request_id
                    || retry.request.service != RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE
                    || retry.request.payload.len() < 7 * 4
                    || retry.request.payload.len() > (8 + 32) * 4
                    || !retry.request.payload.len().is_multiple_of(4)
                    || self
                        .runtime
                        .scheduler_state_for_thread(owner_thread_id)
                        .map_err(error_string)?
                        != (SchedulerThreadState::WaitingHost { request_id })
                {
                    return Err("audio notification retry request identity is corrupt".to_owned());
                }
                let arguments = retry
                    .request
                    .payload
                    .chunks_exact(4)
                    .map(|word| u32::from_le_bytes(word.try_into().expect("DWORD audio wait")))
                    .collect::<Vec<_>>();
                let (count_index, timeout_index, fixed_words) = match arguments[0] {
                    1 => (5, 4, 8),
                    2 => (3, 2, 6),
                    _ => return Err("audio notification retry mode is invalid".to_owned()),
                };
                let count = arguments[count_index] as usize;
                let milliseconds = arguments[1];
                let timeout = arguments[timeout_index];
                if !(1..=32).contains(&count)
                    || arguments.len() != fixed_words + count
                    || !(1..=100).contains(&timeout)
                    || !(1..=timeout).contains(&milliseconds)
                {
                    return Err("audio notification retry exceeds the browser transport".to_owned());
                }
                // Keep the suspended frame and original request identity; only the
                // Rust-owned delay payload changes as its finite deadline approaches.
                self.pending_host
                    .get_mut(&request_id)
                    .expect("validated pending audio notification request")
                    .call
                    .request
                    .payload = retry.request.payload;
                return Ok(json!({
                    "kind": "host-retry",
                    "threadId": owner_thread_id.0,
                    "requestId": request_id.to_string(),
                    "service": RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE,
                    "arguments": arguments,
                }));
            }
            DispatchOutcome::Fault(fault) => {
                restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
                return Err(api_fault_string(fault));
            }
            _ => {
                restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
                return Err("audio notification completion remained suspended".to_owned());
            }
        };
        match self.runtime.pop_scheduler_event() {
            Some(RuntimeSchedulerEvent::HostCompleted(event))
                if event.request_id == request_id
                    && event.thread_id == owner_thread_id
                    && event.service == RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE => {}
            _ => return Err("audio notification scheduler completion event mismatch".to_owned()),
        }
        self.pending_host.remove(&request_id);
        let scheduler = self
            .runtime
            .scheduler_state_for_thread(owner_thread_id)
            .map_err(error_string)?;
        if !scheduler.is_executable() {
            return Err("Rust scheduler did not resume the audio notification request".to_owned());
        }
        restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
        Ok(json!({
            "kind": "host-complete",
            "threadId": owner_thread_id.0,
            "completion": cpu_resume_metadata(&completion),
            "audioNotificationPacing": true,
            "memory": browser_memory_sync(self.runtime.memory()),
        }))
    }

    fn complete_source_read(
        &mut self,
        response: RuntimeSourceReadHostResponse,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>), String> {
        let request_id = response.request_id;
        let pending = self
            .pending_host
            .get(&request_id)
            .ok_or_else(|| "source read completion has no pending API call".to_owned())?;
        if pending.call.request.request_id != request_id {
            return Err("source read completion request identity is corrupt".to_owned());
        }
        let previous_thread_id = self.runtime.current_thread_id();
        let owner_thread_id = pending.thread_id;
        if previous_thread_id != owner_thread_id {
            self.runtime
                .select_thread(owner_thread_id)
                .map_err(error_string)?;
        }
        let dispatcher = &self.dispatcher;
        let outcome =
            complete_kernel32_source_read(dispatcher, &pending.call, response, &mut self.runtime);
        let completion = match outcome {
            DispatchOutcome::Immediate(completion) => completion,
            DispatchOutcome::Fault(fault) => {
                restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
                return Err(api_fault_string(fault));
            }
            _ => {
                restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
                return Err("source read completion produced a suspended outcome".to_owned());
            }
        };
        match self.runtime.pop_scheduler_event() {
            Some(RuntimeSchedulerEvent::HostCompleted(completion))
                if completion.request_id == request_id
                    && completion.thread_id == owner_thread_id => {}
            _ => return Err("source read scheduler completion event mismatch".to_owned()),
        }
        self.pending_host.remove(&request_id);
        let scheduler = self
            .runtime
            .scheduler_state_for_thread(owner_thread_id)
            .map_err(error_string)?;
        if !scheduler.is_executable() {
            return Err("Rust scheduler did not resume the source read request".to_owned());
        }
        restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
        let delta = self.pending_delta()?;
        Ok((
            json!({
                "kind": "host-complete",
                "threadId": owner_thread_id.0,
                "completion": cpu_resume_metadata(&completion),
            }),
            delta,
        ))
    }

    fn complete_gdi_text_raster(
        &mut self,
        response: Gdi32TextRasterHostResponse,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>), String> {
        let request_id = response.request_id;
        let pending = self
            .pending_host
            .get(&request_id)
            .ok_or_else(|| "text raster completion has no pending API call".to_owned())?;
        if pending.call.request.request_id != request_id
            || pending.call.request.service != RUNTIME_GDI_TEXT_RASTER_SERVICE
        {
            return Err("text raster completion request identity is corrupt".to_owned());
        }
        let previous_thread_id = self.runtime.current_thread_id();
        let owner_thread_id = pending.thread_id;
        if previous_thread_id != owner_thread_id {
            self.runtime
                .select_thread(owner_thread_id)
                .map_err(error_string)?;
        }
        let (outcome, runtime_completion) = complete_gdi32_text_raster(
            &self.dispatcher,
            &pending.call,
            response,
            &mut self.runtime,
        );
        let completion = match outcome {
            DispatchOutcome::Immediate(completion) => completion,
            DispatchOutcome::Fault(fault) => {
                restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
                return Err(api_fault_string(fault));
            }
            _ => {
                restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
                return Err("text raster completion remained suspended".to_owned());
            }
        };
        let runtime_completion = runtime_completion.ok_or_else(|| {
            "TextOutA completed without committing its GDI and scheduler state".to_owned()
        })?;
        match self.runtime.pop_scheduler_event() {
            Some(RuntimeSchedulerEvent::HostCompleted(event)) if event == runtime_completion => {}
            _ => return Err("text raster scheduler completion event mismatch".to_owned()),
        }
        self.pending_host.remove(&request_id);
        let scheduler = self
            .runtime
            .scheduler_state_for_thread(owner_thread_id)
            .map_err(error_string)?;
        if !scheduler.is_executable() {
            return Err("Rust scheduler did not resume the text raster request".to_owned());
        }
        restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
        let delta = self.pending_delta()?;
        let metadata = json!({
            "kind": "host-complete",
            "threadId": owner_thread_id.0,
            "completion": cpu_resume_metadata(&completion),
        });
        Ok((metadata, delta))
    }

    fn complete_callback(
        &mut self,
        sentinel: GuestAddress,
        result: u32,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>, RuntimeOutputBytes), String> {
        let outcome = self
            .runtime
            .complete_guest_callback(&self.dispatcher, sentinel, result)
            .map_err(api_fault_string)?;
        match outcome {
            vine_core::GuestCallbackReturn::Com(outcome) => {
                self.com_outcome(outcome, GuestAddress(0))
            }
            vine_core::GuestCallbackReturn::Completed(completion) => {
                let delta = self.pending_delta()?;
                Ok((
                    completion_metadata("callback-complete", &completion),
                    delta,
                    RuntimeOutputBytes::new(),
                ))
            }
            vine_core::GuestCallbackReturn::Continue(prepared) => {
                let metadata = json!({
                    "kind": "guest-callback",
                    "target": prepared.target.0,
                    "stackPointer": prepared.stack_pointer.0,
                    "returnSentinel": prepared.return_sentinel.0,
                });
                let buffers = self
                    .runtime
                    .memory_mut()
                    .pending_delta_binary()
                    .map_err(error_string)?;
                Ok((metadata, Some(buffers), RuntimeOutputBytes::new()))
            }
            vine_core::GuestCallbackReturn::DllCallback {
                target,
                stack_pointer,
            } => Ok((
                json!({"kind": "dll-attach-callback", "target": target.0, "stackPointer": stack_pointer.0}),
                self.pending_delta()?,
                RuntimeOutputBytes::new(),
            )),
            vine_core::GuestCallbackReturn::DllAttached => Ok((
                json!({"kind": "dll-attach-complete"}),
                self.pending_delta()?,
                RuntimeOutputBytes::new(),
            )),
        }
    }

    fn schedule_next_thread(&mut self) -> Result<Value, String> {
        acknowledge_thread_wakes(&mut self.runtime);
        let previous_thread_id = self.runtime.current_thread_id();
        let Some(selection) = self
            .runtime
            .select_next_executable_thread()
            .map_err(error_string)?
        else {
            return Ok(json!({
                "kind": "scheduler-idle",
                "audioEndPending": self.runtime.has_browser_audio_end_pending(),
                "previousThreadId": previous_thread_id.0,
            }));
        };
        Ok(json!({
            "kind": "thread-scheduled",
            "threadId": selection.0,
        }))
    }

    fn finish_thread(
        &mut self,
        exit_code: u32,
    ) -> Result<(Value, Option<vine_core::MemoryDeltaBuffers>), String> {
        let thread_id = self.runtime.current_thread_id();
        let thread = self.runtime.thread_state(thread_id).map_err(error_string)?;
        if thread.guest_bootstrap.is_none() {
            return Err("guest thread exit has no runtime bootstrap authority".to_owned());
        }
        // A CPU-returned thread can discard queued messages, but must not retire
        // a suspended browser request or callback behind the Worker's back.
        if !self
            .runtime
            .scheduler_state_for_thread(thread_id)
            .map_err(error_string)?
            .is_executable()
            || !thread.callback_frames.is_empty()
            || self
                .pending_host
                .values()
                .any(|pending| pending.thread_id == thread_id)
            || self
                .runtime
                .scheduler()
                .host_requests()
                .values()
                .any(|request| request.thread_id == thread_id)
            || self
                .runtime
                .scheduler()
                .wait_registrations()
                .values()
                .any(|registration| registration.thread_id == thread_id)
        {
            return Err("guest thread returned with suspended browser or callback work".to_owned());
        }
        self.runtime
            .request_current_thread_exit(exit_code)
            .map_err(error_string)?;
        self.runtime
            .complete_current_guest_thread_exit()
            .map_err(error_string)?;
        acknowledge_thread_wakes(&mut self.runtime);
        let scheduler = self
            .runtime
            .scheduler_state_for_thread(thread_id)
            .map_err(error_string)?;
        let metadata = json!({
            "kind": "thread-exit",
            "threadId": thread_id.0,
            "exitCode": exit_code,
            "scheduler": {
                "state": "terminated",
                "exitCode": match scheduler {
                    SchedulerThreadState::Terminated { exit_code } => exit_code,
                    _ => return Err("guest thread did not reach terminated scheduler state".to_owned()),
                },
            },
        });
        let delta = self
            .runtime
            .memory_mut()
            .pending_delta_binary()
            .map_err(error_string)?;
        Ok((metadata, Some(delta)))
    }
}

fn browser_memory_sync(memory: &GuestMemory) -> Value {
    let state = memory.synchronization_state();
    json!({ "generation": state.generation, "acknowledgedGeneration": state.acknowledged_generation })
}

fn browser_memory_regions(memory: &GuestMemory) -> Vec<Value> {
    memory.regions().map(|region| {
        let kind = match region.kind {
            MemoryRegionKind::Image => "image",
            MemoryRegionKind::Stack => "stack",
            MemoryRegionKind::ThreadEnvironmentBlock => "thread-environment-block",
            MemoryRegionKind::ProcessEnvironmentBlock => "process-environment-block",
            MemoryRegionKind::ProcessParameters => "process-parameters",
            MemoryRegionKind::CommandLine => "command-line",
            MemoryRegionKind::Heap => "heap",
            MemoryRegionKind::Anonymous => "anonymous",
            MemoryRegionKind::Runtime => "runtime",
            MemoryRegionKind::Surface => "surface",
            MemoryRegionKind::GraphicsResource => "graphics-resource",
            MemoryRegionKind::AudioResource => "audio-resource",
            MemoryRegionKind::GdiResource => "gdi-resource",
        };
        let mut value = json!({
            "owner": region.owner.0,
            "kind": kind,
            "range": { "start": region.range.start().0, "byteLength": region.range.byte_length() },
        });
        if let Some(id) = region.address_allocation_id {
            value["addressAllocationId"] = json!(id);
        }
        value
    }).collect()
}

fn runtime_system_module(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "kernel32.dll"
            | "user32.dll"
            | "gdi32.dll"
            | "gdiplus.dll"
            | "imm32.dll"
            | "shell32.dll"
            | "comdlg32.dll"
            | "advapi32.dll"
            | "wininet.dll"
            | "ole32.dll"
            | "atl.dll"
            | "d3d9.dll"
            | "dinput8.dll"
            | "dsound.dll"
            | "winmm.dll"
    )
}

fn guest_import_names(loader: &vine_core::Pe32LoadPlan) -> Vec<String> {
    let mut seen = BTreeSet::new();
    loader
        .imports
        .iter()
        .filter(|module| !runtime_system_module(&module.name))
        .filter(|module| seen.insert(module.name.to_ascii_lowercase()))
        .map(|module| module.name.clone())
        .collect()
}

fn runtime_catalog_for_load_plan(
    loaders: &[&vine_core::Pe32LoadPlan],
) -> Result<RuntimeExportCatalog, String> {
    let mut keys = BTreeSet::new();
    let mut exports = Vec::new();
    for module in loaders.iter().flat_map(|loader| &loader.imports) {
        if !runtime_system_module(&module.name) {
            continue;
        }
        for symbol in &module.symbols {
            let export = match &symbol.target {
                ImportTarget::Name { name, .. } => {
                    RuntimeExport::named(module.name.to_ascii_lowercase(), name.clone())
                }
                ImportTarget::Ordinal { ordinal } => {
                    RuntimeExport::ordinal(module.name.to_ascii_lowercase(), *ordinal)
                }
            };
            if keys.insert(export.clone()) {
                exports.push(export);
            }
        }
    }
    RuntimeExportCatalog::new(exports).map_err(error_string)
}

fn guest_image_load_base(
    memory: &GuestMemory,
    preferred: u32,
    size_of_image: u32,
) -> Result<GuestAddress, String> {
    let length = u64::from(size_of_image);
    let available = |base: u32| {
        let start = u64::from(base);
        let Some(end) = start.checked_add(length) else {
            return false;
        };
        let thread_stacks = u64::from(vine_core::RUNTIME_GUEST_THREAD_STACK_ARENA_BASE.0);
        start >= 0x0001_0000
            && base.is_multiple_of(0x1_0000)
            && length != 0
            && end <= 0x7fff_0000
            && ![
                (0x0100_0000_u64, 0x1000_0000_u64),
                (0x2000_0000, 0x4e00_0000),
                // Import/callback/export pages, thread environments and callback scratch.
                (0x7f00_0000, 0x7fc8_0000),
                (
                    thread_stacks,
                    thread_stacks + vine_core::RUNTIME_GUEST_THREAD_STACK_ARENA_SIZE,
                ),
            ]
            .iter()
            .any(|&(left, right)| start < right && left < end)
            && memory
                .regions()
                .all(|region| start >= region.range.end() || region.range.start().0 as u64 >= end)
    };
    if available(preferred) {
        return Ok(GuestAddress(preferred));
    }
    // Leave the entire guest thread-stack arena free even before any thread exists.
    for slot in 0..16_u32 {
        let base = 0x6000_0000 + slot * 0x0100_0000;
        if available(base) {
            return Ok(GuestAddress(base));
        }
    }
    Err("no free PE32 guest image range outside runtime-owned memory".to_owned())
}

fn virtual_executable_path(relative_path: &str) -> Result<Vec<u8>, String> {
    let mut path = b"C:\\Vine\\".to_vec();
    for component in relative_path.split(['\\', '/']) {
        if component.is_empty()
            || component.encode_utf16().count() > 255
            || component == "."
            || component == ".."
            || component.ends_with([' ', '.'])
            || component
                .chars()
                .any(|c| c.is_control() || ":*?\"<>|".contains(c))
        {
            return Err("selected executable path is not a Windows relative path".to_owned());
        }
        let (encoded, _, had_errors) = SHIFT_JIS.encode(component);
        if had_errors {
            return Err("selected executable path is not representable in Windows-932".to_owned());
        }
        // A Windows-932 trail byte may be 0x5c, but an encoded Yen sign
        // must not become an unescaped separator in the virtual DOS path.
        let mut index = 0;
        while index < encoded.len() {
            let byte = encoded[index];
            if (0x81..=0x9f).contains(&byte) || (0xe0..=0xfc).contains(&byte) {
                index += 2;
            } else {
                if matches!(byte, b'\\' | b'/' | b':') {
                    return Err("selected executable path encodes a separator".to_owned());
                }
                index += 1;
            }
        }
        path.extend_from_slice(&encoded);
        path.push(b'\\');
        if path.len() > vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_MAX_BYTES {
            return Err("selected executable path exceeds the DOS path limit".to_owned());
        }
    }
    path.pop();
    path.push(0);
    Ok(path)
}

#[allow(clippy::too_many_arguments)] // The existing bootstrap still carries separate loader and process state.
fn prepare_executable_runtime_process(
    memory: GuestMemory,
    loader: &vine_core::Pe32LoadPlan,
    materialization: &vine_core::ImageMaterialization,
    source_dlls: &[MappedSourceDll],
    runtime_catalog: &RuntimeExportCatalog,
    encoded_pointer_cookie: u32,
    executable_name: &str,
    virtual_image_path: Vec<u8>,
) -> Result<PreparedExecutableProcess, String> {
    let mut runtime =
        controlled_executable_runtime_state(memory, encoded_pointer_cookie, virtual_image_path)?;
    let mut module_handles = vec![SYSTEM_KERNEL32_HANDLE];
    let mut canonical_modules = BTreeSet::from(["kernel32.dll".to_owned()]);
    let mut next_module_slot = 2_u16;
    for imported in std::iter::once(loader)
        .chain(source_dlls.iter().map(|dll| &dll.source.loader))
        .flat_map(|loader| &loader.imports)
    {
        let canonical_name = imported.name.to_ascii_lowercase();
        if !canonical_modules.insert(canonical_name.clone()) {
            continue;
        }
        let handle = executable_module_handle(next_module_slot)?;
        next_module_slot = next_module_slot
            .checked_add(1)
            .ok_or_else(|| "runtime module handle slots are exhausted".to_owned())?;
        runtime
            .load_module(RuntimeModule {
                handle,
                requested_name: imported.name.clone(),
                canonical_name,
                runtime_system_module: runtime_system_module(&imported.name),
            })
            .map_err(error_string)?;
        module_handles.push(handle);
    }
    let main_module_handle = executable_module_handle(next_module_slot)?;
    runtime
        .register_main_module(RuntimeModule {
            handle: main_module_handle,
            requested_name: executable_name.to_owned(),
            canonical_name: executable_name.to_ascii_lowercase(),
            runtime_system_module: false,
        })
        .map_err(error_string)?;
    module_handles.push(main_module_handle);
    let mut dll_bindings = Vec::new();
    let mut bound_names = BTreeSet::new();
    while bound_names.len() < source_dlls.len() {
        let Some(dll) = source_dlls.iter().find(|dll| {
            !bound_names.contains(&dll.source.name.to_ascii_lowercase())
                && guest_import_names(&dll.source.loader)
                    .iter()
                    .all(|dependency| bound_names.contains(&dependency.to_ascii_lowercase()))
        }) else {
            return Err(
                "circular source DLL imports require a loader cycle implementation".to_owned(),
            );
        };
        let name = dll.source.name.to_ascii_lowercase();
        let handle = runtime
            .module_by_name(&name)
            .map_err(error_string)?
            .ok_or_else(|| format!("registered source DLL {name} is missing"))?
            .handle;
        let binding = runtime
            .bind_guest_module_imports(handle, &dll.source.loader, &dll.image, runtime_catalog)
            .map_err(error_string)?;
        runtime
            .register_module_image(handle, &dll.source.loader, &dll.image)
            .map_err(error_string)?;
        dll_bindings.push(binding);
        bound_names.insert(name);
    }
    let binding = runtime
        .bind_guest_module_imports(main_module_handle, loader, materialization, runtime_catalog)
        .map_err(error_string)?;
    runtime
        .register_module_image(main_module_handle, loader, materialization)
        .map_err(error_string)?;
    Ok(PreparedExecutableProcess {
        runtime,
        binding,
        dll_bindings,
        module_handles,
    })
}

fn controlled_executable_runtime_state(
    memory: GuestMemory,
    encoded_pointer_cookie: u32,
    virtual_image_path: Vec<u8>,
) -> Result<Runtime, String> {
    let mut startup_info = vec![0; 68];
    startup_info[..4].copy_from_slice(&68_u32.to_le_bytes());
    let mut runtime = Runtime::new(
        RuntimeBootstrap {
            process_id: RuntimeProcessId(PROCESS_ID),
            initial_thread_id: RuntimeThreadId(THREAD_ID),
            initial_last_error: 87,
            clock: RuntimeClockState {
                wall_clock_file_time: 134_282_016_000_000_000,
                tick_count: 3_600_000,
                performance_counter: 12_345_678_901,
                performance_frequency: 10_000_000,
            },
            startup_info: RuntimeStartupInfo {
                bytes: startup_info,
            },
            process_image_path_a: vine_core::RuntimeProcessImagePathA {
                bytes_with_nul: virtual_image_path,
            },
            initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
                bytes_with_nul: b"C:\\Vine\0".to_vec(),
            },
            encoded_pointer_cookie,
        },
        memory,
    )
    .map_err(error_string)?;
    runtime
        .load_module(RuntimeModule {
            handle: SYSTEM_KERNEL32_HANDLE,
            requested_name: "KERNEL32.DLL".to_owned(),
            canonical_name: "kernel32.dll".to_owned(),
            runtime_system_module: true,
        })
        .map_err(error_string)?;
    runtime.start_initial_thread().map_err(error_string)?;
    Ok(runtime)
}

fn executable_module_handle(slot: u16) -> Result<u32, String> {
    if slot == 0 {
        return Err("runtime module handle slot cannot be zero".to_owned());
    }
    Ok(0x4d01_0000 | u32::from(slot))
}

fn finalize_executable_runtime_process(
    process: &mut PreparedExecutableProcess,
    dynamic_thunks: &[vine_core::DynamicExportThunk],
    callback_return_sentinel: GuestAddress,
) -> Result<RuntimeDllLifecycleAction, String> {
    for thunk in dynamic_thunks {
        process
            .runtime
            .register_dynamic_export_thunk(thunk)
            .map_err(error_string)?;
    }
    for handle in &process.module_handles {
        process
            .runtime
            .mark_module_ready(*handle)
            .map_err(error_string)?;
    }
    let action = process
        .runtime
        .begin_dll_process_attach(callback_return_sentinel)
        .map_err(error_string)?;
    if let RuntimeDllLifecycleAction::Complete { phase } = &action
        && *phase != RuntimeDllLifecyclePhase::ProcessAttached
    {
        return Err("module graph did not complete process attach".to_owned());
    }
    Ok(action)
}

fn control_mapping_allocation(
    runtime: &Runtime,
    purpose: ProcessAddressAllocationPurpose,
) -> Result<ProcessAddressAllocationRecord, String> {
    runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&purpose)
        .cloned()
        .map_err(error_string)
}

fn cpu_resume_metadata(completion: &ApiImmediateCompletion) -> Value {
    let mut resume = json!({
        "instructionPointer": completion.instruction_pointer.0,
        "stackPointer": completion.stack_pointer.0,
        "eax": completion.eax,
    });
    match completion.execution_boundary {
        vine_core::RuntimeApiBoundaryDisposition::ContinueQuantum => {}
        vine_core::RuntimeApiBoundaryDisposition::YieldCurrentThread => {
            resume["executionBoundary"] = json!("yield-current-thread");
        }
        vine_core::RuntimeApiBoundaryDisposition::BlockCurrentThread => {
            resume["executionBoundary"] = json!("block-current-thread");
        }
        vine_core::RuntimeApiBoundaryDisposition::TerminateCurrentThread => {
            resume["executionBoundary"] = json!("terminate-current-thread");
        }
    }
    resume
}

fn completion_metadata(kind: &str, completion: &ApiImmediateCompletion) -> Value {
    json!({
        "kind": kind,
        "completion": cpu_resume_metadata(completion),
    })
}

fn decode_u32_payload(payload: &[u8]) -> Vec<u32> {
    payload
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("four-byte host payload word")))
        .collect()
}

fn acknowledge_thread_wakes(runtime: &mut Runtime) {
    // Rust has already committed these wakeups. The browser reads runnable state
    // from the scheduler; do not leave notifications in front of a host receipt.
    while matches!(
        runtime.scheduler().events().front(),
        Some(RuntimeSchedulerEvent::WaitSatisfied(_) | RuntimeSchedulerEvent::MessageReady(_))
    ) {
        runtime.pop_scheduler_event();
    }
}

fn restore_selected_thread(
    runtime: &mut Runtime,
    previous_thread_id: RuntimeThreadId,
    owner_thread_id: RuntimeThreadId,
) -> Result<(), String> {
    if previous_thread_id != owner_thread_id
        && runtime
            .thread_state(previous_thread_id)
            .is_ok_and(|thread| thread.lifecycle == vine_core::RuntimeThreadLifecycleState::Running)
        && runtime
            .scheduler_state_for_thread(previous_thread_id)
            .is_ok_and(SchedulerThreadState::is_executable)
    {
        runtime
            .select_thread(previous_thread_id)
            .map_err(error_string)?;
    }
    Ok(())
}

fn api_fault_string(fault: ApiFault) -> String {
    match fault.endpoint {
        Some(endpoint) => format!("{endpoint}: {}", fault.message),
        None => fault.message,
    }
}

fn error_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn media_request_metadata(pending: &ComMediaDeferred) -> Value {
    let operation = match pending.operation {
        ComMediaOperation::Open => "open",
        ComMediaOperation::NextFrame => "next-frame",
        ComMediaOperation::AudioPush => "audio-push",
        ComMediaOperation::Run => "run",
        ComMediaOperation::Pause => "pause",
        ComMediaOperation::Stop => "stop",
        ComMediaOperation::Volume => "volume",
        ComMediaOperation::Release => "release",
    };
    let source = pending.source.as_ref().map(|source| match source {
        ComMediaSource::GuestAsyncReader { byte_length, .. } => {
            json!({ "kind": "guest-reader", "byteLength": byte_length.to_string() })
        }
        ComMediaSource::SourceFile {
            source_id,
            object_id,
            byte_length,
        } => json!({ "kind": "source-file", "sourceId": source_id,
                "objectId": object_id, "byteLength": byte_length.to_string() }),
        ComMediaSource::WritableFile { relative_path } => {
            json!({ "kind": "writable-file", "relativePath": relative_path })
        }
    });
    let audio = pending.audio.as_ref().map(|audio| {
        json!({
            "start100ns": audio.start_100ns, "end100ns": audio.end_100ns,
            "channels": audio.channels, "samplesPerSecond": audio.samples_per_second,
            "byteLength": audio.pcm_f32le.len(),
        })
    });
    json!({ "kind": "media-request", "requestId": pending.request_id.to_string(),
        "threadId": pending.thread_id.0, "operation": operation,
        "resourceId": pending.resource_id.map(|id| id.to_string()),
        "source": source, "volume": pending.volume,
        "delayMilliseconds": pending.delay_milliseconds, "audio": audio })
}

fn parse_media_response(metadata: &[u8], payload: &[u8]) -> Result<ComMediaResponse, String> {
    let value: Value = serde_json::from_slice(metadata).map_err(error_string)?;
    let kind = value["kind"]
        .as_str()
        .ok_or_else(|| "media response has no kind".to_owned())?;
    if !matches!(kind, "frame" | "audio") && !payload.is_empty() {
        return Err("media control completion has unexpected binary bytes".to_owned());
    }
    let resource_id = || -> Result<u64, String> {
        let text = value["resourceId"]
            .as_str()
            .ok_or_else(|| "media resource ID is not a decimal string".to_owned())?;
        if text.is_empty()
            || text.len() > 20
            || text.starts_with('0')
            || !text.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err("media resource ID is invalid".to_owned());
        }
        text.parse().map_err(error_string)
    };
    let word = |key: &str| -> Result<u32, String> {
        value[key]
            .as_u64()
            .and_then(|number| u32::try_from(number).ok())
            .ok_or_else(|| format!("media {key} is not an unsigned 32-bit integer"))
    };
    let timestamp = |key: &str| -> Result<i64, String> {
        value[key]
            .as_i64()
            .filter(|number| number.unsigned_abs() <= 9_007_199_254_740_991)
            .ok_or_else(|| format!("media {key} is not an exact browser integer"))
    };
    match kind {
        "opened" => Ok(ComMediaResponse::Opened(ComMediaFormat {
            resource_id: resource_id()?,
            width: word("width")?,
            height: word("height")?,
            duration_100ns: if value["duration100ns"].is_null() {
                None
            } else {
                Some(timestamp("duration100ns")?)
            },
            frame_duration_100ns: timestamp("frameDuration100ns")?,
            audio_channels: word("audioChannels")?,
            audio_samples_per_second: word("audioSamplesPerSecond")?,
        })),
        "frame" => Ok(ComMediaResponse::Frame(ComMediaFrame {
            resource_id: resource_id()?,
            start_100ns: timestamp("start100ns")?,
            end_100ns: timestamp("end100ns")?,
            bgra: payload.to_vec(),
        })),
        "audio" => Ok(ComMediaResponse::Audio(ComMediaAudio {
            resource_id: resource_id()?,
            start_100ns: timestamp("start100ns")?,
            end_100ns: timestamp("end100ns")?,
            channels: word("channels")?,
            samples_per_second: word("samplesPerSecond")?,
            pcm_f32le: payload.to_vec(),
        })),
        "acknowledged" => Ok(ComMediaResponse::Acknowledged),
        "file-not-found" => Ok(ComMediaResponse::FileNotFound),
        "ended" => Ok(ComMediaResponse::Ended {
            resource_id: resource_id()?,
        }),
        "stopped" => Ok(ComMediaResponse::Stopped {
            resource_id: resource_id()?,
        }),
        "unsupported" | "backend-failed" => {
            let message = value["message"]
                .as_str()
                .filter(|message| !message.is_empty() && message.len() <= 512)
                .ok_or_else(|| "media unsupported response has no bounded detail".to_owned())?;
            if kind == "backend-failed" {
                Ok(ComMediaResponse::BackendFailed {
                    message: message.to_owned(),
                })
            } else {
                Ok(ComMediaResponse::Unsupported {
                    message: message.to_owned(),
                })
            }
        }
        _ => Err("media response kind is unsupported".to_owned()),
    }
}

#[cfg(feature = "shared-guest-memory")]
pub fn shared_arena_descriptor() -> Option<(u32, u32, u32)> {
    BRIDGE.with(|bridge| {
        bridge
            .borrow()
            .runtime
            .as_ref()?
            .runtime
            .memory()
            .shared_arena_descriptor()
    })
}

#[cfg(feature = "shared-guest-memory")]
pub fn shared_permissions_pointer() -> Option<u32> {
    BRIDGE.with(|bridge| {
        bridge
            .borrow()
            .runtime
            .as_ref()?
            .runtime
            .memory()
            .shared_permissions_pointer()
    })
}

#[cfg(feature = "shared-guest-memory")]
pub fn with_shared_execution(generation: u64, execute: impl FnOnce() -> u32) -> u32 {
    BRIDGE.with(|cell| {
        // An imported JIT compiler may run while execute() is on the stack.
        // Holding this borrow prevents any reentrant Runtime entry from exposing
        // page references or changing ownership underneath the CPU.
        let mut bridge = cell.borrow_mut();
        let validation = bridge.runtime_mut().and_then(|runtime| {
            runtime
                .runtime
                .memory()
                .validate_shared_execution(generation)
                .map_err(error_string)
        });
        if let Err(error) = validation {
            bridge.set_error(error);
            return u32::MAX;
        }
        execute()
    })
}

#[cfg(feature = "shared-guest-memory")]
pub enum SharedDispatch {
    Unhandled,
    Resume([u32; 4]),
    Output,
    Error,
}

/// Execute an ordinary Windows entry inside the unified module. Thread,
/// callback, browser and mapping boundaries still publish the normal receipt.
#[cfg(feature = "shared-guest-memory")]
pub fn dispatch_shared(
    context: X86CpuContext,
    retired: u64,
    elapsed: u64,
    mut invalidate: impl FnMut(Option<u32>),
) -> SharedDispatch {
    BRIDGE.with(|cell| {
        let mut bridge = cell.borrow_mut();
        let result = (|| -> Result<SharedDispatch, String> {
            let runtime = bridge.runtime_mut()?;
            if runtime
                .dispatcher
                .endpoint(GuestAddress(context.eip))
                .is_none()
            {
                return Ok(SharedDispatch::Unhandled);
            }
            let return_bytes = runtime
                .runtime
                .memory()
                .read(GuestAddress(context.esp), 4)
                .map_err(error_string)?;
            let return_address =
                u32::from_le_bytes(return_bytes.try_into().expect("four-byte return address"));
            runtime.synchronize_forward_clock(retired, elapsed)?;
            let (metadata, delta, bytes) = runtime.dispatch(
                GuestAddress(context.eip),
                GuestAddress(context.esp),
                GuestAddress(return_address),
                retired,
                context,
            )?;
            if let DispatchMetadata::ApiCompletion(completion) = metadata {
                if delta.is_none() {
                    return Ok(SharedDispatch::Resume(completion));
                }
                if let Some((generation, addresses)) =
                    runtime.runtime.memory().shared_data_changes()
                {
                    if addresses.is_empty() {
                        invalidate(None);
                    }
                    for address in addresses {
                        invalidate(Some(address));
                    }
                    runtime.acknowledge_fast(generation)?;
                    return Ok(SharedDispatch::Resume(completion));
                }
                bridge.clear_output();
                bridge.api_completion = Some(completion);
                let (control, payload) =
                    delta.map(|d| (d.control, d.page_bytes)).unwrap_or_default();
                bridge.control = control;
                bridge.payload = payload;
            } else if let DispatchMetadata::Json(metadata) = metadata {
                bridge.clear_output();
                bridge.set_runtime_output(metadata, delta, bytes)?;
            }
            Ok(SharedDispatch::Output)
        })();
        match result {
            Ok(result) => result,
            Err(error) => {
                bridge.set_error(error);
                SharedDispatch::Error
            }
        }
    })
}

#[cfg(feature = "shared-guest-memory")]
pub fn shared_acknowledged_generation() -> u64 {
    BRIDGE.with(|cell| {
        cell.borrow().runtime.as_ref().map_or(0, |runtime| {
            runtime
                .runtime
                .memory()
                .synchronization_state()
                .acknowledged_generation
        })
    })
}

#[cfg(feature = "shared-guest-memory")]
#[unsafe(no_mangle)]
pub extern "C" fn vine_shared_generation() -> u64 {
    shared_acknowledged_generation()
}

#[cfg(feature = "shared-guest-memory")]
#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_shared_arena_base() -> u32 {
    shared_arena_descriptor().map_or(0, |arena| arena.0)
}

#[cfg(feature = "shared-guest-memory")]
#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_shared_arena_size() -> u32 {
    shared_arena_descriptor().map_or(0, |arena| arena.1)
}

#[cfg(feature = "shared-guest-memory")]
#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_shared_guest_offset() -> u32 {
    shared_arena_descriptor().map_or(0, |arena| arena.2)
}

fn run_bridge(operation: impl FnOnce(&mut Bridge) -> Result<(), String>) -> u32 {
    BRIDGE.with(|cell| {
        let mut bridge = cell.borrow_mut();
        bridge.clear_output();
        match operation(&mut bridge) {
            Ok(()) => 0,
            Err(error) => {
                bridge.set_error(error);
                1
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_input_resize(byte_length: u32) -> u32 {
    run_bridge(|bridge| {
        let byte_length = byte_length as usize;
        if byte_length > MAX_INPUT_BYTES {
            return Err("bridge input exceeds the 16 MiB limit".to_owned());
        }
        bridge.input.resize(byte_length, 0);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_input_ptr() -> u32 {
    BRIDGE.with(|cell| cell.borrow_mut().input.as_mut_ptr() as usize as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_cpu_snapshot_put(
    thread_id: u32,
    expected_revision_low: u32,
    expected_revision_high: u32,
    backend_tag: u32,
    abi_version: u32,
    input_length: u32,
) -> u32 {
    run_bridge(|bridge| {
        bridge.put_cpu_snapshot(
            thread_id,
            u64::from(expected_revision_low) | (u64::from(expected_revision_high) << 32),
            backend_tag,
            abi_version,
            input_length as usize,
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_cpu_snapshot_get(thread_id: u32) -> u32 {
    run_bridge(|bridge| bridge.get_cpu_snapshot(thread_id))
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_metadata_ptr() -> u32 {
    BRIDGE.with(|cell| cell.borrow().metadata.as_ptr() as usize as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_metadata_len() -> u32 {
    BRIDGE.with(|cell| cell.borrow().metadata.len() as u32)
}

/// The fixed EIP/ESP/EAX/has-EAX output is borrowed until the next bridge operation.
#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_api_completion_ptr() -> u32 {
    BRIDGE.with(|cell| {
        cell.borrow()
            .api_completion
            .as_ref()
            .map_or(0, |completion| completion.as_ptr() as usize as u32)
    })
}

/// Read-only stop-boundary diagnostics use separate output so the original
/// fault and any unacknowledged runtime output remain available to the caller.
#[cold]
#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_stop_context(instruction_pointer: u32) -> u32 {
    BRIDGE.with(|cell| u32::from(cell.borrow_mut().stop_context(instruction_pointer).is_err()))
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_stop_context_ptr() -> u32 {
    BRIDGE.with(|cell| cell.borrow().stop_context_metadata.as_ptr() as usize as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_stop_context_len() -> u32 {
    BRIDGE.with(|cell| cell.borrow().stop_context_metadata.len() as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_control_ptr() -> u32 {
    BRIDGE.with(|cell| cell.borrow().control.as_ptr() as usize as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_control_len() -> u32 {
    BRIDGE.with(|cell| cell.borrow().control.len() as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_payload_ptr() -> u32 {
    BRIDGE.with(|cell| cell.borrow().payload.as_ptr() as usize as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_payload_len() -> u32 {
    BRIDGE.with(|cell| cell.borrow().payload.len() as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_presentation_ptr() -> u32 {
    BRIDGE.with(|cell| cell.borrow().presentation_bytes.as_ptr() as usize as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_presentation_len() -> u32 {
    BRIDGE.with(|cell| cell.borrow().presentation_bytes.len() as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_audio_ptr() -> u32 {
    BRIDGE.with(|cell| cell.borrow().audio_bytes.as_ptr() as usize as u32)
}
#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_audio_len() -> u32 {
    BRIDGE.with(|cell| cell.borrow().audio_bytes.len() as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_media_ptr() -> u32 {
    BRIDGE.with(|cell| cell.borrow().media_bytes.as_ptr() as usize as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_media_len() -> u32 {
    BRIDGE.with(|cell| cell.borrow().media_bytes.len() as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_inspect_executable(executable_length: u32, name_length: u32) -> u32 {
    run_bridge(|bridge| bridge.inspect_executable(executable_length as usize, name_length as usize))
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_register_source_dll(image_length: u32, path_length: u32) -> u32 {
    run_bridge(|bridge| bridge.register_source_dll(image_length as usize, path_length as usize))
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_ready_object_wait_request_id() -> u64 {
    BRIDGE.with(|cell| {
        cell.borrow()
            .runtime
            .as_ref()
            .map_or(0, |runtime| runtime.runtime.ready_object_wait_request_id())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_prepare_executable(
    executable_length: u32,
    path_length: u32,
    encoded_pointer_cookie: u32,
) -> u32 {
    run_bridge(|bridge| {
        bridge.prepare_executable(
            executable_length as usize,
            path_length as usize,
            encoded_pointer_cookie,
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_complete_source_lookup(input_length: u32) -> u32 {
    run_bridge(|bridge| {
        let input = bridge
            .input
            .get(..input_length as usize)
            .ok_or_else(|| "source lookup completion exceeds the input buffer".to_owned())?;
        if input.len() > 2 * 1024 * 1024 {
            return Err("source lookup completion is too large".to_owned());
        }
        let result: Option<Value> =
            serde_json::from_slice(input).map_err(|error| error.to_string())?;
        let metadata = bridge.runtime_mut()?.complete_source_lookup(result)?;
        bridge.set_output(metadata, Vec::new(), Vec::new())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_synchronize_forward_clock(
    retired_instructions_low: u32,
    retired_instructions_high: u32,
    host_elapsed_time_100ns_low: u32,
    host_elapsed_time_100ns_high: u32,
) -> u32 {
    run_bridge(|bridge| {
        let retired_instructions =
            u64::from(retired_instructions_low) | (u64::from(retired_instructions_high) << 32);
        let host_elapsed_time_100ns = u64::from(host_elapsed_time_100ns_low)
            | (u64::from(host_elapsed_time_100ns_high) << 32);
        bridge
            .runtime_mut()?
            .synchronize_forward_clock(retired_instructions, host_elapsed_time_100ns)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_read_timestamp_counter(
    retired_instructions_low: u32,
    retired_instructions_high: u32,
    host_elapsed_time_100ns_low: u32,
    host_elapsed_time_100ns_high: u32,
) -> u32 {
    run_bridge(|bridge| {
        let retired =
            u64::from(retired_instructions_low) | (u64::from(retired_instructions_high) << 32);
        let elapsed = u64::from(host_elapsed_time_100ns_low)
            | (u64::from(host_elapsed_time_100ns_high) << 32);
        let timestamp = bridge
            .runtime_mut()?
            .runtime
            .read_timestamp_counter(retired, elapsed)
            .map_err(error_string)?;
        bridge.set_output(
            json!({ "kind": "timestamp-counter", "low": timestamp as u32,
            "high": (timestamp >> 32) as u32 }),
            Vec::new(),
            Vec::new(),
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_acknowledge(generation_low: u32, generation_high: u32) -> u32 {
    run_bridge(|bridge| {
        let generation = u64::from(generation_low) | (u64::from(generation_high) << 32);
        let metadata = bridge.runtime_mut()?.acknowledge(generation)?;
        bridge.set_output(metadata, Vec::new(), Vec::new())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_acknowledge_fast(generation_low: u32, generation_high: u32) -> u32 {
    run_bridge(|bridge| {
        let generation = u64::from(generation_low) | (u64::from(generation_high) << 32);
        bridge.runtime_mut()?.acknowledge_fast(generation)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_reconcile_writeback(control_length: u32, payload_length: u32) -> u32 {
    run_bridge(|bridge| {
        bridge.reconcile_writeback_input(control_length as usize, payload_length as usize)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_writeback_generation_low() -> u32 {
    BRIDGE.with(|cell| {
        cell.borrow()
            .writeback_generation
            .map_or(0, |generation| generation as u32)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_writeback_generation_high() -> u32 {
    BRIDGE.with(|cell| {
        cell.borrow()
            .writeback_generation
            .map_or(0, |generation| (generation >> 32) as u32)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_service_page_fault(
    fault_address: u32,
    page_fault_error_code: u32,
) -> u32 {
    run_bridge(|bridge| {
        let (metadata, delta) = bridge
            .runtime_mut()?
            .service_page_fault(GuestAddress(fault_address), page_fault_error_code)?;
        let (control, payload) = delta
            .map(|buffers| (buffers.control, buffers.page_bytes))
            .unwrap_or_default();
        bridge.set_output(metadata, control, payload)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_dispatch(
    endpoint_address: u32,
    stack_pointer: u32,
    return_address: u32,
    retired_instructions_low: u32,
    retired_instructions_high: u32,
    eax: u32,
    ecx: u32,
    edx: u32,
    ebx: u32,
    esp: u32,
    ebp: u32,
    esi: u32,
    edi: u32,
    eip: u32,
    eflags: u32,
    es: u32,
    cs: u32,
    ss: u32,
    ds: u32,
    fs: u32,
    gs: u32,
) -> u32 {
    run_bridge(|bridge| {
        let retired_instructions =
            u64::from(retired_instructions_low) | (u64::from(retired_instructions_high) << 32);
        let (metadata, delta, pixels) = bridge.runtime_mut()?.dispatch(
            GuestAddress(endpoint_address),
            GuestAddress(stack_pointer),
            GuestAddress(return_address),
            retired_instructions,
            X86CpuContext {
                eax,
                ecx,
                edx,
                ebx,
                esp,
                ebp,
                esi,
                edi,
                eip,
                eflags,
                es,
                cs,
                ss,
                ds,
                fs,
                gs,
            },
        )?;
        match metadata {
            DispatchMetadata::Json(metadata) => bridge.set_runtime_output(metadata, delta, pixels),
            DispatchMetadata::ApiCompletion(completion) => {
                let (control, payload) = delta
                    .map(|buffers| (buffers.control, buffers.page_bytes))
                    .unwrap_or_default();
                bridge.api_completion = Some(completion);
                bridge.control = control;
                bridge.payload = payload;
                Ok(())
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_complete_com_audio(low: u32, high: u32, accepted: u32) -> u32 {
    run_bridge(|bridge| {
        if accepted > 1 {
            return Err("invalid audio acknowledgement".to_owned());
        }
        let (metadata, delta) = bridge
            .runtime_mut()?
            .complete_com_audio(u64::from(low) | (u64::from(high) << 32), accepted == 1)?;
        let (control, payload) = delta
            .map(|buffers| (buffers.control, buffers.page_bytes))
            .unwrap_or_default();
        bridge.set_output(metadata, control, payload)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_read_com_media(
    request_low: u32,
    request_high: u32,
    offset_low: u32,
    offset_high: u32,
    length: u32,
) -> u32 {
    run_bridge(|bridge| {
        let request_id = u64::from(request_low) | (u64::from(request_high) << 32);
        let offset = u64::from(offset_low) | (u64::from(offset_high) << 32);
        let (metadata, delta, bytes) = bridge
            .runtime_mut()?
            .read_com_media(request_id, offset, length)?;
        bridge.set_runtime_output(metadata, delta, bytes)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_complete_com_media(
    request_low: u32,
    request_high: u32,
    metadata_length: u32,
    payload_length: u32,
) -> u32 {
    run_bridge(|bridge| {
        let metadata_length = metadata_length as usize;
        let payload_length = payload_length as usize;
        let total = metadata_length
            .checked_add(payload_length)
            .ok_or_else(|| "media input length overflow".to_owned())?;
        if metadata_length == 0 || metadata_length > 4096 || total > MAX_INPUT_BYTES {
            return Err("media completion exceeds its input bounds".to_owned());
        }
        let input = bridge
            .input
            .get(..total)
            .ok_or_else(|| "media completion exceeds the bridge input buffer".to_owned())?;
        let response = parse_media_response(&input[..metadata_length], &input[metadata_length..])?;
        let request_id = u64::from(request_low) | (u64::from(request_high) << 32);
        let (metadata, delta, bytes) = bridge
            .runtime_mut()?
            .complete_com_media(request_id, response)?;
        bridge.set_runtime_output(metadata, delta, bytes)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_observe_audio_end(low: u32, high: u32) -> u32 {
    run_bridge(|bridge| {
        let metadata = bridge
            .runtime_mut()?
            .observe_audio_end(u64::from(low) | (u64::from(high) << 32))?;
        bridge.set_output(metadata, Vec::new(), Vec::new())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_complete_host(
    request_low: u32,
    request_high: u32,
    result: u32,
) -> u32 {
    run_bridge(|bridge| {
        let request_id = u64::from(request_low) | (u64::from(request_high) << 32);
        let metadata = bridge.runtime_mut()?.complete_host(request_id, result)?;
        bridge.set_output(metadata, Vec::new(), Vec::new())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_complete_opfs_write(
    request_low: u32,
    request_high: u32,
    observation: u32,
) -> u32 {
    run_bridge(|bridge| {
        let request_id = u64::from(request_low) | (u64::from(request_high) << 32);
        let (metadata, delta) = bridge
            .runtime_mut()?
            .complete_opfs_write(request_id, observation)?;
        let (control, payload) = delta
            .map(|buffers| (buffers.control, buffers.page_bytes))
            .unwrap_or_default();
        bridge.set_output(metadata, control, payload)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_complete_opfs_open(
    request_low: u32,
    request_high: u32,
    observation: u32,
    size_low: u32,
    size_high: u32,
) -> u32 {
    run_bridge(|bridge| {
        let request_id = u64::from(request_low) | (u64::from(request_high) << 32);
        let byte_length = u64::from(size_low) | (u64::from(size_high) << 32);
        let metadata =
            bridge
                .runtime_mut()?
                .complete_opfs_open(request_id, observation, byte_length)?;
        bridge.set_output(metadata, Vec::new(), Vec::new())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_complete_source_read(
    request_low: u32,
    request_high: u32,
    control_length: u32,
    payload_length: u32,
) -> u32 {
    run_bridge(|bridge| {
        let request_id = u64::from(request_low) | (u64::from(request_high) << 32);
        bridge.complete_source_read(request_id, control_length as usize, payload_length as usize)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_complete_gdi_text_raster(
    request_low: u32,
    request_high: u32,
    payload_length: u32,
) -> u32 {
    run_bridge(|bridge| {
        let request_id = u64::from(request_low) | (u64::from(request_high) << 32);
        bridge.complete_gdi_text_raster(request_id, payload_length as usize)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_ingest_pointer_event(
    sequence_low: u32,
    sequence_high: u32,
    kind: u32,
    button: u32,
    x: u32,
    y: u32,
) -> u32 {
    run_bridge(|bridge| {
        let x = u16::try_from(x)
            .map_err(|_| "pointer x is outside the normalized client range".to_owned())?;
        let y = u16::try_from(y)
            .map_err(|_| "pointer y is outside the normalized client range".to_owned())?;
        if !(1..=3).contains(&kind)
            || (kind == 1 && button != 0)
            || (kind != 1 && !matches!(button, 1 | 2))
        {
            return Err("pointer event kind or button is outside the bridge ABI".to_owned());
        }
        bridge.ingest_pointer_event(
            u64::from(sequence_low) | (u64::from(sequence_high) << 32),
            kind,
            button,
            x,
            y,
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_ingest_keyboard_event(
    sequence_low: u32,
    sequence_high: u32,
    kind: u32,
    virtual_key: u32,
    repeat: u32,
) -> u32 {
    run_bridge(|bridge| {
        let kind = match kind {
            1 => RuntimeKeyboardSourceEventKind::FocusGained,
            2 => RuntimeKeyboardSourceEventKind::FocusLost,
            3 => RuntimeKeyboardSourceEventKind::KeyDown,
            4 => RuntimeKeyboardSourceEventKind::KeyUp,
            _ => return Err("keyboard event kind is outside the bridge ABI".to_owned()),
        };
        let virtual_key = if virtual_key == u32::MAX {
            None
        } else {
            Some(
                u16::try_from(virtual_key)
                    .map_err(|_| "keyboard virtual key is outside the x86 range".to_owned())?,
            )
        };
        let repeat = match repeat {
            0 => false,
            1 => true,
            _ => return Err("keyboard repeat flag is outside the bridge ABI".to_owned()),
        };
        bridge.ingest_keyboard_event(RuntimeKeyboardSourceEvent {
            sequence: u64::from(sequence_low) | (u64::from(sequence_high) << 32),
            kind,
            virtual_key,
            repeat,
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_schedule_next_thread() -> u32 {
    run_bridge(|bridge| {
        let metadata = bridge.runtime_mut()?.schedule_next_thread()?;
        bridge.set_output(metadata, Vec::new(), Vec::new())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_finish_thread(exit_code: u32) -> u32 {
    run_bridge(|bridge| {
        let (metadata, delta) = bridge.runtime_mut()?.finish_thread(exit_code)?;
        let (control, payload) = delta
            .map(|buffers| (buffers.control, buffers.page_bytes))
            .unwrap_or_default();
        bridge.set_output(metadata, control, payload)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_complete_callback(sentinel: u32, result: u32) -> u32 {
    run_bridge(|bridge| {
        let (metadata, delta, bytes) = bridge
            .runtime_mut()?
            .complete_callback(GuestAddress(sentinel), result)?;
        bridge.set_runtime_output(metadata, delta, bytes)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND_THREAD_ID: u32 = 8193;

    #[test]
    fn single_pixel_payload_is_shared_across_empty_draws_and_bridge_lifetime() {
        let empty = Arc::new(Vec::new());
        let snapshot = Arc::new(vec![1, 2, 3, 4, 5, 6, 7, 8]);
        let original_pointer = snapshot.as_ptr();
        let mut bytes = RuntimeOutputBytes::new();
        assert_eq!(
            append_presentation_pixels(&mut bytes, &empty, None),
            Value::Null
        );
        let pixels = append_presentation_pixels(&mut bytes, &snapshot, Some(8));
        assert_eq!(pixels["offset"], 0);
        assert_eq!(pixels["length"], 8);
        assert_eq!(pixels["pitch"], 8);
        assert_eq!(
            append_presentation_pixels(&mut bytes, &empty, None),
            Value::Null
        );
        assert_eq!(
            bytes.as_ptr(),
            original_pointer,
            "single payload keeps the draw's pixel allocation"
        );
        let RuntimeOutputBytes::Shared(shared) = &bytes else {
            panic!("single snapshot was copied")
        };
        assert!(Arc::ptr_eq(shared, &snapshot));

        let weak = Arc::downgrade(&snapshot);
        let mut bridge = Bridge::new();
        bridge
            .set_runtime_output(json!({"kind": "com-complete"}), None, bytes)
            .unwrap();
        drop(snapshot); // The frame can retire before the bridge relinquishes its output.
        assert_eq!(bridge.presentation_bytes.as_ptr(), original_pointer);
        assert_eq!(&*bridge.presentation_bytes, &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(weak.strong_count(), 1);
        bridge.clear_output();
        assert!(bridge.presentation_bytes.is_empty());
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn multiple_pixel_payloads_keep_order_and_clear_does_not_mutate_snapshots() {
        let first = Arc::new(vec![1, 2, 3, 4]);
        let second = Arc::new(vec![5, 6, 7, 8]);
        let empty = Arc::new(Vec::new());
        let mut bytes = RuntimeOutputBytes::new();
        assert_eq!(
            append_presentation_pixels(&mut bytes, &first, Some(4))["offset"],
            0
        );
        assert_eq!(
            append_presentation_pixels(&mut bytes, &empty, None),
            Value::Null
        );
        assert_eq!(
            append_presentation_pixels(&mut bytes, &second, Some(4))["offset"],
            4
        );
        assert_eq!(
            append_presentation_pixels(&mut bytes, &first, Some(4))["offset"],
            8
        );
        assert!(matches!(&bytes, RuntimeOutputBytes::Owned(_)));
        assert_eq!(&*bytes, &[1, 2, 3, 4, 5, 6, 7, 8, 1, 2, 3, 4]);
        assert_ne!(bytes.as_ptr(), first.as_ptr());
        bytes.clear();
        assert_eq!(first.as_slice(), &[1, 2, 3, 4]);
        assert_eq!(second.as_slice(), &[5, 6, 7, 8]);

        let mut bridge = Bridge::new();
        bridge.presentation_bytes.append_snapshot(&first);
        bridge.set_error("presentation failed".to_owned());
        assert!(bridge.presentation_bytes.is_empty());
        assert_eq!(
            Arc::strong_count(&first),
            1,
            "error drops the shared owner without copying or clearing it"
        );
        assert_eq!(first.as_slice(), &[1, 2, 3, 4]);
        bridge.presentation_bytes.append_snapshot(&second);
        bridge
            .set_runtime_output(
                json!({"kind": "com-complete"}),
                None,
                RuntimeOutputBytes::new(),
            )
            .unwrap();
        assert!(
            bridge.presentation_bytes.is_empty(),
            "clear-only frame replaces prior output"
        );
        assert_eq!(Arc::strong_count(&second), 1);
    }

    #[test]
    fn owned_media_output_keeps_its_allocation() {
        let bytes = vec![4, 3, 2, 1];
        let pointer = bytes.as_ptr();
        let mut bridge = Bridge::new();
        bridge
            .set_runtime_output(json!({"kind": "media-data"}), None, bytes.into())
            .unwrap();
        assert_eq!(bridge.media_bytes.as_ptr(), pointer);
        assert_eq!(bridge.media_bytes, [4, 3, 2, 1]);
        assert!(bridge.presentation_bytes.is_empty());
    }

    fn running_test_runtime() -> Runtime {
        controlled_executable_runtime_state(
            GuestMemory::new(MemoryLimits::default()).unwrap(),
            0xec02_5c47,
            b"C:\\Vine\\browser-vertical.exe\0".to_vec(),
        )
        .unwrap()
    }

    #[test]
    fn cpu_snapshot_exports_keep_opaque_owned_bytes_and_reject_stale_or_malformed_handoffs() {
        let runtime = running_test_runtime();
        let thread_id = runtime.current_thread_id().0;
        let memory_before = runtime.memory().stats();
        BRIDGE.with(|cell| {
            let mut bridge = Bridge::new();
            bridge.runtime = Some(BrowserRuntime {
                runtime,
                dispatcher: ApiDispatcher::from_endpoints([]).unwrap(),
                callback_sentinel: GuestAddress(0x7f01_0000),
                pending_host: BTreeMap::new(),
                pending_com_audio: None,
                pending_com_media: BTreeMap::new(),
                prepared_source_trap: None,
                pending_source_lookup: None,
                pending_source_search: None,
            });
            *cell.borrow_mut() = bridge;
        });
        assert_eq!(vine_bridge_input_resize(512), 0);
        BRIDGE.with(|cell| cell.borrow_mut().input.fill(0xa5));
        assert_eq!(vine_bridge_cpu_snapshot_put(thread_id, 0, 0, 2, 2, 512), 0);
        BRIDGE.with(|cell| {
            let bridge = cell.borrow();
            let metadata: Value = serde_json::from_slice(&bridge.metadata).unwrap();
            assert_eq!(
                metadata,
                json!({
                    "kind": "cpu-snapshot-stored", "threadId": thread_id,
                    "revisionLow": 1, "revisionHigh": 0,
                    "backendTag": 2, "abiVersion": 2, "byteLength": 512,
                })
            );
            assert!(bridge.payload.is_empty());
        });
        // A later bridge input mutation cannot alias the parked CPU bytes.
        BRIDGE.with(|cell| cell.borrow_mut().input.fill(0x5a));
        for (low, high, tag, version, length) in [
            (0, 0, 2, 2, 512),
            (1, 1, 2, 2, 512),
            (1, 0, 3, 2, 512),
            (1, 0, 2, 1, 512),
            (1, 0, 2, 2, 511),
            (1, 0, 2, 2, 513),
        ] {
            assert_eq!(
                vine_bridge_cpu_snapshot_put(thread_id, low, high, tag, version, length),
                1
            );
            BRIDGE.with(|cell| {
                let bridge = cell.borrow();
                let metadata: Value = serde_json::from_slice(&bridge.metadata).unwrap();
                assert_eq!(metadata["kind"], "error");
                assert!(bridge.control.is_empty());
                assert!(bridge.payload.is_empty());
            });
            assert_eq!(vine_bridge_cpu_snapshot_get(thread_id), 0);
            BRIDGE.with(|cell| {
                let bridge = cell.borrow();
                let metadata: Value = serde_json::from_slice(&bridge.metadata).unwrap();
                assert_eq!(
                    metadata,
                    json!({
                        "kind": "cpu-snapshot", "threadId": thread_id,
                        "revisionLow": 1, "revisionHigh": 0,
                        "backendTag": 2, "abiVersion": 2, "byteLength": 512,
                    })
                );
                assert_eq!(bridge.payload, vec![0xa5; 512]);
                assert!(bridge.control.is_empty());
                assert!(bridge.presentation_bytes.is_empty());
                assert!(bridge.audio_bytes.is_empty());
                assert!(bridge.media_bytes.is_empty());
                let runtime = &bridge.runtime.as_ref().unwrap().runtime;
                assert_eq!(runtime.memory().stats(), memory_before);
                assert_eq!(runtime.current_thread_id().0, thread_id);
                assert_eq!(runtime.current_last_error().unwrap(), 87);
            });
        }
        assert_eq!(vine_bridge_cpu_snapshot_put(thread_id, 1, 0, 2, 2, 512), 0);
        assert_eq!(vine_bridge_cpu_snapshot_get(thread_id), 0);
        BRIDGE.with(|cell| {
            let bridge = cell.borrow();
            let metadata: Value = serde_json::from_slice(&bridge.metadata).unwrap();
            assert_eq!(metadata["revisionLow"], 2);
            assert_eq!(bridge.payload, vec![0x5a; 512]);
        });
        assert_eq!(vine_bridge_cpu_snapshot_get(0), 1);
        assert_eq!(vine_bridge_input_resize(4097), 0);
        assert_eq!(vine_bridge_cpu_snapshot_put(thread_id, 2, 0, 2, 2, 4097), 1);
        assert_eq!(vine_bridge_cpu_snapshot_get(thread_id), 0);
        BRIDGE.with(|cell| {
            let bridge = cell.borrow();
            assert_eq!(bridge.payload, vec![0x5a; 512]);
            let runtime = &bridge.runtime.as_ref().unwrap().runtime;
            assert_eq!(
                runtime
                    .thread_cpu_snapshot(RuntimeThreadId(thread_id))
                    .unwrap()
                    .2,
                2
            );
        });
    }

    #[test]
    fn timestamp_counter_export_preserves_u64_transport_and_publishes_no_guest_outputs() {
        let runtime = running_test_runtime();
        let memory_before = runtime.memory().stats();
        BRIDGE.with(|cell| {
            let mut bridge = Bridge::new();
            bridge.runtime = Some(BrowserRuntime {
                runtime,
                dispatcher: ApiDispatcher::from_endpoints([]).unwrap(),
                callback_sentinel: GuestAddress(0x7f01_0000),
                pending_host: BTreeMap::new(),
                pending_com_audio: None,
                pending_com_media: BTreeMap::new(),
                prepared_source_trap: None,
                pending_source_lookup: None,
                pending_source_search: None,
            });
            bridge.control = vec![1];
            bridge.payload = vec![2];
            bridge
                .presentation_bytes
                .append_snapshot(&Arc::new(vec![3]));
            bridge.audio_bytes = vec![4];
            bridge.media_bytes = vec![5];
            *cell.borrow_mut() = bridge;
        });
        for _ in 0..2 {
            assert_eq!(vine_bridge_read_timestamp_counter(0x11, 2, 7, 1), 0);
            BRIDGE.with(|cell| {
                let bridge = cell.borrow();
                let metadata: Value = serde_json::from_slice(&bridge.metadata).unwrap();
                assert_eq!(
                    metadata,
                    json!({ "kind": "timestamp-counter", "low": 700, "high": 100 })
                );
                assert!(bridge.control.is_empty());
                assert!(bridge.payload.is_empty());
                assert!(bridge.presentation_bytes.is_empty());
                assert!(bridge.audio_bytes.is_empty());
                assert!(bridge.media_bytes.is_empty());
                assert!(bridge.api_completion.is_none());
                let runtime = &bridge.runtime.as_ref().unwrap().runtime;
                assert_eq!(runtime.synchronized_instruction_count(), 0x0002_0000_0011);
                assert_eq!(runtime.memory().stats(), memory_before);
                assert_eq!(runtime.current_last_error().unwrap(), 87);
            });
        }
        // Rejected observations do not advance the retirement cursor or leave
        // stale successful timestamp/output data visible through the bridge.
        for (retired_low, elapsed_low) in [(0x12, 6), (0x10, 8)] {
            assert_eq!(
                vine_bridge_read_timestamp_counter(retired_low, 2, elapsed_low, 1),
                1
            );
            BRIDGE.with(|cell| {
                let bridge = cell.borrow();
                let metadata: Value = serde_json::from_slice(&bridge.metadata).unwrap();
                assert_eq!(metadata["kind"], "error");
                assert!(bridge.control.is_empty());
                assert!(bridge.payload.is_empty());
                let runtime = &bridge.runtime.as_ref().unwrap().runtime;
                assert_eq!(runtime.synchronized_instruction_count(), 0x0002_0000_0011);
                assert_eq!(runtime.memory().stats(), memory_before);
            });
        }
        assert_eq!(vine_bridge_read_timestamp_counter(0x11, 2, 7, 1), 0);
        BRIDGE.with(|cell| {
            let bridge = cell.borrow();
            let metadata: Value = serde_json::from_slice(&bridge.metadata).unwrap();
            assert_eq!(
                metadata,
                json!({ "kind": "timestamp-counter", "low": 700, "high": 100 })
            );
        });
    }

    #[test]
    fn ordinary_api_completion_preserves_eax_and_expires_before_other_bridge_operations() {
        let stack = GuestAddress(0x2000_0100);
        let return_address = GuestAddress(0x0040_1000);
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.work/toolchain-spike/timer-wait.exe");
        let limits = Pe32Limits::default();
        let snapshot =
            ModuleSnapshot::from_bytes(std::fs::read(fixture).unwrap(), &limits).unwrap();
        let plan = build_pe32_load_plan(&snapshot, limits).unwrap();
        let catalog = runtime_catalog_for_load_plan(&[&plan]).unwrap();
        let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
        let image = materialize_pe32_image(
            &mut memory,
            &snapshot,
            &plan,
            GuestAddress(plan.image.preferred_image_base),
        )
        .unwrap();
        let owner = memory
            .create_owner(vine_core::MemoryOwnerKind::Runtime)
            .unwrap();
        let range = memory
            .reserve_fixed(
                owner,
                MemoryRegionKind::Runtime,
                GuestAddress(0x2000_0000),
                0x1000,
            )
            .unwrap();
        memory
            .commit(
                owner,
                range,
                vine_core::PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                },
            )
            .unwrap();
        for (offset, argument) in [(0, 0), (0x20, u32::MAX), (0x40, 0)] {
            let frame = [return_address.0, argument]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>();
            memory
                .write_initialization(owner, GuestAddress(stack.0 + offset), &frame)
                .unwrap();
        }
        memory.seal_owner(owner).unwrap();
        let mut process = prepare_executable_runtime_process(
            memory,
            &plan,
            &image,
            &[],
            &catalog,
            0xec02_5c47,
            "completion.exe",
            b"C:\\Vine\\completion.exe\0".to_vec(),
        )
        .unwrap();
        let reflection = process.runtime.memory_mut().pending_reflection().unwrap();
        process
            .runtime
            .memory_mut()
            .acknowledge_delta(reflection.invalidation.generation)
            .unwrap();
        let endpoints = ["GetLastError", "SetLastError", "Sleep"]
            .into_iter()
            .map(|name| {
                ApiEndpointIdentity::from_runtime_guest_import(
                    process.binding.imports.iter().find(|import| {
                        matches!(&import.target, ImportTarget::Name { name: imported, .. } if imported == name)
                    }).unwrap()
                ).unwrap()
            })
            .collect::<Vec<_>>();
        BRIDGE.with(|cell| {
            let mut bridge = Bridge::new();
            bridge.runtime = Some(BrowserRuntime {
                runtime: process.runtime,
                dispatcher: ApiDispatcher::from_endpoints(endpoints.clone()).unwrap(),
                callback_sentinel: GuestAddress(0x7f01_0000),
                pending_host: BTreeMap::new(),
                pending_com_audio: None,
                pending_com_media: BTreeMap::new(),
                prepared_source_trap: None,
                pending_source_lookup: None,
                pending_source_search: None,
            });
            *cell.borrow_mut() = bridge;
        });
        let dispatch = |endpoint: usize, offset: u32| {
            vine_bridge_dispatch(
                endpoints[endpoint].address.0,
                stack.0 + offset,
                return_address.0,
                0,
                0,
                0xaaaa_bbbb,
                0,
                0,
                0,
                stack.0 + offset,
                0,
                0,
                0,
                endpoints[endpoint].address.0,
                2,
                0,
                0,
                0,
                0,
                0,
                0,
            )
        };
        assert_eq!(dispatch(0, 0), 0);
        assert_ne!(vine_bridge_api_completion_ptr(), 0);
        assert_eq!(vine_bridge_metadata_len(), 0);
        BRIDGE.with(|cell| {
            assert_eq!(
                cell.borrow().api_completion,
                Some([return_address.0, stack.0 + 4, 87, 1])
            );
        });
        assert_eq!(vine_bridge_input_resize(0), 0);
        assert_eq!(vine_bridge_api_completion_ptr(), 0);
        assert_eq!(dispatch(1, 0x20), 0);
        BRIDGE.with(|cell| {
            let bridge = cell.borrow();
            assert_eq!(
                bridge.api_completion,
                Some([return_address.0, stack.0 + 0x28, 0, 0])
            );
            assert_eq!(
                bridge
                    .runtime
                    .as_ref()
                    .unwrap()
                    .runtime
                    .current_last_error()
                    .unwrap(),
                u32::MAX
            );
        });
        assert_eq!(dispatch(0, 0), 0);
        BRIDGE.with(|cell| {
            assert_eq!(
                cell.borrow().api_completion,
                Some([return_address.0, stack.0 + 4, u32::MAX, 1])
            );
        });
        assert_eq!(dispatch(2, 0x40), 0);
        assert_eq!(vine_bridge_api_completion_ptr(), 0);
        BRIDGE.with(|cell| {
            let bridge = cell.borrow();
            let metadata: Value = serde_json::from_slice(&bridge.metadata).unwrap();
            assert_eq!(metadata["kind"], "api-complete");
            assert_eq!(
                metadata["completion"]["executionBoundary"],
                "yield-current-thread"
            );
            assert_eq!(metadata["completion"]["eax"], Value::Null);
        });
        assert_eq!(dispatch(0, 0), 0);
        assert_ne!(vine_bridge_api_completion_ptr(), 0);
        assert_eq!(dispatch(0, 0x2000), 1);
        assert_eq!(vine_bridge_api_completion_ptr(), 0);
        BRIDGE.with(|cell| {
            let bridge = cell.borrow();
            let metadata: Value = serde_json::from_slice(&bridge.metadata).unwrap();
            assert_eq!(metadata["kind"], "error");
            assert!(!metadata["message"].as_str().unwrap().is_empty());
            assert_eq!(
                bridge
                    .runtime
                    .as_ref()
                    .unwrap()
                    .runtime
                    .current_last_error()
                    .unwrap(),
                u32::MAX
            );
        });
        BRIDGE.with(|cell| *cell.borrow_mut() = Bridge::new());
    }

    #[test]
    fn stop_context_without_runtime_preserves_original_fault_and_binary_outputs() {
        BRIDGE.with(|cell| {
            let mut bridge = Bridge::new();
            bridge.metadata = br#"{"kind":"error","message":"original guest fault"}"#.to_vec();
            bridge.control = vec![1];
            bridge.payload = vec![2];
            bridge.presentation_bytes = vec![3].into();
            bridge.audio_bytes = vec![4];
            bridge.media_bytes = vec![5];
            bridge.writeback_generation = Some(77);
            *cell.borrow_mut() = bridge;
        });
        assert_eq!(vine_bridge_stop_context(u32::MAX), 0);
        BRIDGE.with(|cell| {
            let bridge = cell.borrow();
            let context: Value = serde_json::from_slice(&bridge.stop_context_metadata).unwrap();
            assert_eq!(
                context,
                json!({
                    "kind": "stop-context", "instructionPointer": u32::MAX,
                    "threadId": null, "lastError": null, "module": null, "symbol": null,
                })
            );
            assert_eq!(
                bridge.metadata,
                br#"{"kind":"error","message":"original guest fault"}"#
            );
            assert_eq!(bridge.control, [1]);
            assert_eq!(bridge.payload, [2]);
            assert_eq!(&*bridge.presentation_bytes, [3]);
            assert_eq!(bridge.audio_bytes, [4]);
            assert_eq!(bridge.media_bytes, [5]);
            assert_eq!(bridge.writeback_generation, Some(77));
        });
        assert!(vine_bridge_stop_context_len() > 0);
        BRIDGE.with(|cell| *cell.borrow_mut() = Bridge::new());
    }

    #[test]
    fn stop_context_reads_current_thread_and_exact_api_without_acknowledging_memory() {
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.work/toolchain-spike/message-box.exe");
        let limits = Pe32Limits::default();
        let snapshot =
            ModuleSnapshot::from_bytes(std::fs::read(fixture).unwrap(), &limits).unwrap();
        let plan = build_pe32_load_plan(&snapshot, limits).unwrap();
        let catalog = runtime_catalog_for_load_plan(&[&plan]).unwrap();
        let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
        let image = materialize_pe32_image(
            &mut memory,
            &snapshot,
            &plan,
            GuestAddress(plan.image.preferred_image_base),
        )
        .unwrap();
        let process = prepare_executable_runtime_process(
            memory,
            &plan,
            &image,
            &[],
            &catalog,
            0xec02_5c47,
            "message-box.exe",
            b"C:\\Vine\\message-box.exe\0".to_vec(),
        )
        .unwrap();
        let named = ApiEndpointIdentity::from_runtime_guest_import(
            process.binding.imports.iter().find(|import| {
                matches!(&import.target, ImportTarget::Name { name, .. } if name == "MessageBoxA")
            }).unwrap(),
        ).unwrap();
        let named_address = named.address.0;
        let mut runtime = process.runtime;
        runtime
            .register_thread(RuntimeThreadId(SECOND_THREAD_ID), 88)
            .unwrap();
        runtime
            .select_thread(RuntimeThreadId(SECOND_THREAD_ID))
            .unwrap();
        let mut bridge = Bridge::new();
        bridge.runtime = Some(BrowserRuntime {
            runtime,
            dispatcher: ApiDispatcher::from_endpoints([named]).unwrap(),
            callback_sentinel: GuestAddress(0x7f01_0000),
            pending_host: BTreeMap::new(),
            pending_com_audio: None,
            pending_com_media: BTreeMap::new(),
            prepared_source_trap: None,
            pending_source_lookup: None,
            pending_source_search: None,
        });
        let before = bridge.runtime.as_ref().unwrap().runtime.memory().stats();
        assert!(
            bridge
                .runtime
                .as_ref()
                .unwrap()
                .runtime
                .memory()
                .has_pending_delta()
        );
        for (address, module, symbol) in [
            (named_address, Some("user32.dll"), Some("MessageBoxA")),
            (named_address + 1, None, None),
            (0, None, None),
        ] {
            bridge.stop_context(address).unwrap();
            let context: Value = serde_json::from_slice(&bridge.stop_context_metadata).unwrap();
            assert_eq!(
                context,
                json!({
                    "kind": "stop-context", "instructionPointer": address,
                    "threadId": SECOND_THREAD_ID, "lastError": 88,
                    "module": module, "symbol": symbol,
                })
            );
            let browser = bridge.runtime.as_ref().unwrap();
            assert_eq!(
                browser.runtime.current_thread_id(),
                RuntimeThreadId(SECOND_THREAD_ID)
            );
            assert_eq!(browser.runtime.current_last_error().unwrap(), 88);
            assert_eq!(browser.runtime.memory().stats(), before);
            assert_eq!(
                browser.runtime.scheduler_state().unwrap(),
                SchedulerThreadState::Runnable
            );
            assert!(browser.pending_host.is_empty());
            assert!(browser.pending_com_media.is_empty());
        }
    }

    #[test]
    fn message_box_transport_preserves_rejected_results_and_resumes_the_owner() {
        let stack = GuestAddress(0x2000_1040);
        let text = GuestAddress(0x2000_1200);
        let title = GuestAddress(0x2000_1300);
        let return_address = GuestAddress(0x0040_1000);
        let limits = Pe32Limits::default();
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.work/toolchain-spike/message-box.exe");
        let snapshot = ModuleSnapshot::from_bytes(
            std::fs::read(&fixture)
                .expect("build the MessageBox PE32 fixture with check:toolchain"),
            &limits,
        )
        .unwrap();
        let plan = build_pe32_load_plan(&snapshot, limits).unwrap();
        let catalog = runtime_catalog_for_load_plan(&[&plan]).unwrap();
        let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
        let image = materialize_pe32_image(
            &mut memory,
            &snapshot,
            &plan,
            GuestAddress(plan.image.preferred_image_base),
        )
        .unwrap();
        let owner = memory
            .create_owner(vine_core::MemoryOwnerKind::Runtime)
            .unwrap();
        let range = memory
            .reserve_fixed(
                owner,
                MemoryRegionKind::Runtime,
                GuestAddress(0x2000_0000),
                0x2000,
            )
            .unwrap();
        memory
            .commit(
                owner,
                range,
                vine_core::PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                },
            )
            .unwrap();
        let frame = [return_address.0, 0, text.0, title.0, 0x1010_u32]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        memory.write_initialization(owner, stack, &frame).unwrap();
        memory
            .write_initialization(owner, text, b"line1\r\n\x93\xfa\x96\x7b\x8c\xea\0")
            .unwrap();
        memory
            .write_initialization(owner, title, b"Notice\0")
            .unwrap();
        memory.seal_owner(owner).unwrap();
        let mut process = prepare_executable_runtime_process(
            memory,
            &plan,
            &image,
            &[],
            &catalog,
            0xec02_5c47,
            "message-box.exe",
            b"C:\\Vine\\message-box.exe\0".to_vec(),
        )
        .unwrap();
        let reflection = process.runtime.memory_mut().pending_reflection().unwrap();
        process
            .runtime
            .memory_mut()
            .acknowledge_delta(reflection.invalidation.generation)
            .unwrap();
        let import = process.binding.imports.iter().find(|import| {
            matches!(&import.target, ImportTarget::Name { name, .. } if name == "MessageBoxA")
        }).unwrap();
        let endpoint = ApiEndpointIdentity::from_runtime_guest_import(import).unwrap();
        let mut browser = BrowserRuntime {
            runtime: process.runtime,
            dispatcher: ApiDispatcher::from_endpoints([endpoint.clone()]).unwrap(),
            callback_sentinel: GuestAddress(0x7f01_0000),
            pending_host: BTreeMap::new(),
            pending_com_audio: None,
            pending_com_media: BTreeMap::new(),
            prepared_source_trap: None,
            pending_source_lookup: None,
            pending_source_search: None,
        };
        browser.runtime.set_current_last_error(0x1234).unwrap();
        let context = X86CpuContext {
            eax: 0,
            ecx: 0,
            edx: 0,
            ebx: 0,
            esp: stack.0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eip: endpoint.address.0,
            eflags: 0,
            es: 0,
            cs: 0,
            ss: 0,
            ds: 0,
            fs: 0,
            gs: 0,
        };
        let (metadata, delta, presentation) = browser
            .dispatch(endpoint.address, stack, return_address, 0, context)
            .unwrap();
        let DispatchMetadata::Json(metadata) = metadata else {
            panic!("a deferred message box requires a browser host request");
        };
        assert!(delta.is_none());
        assert!(presentation.is_empty());
        assert_eq!(metadata["kind"], "host-request");
        assert_eq!(metadata["service"], RUNTIME_MESSAGE_BOX_SERVICE);
        assert_eq!(metadata["arguments"], json!([]));
        assert_eq!(
            metadata["messageBox"],
            json!({
                "title": "Notice", "text": "line1\r\n日本語",
                "buttons": [{"id": 1, "label": "OK"}],
                "defaultButton": 1, "cancelButton": 1, "icon": "error",
            })
        );
        let request_id = metadata["requestId"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        assert!(
            browser.pending_host[&request_id]
                .call
                .request
                .payload
                .is_empty()
        );
        let dialog = browser.runtime.message_box(request_id).unwrap().clone();
        let initial = RuntimeThreadId(THREAD_ID);
        let second = RuntimeThreadId(SECOND_THREAD_ID);
        browser.runtime.register_thread(second, 0).unwrap();
        browser.runtime.select_thread(second).unwrap();
        browser.runtime.set_current_last_error(0x5678).unwrap();

        assert!(browser.complete_host(request_id + 1, 1).is_err());
        assert!(browser.complete_host(request_id, 2).is_err());
        assert_eq!(browser.runtime.current_thread_id(), second);
        assert!(browser.pending_host.contains_key(&request_id));
        assert_eq!(browser.runtime.message_box(request_id).unwrap(), &dialog);
        assert_eq!(
            browser.runtime.scheduler_state_for_thread(initial).unwrap(),
            SchedulerThreadState::WaitingHost { request_id }
        );
        assert!(browser.runtime.scheduler().events().is_empty());
        assert_eq!(
            browser.runtime.thread_state(initial).unwrap().last_error,
            0x1234
        );
        assert_eq!(browser.runtime.current_last_error().unwrap(), 0x5678);

        let completion = browser.complete_host(request_id, 1).unwrap();
        assert_eq!(completion["kind"], "host-complete");
        assert_eq!(completion["threadId"], initial.0);
        assert_eq!(
            completion["completion"],
            json!({
                "instructionPointer": return_address.0, "stackPointer": stack.0 + 20, "eax": 1,
            })
        );
        assert_eq!(browser.runtime.current_thread_id(), second);
        assert_eq!(
            browser.runtime.scheduler_state_for_thread(initial).unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(
            browser.runtime.thread_state(initial).unwrap().last_error,
            0x1234
        );
        assert_eq!(browser.runtime.current_last_error().unwrap(), 0x5678);
        assert!(!browser.pending_host.contains_key(&request_id));
        assert!(browser.runtime.message_box(request_id).is_err());
        assert!(browser.complete_host(request_id, 1).is_err());
        assert_eq!(browser.runtime.current_thread_id(), second);
    }

    #[test]
    fn source_handoff_rejects_pending_wrong_trap_and_wrong_thread_without_consuming_retry() {
        let initial = RuntimeThreadId(THREAD_ID);
        let identity = (
            initial,
            GuestAddress(0x7f001000),
            GuestAddress(0x20000000),
            GuestAddress(0x401000),
        );
        let mut browser = BrowserRuntime {
            runtime: running_test_runtime(),
            dispatcher: ApiDispatcher::from_endpoints([]).unwrap(),
            callback_sentinel: GuestAddress(0x7f010000),
            pending_host: BTreeMap::new(),
            pending_com_audio: None,
            pending_com_media: BTreeMap::new(),
            prepared_source_trap: Some(identity),
            pending_source_lookup: Some((12, b"missing.dat".to_vec())),
            pending_source_search: None,
        };
        let context = X86CpuContext {
            eax: 0,
            ecx: 0,
            edx: 0,
            ebx: 0,
            esp: identity.2.0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eip: identity.1.0,
            eflags: 0,
            es: 0,
            cs: 0,
            ss: 0,
            ds: 0,
            fs: 0,
            gs: 0,
        };
        let before = browser.runtime.memory().stats();
        browser.runtime.set_current_last_error(77).unwrap();
        assert!(
            browser
                .dispatch(identity.1, identity.2, identity.3, 0, context)
                .is_err()
        );
        assert!(browser.pending_source_lookup.is_some());
        assert_eq!(browser.prepared_source_trap, Some(identity));
        assert!(
            browser
                .complete_source_lookup(Some(json!({ "kind": "unknown" })))
                .is_err()
        );
        assert_eq!(browser.pending_source_lookup.as_ref().unwrap().0, 12);
        assert_eq!(
            browser.pending_source_lookup.as_ref().unwrap().1,
            b"missing.dat"
        );
        assert_eq!(browser.prepared_source_trap, Some(identity));
        browser.complete_source_lookup(None).unwrap();
        assert!(browser.pending_source_lookup.is_none());
        for changed in [
            (GuestAddress(identity.1.0 + 16), identity.2, identity.3),
            (identity.1, GuestAddress(identity.2.0 + 4), identity.3),
            (identity.1, identity.2, GuestAddress(identity.3.0 + 4)),
        ] {
            assert!(
                browser
                    .dispatch(changed.0, changed.1, changed.2, 0, context)
                    .is_err()
            );
            assert_eq!(browser.prepared_source_trap, Some(identity));
        }
        let second = RuntimeThreadId(SECOND_THREAD_ID);
        browser.runtime.register_thread(second, 0).unwrap();
        browser.runtime.select_thread(second).unwrap();
        assert!(
            browser
                .dispatch(identity.1, identity.2, identity.3, 0, context)
                .is_err()
        );
        assert_eq!(browser.prepared_source_trap, Some(identity));
        browser.runtime.select_thread(initial).unwrap();
        // The matching trap resumes ordinary dispatch. This deliberately unowned
        // endpoint then faults in COM, rather than reissuing the missing-file query.
        assert!(
            browser
                .dispatch(identity.1, identity.2, identity.3, 0, context)
                .is_err()
        );
        assert!(browser.prepared_source_trap.is_none());
        assert_eq!(browser.runtime.memory().stats(), before);
        assert_eq!(browser.runtime.current_last_error().unwrap(), 77);
    }

    #[test]
    fn guest_image_fallback_avoids_unmapped_runtime_arenas_and_existing_images() {
        let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
        assert_eq!(
            guest_image_load_base(&memory, 0x5000_0000, 0x2000).unwrap(),
            GuestAddress(0x6000_0000)
        );
        let owner = memory
            .create_owner(vine_core::MemoryOwnerKind::Runtime)
            .unwrap();
        memory
            .reserve_fixed(
                owner,
                vine_core::MemoryRegionKind::Runtime,
                GuestAddress(0x6000_0000),
                0x1000,
            )
            .unwrap();
        assert_eq!(
            guest_image_load_base(&memory, 0x5000_0000, 0x2000).unwrap(),
            GuestAddress(0x6100_0000)
        );
        // The guest private-heap band is reserved even before backing is committed.
        assert_eq!(
            guest_image_load_base(&memory, 0x3000_0000, 0x2000).unwrap(),
            GuestAddress(0x6100_0000)
        );
        assert_eq!(
            guest_image_load_base(&memory, 0x4e00_0000, 0x2000).unwrap(),
            GuestAddress(0x4e00_0000)
        );
        assert_eq!(
            guest_image_load_base(&memory, 0x7ffe_0000, 0x1000).unwrap(),
            GuestAddress(0x7ffe_0000)
        );
        for preferred in [0x3200_0000, 0x7f00_0000, 0x7f03_0000, 0x7fc4_0000, 0x400001] {
            assert_eq!(
                guest_image_load_base(&memory, preferred, 0x2000).unwrap(),
                GuestAddress(0x6100_0000)
            );
        }
    }

    #[test]
    fn executable_virtual_path_is_windows_932_and_rejects_escapes() {
        assert_eq!(
            virtual_executable_path("game.exe").unwrap(),
            b"C:\\Vine\\game.exe\0"
        );
        assert_eq!(
            virtual_executable_path("日本語.exe").unwrap(),
            [
                b"C:\\Vine\\".as_slice(),
                &[0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea],
                b".exe\0"
            ]
            .concat(),
        );
        assert_eq!(
            virtual_executable_path("data/日本語.exe").unwrap(),
            [
                b"C:\\Vine\\data\\".as_slice(),
                &[0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea],
                b".exe\0"
            ]
            .concat(),
        );
        for name in [
            "",
            ".",
            "..",
            "../outside.exe",
            "/absolute.exe",
            "data//game.exe",
            "data/../game.exe",
            "a\\\\b.exe",
            "bad:drive.exe",
            "trailing. ",
            "🦀.exe",
            "¥.exe",
        ] {
            assert!(virtual_executable_path(name).is_err(), "accepted {name:?}");
        }
    }

    #[test]
    fn acknowledged_thread_join_wakes_do_not_consume_host_receipts() {
        let mut runtime = running_test_runtime();
        let initial = RuntimeThreadId(THREAD_ID);
        let second = RuntimeThreadId(SECOND_THREAD_ID);
        let handle = runtime.register_thread(second, 0).unwrap();
        runtime.wait_current_thread_on(handle).unwrap();
        runtime.select_thread(second).unwrap();
        runtime.request_current_thread_exit(0).unwrap();
        runtime.complete_current_thread_exit().unwrap();
        runtime.select_thread(initial).unwrap();
        let request = runtime
            .begin_host_request("fixture.after-thread-join")
            .unwrap();
        let completion = runtime.complete_host_request(request.request_id).unwrap();
        assert_eq!(runtime.scheduler().events().len(), 2);
        acknowledge_thread_wakes(&mut runtime);
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(
            runtime.pop_scheduler_event(),
            Some(RuntimeSchedulerEvent::HostCompleted(completion))
        );
        assert!(runtime.scheduler().events().is_empty());
        acknowledge_thread_wakes(&mut runtime);
    }

    #[test]
    fn host_completion_selection_does_not_restore_a_blocked_previous_thread() {
        let mut runtime = running_test_runtime();
        let initial_thread = RuntimeThreadId(THREAD_ID);
        let completion_owner = RuntimeThreadId(SECOND_THREAD_ID);
        runtime
            .register_thread(completion_owner, 0)
            .expect("register completion owner");
        runtime
            .begin_host_request("fixture.blocked-previous-thread")
            .expect("block initial thread");
        runtime
            .select_thread(completion_owner)
            .expect("select completion owner");

        restore_selected_thread(&mut runtime, initial_thread, completion_owner)
            .expect("retain executable completion owner");

        assert_eq!(runtime.current_thread_id(), completion_owner);
        assert!(matches!(
            runtime
                .scheduler_state_for_thread(initial_thread)
                .expect("blocked initial thread state"),
            SchedulerThreadState::WaitingHost { .. }
        ));
    }

    #[test]
    fn timer_rearm_export_keeps_waiting_identity_and_emits_only_a_retry_delay() {
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.work/toolchain-spike/timer-wait.exe");
        let limits = Pe32Limits::default();
        let snapshot =
            ModuleSnapshot::from_bytes(std::fs::read(fixture).unwrap(), &limits).unwrap();
        let plan = build_pe32_load_plan(&snapshot, limits).unwrap();
        let catalog = runtime_catalog_for_load_plan(&[&plan]).unwrap();
        let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
        let image = materialize_pe32_image(
            &mut memory,
            &snapshot,
            &plan,
            GuestAddress(plan.image.preferred_image_base),
        )
        .unwrap();
        let base = GuestAddress(0x2000_0000);
        let stack = GuestAddress(base.0 + 0x100);
        let second_stack = GuestAddress(base.0 + 0x300);
        let due = GuestAddress(base.0 + 0x800);
        let owner = memory
            .create_owner(vine_core::MemoryOwnerKind::Runtime)
            .unwrap();
        let range = memory
            .reserve_fixed(owner, MemoryRegionKind::Runtime, base, 0x1000)
            .unwrap();
        memory
            .commit(
                owner,
                range,
                vine_core::PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                },
            )
            .unwrap();
        memory.seal_owner(owner).unwrap();
        let process = prepare_executable_runtime_process(
            memory,
            &plan,
            &image,
            &[],
            &catalog,
            0xec02_5c47,
            "timer-wait.exe",
            b"C:\\Vine\\timer-wait.exe\0".to_vec(),
        )
        .unwrap();
        let endpoints = process
            .binding
            .imports
            .iter()
            .filter_map(ApiEndpointIdentity::from_runtime_guest_import)
            .collect::<Vec<_>>();
        let endpoint = |wanted: &str| {
            ApiEndpointIdentity::from_runtime_guest_import(
                process.binding.imports.iter().find(|import|
                    matches!(&import.target, ImportTarget::Name { name, .. } if name == wanted)
                ).unwrap()
            ).unwrap()
        };
        let create = endpoint("CreateWaitableTimerA");
        let arm = endpoint("SetWaitableTimer");
        let wait = endpoint("WaitForSingleObjectEx");
        let mut browser = BrowserRuntime {
            runtime: process.runtime,
            dispatcher: ApiDispatcher::from_endpoints(endpoints).unwrap(),
            callback_sentinel: GuestAddress(0x7f01_0000),
            pending_host: BTreeMap::new(),
            pending_com_audio: None,
            pending_com_media: BTreeMap::new(),
            prepared_source_trap: None,
            pending_source_lookup: None,
            pending_source_search: None,
        };
        let reflection = browser.runtime.memory_mut().pending_reflection().unwrap();
        browser
            .acknowledge_fast(reflection.invalidation.generation)
            .unwrap();
        let call = |browser: &mut BrowserRuntime,
                    endpoint: &ApiEndpointIdentity,
                    stack: GuestAddress,
                    arguments: &[u32]| {
            let words = std::iter::once(image.entry_point_address.0)
                .chain(arguments.iter().copied())
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>();
            browser.runtime.memory_mut().write(stack, &words).unwrap();
            let generation = browser.runtime.memory().stats().generation;
            browser.runtime.memory_mut().pending_delta_binary().unwrap();
            browser.acknowledge_fast(generation).unwrap();
            browser.runtime.dispatch_api_call(
                &browser.dispatcher,
                ApiOwnedTrap {
                    endpoint_id: endpoint.id,
                    endpoint_address: endpoint.address,
                    stack_pointer: stack,
                    return_address: image.entry_point_address,
                },
                browser.callback_sentinel,
                None,
            )
        };
        let creation = call(&mut browser, &create, stack, &[0, 1, 0]);
        let DispatchOutcome::Immediate(created) = creation else {
            panic!("timer creation must complete: {creation:?}");
        };
        let timer = created.eax.unwrap();
        browser
            .runtime
            .memory_mut()
            .write(due, &(-400_000_i64).to_le_bytes())
            .unwrap();
        assert!(matches!(
            call(&mut browser, &arm, stack, &[timer, due.0, 0, 0, 0, 0]),
            DispatchOutcome::Immediate(_)
        ));
        browser.runtime.set_current_last_error(11).unwrap();
        let DispatchOutcome::Deferred(waiting) = call(&mut browser, &wait, stack, &[timer, 200, 1])
        else {
            panic!("armed timer wait must defer");
        };
        let request_id = waiting.request.request_id;
        assert_eq!(waiting.request.payload, 40_u32.to_le_bytes());
        let initial = browser.runtime.current_thread_id();
        browser.pending_host.insert(
            request_id,
            PendingHostCall {
                thread_id: initial,
                call: waiting,
            },
        );
        let second = RuntimeThreadId(SECOND_THREAD_ID);
        browser.runtime.register_thread(second, 0).unwrap();
        browser.runtime.select_thread(second).unwrap();
        browser
            .runtime
            .memory_mut()
            .write(due, &(-1_200_000_i64).to_le_bytes())
            .unwrap();
        assert!(matches!(
            call(
                &mut browser,
                &arm,
                second_stack,
                &[timer, due.0, 0, 0, 0, 0]
            ),
            DispatchOutcome::Immediate(_)
        ));
        browser.runtime.set_current_last_error(22).unwrap();
        let other_request = browser
            .runtime
            .begin_host_request("fixture.blocked-other-timer")
            .unwrap();
        let memory_before = browser.runtime.memory().stats();
        BRIDGE.with(|cell| {
            let mut transport = Bridge::new();
            transport.runtime = Some(browser);
            transport.control = vec![1];
            transport.payload = vec![2];
            transport.audio_bytes = vec![3];
            *cell.borrow_mut() = transport;
        });
        assert_eq!(
            vine_bridge_complete_host(request_id as u32, (request_id >> 32) as u32, 1),
            0
        );
        BRIDGE.with(|cell| {
            let transport = cell.borrow();
            let metadata: Value = serde_json::from_slice(&transport.metadata).unwrap();
            assert_eq!(metadata, json!({
                "kind": "host-retry", "threadId": initial.0,
                "requestId": request_id.to_string(), "service": RUNTIME_WAITABLE_TIMER_PACING_SERVICE,
                "arguments": [80],
            }));
            assert!(transport.control.is_empty() && transport.payload.is_empty());
            assert!(transport.audio_bytes.is_empty() && transport.presentation_bytes.is_empty() && transport.media_bytes.is_empty());
            let browser = transport.runtime.as_ref().unwrap();
            assert_eq!(browser.runtime.current_thread_id(), second);
            assert_eq!(browser.runtime.current_last_error().unwrap(), 22);
            assert_eq!(browser.runtime.thread_state(initial).unwrap().last_error, 11);
            assert_eq!(browser.runtime.scheduler_state_for_thread(initial).unwrap(), SchedulerThreadState::WaitingHost { request_id });
            assert_eq!(browser.runtime.scheduler_state_for_thread(second).unwrap(), SchedulerThreadState::WaitingHost { request_id: other_request.request_id });
            assert_eq!(browser.pending_host[&request_id].call.request.payload, 80_u32.to_le_bytes());
            assert_eq!(browser.runtime.memory().stats(), memory_before);
            assert!(browser.runtime.scheduler().events().is_empty());
        });
        assert_eq!(
            vine_bridge_complete_host(request_id as u32, (request_id >> 32) as u32, 1),
            0
        );
        BRIDGE.with(|cell| {
            let mut transport = cell.borrow_mut();
            let metadata: Value = serde_json::from_slice(&transport.metadata).unwrap();
            assert_eq!(metadata["kind"], "host-complete");
            assert_eq!(metadata["threadId"], initial.0);
            assert_eq!(
                metadata["completion"],
                json!({
                    "instructionPointer": image.entry_point_address.0,
                    "stackPointer": stack.0 + 16, "eax": 0,
                })
            );
            let browser = transport.runtime.as_ref().unwrap();
            assert!(!browser.pending_host.contains_key(&request_id));
            assert_eq!(
                browser.runtime.scheduler_state_for_thread(initial).unwrap(),
                SchedulerThreadState::Runnable
            );
            assert_eq!(
                browser.runtime.thread_state(initial).unwrap().last_error,
                11
            );
            *transport = Bridge::new();
        });
    }

    #[test]
    fn finite_object_wait_export_preserves_owner_retry_and_latched_signal() {
        for retire_selected in [false, true] {
            let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../.work/toolchain-spike/timer-threads.exe");
            let limits = Pe32Limits::default();
            let snapshot =
                ModuleSnapshot::from_bytes(std::fs::read(fixture).unwrap(), &limits).unwrap();
            let plan = build_pe32_load_plan(&snapshot, limits).unwrap();
            let catalog = runtime_catalog_for_load_plan(&[&plan]).unwrap();
            let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
            let image = materialize_pe32_image(
                &mut memory,
                &snapshot,
                &plan,
                GuestAddress(plan.image.preferred_image_base),
            )
            .unwrap();
            let stack = GuestAddress(0x2000_0100);
            let owner = memory
                .create_owner(vine_core::MemoryOwnerKind::Runtime)
                .unwrap();
            let range = memory
                .reserve_fixed(
                    owner,
                    MemoryRegionKind::Runtime,
                    GuestAddress(0x2000_0000),
                    0x1000,
                )
                .unwrap();
            memory
                .commit(
                    owner,
                    range,
                    vine_core::PageProtection {
                        read: true,
                        write: true,
                        execute: false,
                    },
                )
                .unwrap();
            memory.seal_owner(owner).unwrap();
            let process = prepare_executable_runtime_process(
                memory,
                &plan,
                &image,
                &[],
                &catalog,
                0xec02_5c47,
                "timer-threads.exe",
                b"C:\\Vine\\timer-threads.exe\0".to_vec(),
            )
            .unwrap();
            let endpoints = process
                .binding
                .imports
                .iter()
                .filter_map(ApiEndpointIdentity::from_runtime_guest_import)
                .collect::<Vec<_>>();
            let endpoint = |wanted: &str| {
                ApiEndpointIdentity::from_runtime_guest_import(
            process.binding.imports.iter().find(|import|
                matches!(&import.target, ImportTarget::Name { name, .. } if name == wanted)).unwrap()).unwrap()
            };
            let wait = endpoint("WaitForSingleObject");
            let create = endpoint("CreateEventA");
            let signal = endpoint("SetEvent");
            let mut browser = BrowserRuntime {
                runtime: process.runtime,
                dispatcher: ApiDispatcher::from_endpoints(endpoints).unwrap(),
                callback_sentinel: GuestAddress(0x7f01_0000),
                pending_host: BTreeMap::new(),
                pending_com_audio: None,
                pending_com_media: BTreeMap::new(),
                prepared_source_trap: None,
                pending_source_lookup: None,
                pending_source_search: None,
            };
            let reflection = browser.runtime.memory_mut().pending_reflection().unwrap();
            browser
                .acknowledge_fast(reflection.invalidation.generation)
                .unwrap();
            let call = |browser: &mut BrowserRuntime,
                        endpoint: &ApiEndpointIdentity,
                        call_stack: GuestAddress,
                        arguments: &[u32]| {
                let words = std::iter::once(image.entry_point_address.0)
                    .chain(arguments.iter().copied())
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>();
                browser
                    .runtime
                    .memory_mut()
                    .write(call_stack, &words)
                    .unwrap();
                let generation = browser.runtime.memory().stats().generation;
                browser.runtime.memory_mut().pending_delta_binary().unwrap();
                browser.acknowledge_fast(generation).unwrap();
                browser.runtime.dispatch_api_call(
                    &browser.dispatcher,
                    ApiOwnedTrap {
                        endpoint_id: endpoint.id,
                        endpoint_address: endpoint.address,
                        stack_pointer: call_stack,
                        return_address: image.entry_point_address,
                    },
                    browser.callback_sentinel,
                    None,
                )
            };
            let DispatchOutcome::Immediate(created) =
                call(&mut browser, &create, stack, &[0, 0, 0, 0])
            else {
                panic!("create event");
            };
            let event = created.eax.unwrap();
            let initial = browser.runtime.current_thread_id();
            let second = RuntimeThreadId(SECOND_THREAD_ID);
            browser.runtime.register_thread(second, 0).unwrap();
            browser.runtime.set_current_last_error(11).unwrap();
            browser
                .runtime
                .memory_mut()
                .write(
                    stack,
                    &[
                        image.entry_point_address.0.to_le_bytes(),
                        event.to_le_bytes(),
                        1_u32.to_le_bytes(),
                    ]
                    .concat(),
                )
                .unwrap();
            let generation = browser.runtime.memory().stats().generation;
            browser.runtime.memory_mut().pending_delta_binary().unwrap();
            browser.acknowledge_fast(generation).unwrap();
            let DispatchOutcome::Deferred(waiting) = browser.runtime.dispatch_api_call(
                &browser.dispatcher,
                ApiOwnedTrap {
                    endpoint_id: wait.id,
                    endpoint_address: wait.address,
                    stack_pointer: stack,
                    return_address: image.entry_point_address,
                },
                browser.callback_sentinel,
                None,
            ) else {
                panic!("finite event wait must defer");
            };
            let id = waiting.request.request_id;
            assert_eq!(waiting.request.service, RUNTIME_OBJECT_WAIT_SERVICE);
            assert_eq!(waiting.request.payload, 1_u32.to_le_bytes());
            browser.pending_host.insert(
                id,
                PendingHostCall {
                    thread_id: initial,
                    call: waiting,
                },
            );
            browser.runtime.select_thread(second).unwrap();
            browser.runtime.set_current_last_error(22).unwrap();
            BRIDGE.with(|cell| {
                let mut bridge = Bridge::new();
                bridge.runtime = Some(browser);
                *cell.borrow_mut() = bridge;
            });
            assert_eq!(vine_bridge_ready_object_wait_request_id(), 0);
            // No elapsed guest time: a timer ACK is a retry, never an early timeout.
            assert_eq!(
                vine_bridge_complete_host(id as u32, (id >> 32) as u32, 1),
                0
            );
            BRIDGE.with(|cell| {
            let mut bridge = cell.borrow_mut();
            let metadata: Value = serde_json::from_slice(&bridge.metadata).unwrap();
            assert_eq!(metadata, json!({ "kind":"host-retry", "threadId":initial.0,
                "requestId":id.to_string(), "service":RUNTIME_OBJECT_WAIT_SERVICE, "arguments":[1] }));
            let browser = bridge.runtime.as_mut().unwrap();
            assert_eq!(browser.runtime.current_thread_id(), second);
            assert_eq!(browser.runtime.current_last_error().unwrap(), 22);
            browser.runtime.synchronize_scheduled_clock_for_host_execution(0, 9_999).unwrap();
            assert!(matches!(call(browser, &signal, GuestAddress(stack.0 + 0x100), &[event]), DispatchOutcome::Immediate(_)));
            if retire_selected {
                browser.runtime.request_current_thread_exit(0).unwrap();
                browser.runtime.complete_current_thread_exit().unwrap();
            }
            browser.runtime.synchronize_scheduled_clock_for_host_execution(0, 20_000).unwrap();
            bridge.control = vec![1]; bridge.payload = vec![2]; bridge.metadata = vec![3];
        });
            assert_eq!(vine_bridge_ready_object_wait_request_id(), id);
            BRIDGE.with(|cell| {
                let bridge = cell.borrow();
                assert_eq!(bridge.control, vec![1]);
                assert_eq!(bridge.payload, vec![2]);
                assert_eq!(bridge.metadata, vec![3]);
            });
            assert_eq!(
                vine_bridge_complete_host(id as u32, (id >> 32) as u32, 0),
                0
            );
            assert_eq!(vine_bridge_ready_object_wait_request_id(), 0);
            BRIDGE.with(|cell| {
                let bridge = cell.borrow();
                let metadata: Value = serde_json::from_slice(&bridge.metadata).unwrap();
                assert_eq!(metadata["kind"], "host-complete");
                assert_eq!(metadata["threadId"], initial.0);
                assert_eq!(
                    metadata["completion"],
                    json!({"instructionPointer":image.entry_point_address.0,
                "stackPointer":stack.0 + 12, "eax":0})
                );
                let browser = bridge.runtime.as_ref().unwrap();
                assert_eq!(
                    browser.runtime.current_thread_id(),
                    if retire_selected { initial } else { second }
                );
                assert_eq!(
                    browser.runtime.thread_state(initial).unwrap().last_error,
                    11
                );
                assert_eq!(browser.runtime.thread_state(second).unwrap().last_error, 22);
                assert!(browser.pending_host.is_empty());
                assert!(browser.runtime.scheduler().wait_registrations().is_empty());
                assert_eq!(
                    browser.runtime.scheduler_state_for_thread(initial).unwrap(),
                    SchedulerThreadState::Runnable
                );
            });
            assert_ne!(
                vine_bridge_complete_host(id as u32, (id >> 32) as u32, 0),
                0
            );
            BRIDGE.with(|cell| {
                *cell.borrow_mut() = Bridge::new();
            });
        }
    }

    #[test]
    fn media_ingress_rejects_inexact_identity_time_and_unexpected_bytes() {
        let valid = json!({ "kind": "frame", "resourceId": "1",
            "start100ns": 0, "end100ns": 400000 });
        let bytes = [129, 129, 129, 255];
        let ComMediaResponse::Frame(frame) =
            parse_media_response(&serde_json::to_vec(&valid).unwrap(), &bytes).unwrap()
        else {
            panic!("frame");
        };
        assert_eq!(frame.bgra, bytes);
        assert_eq!(frame.end_100ns, 400000);
        for id in ["0", "01", "-1", "18446744073709551616"] {
            let mut invalid = valid.clone();
            invalid["resourceId"] = json!(id);
            assert!(parse_media_response(&serde_json::to_vec(&invalid).unwrap(), &bytes).is_err());
        }
        for time in [json!(0.5), json!(9_007_199_254_740_992_i64)] {
            let mut invalid = valid.clone();
            invalid["end100ns"] = time;
            assert!(parse_media_response(&serde_json::to_vec(&invalid).unwrap(), &bytes).is_err());
        }
        assert!(parse_media_response(br#"{"kind":"acknowledged"}"#, &bytes).is_err());
        assert!(parse_media_response(br#"{"kind":"failed","hresult":0}"#, &[]).is_err());
        assert!(parse_media_response(br#"{"kind":"failed","hresult":2147500037}"#, &[]).is_err());
        assert!(matches!(
            parse_media_response(
                br#"{"kind":"backend-failed","message":"decode failed"}"#,
                &[]
            )
            .unwrap(),
            ComMediaResponse::BackendFailed { .. }
        ));
    }

    #[test]
    fn actual_media_token_rejects_malformed_completion_and_accepts_retry() {
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.work/toolchain-spike/directshow.exe");
        let limits = Pe32Limits::default();
        let snapshot =
            ModuleSnapshot::from_bytes(std::fs::read(fixture).unwrap(), &limits).unwrap();
        let plan = build_pe32_load_plan(&snapshot, limits).unwrap();
        let catalog = runtime_catalog_for_load_plan(&[&plan]).unwrap();
        let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
        let image = materialize_pe32_image(
            &mut memory,
            &snapshot,
            &plan,
            GuestAddress(plan.image.preferred_image_base),
        )
        .unwrap();
        let base = GuestAddress(0x2000_0000);
        let stack = GuestAddress(base.0 + 0x100);
        let class = GuestAddress(base.0 + 0x300);
        let iid = GuestAddress(base.0 + 0x310);
        let output = GuestAddress(base.0 + 0x340);
        let path = GuestAddress(base.0 + 0x400);
        let owner = memory
            .create_owner(vine_core::MemoryOwnerKind::Runtime)
            .unwrap();
        let range = memory
            .reserve_fixed(owner, MemoryRegionKind::Runtime, base, 0x1000)
            .unwrap();
        memory
            .commit(
                owner,
                range,
                vine_core::PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                },
            )
            .unwrap();
        memory.seal_owner(owner).unwrap();
        let process = prepare_executable_runtime_process(
            memory,
            &plan,
            &image,
            &[],
            &catalog,
            0xec02_5c47,
            "directshow.exe",
            b"C:\\Vine\\directshow.exe\0".to_vec(),
        )
        .unwrap();
        let endpoints = process
            .binding
            .imports
            .iter()
            .filter_map(ApiEndpointIdentity::from_runtime_guest_import)
            .collect::<Vec<_>>();
        let endpoint_for = |wanted: &str| {
            ApiEndpointIdentity::from_runtime_guest_import(
            process.binding.imports.iter().find(|import|
                matches!(&import.target, ImportTarget::Name { name, .. } if name == wanted)).unwrap()).unwrap()
        };
        let co_initialize = endpoint_for("CoInitialize");
        let co_create = endpoint_for("CoCreateInstance");
        let mut browser = BrowserRuntime {
            runtime: process.runtime,
            dispatcher: ApiDispatcher::from_endpoints(endpoints).unwrap(),
            callback_sentinel: GuestAddress(0x7f01_0000),
            pending_host: BTreeMap::new(),
            pending_com_audio: None,
            pending_com_media: BTreeMap::new(),
            prepared_source_trap: None,
            pending_source_lookup: None,
            pending_source_search: None,
        };
        let identity = snapshot.identity();
        browser
            .runtime
            .register_source_snapshot(RuntimeSourceSnapshotManifest {
                mount: RuntimeSourceMountManifest {
                    source_id: 1,
                    root_path_a: b"C:\\Vine\\\0".to_vec(),
                },
                files: vec![RuntimeSourceFileManifest {
                    object_id: 1,
                    relative_path_a: b"directshow.exe".to_vec(),
                    byte_length: identity.bytes,
                }],
            })
            .unwrap();
        let acknowledge = |browser: &mut BrowserRuntime| {
            if browser.runtime.memory().has_pending_delta() {
                let generation = browser.runtime.memory().stats().generation;
                browser.runtime.memory_mut().pending_delta_binary().unwrap();
                browser
                    .runtime
                    .memory_mut()
                    .acknowledge_delta(generation)
                    .unwrap();
            }
        };
        let write_words = |browser: &mut BrowserRuntime, words: &[u32]| {
            let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
            browser.runtime.memory_mut().write(stack, &bytes).unwrap();
            acknowledge(browser);
        };
        let read_word = |browser: &BrowserRuntime, address| {
            u32::from_le_bytes(
                browser
                    .runtime
                    .memory()
                    .read(address, 4)
                    .unwrap()
                    .try_into()
                    .unwrap(),
            )
        };
        let return_address = image.entry_point_address;
        let reflection = browser.runtime.memory_mut().pending_reflection().unwrap();
        browser
            .runtime
            .memory_mut()
            .acknowledge_delta(reflection.invalidation.generation)
            .unwrap();
        for (endpoint, arguments) in [
            (&co_initialize, vec![0]),
            (&co_create, vec![class.0, 0, 1, iid.0, output.0]),
        ] {
            browser
                .runtime
                .memory_mut()
                .write(class, &vine_core::directshow::CLSID_FILTER_GRAPH.0)
                .unwrap();
            browser
                .runtime
                .memory_mut()
                .write(iid, &vine_core::directshow::IID_GRAPH_BUILDER.0)
                .unwrap();
            let mut words = vec![return_address.0];
            words.extend(arguments);
            write_words(&mut browser, &words);
            let outcome = browser.runtime.dispatch_api_call(
                &browser.dispatcher,
                ApiOwnedTrap {
                    endpoint_id: endpoint.id,
                    endpoint_address: endpoint.address,
                    stack_pointer: stack,
                    return_address,
                },
                browser.callback_sentinel,
                None,
            );
            let DispatchOutcome::Immediate(completion) = outcome else {
                panic!("factory {outcome:?}");
            };
            assert_eq!(completion.eax, Some(0));
            acknowledge(&mut browser);
        }
        let graph = read_word(&browser, output);
        let vtable = read_word(&browser, GuestAddress(graph));
        let render = GuestAddress(read_word(&browser, GuestAddress(vtable + 13 * 4)));
        let path_bytes: Vec<u8> = "missing-media.mpg\0"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        browser
            .runtime
            .memory_mut()
            .write(path, &path_bytes)
            .unwrap();
        write_words(&mut browser, &[return_address.0, graph, path.0, 0]);
        browser.runtime.set_current_last_error(77).unwrap();
        let (metadata, _, _) = browser
            .dispatch_com(render, stack, return_address, 0)
            .unwrap();
        assert_eq!(metadata["kind"], "media-request");
        let request_id = metadata["requestId"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        let initial = browser.runtime.current_thread_id();
        let second = RuntimeThreadId(SECOND_THREAD_ID);
        browser.runtime.register_thread(second, 0).unwrap();
        browser.runtime.select_thread(second).unwrap();
        browser.runtime.set_current_last_error(88).unwrap();
        let before = browser.runtime.memory().stats();
        let owner_state = browser.runtime.scheduler_state_for_thread(initial).unwrap();
        for response in [
            ComMediaResponse::Acknowledged,
            ComMediaResponse::Opened(ComMediaFormat {
                resource_id: request_id,
                width: 8193,
                height: 16,
                duration_100ns: Some(800000),
                frame_duration_100ns: 400000,
                audio_channels: 0,
                audio_samples_per_second: 0,
            }),
            ComMediaResponse::Opened(ComMediaFormat {
                resource_id: request_id + 1,
                width: 16,
                height: 16,
                duration_100ns: Some(800000),
                frame_duration_100ns: 400000,
                audio_channels: 0,
                audio_samples_per_second: 0,
            }),
        ] {
            assert!(browser.complete_com_media(request_id, response).is_err());
            assert!(browser.pending_com_media.contains_key(&request_id));
            assert_eq!(browser.runtime.current_thread_id(), second);
            assert_eq!(browser.runtime.current_last_error().unwrap(), 88);
            assert_eq!(
                browser.runtime.thread_state(initial).unwrap().last_error,
                77
            );
            assert_eq!(
                browser.runtime.scheduler_state_for_thread(initial).unwrap(),
                owner_state
            );
            assert_eq!(browser.runtime.memory().stats(), before);
        }
        let (metadata, _, _) = browser
            .complete_com_media(request_id, ComMediaResponse::FileNotFound)
            .unwrap();
        assert_eq!(metadata["kind"], "com-complete");
        assert_eq!(metadata["threadId"], initial.0);
        assert_eq!(
            metadata["completion"]["eax"],
            vine_core::directshow::VFW_E_NOT_FOUND
        );
        assert!(!browser.pending_com_media.contains_key(&request_id));
        assert_eq!(browser.runtime.current_thread_id(), initial);
        assert_eq!(browser.runtime.current_last_error().unwrap(), 77);
    }

    #[test]
    fn orphan_media_completion_preserves_thread_memory_and_last_error() {
        let mut browser = BrowserRuntime {
            runtime: running_test_runtime(),
            dispatcher: ApiDispatcher::from_endpoints([]).unwrap(),
            callback_sentinel: GuestAddress(0x7f010000),
            pending_host: BTreeMap::new(),
            pending_com_audio: None,
            pending_com_media: BTreeMap::new(),
            prepared_source_trap: None,
            pending_source_lookup: None,
            pending_source_search: None,
        };
        browser.runtime.set_current_last_error(77).unwrap();
        let before = browser.runtime.memory().stats();
        let thread = browser.runtime.current_thread_id();
        assert!(browser.read_com_media(1, 0, 4).is_err());
        assert!(
            browser
                .complete_com_media(1, ComMediaResponse::Acknowledged)
                .is_err()
        );
        assert_eq!(browser.runtime.current_thread_id(), thread);
        assert_eq!(browser.runtime.current_last_error().unwrap(), 77);
        assert_eq!(browser.runtime.memory().stats(), before);
    }
}
