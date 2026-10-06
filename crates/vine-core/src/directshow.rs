//! Reached DirectShow graph operations. Windows identities and decoded sample
//! backing stay in Rust; the browser owns only its codec and audio resources.
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "directshow_media_completion_tests.rs"]
mod media_completion_tests;

use crate::com::{
    ComCallFrame, ComCallTrap, ComDispatchOutcome, ComFrontendCall, ComInterfaceSpec,
    ComMethodSpec, ComObjectSpec,
};
use crate::memory::{
    GuestAddress, GuestRange, MemoryAccess, MemoryOwnerKind, MemoryRegionInfo, MemoryRegionKind,
    PageProtection,
};
use crate::runtime::{
    Runtime, RuntimeCallbackFrameId, RuntimeComClass, RuntimeComIid, RuntimeComMethodTarget,
    RuntimeComObjectId, RuntimeStateError, RuntimeThreadId,
};

pub const CLSID_FILTER_GRAPH: RuntimeComIid = RuntimeComIid([
    0xb3, 0xeb, 0x36, 0xe4, 0x4f, 0x52, 0xce, 0x11, 0x9f, 0x53, 0x00, 0x20, 0xaf, 0x0b, 0xa7, 0x70,
]);
pub const IID_GRAPH_BUILDER: RuntimeComIid = dshow_iid(0xa9);
pub(crate) const IID_FILTER_GRAPH: RuntimeComIid = dshow_iid(0x9f);
pub(crate) const IID_MEDIA_CONTROL: RuntimeComIid = dshow_iid(0xb1);
pub(crate) const IID_MEDIA_EVENT_EX: RuntimeComIid = dshow_iid(0xc0);
pub(crate) const IID_MEDIA_EVENT_SINK: RuntimeComIid = dshow_iid(0xa2);
pub(crate) const IID_BASIC_AUDIO: RuntimeComIid = dshow_iid(0xb3);
pub(crate) const IID_MEDIA_SEEKING: RuntimeComIid = RuntimeComIid([
    0x80, 0x38, 0xb7, 0x36, 0xc8, 0xc2, 0xcf, 0x11, 0x8b, 0x46, 0x00, 0x80, 0x5f, 0x6c, 0xef, 0x60,
]);
const IID_MEM_INPUT_PIN: RuntimeComIid = dshow_iid(0x9d);
pub(crate) const IID_ASYNC_READER: RuntimeComIid = dshow_iid(0xaa);
const IID_MEDIA_SAMPLE: RuntimeComIid = dshow_iid(0x9a);
const IID_MEM_ALLOCATOR: RuntimeComIid = dshow_iid(0x9c);
const IID_PIN: RuntimeComIid = dshow_iid(0x91);
const TIME_FORMAT_MEDIA_TIME: RuntimeComIid = RuntimeComIid([
    0x65, 0x57, 0x0f, 0x7b, 0x54, 0x2c, 0xce, 0x11, 0x8f, 0x17, 0x00, 0xaa, 0x00, 0x5f, 0xe2, 0x0b,
]);
pub const VFW_E_NOT_FOUND: u32 = 0x8004_0216;
const VFW_E_NOT_CONNECTED: u32 = 0x8004_0209;
const VFW_E_WRONG_STATE: u32 = 0x8004_0227;
const E_INVALIDARG: u32 = 0x8007_0057;
const EC_WINDOW_DESTROYED: u32 = 21;
const IID_STREAM_ENTRY: RuntimeComIid = RuntimeComIid([
    0x31, 0x5b, 0x65, 0xac, 0xca, 0x68, 0xa0, 0x48, 0xa7, 0xd2, 0x61, 0x95, 0xe3, 0x3c, 0x71, 0x39,
]);

const fn dshow_iid(low: u8) -> RuntimeComIid {
    RuntimeComIid([
        low, 0x68, 0xa8, 0x56, 0xd4, 0x0a, 0xce, 0x11, 0xb0, 0x3a, 0x00, 0x20, 0xaf, 0x0b, 0xa7,
        0x70,
    ])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphState {
    Stopped,
    Paused,
    Running,
}

#[derive(Debug)]
pub(crate) struct GuestFilter {
    pub pointer: GuestAddress,
}

#[derive(Debug)]
pub(crate) struct DirectShowGraph {
    pub filters: Vec<GuestFilter>,
    pub state: GraphState,
    pub position_100ns: i64,
    pub duration_100ns: Option<i64>,
    pub volume: i32,
    pub balance: i32,
    pub notify: Option<(u32, u32, u32)>,
    pub completed: bool,
    event_pending: bool,
    reader: Option<(GuestAddress, u64)>,
    source_pin: Option<GuestAddress>,
    input_pin: Option<GuestAddress>,
    mem_input: Option<GuestAddress>,
    format: Option<ComMediaFormat>,
    encoded_buffer: Option<MediaBacking>,
    allocator: Option<RuntimeComObjectId>,
    output_pin: Option<RuntimeComObjectId>,
    stream: Option<RuntimeComObjectId>,
    stream_thread: Option<RuntimeThreadId>,
    playback_origin: Option<(i64, u64)>,
    file_handle: Option<crate::runtime::RuntimeHandle>,
    last_audio_end_100ns: i64,
}

impl Default for DirectShowGraph {
    fn default() -> Self {
        Self {
            filters: Vec::new(),
            state: GraphState::Stopped,
            position_100ns: 0,
            duration_100ns: None,
            volume: 0,
            balance: 0,
            notify: None,
            completed: false,
            event_pending: false,
            reader: None,
            source_pin: None,
            input_pin: None,
            mem_input: None,
            format: None,
            encoded_buffer: None,
            allocator: None,
            output_pin: None,
            stream: None,
            stream_thread: None,
            playback_origin: None,
            file_handle: None,
            last_audio_end_100ns: 0,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeDirectShow {
    pub graphs: BTreeMap<RuntimeComObjectId, DirectShowGraph>,
    pub callbacks: BTreeMap<RuntimeCallbackFrameId, DirectShowCallback>,
    active: BTreeMap<u64, ActiveMediaCall>,
    pub(crate) allocators: BTreeMap<RuntimeComObjectId, MediaAllocator>,
    pub(crate) samples: BTreeMap<RuntimeComObjectId, MediaSample>,
    pub(crate) output_pins: BTreeMap<RuntimeComObjectId, RuntimeComObjectId>,
    pub(crate) streams: BTreeMap<RuntimeComObjectId, RuntimeComObjectId>,
}

#[derive(Debug, Clone)]
pub enum ComMediaSource {
    GuestAsyncReader {
        pointer: GuestAddress,
        byte_length: u64,
    },
    SourceFile {
        source_id: u32,
        object_id: u32,
        byte_length: u64,
    },
    WritableFile {
        relative_path: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComMediaOperation {
    Open,
    NextFrame,
    Run,
    Pause,
    Stop,
    Volume,
    Release,
    AudioPush,
}

#[derive(Debug)]
#[non_exhaustive]
pub struct ComMediaDeferred {
    pub request_id: u64,
    pub graph_id: RuntimeComObjectId,
    pub thread_id: RuntimeThreadId,
    pub operation: ComMediaOperation,
    pub resource_id: Option<u64>,
    pub source: Option<ComMediaSource>,
    pub volume: i32,
    pub delay_milliseconds: u32,
    pub audio: Option<ComMediaAudio>,
}

#[derive(Debug)]
pub struct ComMediaData {
    pub request_id: u64,
    pub offset: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComMediaFormat {
    pub resource_id: u64,
    pub width: u32,
    pub height: u32,
    pub duration_100ns: Option<i64>,
    pub frame_duration_100ns: i64,
    pub audio_channels: u32,
    pub audio_samples_per_second: u32,
}

#[derive(Debug)]
pub struct ComMediaFrame {
    pub resource_id: u64,
    pub start_100ns: i64,
    pub end_100ns: i64,
    pub bgra: Vec<u8>,
}

#[derive(Debug)]
pub enum ComMediaResponse {
    Opened(ComMediaFormat),
    Frame(ComMediaFrame),
    Audio(ComMediaAudio),
    Acknowledged,
    Ended { resource_id: u64 },
    Stopped { resource_id: u64 },
    Failed { hresult: u32 },
    BackendFailed { message: String },
    FileNotFound,
    Unsupported { message: String },
}

#[derive(Debug, Clone)]
pub struct ComMediaAudio {
    pub resource_id: u64,
    pub start_100ns: i64,
    pub end_100ns: i64,
    pub channels: u32,
    pub samples_per_second: u32,
    pub pcm_f32le: Vec<u8>,
}

#[derive(Debug)]
struct ActiveMediaCall {
    origin: ComOrigin,
    operation: ComMediaOperation,
    source: Option<ComMediaSource>,
    thread_id: RuntimeThreadId,
    host_wait: Option<u64>,
    approved_audio: Option<ComMediaAudio>,
}

#[derive(Debug, Clone)]
struct ComOrigin {
    trap: ComCallTrap,
    target: RuntimeComMethodTarget,
    frame: ComCallFrame,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MediaBacking {
    pub allocation_id: crate::runtime::ProcessAddressAllocationId,
    pub owner: crate::memory::MemoryOwnerId,
    pub range: GuestRange,
}

#[derive(Debug)]
pub(crate) struct MediaAllocator {
    graph_id: RuntimeComObjectId,
    properties: [i32; 4],
    committed: bool,
    free: Vec<MediaBacking>,
}

#[derive(Debug)]
pub(crate) struct MediaSample {
    pub(crate) allocator_id: RuntimeComObjectId,
    pub(crate) backing: MediaBacking,
    byte_length: u32,
    start_100ns: i64,
    end_100ns: i64,
    sync_point: bool,
    preroll: bool,
    discontinuity: bool,
}

impl ComOrigin {
    fn validate(&self, runtime: &Runtime) -> Result<(), RuntimeStateError> {
        self.validate_with(runtime, false)
    }

    fn validate_host(&self, runtime: &Runtime) -> Result<(), RuntimeStateError> {
        self.validate_with(runtime, true)
    }

    fn validate_with(
        &self,
        runtime: &Runtime,
        preparing_owned_bytes: bool,
    ) -> Result<(), RuntimeStateError> {
        let live = runtime
            .resolve_com_method(self.trap.endpoint_address)?
            .ok_or_else(|| RuntimeStateError::new("pending DirectShow COM endpoint was retired"))?;
        if live.object_id != self.target.object_id
            || live.interface_id != self.target.interface_id
            || live.guest_pointer != self.target.guest_pointer
            || live.iid != self.target.iid
            || live.method.slot != self.target.method.slot
        {
            return Err(RuntimeStateError::new(
                "DirectShow handoff no longer identifies its live COM call",
            ));
        }
        let words = self.frame.explicit_arguments.len() + 2;
        let bytes = if preparing_owned_bytes {
            let stack = runtime
                .current_thread_state()?
                .stack
                .as_ref()
                .ok_or_else(|| {
                    RuntimeStateError::new("pending native COM call has no stack owner")
                })?;
            runtime
                .memory
                .read_owned_host(self.trap.stack_pointer, words * 4, stack.memory_owner)
        } else {
            runtime.memory.read(self.trap.stack_pointer, words * 4)
        }
        .map_err(RuntimeStateError::from_memory)?;
        let actual: Vec<u32> = bytes
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
            .collect();
        if actual[0] != self.trap.return_address.0
            || actual[1] != self.target.guest_pointer.0
            || actual[2..] != self.frame.explicit_arguments
        {
            return Err(RuntimeStateError::new(
                "DirectShow caller stack changed during its native handoff",
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
enum NativeArgument {
    Value(u32),
    Scratch(u32),
}

#[derive(Debug)]
struct NativeCall {
    pointer: GuestAddress,
    slot: u16,
    arguments: Vec<NativeArgument>,
    scratch: Vec<u8>,
    relocations: Vec<(u32, u32)>,
}

#[derive(Debug)]
pub(crate) struct DirectShowCallback {
    origin: Box<ComOrigin>,
    step: GraphStep,
    native: NativeCall,
    target: GuestAddress,
    vtable: GuestAddress,
    mappings: [MemoryRegionInfo; 3],
    scratch: GuestAddress,
}

#[derive(Debug)]
enum GraphStep {
    AddFilterRef {
        pointer: GuestAddress,
        name: String,
    },
    JoinFilter {
        pointer: GuestAddress,
    },
    RejectFilter {
        hresult: u32,
    },
    QueryReader {
        pin: GuestAddress,
    },
    ReaderLength {
        pin: GuestAddress,
        reader: GuestAddress,
    },
    EncodedRead {
        request_id: u64,
        offset: u64,
        length: u32,
        backing: MediaBacking,
    },
    EnumeratePins {
        filter_index: usize,
        format: ComMediaFormat,
    },
    NextPin {
        filter_index: usize,
        enumerator: GuestAddress,
        format: ComMediaFormat,
    },
    PinDirection {
        filter_index: usize,
        enumerator: GuestAddress,
        pin: GuestAddress,
        format: ComMediaFormat,
    },
    SkipPin {
        filter_index: usize,
        enumerator: GuestAddress,
        format: ComMediaFormat,
    },
    ReleaseEnumerator {
        pin: GuestAddress,
        format: ComMediaFormat,
    },
    QueryInput {
        pin: GuestAddress,
        format: ComMediaFormat,
    },
    ConnectInput {
        mem_input: GuestAddress,
    },
    NotifyInput,
    FilterTransition {
        state: GraphState,
        index: usize,
        preparing_run: bool,
    },
    ReceiveSample {
        sample: RuntimeComObjectId,
        end_100ns: i64,
    },
    EndOfStream,
    Teardown {
        remaining: std::collections::VecDeque<NativeCall>,
    },
}

fn runtime_failure(origin: ComOrigin, error: RuntimeStateError) -> ComDispatchOutcome {
    ComDispatchOutcome::Fault(crate::com::ComFault {
        message: error.message,
        call_site: origin.trap.call_site,
        target: Some(crate::com::ComFaultTarget {
            interface: origin.target.interface_name,
            vtable_slot: origin.target.method.slot,
            method: origin.target.method.name,
        }),
    })
}

impl Runtime {
    fn start_directshow_callback(
        &mut self,
        origin: Box<ComOrigin>,
        step: GraphStep,
        native: NativeCall,
    ) -> Result<crate::api::PreparedGuestCallback, RuntimeStateError> {
        origin.validate_host(self)?;
        let vtable = read_address_host(self, native.pointer)?;
        let method_address = GuestAddress(
            vtable
                .0
                .checked_add(u32::from(native.slot) * 4)
                .ok_or_else(|| RuntimeStateError::new("guest COM vtable address overflow"))?,
        );
        let target = read_address_host(self, method_address)?;
        self.memory
            .validate_host_execute(target, 1)
            .map_err(RuntimeStateError::from_memory)?;
        let mappings = [
            region_for(self, native.pointer)?,
            region_for(self, method_address)?,
            region_for(self, target)?,
        ];
        let scratch_bytes = u32::try_from(native.scratch.len())
            .map_err(|_| RuntimeStateError::new("native COM scratch overflow"))?;
        let sentinel = crate::callbacks::CALLBACK_RETURN_ARENA_BASE;
        self.memory
            .validate_host_execute(sentinel, 1)
            .map_err(RuntimeStateError::from_memory)?;
        let frame = self.prepare_callback_frame(None, target, sentinel, scratch_bytes)?;
        let scratch = frame
            .scratch
            .map_or(GuestAddress(0), |scratch| scratch.address);
        let frame_bytes = u32::try_from((native.arguments.len() + 2) * 4)
            .map_err(|_| RuntimeStateError::new("native COM stack overflow"))?;
        let stack_pointer = GuestAddress(
            origin
                .trap
                .stack_pointer
                .0
                .checked_sub(frame_bytes)
                .ok_or_else(|| RuntimeStateError::new("native COM callback stack underflow"))?,
        );
        let stack = self.current_thread_state()?.stack.as_ref().ok_or_else(|| {
            RuntimeStateError::new("native COM callback has no owned guest stack")
        })?;
        let stack_range = GuestRange::new(stack_pointer, u64::from(frame_bytes))
            .map_err(RuntimeStateError::from_memory)?;
        if stack_range.start().0 < stack.committed_range.start().0
            || stack_range.end() > stack.committed_range.end()
        {
            return Err(RuntimeStateError::new(
                "native COM callback extends outside the current committed stack",
            ));
        }
        self.memory
            .validate_host_write(stack_pointer, frame_bytes as usize)
            .map_err(RuntimeStateError::from_memory)?;
        let mut words = vec![sentinel.0, native.pointer.0];
        for argument in &native.arguments {
            words.push(match argument {
                NativeArgument::Value(value) => *value,
                NativeArgument::Scratch(offset) => {
                    if *offset >= scratch_bytes {
                        return Err(RuntimeStateError::new(
                            "native COM argument left its scratch",
                        ));
                    }
                    scratch.0.checked_add(*offset).ok_or_else(|| {
                        RuntimeStateError::new("native COM scratch pointer overflow")
                    })?
                }
            });
        }
        let stack_bytes: Vec<u8> = words.into_iter().flat_map(u32::to_le_bytes).collect();
        let mut scratch_image = native.scratch.clone();
        for (write, pointee) in &native.relocations {
            let write = *write as usize;
            if write
                .checked_add(4)
                .is_none_or(|end| end > scratch_image.len())
                || *pointee >= scratch_bytes
            {
                return Err(RuntimeStateError::new(
                    "native COM scratch relocation left its image",
                ));
            }
            let pointer = scratch
                .0
                .checked_add(*pointee)
                .ok_or_else(|| RuntimeStateError::new("native COM relocation overflow"))?;
            scratch_image[write..write + 4].copy_from_slice(&pointer.to_le_bytes());
        }
        let allocation = self.callback_scratch_allocation()?;
        let (allocation_id, allocation_range) = (allocation.id, allocation.range);
        let region = self
            .memory
            .regions()
            .find(|region| region.address_allocation_id == Some(allocation_id.0));
        if let Some(region) = region
            && (region.kind != MemoryRegionKind::Runtime
                || region.range.start() != allocation_range.base
                || region.range.byte_length() != allocation_range.byte_length)
        {
            return Err(RuntimeStateError::new(
                "native COM callback scratch mapping lost its allocation owner",
            ));
        }
        let writable = PageProtection {
            read: true,
            write: true,
            execute: false,
        };
        let mut transaction = self.memory.begin_transaction();
        if let Some(slot) = frame.scratch {
            let owner = match region {
                Some(region) => region.owner,
                None => {
                    let owner = transaction
                        .create_owner(MemoryOwnerKind::Runtime)
                        .map_err(RuntimeStateError::from_memory)?;
                    transaction
                        .reserve_fixed_for_address_allocation(
                            allocation_id.0,
                            owner,
                            MemoryRegionKind::Runtime,
                            allocation_range.base,
                            allocation_range.byte_length,
                        )
                        .map_err(RuntimeStateError::from_memory)?;
                    owner
                }
            };
            match transaction
                .committed_page(slot.address)
                .map_err(RuntimeStateError::from_memory)?
            {
                None => {
                    transaction
                        .commit(
                            owner,
                            GuestRange::new(slot.address, u64::from(slot.capacity))
                                .map_err(RuntimeStateError::from_memory)?,
                            writable,
                        )
                        .map_err(RuntimeStateError::from_memory)?;
                }
                Some((actual_owner, protection))
                    if actual_owner == owner && protection == writable => {}
                Some(_) => {
                    return Err(RuntimeStateError::new(
                        "native COM scratch page has wrong owner or protection",
                    ));
                }
            }
            transaction
                .write(scratch, &scratch_image)
                .map_err(RuntimeStateError::from_memory)?;
        }
        transaction
            .write(stack_pointer, &stack_bytes)
            .map_err(RuntimeStateError::from_memory)?;
        transaction.commit_transaction();
        self.publish_callback_frame(frame, false);
        self.direct_show.callbacks.insert(
            frame.id,
            DirectShowCallback {
                origin,
                step,
                native,
                target,
                vtable,
                mappings,
                scratch,
            },
        );
        Ok(crate::api::PreparedGuestCallback {
            frame_id: frame.id,
            target,
            return_sentinel: sentinel,
            allocation_start: stack_pointer,
            stack_pointer,
            scratch_address: scratch,
            scratch_bytes,
            frame_bytes,
        })
    }

    pub(crate) fn complete_directshow_callback(
        &mut self,
        sentinel: GuestAddress,
        result: u32,
    ) -> Result<ComDispatchOutcome, RuntimeStateError> {
        let frame = *self
            .current_thread_state()?
            .callback_frames
            .last()
            .ok_or_else(|| {
                RuntimeStateError::new("DirectShow callback return has no active frame")
            })?;
        self.validate_callback_frame_return(frame.id)?;
        let pending = self.direct_show.callbacks.get(&frame.id).ok_or_else(|| {
            RuntimeStateError::new("DirectShow callback has no pending native call")
        })?;
        pending.origin.validate(self)?;
        let completed_release = pending.native.slot == 2 && result == 0;
        if frame.return_sentinel != sentinel
            || frame.target != pending.target
            || (!completed_release && read_address(self, pending.native.pointer)? != pending.vtable)
        {
            return Err(RuntimeStateError::new(
                "DirectShow callback return lost its target identity",
            ));
        }
        for mapping in pending.mappings.iter().filter(|_| !completed_release) {
            if !self.memory.regions().any(|live| live == *mapping) {
                return Err(RuntimeStateError::new(
                    "DirectShow guest interface backing was retired during callback",
                ));
            }
        }
        let pending = self
            .direct_show
            .callbacks
            .remove(&frame.id)
            .expect("validated pending callback");
        self.pop_callback_frame(frame.id)?;
        advance_graph_callback(self, pending, result)
    }
}

fn read_address(
    runtime: &Runtime,
    address: GuestAddress,
) -> Result<GuestAddress, RuntimeStateError> {
    let bytes = runtime
        .memory
        .read(address, 4)
        .map_err(RuntimeStateError::from_memory)?;
    Ok(GuestAddress(u32::from_le_bytes(bytes.try_into().unwrap())))
}

fn read_address_host(
    runtime: &Runtime,
    address: GuestAddress,
) -> Result<GuestAddress, RuntimeStateError> {
    let owner = region_for(runtime, address)?.owner;
    let bytes = runtime
        .memory
        .read_owned_host(address, 4, owner)
        .map_err(RuntimeStateError::from_memory)?;
    Ok(GuestAddress(u32::from_le_bytes(bytes.try_into().unwrap())))
}

fn region_for(
    runtime: &Runtime,
    address: GuestAddress,
) -> Result<MemoryRegionInfo, RuntimeStateError> {
    runtime
        .memory
        .regions()
        .find(|region| {
            u64::from(address.0) >= u64::from(region.range.start().0)
                && u64::from(address.0) < region.range.end()
        })
        .ok_or_else(|| RuntimeStateError::new("guest COM address has no live memory owner"))
}

fn read_wide(
    runtime: &Runtime,
    address: GuestAddress,
    limit: usize,
) -> Result<String, RuntimeStateError> {
    let mut words = Vec::new();
    for index in 0..limit {
        let offset = u32::try_from(index * 2)
            .map_err(|_| RuntimeStateError::new("wide string offset overflow"))?;
        let pointer = GuestAddress(
            address
                .0
                .checked_add(offset)
                .ok_or_else(|| RuntimeStateError::new("wide string address overflow"))?,
        );
        let bytes = runtime
            .memory
            .read(pointer, 2)
            .map_err(RuntimeStateError::from_memory)?;
        let word = u16::from_le_bytes(bytes.try_into().unwrap());
        if word == 0 {
            return String::from_utf16(&words)
                .map_err(|_| RuntimeStateError::new("guest wide string contains invalid UTF16"));
        }
        words.push(word);
    }
    Err(RuntimeStateError::new(
        "guest wide string has no bounded terminator",
    ))
}

fn plain_call(pointer: GuestAddress, slot: u16, arguments: &[u32]) -> NativeCall {
    NativeCall {
        pointer,
        slot,
        arguments: arguments
            .iter()
            .copied()
            .map(NativeArgument::Value)
            .collect(),
        scratch: Vec::new(),
        relocations: Vec::new(),
    }
}

fn query_call(pointer: GuestAddress, iid: RuntimeComIid) -> NativeCall {
    let mut scratch = iid.0.to_vec();
    scratch.extend_from_slice(&0u32.to_le_bytes());
    NativeCall {
        pointer,
        slot: 0,
        arguments: vec![NativeArgument::Scratch(0), NativeArgument::Scratch(16)],
        scratch,
        relocations: Vec::new(),
    }
}

fn callback_outcome(
    runtime: &mut Runtime,
    origin: Box<ComOrigin>,
    step: GraphStep,
    native: NativeCall,
) -> ComDispatchOutcome {
    match runtime.start_directshow_callback(origin, step, native) {
        Ok(prepared) => ComDispatchOutcome::GuestCallback(prepared),
        Err(error) => ComDispatchOutcome::Fault(crate::com::ComFault {
            message: error.message,
            call_site: GuestAddress(0),
            target: None,
        }),
    }
}

fn advance_graph_callback(
    runtime: &mut Runtime,
    pending: DirectShowCallback,
    result: u32,
) -> Result<ComDispatchOutcome, RuntimeStateError> {
    let id = origin_graph_id(runtime, &pending.origin)?;
    match pending.step {
        GraphStep::AddFilterRef { pointer, name } => {
            if result == 0 {
                return Err(RuntimeStateError::new("guest filter AddRef returned zero"));
            }
            let graph_pointer = runtime
                .com()
                .interface_for_object_iid(id, IID_FILTER_GRAPH)?
                .guest_pointer;
            let name_bytes: Vec<u8> = name
                .encode_utf16()
                .chain([0])
                .flat_map(u16::to_le_bytes)
                .collect();
            let call = NativeCall {
                pointer,
                slot: 13,
                arguments: vec![
                    NativeArgument::Value(graph_pointer.0),
                    NativeArgument::Scratch(0),
                ],
                scratch: name_bytes,
                relocations: Vec::new(),
            };
            Ok(callback_outcome(
                runtime,
                pending.origin,
                GraphStep::JoinFilter { pointer },
                call,
            ))
        }
        GraphStep::JoinFilter { pointer } => {
            if (result as i32) < 0 {
                return Ok(callback_outcome(
                    runtime,
                    pending.origin,
                    GraphStep::RejectFilter { hresult: result },
                    plain_call(pointer, 2, &[]),
                ));
            }
            runtime
                .direct_show
                .graphs
                .get_mut(&id)
                .expect("validated graph")
                .filters
                .push(GuestFilter { pointer });
            Ok(crate::com::complete_call(pending.origin.frame, 0))
        }
        GraphStep::RejectFilter { hresult } => {
            Ok(crate::com::complete_call(pending.origin.frame, hresult))
        }
        GraphStep::QueryReader { pin } => {
            if (result as i32) < 0 {
                return Err(RuntimeStateError::new(
                    "DirectShow rendering requires a real IAsyncReader source pin",
                ));
            }
            let reader = read_address(runtime, GuestAddress(pending.scratch.0 + 16))?;
            if reader.0 == 0 {
                return Err(RuntimeStateError::new(
                    "successful source QI returned a null IAsyncReader",
                ));
            }
            let native = NativeCall {
                pointer: reader,
                slot: 8,
                arguments: vec![NativeArgument::Scratch(0), NativeArgument::Scratch(8)],
                scratch: vec![0; 16],
                relocations: Vec::new(),
            };
            Ok(callback_outcome(
                runtime,
                pending.origin,
                GraphStep::ReaderLength { pin, reader },
                native,
            ))
        }
        GraphStep::ReaderLength { pin, reader } => {
            if (result as i32) < 0 {
                return Ok(crate::com::complete_call(pending.origin.frame, result));
            }
            let bytes = runtime
                .memory
                .read(pending.scratch, 16)
                .map_err(RuntimeStateError::from_memory)?;
            let total = i64::from_le_bytes(bytes[..8].try_into().unwrap());
            let available = i64::from_le_bytes(bytes[8..].try_into().unwrap());
            if total <= 0 || available < 0 || available > total {
                return Err(RuntimeStateError::new(
                    "IAsyncReader Length returned invalid stream bounds",
                ));
            }
            if available != total {
                return Err(RuntimeStateError::new(
                    "partial IAsyncReader availability is not implemented",
                ));
            }
            let graph = runtime
                .direct_show
                .graphs
                .get_mut(&id)
                .expect("validated graph");
            graph.reader = Some((reader, total as u64));
            graph.source_pin = Some(pin);
            Ok(runtime.begin_media_call(
                *pending.origin,
                ComMediaOperation::Open,
                Some(ComMediaSource::GuestAsyncReader {
                    pointer: reader,
                    byte_length: total as u64,
                }),
            )?)
        }
        GraphStep::EncodedRead {
            request_id,
            offset,
            length,
            backing,
        } => {
            let active = runtime.direct_show.active.get(&request_id).ok_or_else(|| {
                RuntimeStateError::new("encoded media read outlived its decoder operation")
            })?;
            active.origin.validate(runtime)?;
            runtime.validate_media_backing(backing)?;
            if result != 0 {
                return Err(RuntimeStateError::new(format!(
                    "IAsyncReader SyncRead failed with HRESULT {result:#010x}"
                )));
            }
            let bytes = runtime
                .memory
                .read(backing.range.start(), length as usize)
                .map_err(RuntimeStateError::from_memory)?;
            let host = runtime.begin_host_request("browser.media")?;
            runtime
                .direct_show
                .active
                .get_mut(&request_id)
                .unwrap()
                .host_wait = Some(host.request_id);
            Ok(ComDispatchOutcome::MediaData(ComMediaData {
                request_id,
                offset,
                bytes,
            }))
        }
        GraphStep::EnumeratePins {
            filter_index,
            format,
        } => {
            if (result as i32) < 0 {
                return Ok(crate::com::complete_call(pending.origin.frame, result));
            }
            let enumerator = read_address(runtime, pending.scratch)?;
            if enumerator.0 == 0 {
                return Err(RuntimeStateError::new(
                    "EnumPins succeeded with a null enumerator",
                ));
            }
            let native = NativeCall {
                pointer: enumerator,
                slot: 3,
                arguments: vec![
                    NativeArgument::Value(1),
                    NativeArgument::Scratch(0),
                    NativeArgument::Scratch(4),
                ],
                scratch: vec![0; 8],
                relocations: Vec::new(),
            };
            Ok(callback_outcome(
                runtime,
                pending.origin,
                GraphStep::NextPin {
                    filter_index,
                    enumerator,
                    format,
                },
                native,
            ))
        }
        GraphStep::NextPin {
            filter_index,
            enumerator,
            format,
        } => {
            if result == 1 {
                return Err(RuntimeStateError::new(
                    "filter has no input pin available for decoded video",
                ));
            }
            if result != 0 {
                return Ok(crate::com::complete_call(pending.origin.frame, result));
            }
            let bytes = runtime
                .memory
                .read(pending.scratch, 8)
                .map_err(RuntimeStateError::from_memory)?;
            let pin = GuestAddress(u32::from_le_bytes(bytes[..4].try_into().unwrap()));
            if pin.0 == 0 || u32::from_le_bytes(bytes[4..].try_into().unwrap()) != 1 {
                return Err(RuntimeStateError::new(
                    "IEnumPins Next result did not publish exactly one live pin",
                ));
            }
            let native = NativeCall {
                pointer: pin,
                slot: 9,
                arguments: vec![NativeArgument::Scratch(0)],
                scratch: vec![0; 4],
                relocations: Vec::new(),
            };
            Ok(callback_outcome(
                runtime,
                pending.origin,
                GraphStep::PinDirection {
                    filter_index,
                    enumerator,
                    pin,
                    format,
                },
                native,
            ))
        }
        GraphStep::PinDirection {
            filter_index,
            enumerator,
            pin,
            format,
        } => {
            if result != 0 {
                return Ok(crate::com::complete_call(pending.origin.frame, result));
            }
            let direction = read_address(runtime, pending.scratch)?.0;
            if direction == 0 {
                return Ok(callback_outcome(
                    runtime,
                    pending.origin,
                    GraphStep::ReleaseEnumerator { pin, format },
                    plain_call(enumerator, 2, &[]),
                ));
            }
            if direction != 1 {
                return Err(RuntimeStateError::new(
                    "guest pin returned an invalid PIN_DIRECTION",
                ));
            }
            Ok(callback_outcome(
                runtime,
                pending.origin,
                GraphStep::SkipPin {
                    filter_index,
                    enumerator,
                    format,
                },
                plain_call(pin, 2, &[]),
            ))
        }
        GraphStep::SkipPin {
            filter_index,
            enumerator,
            format,
        } => {
            let native = NativeCall {
                pointer: enumerator,
                slot: 3,
                arguments: vec![
                    NativeArgument::Value(1),
                    NativeArgument::Scratch(0),
                    NativeArgument::Scratch(4),
                ],
                scratch: vec![0; 8],
                relocations: Vec::new(),
            };
            Ok(callback_outcome(
                runtime,
                pending.origin,
                GraphStep::NextPin {
                    filter_index,
                    enumerator,
                    format,
                },
                native,
            ))
        }
        GraphStep::ReleaseEnumerator { pin, format } => {
            runtime.direct_show.graphs.get_mut(&id).unwrap().input_pin = Some(pin);
            Ok(callback_outcome(
                runtime,
                pending.origin,
                GraphStep::QueryInput { pin, format },
                query_call(pin, IID_MEM_INPUT_PIN),
            ))
        }
        GraphStep::QueryInput { pin, format } => {
            if (result as i32) < 0 {
                return Ok(crate::com::complete_call(pending.origin.frame, result));
            }
            let mem_input = read_address(runtime, GuestAddress(pending.scratch.0 + 16))?;
            if mem_input.0 == 0 {
                return Err(RuntimeStateError::new(
                    "input pin QI succeeded with null IMemInputPin",
                ));
            }
            let graph_interface = runtime
                .com()
                .interface_for_object_iid(id, IID_GRAPH_BUILDER)?
                .id;
            let output = runtime.install_com_object_with_guest_writes(
                Some(graph_interface),
                output_pin_spec(),
                |_| Ok(Vec::new()),
            )?;
            runtime.direct_show.output_pins.insert(output.object_id, id);
            runtime.direct_show.graphs.get_mut(&id).unwrap().output_pin = Some(output.object_id);
            let output_pointer = output
                .interfaces
                .iter()
                .find(|(iid, _)| *iid == IID_PIN)
                .unwrap()
                .1;
            let allocator = runtime.install_com_object_with_guest_writes(
                Some(graph_interface),
                allocator_spec(),
                |_| Ok(Vec::new()),
            )?;
            runtime.direct_show.allocators.insert(
                allocator.object_id,
                MediaAllocator {
                    graph_id: id,
                    properties: [2, (format.width * format.height * 4) as i32, 1, 0],
                    committed: false,
                    free: Vec::new(),
                },
            );
            runtime.direct_show.graphs.get_mut(&id).unwrap().allocator = Some(allocator.object_id);
            runtime.commit_media_allocator(allocator.object_id)?;
            let (scratch, relocations) = video_media_type(format);
            let native = NativeCall {
                pointer: pin,
                slot: 4,
                arguments: vec![
                    NativeArgument::Value(output_pointer.0),
                    NativeArgument::Scratch(0),
                ],
                scratch,
                relocations,
            };
            Ok(callback_outcome(
                runtime,
                pending.origin,
                GraphStep::ConnectInput { mem_input },
                native,
            ))
        }
        GraphStep::ConnectInput { mem_input } => {
            if (result as i32) < 0 {
                return Ok(crate::com::complete_call(pending.origin.frame, result));
            }
            runtime.direct_show.graphs.get_mut(&id).unwrap().mem_input = Some(mem_input);
            let allocator_id = runtime.direct_show.graph(id)?.allocator.unwrap();
            let allocator = runtime
                .com()
                .interface_for_object_iid(allocator_id, IID_MEM_ALLOCATOR)?
                .guest_pointer;
            Ok(callback_outcome(
                runtime,
                pending.origin,
                GraphStep::NotifyInput,
                plain_call(mem_input, 4, &[allocator.0, 1]),
            ))
        }
        GraphStep::NotifyInput => {
            if (result as i32) < 0 {
                return Ok(crate::com::complete_call(pending.origin.frame, result));
            }
            Ok(crate::com::complete_call(pending.origin.frame, 0))
        }
        GraphStep::FilterTransition {
            state,
            index,
            preparing_run,
        } => {
            if (result as i32) < 0 {
                return Ok(crate::com::complete_call(pending.origin.frame, result));
            }
            continue_filter_transition(runtime, pending.origin, state, index + 1, preparing_run)
        }
        GraphStep::ReceiveSample { sample, end_100ns } => {
            let interface = runtime
                .com()
                .interface_for_object_iid(sample, IID_MEDIA_SAMPLE)?
                .id;
            runtime.release_com_interface(interface)?;
            if (result as i32) < 0 {
                return Err(RuntimeStateError::new(format!(
                    "guest renderer Receive failed with HRESULT {result:#010x}"
                )));
            }
            runtime
                .direct_show
                .graphs
                .get_mut(&id)
                .unwrap()
                .position_100ns = end_100ns;
            Ok(repeat_stream(*pending.origin))
        }
        GraphStep::EndOfStream => {
            if (result as i32) < 0 {
                return Err(RuntimeStateError::new(format!(
                    "guest input EndOfStream failed with HRESULT {result:#010x}"
                )));
            }
            runtime.finish_media_eos(id)?;
            Ok(crate::com::complete_call(pending.origin.frame, 0))
        }
        GraphStep::Teardown { mut remaining } => {
            if pending.native.slot != 2 && (result as i32) < 0 {
                return Err(RuntimeStateError::new(format!(
                    "DirectShow filter teardown failed with HRESULT {result:#010x}"
                )));
            }
            if let Some(next) = remaining.pop_front() {
                return Ok(callback_outcome(
                    runtime,
                    pending.origin,
                    GraphStep::Teardown { remaining },
                    next,
                ));
            }
            runtime.finish_graph_teardown(*pending.origin)
        }
    }
}

pub(crate) fn dispatch_directshow_com(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let ComFrontendCall {
        runtime,
        trap,
        target,
        frame,
    } = call;
    let origin = ComOrigin {
        trap,
        target,
        frame,
    };
    match origin.target.class {
        RuntimeComClass::DirectShowSample => return dispatch_sample(runtime, origin),
        RuntimeComClass::DirectShowAllocator => return dispatch_allocator(runtime, origin),
        RuntimeComClass::DirectShowOutputPin => return dispatch_output_pin(runtime, origin),
        RuntimeComClass::DirectShowStream => return dispatch_stream(runtime, origin),
        _ => {}
    }
    if let Err(error) = runtime.direct_show.graph(origin.target.object_id) {
        return runtime_failure(origin, error);
    }
    match (origin.frame.iid, origin.frame.vtable_slot) {
        (IID_GRAPH_BUILDER | IID_FILTER_GRAPH, 3) => {
            let pointer = GuestAddress(origin.frame.explicit_arguments[0]);
            if pointer.0 == 0 {
                return crate::com::complete_call(origin.frame, 0x8000_4003);
            }
            let graph = runtime
                .direct_show
                .graph(origin.target.object_id)
                .expect("validated graph");
            if graph.filters.iter().any(|filter| filter.pointer == pointer) {
                return crate::com::complete_call(origin.frame, 0x8004_025f);
            }
            if graph.filters.len() >= 32 {
                return crate::com::complete_call(origin.frame, 0x8007_000e);
            }
            let name = if origin.frame.explicit_arguments[1] == 0 {
                format!("Filter {}", graph.filters.len() + 1)
            } else {
                match read_wide(
                    runtime,
                    GuestAddress(origin.frame.explicit_arguments[1]),
                    128,
                ) {
                    Ok(name) => name,
                    Err(error) => return runtime_failure(origin, error),
                }
            };
            callback_outcome(
                runtime,
                Box::new(origin),
                GraphStep::AddFilterRef { pointer, name },
                plain_call(pointer, 1, &[]),
            )
        }
        (IID_GRAPH_BUILDER, 12) => {
            let pin = GuestAddress(origin.frame.explicit_arguments[0]);
            if pin.0 == 0 {
                return crate::com::complete_call(origin.frame, 0x8000_4003);
            }
            if runtime
                .direct_show
                .graph(origin.target.object_id)
                .expect("validated graph")
                .format
                .is_some()
            {
                return crate::com::complete_call(origin.frame, VFW_E_WRONG_STATE);
            }
            callback_outcome(
                runtime,
                Box::new(origin),
                GraphStep::QueryReader { pin },
                query_call(pin, IID_ASYNC_READER),
            )
        }
        (IID_GRAPH_BUILDER, 13) => dispatch_render_file(runtime, origin),
        (IID_MEDIA_CONTROL, 7..=9) => {
            let graph = runtime.direct_show.graph(origin.target.object_id).unwrap();
            let state = match origin.frame.vtable_slot {
                7 => GraphState::Running,
                8 => GraphState::Paused,
                _ => GraphState::Stopped,
            };
            if graph.state == state {
                return crate::com::complete_call(origin.frame, 0);
            }
            if graph.format.is_none() || graph.mem_input.is_none() {
                return crate::com::complete_call(origin.frame, VFW_E_NOT_CONNECTED);
            }
            if state == GraphState::Stopped {
                return match runtime.begin_media_call(origin, ComMediaOperation::Stop, None) {
                    Ok(result) => result,
                    Err(error) => ComDispatchOutcome::Fault(crate::com::ComFault {
                        message: error.message,
                        call_site: GuestAddress(0),
                        target: None,
                    }),
                };
            }
            match continue_filter_transition(
                runtime,
                Box::new(origin),
                if state == GraphState::Running {
                    GraphState::Paused
                } else {
                    state
                },
                0,
                state == GraphState::Running,
            ) {
                Ok(outcome) => outcome,
                Err(error) => ComDispatchOutcome::Fault(crate::com::ComFault {
                    message: error.message,
                    call_site: GuestAddress(0),
                    target: None,
                }),
            }
        }
        (IID_MEDIA_CONTROL, 10) => {
            let state = runtime
                .direct_show
                .graph(origin.target.object_id)
                .unwrap()
                .state as u32;
            let pointer = GuestAddress(origin.frame.explicit_arguments[1]);
            finish_writes(
                runtime,
                origin,
                vec![(pointer, state.to_le_bytes().to_vec())],
                0,
            )
        }
        (IID_MEDIA_EVENT_EX, 8) => dispatch_get_event(runtime, origin),
        (IID_MEDIA_EVENT_EX, 12) => {
            if origin.frame.explicit_arguments != [1, 0, 0] {
                return crate::com::complete_call(origin.frame, E_INVALIDARG);
            }
            crate::com::complete_call(origin.frame, 0)
        }
        (IID_MEDIA_EVENT_SINK, 3) => {
            if origin.frame.explicit_arguments[0] == EC_WINDOW_DESTROYED {
                let renderer = GuestAddress(origin.frame.explicit_arguments[1]);
                if origin.frame.explicit_arguments[2] != 0 {
                    return crate::com::complete_call(origin.frame, E_INVALIDARG);
                }
                let graph = runtime.direct_show.graph(origin.target.object_id).unwrap();
                if !graph
                    .filters
                    .iter()
                    .any(|filter| filter.pointer == renderer)
                {
                    return runtime_failure(
                        origin,
                        RuntimeStateError::new(
                            "window-destroyed event does not identify a registered graph renderer",
                        ),
                    );
                }
                if let Err(error) = validate_native_method(runtime, renderer, 13) {
                    return runtime_failure(origin, error);
                }
                // Default graph handling consumes this event. No resource-manager
                // focus grant is owned here, and no application event is posted.
                return crate::com::complete_call(origin.frame, 0);
            }
            if origin.frame.explicit_arguments[0] != 1 {
                return runtime_failure(
                    origin,
                    RuntimeStateError::new("unsupported DirectShow graph event"),
                );
            }
            // The guest renderer announces its own drained video endpoint.
            crate::com::complete_call(origin.frame, 0)
        }
        (IID_MEDIA_SEEKING, _) => dispatch_seeking(runtime, origin),
        (IID_BASIC_AUDIO, _) => dispatch_basic_audio(runtime, origin),
        (IID_MEDIA_EVENT_EX, 13) => {
            let arguments = &origin.frame.explicit_arguments;
            let notify = (arguments[0] != 0).then_some((arguments[0], arguments[1], arguments[2]));
            if let Some((hwnd, _, _)) = notify
                && let Err(error) = runtime.resolve_window_procedure(hwnd)
            {
                return runtime_failure(origin, error);
            }
            runtime
                .direct_show
                .graphs
                .get_mut(&origin.target.object_id)
                .expect("validated graph")
                .notify = notify;
            crate::com::complete_call(origin.frame, 0)
        }
        _ => runtime_failure(
            origin,
            RuntimeStateError::new("reached DirectShow graph operation is not connected yet"),
        ),
    }
}

impl Runtime {
    fn begin_media_call(
        &mut self,
        origin: ComOrigin,
        operation: ComMediaOperation,
        source: Option<ComMediaSource>,
    ) -> Result<ComDispatchOutcome, RuntimeStateError> {
        origin.validate(self)?;
        let graph_id = origin_graph_id(self, &origin)?;
        let graph = self.direct_show.graph(graph_id)?;
        let resource_id = graph.format.map(|format| format.resource_id);
        let volume = if operation == ComMediaOperation::Volume {
            origin.frame.explicit_arguments[0] as i32
        } else {
            graph.volume
        };
        let source = source.or_else(|| {
            graph
                .reader
                .map(|(pointer, byte_length)| ComMediaSource::GuestAsyncReader {
                    pointer,
                    byte_length,
                })
        });
        let delay_milliseconds = if operation == ComMediaOperation::NextFrame {
            graph.playback_origin.map_or(0, |(position, time)| {
                let elapsed =
                    u64::try_from(graph.position_100ns.saturating_sub(position)).unwrap_or(0);
                let due = time.saturating_add(elapsed);
                u32::try_from(due.saturating_sub(self.timer_time_100ns()).div_ceil(10_000))
                    .unwrap_or(u32::MAX)
            })
        } else {
            0
        };
        let thread_id = self.current_thread_id();
        let host = self.begin_host_request("browser.media")?;
        let request_id = host.request_id;
        self.direct_show.active.insert(
            request_id,
            ActiveMediaCall {
                origin,
                operation,
                source: source.clone(),
                thread_id,
                host_wait: Some(request_id),
                approved_audio: None,
            },
        );
        Ok(ComDispatchOutcome::MediaPending(ComMediaDeferred {
            request_id,
            graph_id,
            thread_id,
            operation,
            resource_id: if operation == ComMediaOperation::Open {
                Some(request_id)
            } else {
                resource_id
            },
            source,
            volume,
            delay_milliseconds,
            audio: None,
        }))
    }

    /// Start the source's real synchronous read on the pending decoder's guest
    /// thread. The returned callback must run before the browser sees bytes.
    pub fn read_com_media(
        &mut self,
        request_id: u64,
        offset: u64,
        length: u32,
    ) -> Result<ComDispatchOutcome, RuntimeStateError> {
        let active = self
            .direct_show
            .active
            .get(&request_id)
            .ok_or_else(|| RuntimeStateError::new("media read has no live decoder request"))?;
        active.origin.validate(self)?;
        let wait = active.host_wait.ok_or_else(|| {
            RuntimeStateError::new("decoder read is already executing a guest callback")
        })?;
        if self.scheduler_state_for_thread(active.thread_id)?
            != (crate::runtime::SchedulerThreadState::WaitingHost { request_id: wait })
        {
            return Err(RuntimeStateError::new(
                "decoder read lost its actual scheduler wait identity",
            ));
        }
        if active.thread_id != self.current_thread_id()
            || length == 0
            || length > 2 * 1024 * 1024
            || length as usize > self.memory.limits().max_access_bytes
        {
            return Err(RuntimeStateError::new(
                "media read has invalid caller or transport bounds",
            ));
        }
        let Some(ComMediaSource::GuestAsyncReader {
            pointer,
            byte_length,
        }) = active.source
        else {
            return Err(RuntimeStateError::new(
                "media source is not a guest IAsyncReader",
            ));
        };
        if offset
            .checked_add(u64::from(length))
            .is_none_or(|end| end > byte_length)
            || offset > i64::MAX as u64
        {
            return Err(RuntimeStateError::new(
                "media read extends beyond the authoritative source length",
            ));
        }
        let origin = Box::new(active.origin.clone());
        // Validate the actual deferred callback before publishing source bytes.
        let target = validate_native_method(self, pointer, 7)?;
        let native_vtable = read_address(self, pointer)?;
        let method_owner =
            region_for(
                self,
                GuestAddress(native_vtable.0.checked_add(28).ok_or_else(|| {
                    RuntimeStateError::new("media reader method address overflow")
                })?),
            )?
            .owner;
        if self
            .direct_show
            .graph(origin_graph_id(self, &origin)?)?
            .encoded_buffer
            .is_none()
            && self.memory.limits().max_access_bytes < 2 * 1024 * 1024
        {
            return Err(RuntimeStateError::new(
                "media encoded buffer exceeds the owned transport access limit",
            ));
        }
        self.memory
            .validate_access(
                crate::callbacks::CALLBACK_RETURN_ARENA_BASE,
                1,
                MemoryAccess::Execute,
            )
            .map_err(RuntimeStateError::from_memory)?;
        let stack_pointer = GuestAddress(
            origin
                .trap
                .stack_pointer
                .0
                .checked_sub(24)
                .ok_or_else(|| RuntimeStateError::new("media read callback stack underflow"))?,
        );
        self.prepare_media_callback_frame(
            wait,
            target,
            0,
            GuestRange::new(stack_pointer, 24).map_err(RuntimeStateError::from_memory)?,
        )?;
        let graph_id = origin_graph_id(self, &origin)?;
        let backing = match self.direct_show.graph(graph_id)?.encoded_buffer {
            Some(backing) => backing,
            None => {
                let backing = self.allocate_media_buffer(2 * 1024 * 1024)?;
                self.direct_show
                    .graphs
                    .get_mut(&graph_id)
                    .unwrap()
                    .encoded_buffer = Some(backing);
                backing
            }
        };
        self.validate_media_backing(backing)?;
        if region_for(self, pointer)?.owner == backing.owner || method_owner == backing.owner {
            return Err(RuntimeStateError::new(
                "media reader interface aliases its encoded destination buffer",
            ));
        }
        // Clear the actual read destination before entry, so it cannot expose
        // bytes left by an earlier request as a successful new chunk.
        self.memory
            .write(backing.range.start(), &vec![0; length as usize])
            .map_err(RuntimeStateError::from_memory)?;
        self.complete_media_host_with(wait, |_| Ok(()))?;
        self.direct_show
            .active
            .get_mut(&request_id)
            .unwrap()
            .host_wait = None;
        let native = plain_call(
            pointer,
            7,
            &[
                offset as u32,
                (offset >> 32) as u32,
                length,
                backing.range.start().0,
            ],
        );
        Ok(callback_outcome(
            self,
            origin,
            GraphStep::EncodedRead {
                request_id,
                offset,
                length,
                backing,
            },
            native,
        ))
    }

    pub fn complete_com_media(
        &mut self,
        pending: &ComMediaDeferred,
        mut response: ComMediaResponse,
    ) -> Result<ComDispatchOutcome, RuntimeStateError> {
        let active = self
            .direct_show
            .active
            .get(&pending.request_id)
            .ok_or_else(|| {
                RuntimeStateError::new("decoder completion has no live native request")
            })?;
        active.origin.validate(self)?;
        if active.thread_id != pending.thread_id
            || self.current_thread_id() != pending.thread_id
            || active.operation != pending.operation
            || origin_graph_id(self, &active.origin)? != pending.graph_id
        {
            return Err(RuntimeStateError::new(
                "decoder completion lost its operation or guest thread identity",
            ));
        }
        let graph = self.direct_show.graph(pending.graph_id)?;
        let expected_resource = if pending.operation == ComMediaOperation::Open {
            Some(pending.request_id)
        } else {
            graph.format.map(|format| format.resource_id)
        };
        if pending.resource_id != expected_resource {
            return Err(RuntimeStateError::new(
                "decoder request no longer owns its codec resource",
            ));
        }
        let wait = active.host_wait.ok_or_else(|| {
            RuntimeStateError::new(
                "decoder completed while its guest read callback was still active",
            )
        })?;
        match &response {
            ComMediaResponse::Opened(format) if pending.operation == ComMediaOperation::Open => {
                validate_format(*format, pending.request_id)?;
                let filter = graph
                    .filters
                    .first()
                    .ok_or_else(|| {
                        RuntimeStateError::new("DirectShow graph has no guest video renderer")
                    })?
                    .pointer;
                let target = validate_native_method(self, filter, 10)?;
                let stack = active
                    .origin
                    .trap
                    .stack_pointer
                    .0
                    .checked_sub(12)
                    .ok_or_else(|| RuntimeStateError::new("media callback stack underflow"))?;
                self.prepare_media_callback_frame(
                    wait,
                    target,
                    4,
                    GuestRange::new(GuestAddress(stack), 12)
                        .map_err(RuntimeStateError::from_memory)?,
                )?;
            }
            ComMediaResponse::Frame(frame) if pending.operation == ComMediaOperation::NextFrame => {
                let format = graph.format.ok_or_else(|| {
                    RuntimeStateError::new("video sample arrived before a connected format")
                })?;
                if frame.resource_id != format.resource_id
                    || frame.start_100ns < 0
                    || frame.end_100ns <= frame.start_100ns
                    || frame.start_100ns < graph.position_100ns
                    || frame.bgra.len() as u64
                        != u64::from(format.width) * u64::from(format.height) * 4
                {
                    return Err(RuntimeStateError::new(
                        "decoded video sample has invalid resource, timestamps, or pixel bounds",
                    ));
                }
                let allocator = graph
                    .allocator
                    .and_then(|id| self.direct_show.allocators.get(&id))
                    .ok_or_else(|| {
                        RuntimeStateError::new("video sample has no live negotiated allocator")
                    })?;
                if !allocator.committed {
                    return Err(RuntimeStateError::new(
                        "video sample allocator was decommitted",
                    ));
                }
                let backing = allocator.free.last().copied().ok_or_else(|| {
                    RuntimeStateError::new("video allocator has no available sample backing")
                })?;
                self.validate_media_backing(backing)?;
                let receiver = graph.mem_input.ok_or_else(|| {
                    RuntimeStateError::new("video sample has no connected receiver")
                })?;
                let target = validate_native_method(self, receiver, 6)?;
                let stack = active
                    .origin
                    .trap
                    .stack_pointer
                    .0
                    .checked_sub(12)
                    .ok_or_else(|| RuntimeStateError::new("media callback stack underflow"))?;
                self.prepare_media_callback_frame(
                    wait,
                    target,
                    0,
                    GuestRange::new(GuestAddress(stack), 12)
                        .map_err(RuntimeStateError::from_memory)?,
                )?;
            }
            ComMediaResponse::Audio(audio) if pending.operation == ComMediaOperation::NextFrame => {
                let format = graph.format.ok_or_else(|| {
                    RuntimeStateError::new("audio sample arrived before a connected format")
                })?;
                if audio.resource_id != format.resource_id
                    || audio.start_100ns < 0
                    || audio.end_100ns <= audio.start_100ns
                    || audio.start_100ns < graph.last_audio_end_100ns
                    || audio.channels != format.audio_channels
                    || audio.samples_per_second != format.audio_samples_per_second
                    || audio.channels == 0
                    || audio.pcm_f32le.is_empty()
                    || audio.pcm_f32le.len() > 2 * 1024 * 1024
                    || !audio
                        .pcm_f32le
                        .len()
                        .is_multiple_of(audio.channels as usize * 4)
                    || audio
                        .pcm_f32le
                        .chunks_exact(4)
                        .any(|bytes| !f32::from_le_bytes(bytes.try_into().unwrap()).is_finite())
                {
                    return Err(RuntimeStateError::new(
                        "decoded audio sample has invalid resource, format, timestamps, or PCM bounds",
                    ));
                }
                let frames = audio.pcm_f32le.len() / (audio.channels as usize * 4);
                let duration =
                    (frames as u64 * 10_000_000).div_ceil(u64::from(audio.samples_per_second));
                if u64::try_from(audio.end_100ns - audio.start_100ns)
                    .unwrap()
                    .abs_diff(duration)
                    > 2
                {
                    return Err(RuntimeStateError::new(
                        "PCM sample time range disagrees with its actual sample count",
                    ));
                }
            }
            ComMediaResponse::Acknowledged
                if matches!(
                    pending.operation,
                    ComMediaOperation::Run
                        | ComMediaOperation::Pause
                        | ComMediaOperation::Stop
                        | ComMediaOperation::Volume
                        | ComMediaOperation::Release
                        | ComMediaOperation::AudioPush
                ) => {}
            ComMediaResponse::Ended { resource_id }
                if pending.operation == ComMediaOperation::NextFrame
                    && Some(*resource_id) == expected_resource =>
            {
                let input = graph.input_pin.ok_or_else(|| {
                    RuntimeStateError::new("decoded end has no connected video input")
                })?;
                let target = validate_native_method(self, input, 14)?;
                let stack = active
                    .origin
                    .trap
                    .stack_pointer
                    .0
                    .checked_sub(8)
                    .ok_or_else(|| RuntimeStateError::new("media callback stack underflow"))?;
                self.prepare_media_callback_frame(
                    wait,
                    target,
                    0,
                    GuestRange::new(GuestAddress(stack), 8)
                        .map_err(RuntimeStateError::from_memory)?,
                )?;
            }
            ComMediaResponse::Stopped { resource_id }
                if matches!(
                    pending.operation,
                    ComMediaOperation::NextFrame | ComMediaOperation::AudioPush
                ) && Some(*resource_id) == expected_resource
                    && self.direct_show.active.values().any(|control| {
                        matches!(
                            control.operation,
                            ComMediaOperation::Stop | ComMediaOperation::Release
                        ) && origin_graph_id(self, &control.origin).ok() == Some(pending.graph_id)
                    }) => {}
            ComMediaResponse::FileNotFound if pending.operation == ComMediaOperation::Open => {}
            ComMediaResponse::BackendFailed { message }
            | ComMediaResponse::Unsupported { message }
                if !message.is_empty() && message.len() <= 4096 => {}
            ComMediaResponse::Failed { hresult } if (*hresult as i32) < 0 => {}
            _ => {
                return Err(RuntimeStateError::new(
                    "decoder response kind does not match its held operation",
                ));
            }
        }
        let allocator_id = graph.allocator;
        let prepared_sample = self.complete_media_host_with(wait, |runtime| {
            if let ComMediaResponse::Frame(frame) = &mut response {
                let pixel_bytes = std::mem::take(&mut frame.bgra);
                let owned_frame = ComMediaFrame {
                    resource_id: frame.resource_id,
                    start_100ns: frame.start_100ns,
                    end_100ns: frame.end_100ns,
                    bgra: pixel_bytes,
                };
                runtime
                    .create_media_sample(
                        allocator_id.expect("validated sample allocator"),
                        owned_frame,
                    )
                    .map(Some)
            } else {
                Ok(None)
            }
        })?;
        let active = self
            .direct_show
            .active
            .remove(&pending.request_id)
            .expect("validated active decoder call");
        let id = pending.graph_id;
        match response {
            ComMediaResponse::Opened(format) => {
                let graph = self.direct_show.graphs.get_mut(&id).unwrap();
                graph.format = Some(format);
                graph.duration_100ns = format.duration_100ns;
                let filter = graph
                    .filters
                    .first()
                    .expect("validated guest renderer")
                    .pointer;
                let native = NativeCall {
                    pointer: filter,
                    slot: 10,
                    arguments: vec![NativeArgument::Scratch(0)],
                    scratch: vec![0; 4],
                    relocations: Vec::new(),
                };
                Ok(callback_outcome(
                    self,
                    Box::new(active.origin),
                    GraphStep::EnumeratePins {
                        filter_index: 0,
                        format,
                    },
                    native,
                ))
            }
            ComMediaResponse::Frame(frame) => {
                let graph = self.direct_show.graph(id)?;
                if graph.state == GraphState::Stopped {
                    return Ok(crate::com::complete_call(active.origin.frame, 0));
                }
                let receiver = graph.mem_input.expect("validated receiver");
                let end_100ns = frame.end_100ns;
                let sample = prepared_sample.expect("transactionally installed media sample");
                let pointer = sample
                    .interfaces
                    .iter()
                    .find(|(iid, _)| *iid == IID_MEDIA_SAMPLE)
                    .unwrap()
                    .1;
                Ok(callback_outcome(
                    self,
                    Box::new(active.origin),
                    GraphStep::ReceiveSample {
                        sample: sample.object_id,
                        end_100ns,
                    },
                    plain_call(receiver, 6, &[pointer.0]),
                ))
            }
            ComMediaResponse::Audio(audio) => {
                let mut result =
                    self.begin_media_call(active.origin, ComMediaOperation::AudioPush, None)?;
                let ComDispatchOutcome::MediaPending(next) = &mut result else {
                    unreachable!()
                };
                self.direct_show
                    .active
                    .get_mut(&next.request_id)
                    .unwrap()
                    .approved_audio = Some(audio.clone());
                next.audio = Some(audio);
                Ok(result)
            }
            ComMediaResponse::Acknowledged => match pending.operation {
                ComMediaOperation::Run => {
                    let now = self.timer_time_100ns();
                    let graph = self.direct_show.graphs.get_mut(&id).unwrap();
                    graph.state = GraphState::Running;
                    graph.playback_origin = Some((graph.position_100ns, now));
                    let created = self.start_media_stream(id)?;
                    let mut outcome = crate::com::complete_call(active.origin.frame, 0);
                    if let ComDispatchOutcome::Completed(completion) = &mut outcome {
                        completion.created_thread = created;
                    }
                    Ok(outcome)
                }
                ComMediaOperation::Pause => {
                    let graph = self.direct_show.graphs.get_mut(&id).unwrap();
                    graph.state = GraphState::Paused;
                    graph.playback_origin = None;
                    Ok(crate::com::complete_call(active.origin.frame, 0))
                }
                ComMediaOperation::Stop => {
                    self.direct_show
                        .graphs
                        .get_mut(&id)
                        .unwrap()
                        .playback_origin = None;
                    continue_filter_transition(
                        self,
                        Box::new(active.origin),
                        GraphState::Stopped,
                        0,
                        false,
                    )
                }
                ComMediaOperation::Volume => {
                    self.direct_show.graphs.get_mut(&id).unwrap().volume =
                        active.origin.frame.explicit_arguments[0] as i32;
                    Ok(crate::com::complete_call(active.origin.frame, 0))
                }
                ComMediaOperation::AudioPush => {
                    let audio = active.approved_audio.ok_or_else(|| {
                        RuntimeStateError::new("audio ACK has no Rust-approved PCM snapshot")
                    })?;
                    self.direct_show
                        .graphs
                        .get_mut(&id)
                        .unwrap()
                        .last_audio_end_100ns = audio.end_100ns;
                    Ok(repeat_stream(active.origin))
                }
                ComMediaOperation::Release => self.begin_graph_teardown(active.origin),
                _ => unreachable!(),
            },
            ComMediaResponse::Ended { .. } => {
                let graph = self.direct_show.graph(id)?;
                let input = graph.input_pin.ok_or_else(|| {
                    RuntimeStateError::new("decoded end has no connected video input")
                })?;
                Ok(callback_outcome(
                    self,
                    Box::new(active.origin),
                    GraphStep::EndOfStream,
                    plain_call(input, 14, &[]),
                ))
            }
            ComMediaResponse::Stopped { .. } => {
                self.direct_show.graphs.get_mut(&id).unwrap().stream_thread = None;
                Ok(crate::com::complete_call(active.origin.frame, 0))
            }
            ComMediaResponse::FileNotFound => Ok(crate::com::complete_call(
                active.origin.frame,
                VFW_E_NOT_FOUND,
            )),
            ComMediaResponse::BackendFailed { .. } => {
                Ok(crate::com::complete_call(active.origin.frame, 0x8000_4005))
            }
            ComMediaResponse::Failed { hresult } => {
                Ok(crate::com::complete_call(active.origin.frame, hresult))
            }
            ComMediaResponse::Unsupported { message } => Ok(runtime_failure(
                active.origin,
                RuntimeStateError::new(message),
            )),
        }
    }

    fn start_media_stream(
        &mut self,
        graph_id: RuntimeComObjectId,
    ) -> Result<Option<RuntimeThreadId>, RuntimeStateError> {
        if self.direct_show.graph(graph_id)?.stream_thread.is_some() {
            return Ok(None);
        }
        let stream_id = match self.direct_show.graph(graph_id)?.stream {
            Some(id) => {
                if self.direct_show.streams.get(&id) != Some(&graph_id) {
                    return Err(RuntimeStateError::new(
                        "media stream child lost its graph owner",
                    ));
                }
                id
            }
            None => {
                let parent = self
                    .com()
                    .interface_for_object_iid(graph_id, IID_GRAPH_BUILDER)?
                    .id;
                let object =
                    self.install_com_object_with_guest_writes(Some(parent), stream_spec(), |_| {
                        Ok(Vec::new())
                    })?;
                self.direct_show.streams.insert(object.object_id, graph_id);
                self.direct_show.graphs.get_mut(&graph_id).unwrap().stream = Some(object.object_id);
                object.object_id
            }
        };
        let interface = self
            .com()
            .interface_for_object_iid(stream_id, IID_STREAM_ENTRY)?;
        let entry = interface.methods.get(&3).unwrap().endpoint;
        let pointer = interface.guest_pointer;
        let Some((thread, handle)) = self.create_media_guest_thread(
            crate::runtime::RuntimeCreateThreadRequest {
                stack_reserve_bytes: 1024 * 1024,
                stack_commit_bytes: 64 * 1024,
                entry_point: entry,
                parameter: pointer.0,
                thread_id_output: None,
                initial_suspend_count: 0,
            },
            stream_id,
        )?
        else {
            return Err(RuntimeStateError::new(
                "media streaming guest thread capacity exhausted",
            ));
        };
        self.close_handle(handle)?;
        self.direct_show
            .graphs
            .get_mut(&graph_id)
            .unwrap()
            .stream_thread = Some(thread);
        Ok(Some(thread))
    }

    fn finish_media_eos(&mut self, graph_id: RuntimeComObjectId) -> Result<(), RuntimeStateError> {
        let graph = self.direct_show.graph(graph_id)?;
        let notify = graph.notify;
        let end = graph.position_100ns.max(graph.last_audio_end_100ns);
        if let Some((hwnd, message, instance)) = notify {
            let target = self.window(hwnd)?.creation.owner_thread_id;
            self.post_thread_message(target, hwnd, message, 0, instance)?;
        }
        let graph = self.direct_show.graphs.get_mut(&graph_id).unwrap();
        graph.completed = true;
        graph.stream_thread = None;
        graph.event_pending = true;
        graph.duration_100ns = Some(end);
        graph.position_100ns = end;
        Ok(())
    }

    fn begin_graph_teardown(
        &mut self,
        origin: ComOrigin,
    ) -> Result<ComDispatchOutcome, RuntimeStateError> {
        let id = origin.target.object_id;
        if self
            .direct_show
            .active
            .values()
            .any(|active| origin_graph_id(self, &active.origin).ok() == Some(id))
        {
            return Err(RuntimeStateError::new(
                "graph teardown still has an executing codec or source callback",
            ));
        }
        let graph = self.direct_show.graph(id)?;
        let mut remaining = std::collections::VecDeque::new();
        for filter in &graph.filters {
            remaining.push_back(plain_call(filter.pointer, 4, &[]));
        }
        if let Some(pin) = graph.input_pin {
            remaining.push_back(plain_call(pin, 5, &[]));
        }
        if let Some(input) = graph.mem_input {
            remaining.push_back(plain_call(input, 2, &[]));
        }
        if let Some(pin) = graph.input_pin {
            remaining.push_back(plain_call(pin, 2, &[]));
        }
        if let Some((reader, _)) = graph.reader {
            remaining.push_back(plain_call(reader, 2, &[]));
        }
        for filter in &graph.filters {
            remaining.push_back(plain_call(filter.pointer, 13, &[0, 0]));
        }
        for filter in &graph.filters {
            remaining.push_back(plain_call(filter.pointer, 2, &[]));
        }
        if let Some(next) = remaining.pop_front() {
            return Ok(callback_outcome(
                self,
                Box::new(origin),
                GraphStep::Teardown { remaining },
                next,
            ));
        }
        self.finish_graph_teardown(origin)
    }

    fn finish_graph_teardown(
        &mut self,
        origin: ComOrigin,
    ) -> Result<ComDispatchOutcome, RuntimeStateError> {
        let id = origin.target.object_id;
        let graph = self.direct_show.graphs.get_mut(&id).unwrap();
        graph.filters.clear();
        graph.reader = None;
        graph.source_pin = None;
        graph.input_pin = None;
        graph.mem_input = None;
        graph.format = None;
        graph.state = GraphState::Stopped;
        graph.playback_origin = None;
        let children = [
            graph.stream.take(),
            graph.output_pin.take(),
            graph.allocator.take(),
        ];
        let file = graph.file_handle.take();
        for child in children.into_iter().flatten() {
            let interface = self.com().object(child)?.canonical_iunknown_interface_id;
            self.release_com_interface(interface)?;
        }
        if let Some(file) = file {
            self.close_handle(file)?;
        }
        let count = self.release_com_interface(origin.target.interface_id)?;
        Ok(crate::com::complete_call(origin.frame, count))
    }

    fn allocate_media_buffer(&mut self, bytes: u32) -> Result<MediaBacking, RuntimeStateError> {
        Ok(self.allocate_media_buffers(bytes, 1)?.remove(0))
    }

    fn allocate_media_buffers(
        &mut self,
        bytes: u32,
        count: usize,
    ) -> Result<Vec<MediaBacking>, RuntimeStateError> {
        if count == 0 {
            return Ok(Vec::new());
        }
        let reserve_bytes = u64::from(bytes)
            .checked_add(4095)
            .map(|value| value & !4095)
            .filter(|value| *value != 0)
            .ok_or_else(|| RuntimeStateError::new("media buffer size overflow"))?;
        let request = crate::runtime::ProcessAddressAllocationRequest {
            purpose: crate::runtime::ProcessAddressAllocationPurpose::MediaResource,
            preferred_address: None,
            search_range: crate::runtime::RuntimeAddressRange::new(
                GuestAddress(0x1000_0000),
                0x5000_0000,
            )?,
            reserve_size: reserve_bytes,
            alignment: 4096,
            guard_pages: 0,
            classification: crate::runtime::ProcessAddressAllocationClassification::Com,
            release_policy: crate::runtime::ProcessAddressAllocationReleasePolicy::Explicit,
            executable: false,
        };
        let memory = &mut self.memory;
        self.address_space_allocator.reserve_batch_with(
            (0..count).map(|_| (None, request.clone())).collect(),
            |allocations| {
                let mut transaction = memory.begin_transaction();
                let mut backings = Vec::with_capacity(allocations.len());
                for allocation in allocations {
                    let owner = transaction
                        .create_owner(MemoryOwnerKind::Runtime)
                        .map_err(RuntimeStateError::from_memory)?;
                    let range = transaction
                        .reserve_fixed_for_address_allocation(
                            allocation.id.0,
                            owner,
                            MemoryRegionKind::Runtime,
                            allocation.range.base,
                            allocation.range.byte_length,
                        )
                        .map_err(RuntimeStateError::from_memory)?;
                    transaction
                        .commit(
                            owner,
                            range,
                            PageProtection {
                                read: true,
                                write: true,
                                execute: false,
                            },
                        )
                        .map_err(RuntimeStateError::from_memory)?;
                    transaction
                        .seal_owner(owner)
                        .map_err(RuntimeStateError::from_memory)?;
                    backings.push(MediaBacking {
                        allocation_id: allocation.id,
                        owner,
                        range,
                    });
                }
                transaction.commit_transaction();
                Ok(backings)
            },
        )
    }

    pub(crate) fn validate_media_backing(
        &self,
        backing: MediaBacking,
    ) -> Result<(), RuntimeStateError> {
        let allocation = self
            .address_space_allocator
            .allocation(backing.allocation_id)?;
        if allocation.purpose != crate::runtime::ProcessAddressAllocationPurpose::MediaResource
            || allocation.range.base != backing.range.start()
            || allocation.range.byte_length != backing.range.byte_length()
            || allocation.executable
            || !self.memory.regions().any(|region| {
                region.owner == backing.owner
                    && region.address_allocation_id == Some(backing.allocation_id.0)
                    && region.range == backing.range
                    && region.kind == MemoryRegionKind::Runtime
            })
        {
            return Err(RuntimeStateError::new(
                "media byte backing lost its allocator, mapping, or owner identity",
            ));
        }
        self.memory
            .validate_host_write(backing.range.start(), backing.range.byte_length() as usize)
            .map_err(RuntimeStateError::from_memory)
    }
}

fn validate_format(format: ComMediaFormat, resource_id: u64) -> Result<(), RuntimeStateError> {
    if format.resource_id != resource_id
        || format.width == 0
        || format.height == 0
        || format.width > 8192
        || format.height > 8192
        || u64::from(format.width) * u64::from(format.height) * 4 > 64 * 1024 * 1024
        || format.frame_duration_100ns <= 0
        || format.duration_100ns.is_some_and(|duration| duration <= 0)
        || (format.audio_channels == 0 && format.audio_samples_per_second != 0)
        || (format.audio_channels != 0
            && (!(1..=8).contains(&format.audio_channels)
                || !(8000..=192000).contains(&format.audio_samples_per_second)))
    {
        return Err(RuntimeStateError::new(
            "decoder metadata does not describe a bounded RGB32/PCM resource",
        ));
    }
    Ok(())
}

fn dispatch_render_file(runtime: &mut Runtime, origin: ComOrigin) -> ComDispatchOutcome {
    if origin.frame.explicit_arguments[0] == 0 {
        return crate::com::complete_call(origin.frame, 0x8000_4003);
    }
    if origin.frame.explicit_arguments[1] != 0 {
        return crate::com::complete_call(origin.frame, E_INVALIDARG);
    }
    let path = match read_wide(
        runtime,
        GuestAddress(origin.frame.explicit_arguments[0]),
        1024,
    ) {
        Ok(path) => path,
        Err(error) => return runtime_failure(origin, error),
    };
    if path.is_empty() {
        return crate::com::complete_call(origin.frame, VFW_E_NOT_FOUND);
    }
    let (path_bytes, _, unmappable) = encoding_rs::SHIFT_JIS.encode(&path);
    if unmappable {
        return runtime_failure(
            origin,
            RuntimeStateError::new("DirectShow path is outside the fixed Windows-932 namespace"),
        );
    }
    let mut path = path_bytes.into_owned();
    path.push(0);
    let source = match runtime.open_file_a(&path, 0x8000_0000, 1, 3, 0) {
        Ok(Ok(Some(handle))) => {
            let file = match runtime.source_file_for_handle(handle) {
                Ok(file) => file,
                Err(error) => return runtime_failure(origin, error),
            };
            let crate::runtime::RuntimeFileBacking::ImmutableSource {
                source_id,
                object_id,
            } = file.backing
            else {
                return runtime_failure(
                    origin,
                    RuntimeStateError::new("immutable media open returned another backing"),
                );
            };
            let length = match runtime.source_file_byte_length(handle) {
                Ok(length) => length,
                Err(error) => return runtime_failure(origin, error),
            };
            runtime
                .direct_show
                .graphs
                .get_mut(&origin.target.object_id)
                .unwrap()
                .file_handle = Some(handle);
            ComMediaSource::SourceFile {
                source_id,
                object_id,
                byte_length: length,
            }
        }
        Ok(Ok(None)) => match runtime.opfs_relative_path(&path) {
            Ok(relative_path) => ComMediaSource::WritableFile { relative_path },
            Err(_) => return crate::com::complete_call(origin.frame, VFW_E_NOT_FOUND),
        },
        Ok(Err(_)) => return crate::com::complete_call(origin.frame, 0x8007_0020),
        Err(error) => return runtime_failure(origin, error),
    };
    match runtime.begin_media_call(origin, ComMediaOperation::Open, Some(source)) {
        Ok(outcome) => outcome,
        Err(error) => ComDispatchOutcome::Fault(crate::com::ComFault {
            message: error.message,
            call_site: GuestAddress(0),
            target: None,
        }),
    }
}

fn origin_graph_id(
    runtime: &Runtime,
    origin: &ComOrigin,
) -> Result<RuntimeComObjectId, RuntimeStateError> {
    if origin.target.class == RuntimeComClass::DirectShowStream {
        runtime
            .direct_show
            .streams
            .get(&origin.target.object_id)
            .copied()
            .ok_or_else(|| RuntimeStateError::new("live stream has no graph identity"))
    } else {
        Ok(origin.target.object_id)
    }
}

fn validate_native_method(
    runtime: &Runtime,
    pointer: GuestAddress,
    slot: u16,
) -> Result<GuestAddress, RuntimeStateError> {
    let vtable = read_address(runtime, pointer)?;
    let address = GuestAddress(
        vtable
            .0
            .checked_add(u32::from(slot) * 4)
            .ok_or_else(|| RuntimeStateError::new("native method vtable overflow"))?,
    );
    let target = read_address(runtime, address)?;
    runtime
        .memory
        .validate_access(target, 1, MemoryAccess::Execute)
        .map_err(RuntimeStateError::from_memory)?;
    Ok(target)
}

fn video_media_type(format: ComMediaFormat) -> (Vec<u8>, Vec<(u32, u32)>) {
    let mut bytes = vec![0; 72 + 88];
    bytes[..16].copy_from_slice(&[
        0x76, 0x69, 0x64, 0x73, 0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71,
    ]);
    bytes[16..32].copy_from_slice(&[
        0x7e, 0xeb, 0x36, 0xe4, 0x4f, 0x52, 0xce, 0x11, 0x9f, 0x53, 0, 0x20, 0xaf, 0x0b, 0xa7, 0x70,
    ]);
    bytes[32..36].copy_from_slice(&1u32.to_le_bytes());
    let pixel_bytes = format.width * format.height * 4;
    bytes[40..44].copy_from_slice(&pixel_bytes.to_le_bytes());
    bytes[44..60].copy_from_slice(&[
        0x80, 0x9f, 0x58, 5, 0x56, 0xc3, 0xce, 0x11, 0xbf, 1, 0, 0xaa, 0, 0x55, 0x59, 0x5a,
    ]);
    bytes[64..68].copy_from_slice(&88u32.to_le_bytes());
    bytes[112..120].copy_from_slice(&format.frame_duration_100ns.to_le_bytes());
    bytes[120..124].copy_from_slice(&40u32.to_le_bytes());
    bytes[124..128].copy_from_slice(&(format.width as i32).to_le_bytes());
    bytes[128..132].copy_from_slice(&(-(format.height as i32)).to_le_bytes());
    bytes[132..134].copy_from_slice(&1u16.to_le_bytes());
    bytes[134..136].copy_from_slice(&32u16.to_le_bytes());
    bytes[140..144].copy_from_slice(&pixel_bytes.to_le_bytes());
    (bytes, vec![(68, 72)])
}

fn continue_filter_transition(
    runtime: &mut Runtime,
    origin: Box<ComOrigin>,
    state: GraphState,
    index: usize,
    preparing_run: bool,
) -> Result<ComDispatchOutcome, RuntimeStateError> {
    let id = origin_graph_id(runtime, &origin)?;
    let graph = runtime.direct_show.graph(id)?;
    if index < graph.filters.len() {
        let filter = graph.filters[index].pointer;
        let slot = match state {
            GraphState::Stopped => 4,
            GraphState::Paused => 5,
            GraphState::Running => 6,
        };
        let native = plain_call(
            filter,
            slot,
            if state == GraphState::Running {
                &[0, 0]
            } else {
                &[]
            },
        );
        return Ok(callback_outcome(
            runtime,
            origin,
            GraphStep::FilterTransition {
                state,
                index,
                preparing_run,
            },
            native,
        ));
    }
    if preparing_run && state == GraphState::Paused {
        return continue_filter_transition(runtime, origin, GraphState::Running, 0, false);
    }
    if state == GraphState::Stopped {
        runtime.direct_show.graphs.get_mut(&id).unwrap().state = state;
        return Ok(crate::com::complete_call(origin.frame, 0));
    }
    runtime.begin_media_call(
        *origin,
        if state == GraphState::Running {
            ComMediaOperation::Run
        } else {
            ComMediaOperation::Pause
        },
        None,
    )
}

fn repeat_stream(origin: ComOrigin) -> ComDispatchOutcome {
    ComDispatchOutcome::Completed(crate::com::ComCallCompletion {
        created_thread: None,
        presented_device: None,
        instruction_pointer: origin.trap.endpoint_address,
        stack_pointer: origin.trap.stack_pointer,
        eax: 0,
    })
}

fn dispatch_get_event(runtime: &mut Runtime, origin: ComOrigin) -> ComDispatchOutcome {
    let graph = runtime.direct_show.graph(origin.target.object_id).unwrap();
    if !graph.event_pending {
        return crate::com::complete_call(origin.frame, 0x8000_4004);
    }
    let id = origin.target.object_id;
    let arguments = &origin.frame.explicit_arguments;
    let writes = vec![
        (GuestAddress(arguments[0]), 1u32.to_le_bytes().to_vec()),
        (GuestAddress(arguments[1]), 0u32.to_le_bytes().to_vec()),
        (GuestAddress(arguments[2]), 0u32.to_le_bytes().to_vec()),
    ];
    let result = finish_writes(runtime, origin, writes, 0);
    if matches!(result, ComDispatchOutcome::Completed(ref completion) if completion.eax == 0) {
        runtime
            .direct_show
            .graphs
            .get_mut(&id)
            .unwrap()
            .event_pending = false;
    }
    result
}

pub(crate) fn dispatch_graph_release(call: ComFrontendCall<'_>) -> ComDispatchOutcome {
    let ComFrontendCall {
        runtime,
        trap,
        target,
        frame,
    } = call;
    let origin = ComOrigin {
        trap,
        target,
        frame,
    };
    let graph = match runtime.direct_show.graph(origin.target.object_id) {
        Ok(graph) => graph,
        Err(error) => return runtime_failure(origin, error),
    };
    let internal_references = u32::from(graph.allocator.is_some())
        + u32::from(graph.output_pin.is_some())
        + u32::from(graph.stream.is_some());
    let count = match runtime.com().object(origin.target.object_id) {
        Ok(object) => object.reference_count,
        Err(error) => return runtime_failure(origin, error),
    };
    if count > internal_references + 1 {
        return match runtime.release_com_interface(origin.target.interface_id) {
            Ok(count) => crate::com::complete_call(origin.frame, count),
            Err(error) => runtime_failure(origin, error),
        };
    }
    let result = if graph.format.is_some() {
        runtime.begin_media_call(origin, ComMediaOperation::Release, None)
    } else {
        runtime.begin_graph_teardown(origin)
    };
    match result {
        Ok(outcome) => outcome,
        Err(error) => ComDispatchOutcome::Fault(crate::com::ComFault {
            message: error.message,
            call_site: GuestAddress(0),
            target: None,
        }),
    }
}

fn sample_spec() -> ComObjectSpec {
    ComObjectSpec {
        class: RuntimeComClass::DirectShowSample,
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            interface(
                IID_MEDIA_SAMPLE,
                "IMediaSample",
                &[
                    ("GetPointer", Some(1)),
                    ("GetSize", Some(0)),
                    ("GetTime", Some(2)),
                    ("SetTime", Some(2)),
                    ("IsSyncPoint", Some(0)),
                    ("SetSyncPoint", Some(1)),
                    ("IsPreroll", Some(0)),
                    ("SetPreroll", Some(1)),
                    ("GetActualDataLength", Some(0)),
                    ("SetActualDataLength", Some(1)),
                    ("GetMediaType", Some(1)),
                    ("SetMediaType", None),
                    ("IsDiscontinuity", Some(0)),
                    ("SetDiscontinuity", Some(1)),
                    ("GetMediaTime", None),
                    ("SetMediaTime", None),
                ],
            ),
        ],
    }
}

fn allocator_spec() -> ComObjectSpec {
    ComObjectSpec {
        class: RuntimeComClass::DirectShowAllocator,
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            interface(
                IID_MEM_ALLOCATOR,
                "IMemAllocator",
                &[
                    ("SetProperties", Some(2)),
                    ("GetProperties", Some(1)),
                    ("Commit", Some(0)),
                    ("Decommit", Some(0)),
                    ("GetBuffer", Some(4)),
                    ("ReleaseBuffer", Some(1)),
                ],
            ),
        ],
    }
}

fn output_pin_spec() -> ComObjectSpec {
    ComObjectSpec {
        class: RuntimeComClass::DirectShowOutputPin,
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            interface(
                IID_PIN,
                "IPin",
                &[
                    ("Connect", None),
                    ("ReceiveConnection", None),
                    ("Disconnect", Some(0)),
                    ("ConnectedTo", Some(1)),
                    ("ConnectionMediaType", None),
                    ("QueryPinInfo", None),
                    ("QueryDirection", Some(1)),
                    ("QueryId", None),
                    ("QueryAccept", None),
                    ("EnumMediaTypes", None),
                    ("QueryInternalConnections", None),
                    ("EndOfStream", None),
                    ("BeginFlush", None),
                    ("EndFlush", None),
                    ("NewSegment", None),
                ],
            ),
        ],
    }
}

fn stream_spec() -> ComObjectSpec {
    ComObjectSpec {
        class: RuntimeComClass::DirectShowStream,
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            interface(
                IID_STREAM_ENTRY,
                "VineMediaStreamEntry",
                &[("DecodeNextSample", Some(0))],
            ),
        ],
    }
}

fn dispatch_sample(runtime: &mut Runtime, origin: ComOrigin) -> ComDispatchOutcome {
    let Some(sample) = runtime.direct_show.samples.get(&origin.target.object_id) else {
        return runtime_failure(
            origin,
            RuntimeStateError::new("live media sample has no byte owner"),
        );
    };
    if let Err(error) = runtime.validate_media_backing(sample.backing) {
        return runtime_failure(origin, error);
    }
    let arguments = &origin.frame.explicit_arguments;
    let writes = match origin.frame.vtable_slot {
        3 => vec![(
            GuestAddress(arguments[0]),
            sample.backing.range.start().0.to_le_bytes().to_vec(),
        )],
        4 => {
            return crate::com::complete_call(
                origin.frame,
                sample.backing.range.byte_length() as u32,
            );
        }
        5 => vec![
            (
                GuestAddress(arguments[0]),
                sample.start_100ns.to_le_bytes().to_vec(),
            ),
            (
                GuestAddress(arguments[1]),
                sample.end_100ns.to_le_bytes().to_vec(),
            ),
        ],
        6 => {
            if arguments[0] == 0 || arguments[1] == 0 {
                return crate::com::complete_call(origin.frame, 0x8000_4003);
            }
            let read = |pointer| {
                runtime
                    .memory
                    .read(GuestAddress(pointer), 8)
                    .map_err(RuntimeStateError::from_memory)
                    .map(|bytes| i64::from_le_bytes(bytes.try_into().unwrap()))
            };
            let (start, end) = match (read(arguments[0]), read(arguments[1])) {
                (Ok(start), Ok(end)) => (start, end),
                (Err(error), _) | (_, Err(error)) => return runtime_failure(origin, error),
            };
            if end < start {
                return crate::com::complete_call(origin.frame, E_INVALIDARG);
            }
            let sample = runtime
                .direct_show
                .samples
                .get_mut(&origin.target.object_id)
                .unwrap();
            sample.start_100ns = start;
            sample.end_100ns = end;
            return crate::com::complete_call(origin.frame, 0);
        }
        7 => return crate::com::complete_call(origin.frame, u32::from(!sample.sync_point)),
        8 | 10 | 16 => {
            let sample = runtime
                .direct_show
                .samples
                .get_mut(&origin.target.object_id)
                .unwrap();
            match origin.frame.vtable_slot {
                8 => sample.sync_point = arguments[0] != 0,
                10 => sample.preroll = arguments[0] != 0,
                _ => sample.discontinuity = arguments[0] != 0,
            }
            return crate::com::complete_call(origin.frame, 0);
        }
        9 => return crate::com::complete_call(origin.frame, u32::from(!sample.preroll)),
        11 => return crate::com::complete_call(origin.frame, sample.byte_length),
        12 => {
            if (arguments[0] as i32) < 0
                || u64::from(arguments[0]) > sample.backing.range.byte_length()
            {
                return crate::com::complete_call(origin.frame, E_INVALIDARG);
            }
            runtime
                .direct_show
                .samples
                .get_mut(&origin.target.object_id)
                .unwrap()
                .byte_length = arguments[0];
            return crate::com::complete_call(origin.frame, 0);
        }
        13 => {
            let output = GuestAddress(arguments[0]);
            return finish_writes(
                runtime,
                origin,
                vec![(output, 0u32.to_le_bytes().to_vec())],
                1,
            );
        }
        15 => return crate::com::complete_call(origin.frame, u32::from(!sample.discontinuity)),
        _ => {
            return runtime_failure(
                origin,
                RuntimeStateError::new("unsupported media sample method"),
            );
        }
    };
    finish_writes(runtime, origin, writes, 0)
}

fn dispatch_allocator(runtime: &mut Runtime, origin: ComOrigin) -> ComDispatchOutcome {
    let id = origin.target.object_id;
    let Some(allocator) = runtime.direct_show.allocators.get(&id) else {
        return runtime_failure(
            origin,
            RuntimeStateError::new("live media allocator has no pool"),
        );
    };
    let arguments = &origin.frame.explicit_arguments;
    match origin.frame.vtable_slot {
        3 => {
            if arguments[0] == 0 || arguments[1] == 0 {
                return crate::com::complete_call(origin.frame, 0x8000_4003);
            }
            let bytes = match runtime.memory.read(GuestAddress(arguments[0]), 16) {
                Ok(bytes) => bytes,
                Err(error) => {
                    return runtime_failure(origin, RuntimeStateError::from_memory(error));
                }
            };
            let requested: [i32; 4] = std::array::from_fn(|index| {
                i32::from_le_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap())
            });
            if requested[0] <= 0 || requested[1] <= 0 || requested[2] <= 0 || requested[3] < 0 {
                return crate::com::complete_call(origin.frame, E_INVALIDARG);
            }
            if allocator.committed {
                return crate::com::complete_call(origin.frame, 0x8004_020f);
            }
            if runtime
                .direct_show
                .samples
                .values()
                .any(|sample| sample.allocator_id == id)
            {
                return crate::com::complete_call(origin.frame, 0x8004_020e);
            }
            if requested[0] > 8
                || requested[1] > 64 * 1024 * 1024
                || requested[2] != 1
                || requested[3] != 0
            {
                return runtime_failure(
                    origin,
                    RuntimeStateError::new(
                        "media allocator properties exceed the supported bounded pool",
                    ),
                );
            }
            let output = GuestAddress(arguments[1]);
            let result = finish_writes(runtime, origin, vec![(output, bytes)], 0);
            if matches!(result, ComDispatchOutcome::Completed(_)) {
                runtime
                    .direct_show
                    .allocators
                    .get_mut(&id)
                    .unwrap()
                    .properties = requested;
            }
            result
        }
        4 => {
            let bytes = allocator
                .properties
                .into_iter()
                .flat_map(i32::to_le_bytes)
                .collect();
            let output = GuestAddress(arguments[0]);
            finish_writes(runtime, origin, vec![(output, bytes)], 0)
        }
        5 => match runtime.commit_media_allocator(id) {
            Ok(()) => crate::com::complete_call(origin.frame, 0),
            Err(error) => runtime_failure(origin, error),
        },
        6 => {
            // Existing samples retain their backing while decommitted; all
            // future GetBuffer calls fail until Commit resumes this pool.
            runtime
                .direct_show
                .allocators
                .get_mut(&id)
                .unwrap()
                .committed = false;
            crate::com::complete_call(origin.frame, 0)
        }
        7 => runtime_failure(
            origin,
            RuntimeStateError::new("guest-driven media allocator GetBuffer is not implemented"),
        ),
        8 => runtime_failure(
            origin,
            RuntimeStateError::new("guest-driven media allocator ReleaseBuffer is not implemented"),
        ),
        _ => runtime_failure(
            origin,
            RuntimeStateError::new("unsupported media allocator method"),
        ),
    }
}

fn dispatch_output_pin(runtime: &mut Runtime, origin: ComOrigin) -> ComDispatchOutcome {
    let Some(graph_id) = runtime
        .direct_show
        .output_pins
        .get(&origin.target.object_id)
        .copied()
    else {
        return runtime_failure(
            origin,
            RuntimeStateError::new("decoded output pin has no graph owner"),
        );
    };
    match origin.frame.vtable_slot {
        9 => {
            let output = GuestAddress(origin.frame.explicit_arguments[0]);
            finish_writes(
                runtime,
                origin,
                vec![(output, 1u32.to_le_bytes().to_vec())],
                0,
            )
        }
        6 => {
            let Some(input) = runtime.direct_show.graph(graph_id).unwrap().input_pin else {
                return crate::com::complete_call(origin.frame, VFW_E_NOT_CONNECTED);
            };
            // The pin's peer is guest-owned and must acquire a real reference.
            runtime_failure(
                origin,
                RuntimeStateError::new(format!(
                    "decoded output ConnectedTo callback for {:#x} is not implemented",
                    input.0
                )),
            )
        }
        5 => crate::com::complete_call(origin.frame, 1),
        _ => runtime_failure(
            origin,
            RuntimeStateError::new("unsupported decoded output pin method"),
        ),
    }
}

fn dispatch_stream(runtime: &mut Runtime, origin: ComOrigin) -> ComDispatchOutcome {
    let Some(graph_id) = runtime
        .direct_show
        .streams
        .get(&origin.target.object_id)
        .copied()
    else {
        return runtime_failure(
            origin,
            RuntimeStateError::new("media stream entry has no graph owner"),
        );
    };
    let graph = match runtime.direct_show.graph(graph_id) {
        Ok(graph) => graph,
        Err(error) => return runtime_failure(origin, error),
    };
    if graph.stream_thread != Some(runtime.current_thread_id()) {
        return runtime_failure(
            origin,
            RuntimeStateError::new("media stream entry belongs to another guest thread"),
        );
    }
    if graph.completed || graph.state != GraphState::Running {
        // Retire this real internal guest worker through its existing return
        // sentinel. A later acknowledged Run reuses the COM child and starts a
        // new worker; paused graphs never request another decoded sample.
        runtime
            .direct_show
            .graphs
            .get_mut(&graph_id)
            .unwrap()
            .stream_thread = None;
        return crate::com::complete_call(origin.frame, 0);
    }
    match runtime.begin_media_call(origin, ComMediaOperation::NextFrame, None) {
        Ok(outcome) => outcome,
        Err(error) => ComDispatchOutcome::Fault(crate::com::ComFault {
            message: error.message,
            call_site: GuestAddress(0),
            target: None,
        }),
    }
}

impl Runtime {
    fn commit_media_allocator(&mut self, id: RuntimeComObjectId) -> Result<(), RuntimeStateError> {
        let allocator = self
            .direct_show
            .allocators
            .get(&id)
            .ok_or_else(|| RuntimeStateError::new("media allocator is retired"))?;
        self.direct_show.graph(allocator.graph_id)?;
        if allocator.committed {
            return Ok(());
        }
        let existing = allocator.free.len()
            + self
                .direct_show
                .samples
                .values()
                .filter(|sample| sample.allocator_id == id)
                .count();
        let missing = usize::try_from(allocator.properties[0])
            .unwrap()
            .saturating_sub(existing);
        let bytes = allocator.properties[1] as u32;
        let added = self.allocate_media_buffers(bytes, missing)?;
        let allocator = self.direct_show.allocators.get_mut(&id).unwrap();
        allocator.free.extend(added);
        allocator.committed = true;
        Ok(())
    }

    fn create_media_sample(
        &mut self,
        allocator_id: RuntimeComObjectId,
        frame: ComMediaFrame,
    ) -> Result<crate::com::InstalledComObject, RuntimeStateError> {
        let allocator = self
            .direct_show
            .allocators
            .get(&allocator_id)
            .ok_or_else(|| RuntimeStateError::new("sample allocator is retired"))?;
        if !allocator.committed {
            return Err(RuntimeStateError::new(
                "sample delivery requires a committed allocator",
            ));
        }
        let backing = *allocator
            .free
            .last()
            .ok_or_else(|| RuntimeStateError::new("media sample pool has no free buffer"))?;
        self.validate_media_backing(backing)?;
        if frame.bgra.is_empty() || frame.bgra.len() as u64 > backing.range.byte_length() {
            return Err(RuntimeStateError::new(
                "decoded sample exceeds its owned buffer",
            ));
        }
        let parent = self
            .com()
            .interface_for_object_iid(allocator_id, IID_MEM_ALLOCATOR)?
            .id;
        let length = frame.bgra.len() as u32;
        let installed =
            self.install_com_object_with_guest_writes(Some(parent), sample_spec(), |_| {
                Ok(vec![(backing.range.start(), frame.bgra)])
            })?;
        self.direct_show
            .allocators
            .get_mut(&allocator_id)
            .unwrap()
            .free
            .pop();
        self.direct_show.samples.insert(
            installed.object_id,
            MediaSample {
                allocator_id,
                backing,
                byte_length: length,
                start_100ns: frame.start_100ns,
                end_100ns: frame.end_100ns,
                sync_point: true,
                preroll: false,
                discontinuity: false,
            },
        );
        Ok(installed)
    }
}

fn dispatch_seeking(runtime: &mut Runtime, origin: ComOrigin) -> ComDispatchOutcome {
    let graph = runtime
        .direct_show
        .graph(origin.target.object_id)
        .expect("validated graph");
    let arguments = &origin.frame.explicit_arguments;
    let duration = graph.duration_100ns;
    let position = graph.position_100ns;
    let mut writes = Vec::new();
    match origin.frame.vtable_slot {
        7 => writes.push((
            GuestAddress(arguments[0]),
            TIME_FORMAT_MEDIA_TIME.0.to_vec(),
        )),
        10 | 11 => match duration {
            Some(duration) => {
                writes.push((GuestAddress(arguments[0]), duration.to_le_bytes().to_vec()))
            }
            None => return crate::com::complete_call(origin.frame, 0x8000_4001),
        },
        12 => writes.push((GuestAddress(arguments[0]), position.to_le_bytes().to_vec())),
        13 => {
            for pointer in [arguments[1], arguments[4]] {
                if pointer != 0 {
                    let format = match runtime.memory.read(GuestAddress(pointer), 16) {
                        Ok(bytes) => bytes,
                        Err(error) => {
                            return runtime_failure(origin, RuntimeStateError::from_memory(error));
                        }
                    };
                    if format != TIME_FORMAT_MEDIA_TIME.0 {
                        return crate::com::complete_call(origin.frame, E_INVALIDARG);
                    }
                }
            }
            let time = (u64::from(arguments[2]) | (u64::from(arguments[3]) << 32))
                .to_le_bytes()
                .to_vec();
            writes.push((GuestAddress(arguments[0]), time));
        }
        15 | 16 => {
            let Some(duration) = duration else {
                return crate::com::complete_call(origin.frame, 0x8000_4001);
            };
            writes.push((
                GuestAddress(arguments[0]),
                if origin.frame.vtable_slot == 15 {
                    position
                } else {
                    0i64
                }
                .to_le_bytes()
                .to_vec(),
            ));
            writes.push((GuestAddress(arguments[1]), duration.to_le_bytes().to_vec()));
        }
        17 => {
            let rate = f64::from_bits(u64::from(arguments[0]) | (u64::from(arguments[1]) << 32));
            if !rate.is_finite() || rate <= 0.0 {
                return crate::com::complete_call(origin.frame, E_INVALIDARG);
            }
            if rate != 1.0 {
                return runtime_failure(
                    origin,
                    RuntimeStateError::new(
                        "DirectShow playback rate other than 1.0 is not implemented",
                    ),
                );
            }
        }
        18 => writes.push((GuestAddress(arguments[0]), 1.0f64.to_le_bytes().to_vec())),
        19 => writes.push((GuestAddress(arguments[0]), 0i64.to_le_bytes().to_vec())),
        _ => {
            return runtime_failure(
                origin,
                RuntimeStateError::new("unimplemented DirectShow seeking method"),
            );
        }
    }
    finish_writes(runtime, origin, writes, 0)
}

fn dispatch_basic_audio(runtime: &mut Runtime, origin: ComOrigin) -> ComDispatchOutcome {
    let graph = runtime
        .direct_show
        .graph(origin.target.object_id)
        .expect("validated graph");
    let value = origin.frame.explicit_arguments[0] as i32;
    match origin.frame.vtable_slot {
        7 => {
            if !(-10_000..=0).contains(&value) {
                return crate::com::complete_call(origin.frame, E_INVALIDARG);
            }
            if graph.format.is_some() {
                return match runtime.begin_media_call(origin, ComMediaOperation::Volume, None) {
                    Ok(outcome) => outcome,
                    Err(error) => ComDispatchOutcome::Fault(crate::com::ComFault {
                        message: error.message,
                        call_site: GuestAddress(0),
                        target: None,
                    }),
                };
            }
            runtime
                .direct_show
                .graphs
                .get_mut(&origin.target.object_id)
                .unwrap()
                .volume = value;
            crate::com::complete_call(origin.frame, 0)
        }
        8 => finish_writes(
            runtime,
            origin,
            vec![(
                GuestAddress(value as u32),
                graph.volume.to_le_bytes().to_vec(),
            )],
            0,
        ),
        9 => {
            if !(-10_000..=10_000).contains(&value) {
                return crate::com::complete_call(origin.frame, E_INVALIDARG);
            }
            if value != 0 {
                return runtime_failure(
                    origin,
                    RuntimeStateError::new("DirectShow audio balance is not implemented"),
                );
            }
            runtime
                .direct_show
                .graphs
                .get_mut(&origin.target.object_id)
                .unwrap()
                .balance = 0;
            crate::com::complete_call(origin.frame, 0)
        }
        10 => finish_writes(
            runtime,
            origin,
            vec![(
                GuestAddress(value as u32),
                graph.balance.to_le_bytes().to_vec(),
            )],
            0,
        ),
        _ => runtime_failure(
            origin,
            RuntimeStateError::new("unimplemented DirectShow audio method"),
        ),
    }
}

fn finish_writes(
    runtime: &mut Runtime,
    origin: ComOrigin,
    writes: Vec<(GuestAddress, Vec<u8>)>,
    eax: u32,
) -> ComDispatchOutcome {
    if writes.iter().any(|(address, _)| address.0 == 0) {
        return crate::com::complete_call(origin.frame, 0x8000_4003);
    }
    for (address, bytes) in &writes {
        if let Err(error) =
            runtime
                .memory
                .validate_access(*address, bytes.len(), MemoryAccess::Write)
        {
            return runtime_failure(origin, RuntimeStateError::from_memory(error));
        }
    }
    let mut transaction = runtime.memory.begin_transaction();
    for (address, bytes) in writes {
        if let Err(error) = transaction.write(address, &bytes) {
            return runtime_failure(origin, RuntimeStateError::from_memory(error));
        }
    }
    transaction.commit_transaction();
    crate::com::complete_call(origin.frame, eax)
}

impl RuntimeDirectShow {
    pub(crate) fn graph(
        &self,
        id: RuntimeComObjectId,
    ) -> Result<&DirectShowGraph, RuntimeStateError> {
        self.graphs
            .get(&id)
            .ok_or_else(|| RuntimeStateError::new("live FilterGraph has no DirectShow state"))
    }

    pub(crate) fn retirement_backings(
        &self,
        ids: &std::collections::BTreeSet<RuntimeComObjectId>,
    ) -> Result<Vec<MediaBacking>, RuntimeStateError> {
        let mut backings = Vec::new();
        for id in ids {
            if let Some(graph) = self.graphs.get(id) {
                if !graph.filters.is_empty()
                    || graph.reader.is_some()
                    || graph.input_pin.is_some()
                    || graph.mem_input.is_some()
                    || graph.format.is_some()
                    || graph.allocator.is_some()
                    || graph.output_pin.is_some()
                    || graph.stream.is_some()
                    || self
                        .active
                        .values()
                        .any(|active| active.origin.target.object_id == *id)
                {
                    return Err(RuntimeStateError::new(
                        "FilterGraph retirement still owns filters, codec, or pending work",
                    ));
                }
                if let Some(backing) = graph.encoded_buffer {
                    backings.push(backing);
                }
            }
            if let Some(allocator) = self.allocators.get(id) {
                if self.samples.iter().any(|(sample_id, sample)| {
                    sample.allocator_id == *id && !ids.contains(sample_id)
                }) {
                    return Err(RuntimeStateError::new(
                        "media allocator retired with a live retained sample",
                    ));
                }
                backings.extend(allocator.free.iter().copied());
            }
            if let Some(sample) = self.samples.get(id) {
                if ids.contains(&sample.allocator_id) {
                    backings.push(sample.backing);
                } else if !self.allocators.contains_key(&sample.allocator_id) {
                    return Err(RuntimeStateError::new(
                        "media sample outlived its actual allocator",
                    ));
                }
            }
        }
        Ok(backings)
    }

    pub(crate) fn retire_objects(&mut self, ids: &[RuntimeComObjectId]) {
        for id in ids {
            if let Some(sample) = self.samples.remove(id)
                && !ids.contains(&sample.allocator_id)
            {
                self.allocators
                    .get_mut(&sample.allocator_id)
                    .expect("validated sample allocator")
                    .free
                    .push(sample.backing);
            }
        }
        for id in ids {
            self.graphs.remove(id);
            self.allocators.remove(id);
            self.output_pins.remove(id);
            self.streams.remove(id);
        }
    }
}

fn interface(iid: RuntimeComIid, name: &str, methods: &[(&str, Option<u16>)]) -> ComInterfaceSpec {
    let mut slots = vec![
        ComMethodSpec::query_interface(),
        ComMethodSpec::add_ref(),
        ComMethodSpec::release(),
    ];
    slots.extend(methods.iter().map(|(name, count)| match count {
        Some(count) => ComMethodSpec::frontend(*name, *count),
        None => ComMethodSpec::unsupported(*name),
    }));
    ComInterfaceSpec {
        iid,
        name: name.to_owned(),
        methods: slots,
    }
}

pub(crate) fn filter_graph_spec() -> ComObjectSpec {
    let graph_methods = [
        ("AddFilter", Some(2)),
        ("RemoveFilter", None),
        ("EnumFilters", None),
        ("FindFilterByName", None),
        ("ConnectDirect", None),
        ("Reconnect", None),
        ("Disconnect", None),
        ("SetDefaultSyncSource", None),
        ("Connect", None),
        ("Render", Some(1)),
        ("RenderFile", Some(2)),
        ("AddSourceFilter", None),
        ("SetLogFile", None),
        ("Abort", None),
        ("ShouldOperationContinue", None),
    ];
    // IDispatch methods remain explicit unsupported endpoints; the caller uses
    // these dual interfaces through their native vtables.
    let control = [
        ("GetTypeInfoCount", None),
        ("GetTypeInfo", None),
        ("GetIDsOfNames", None),
        ("Invoke", None),
        ("Run", Some(0)),
        ("Pause", Some(0)),
        ("Stop", Some(0)),
        ("GetState", Some(2)),
        ("RenderFile", None),
        ("AddSourceFilter", None),
        ("get_FilterCollection", None),
        ("get_RegFilterCollection", None),
        ("StopWhenReady", None),
    ];
    let event = [
        ("GetTypeInfoCount", None),
        ("GetTypeInfo", None),
        ("GetIDsOfNames", None),
        ("Invoke", None),
        ("GetEventHandle", None),
        ("GetEvent", Some(4)),
        ("WaitForCompletion", None),
        ("CancelDefaultHandling", None),
        ("RestoreDefaultHandling", None),
        ("FreeEventParams", Some(3)),
        ("SetNotifyWindow", Some(3)),
        ("SetNotifyFlags", None),
        ("GetNotifyFlags", None),
    ];
    let seeking = [
        ("GetCapabilities", None),
        ("CheckCapabilities", None),
        ("IsFormatSupported", None),
        ("QueryPreferredFormat", None),
        ("GetTimeFormat", Some(1)),
        ("IsUsingTimeFormat", None),
        ("SetTimeFormat", None),
        ("GetDuration", Some(1)),
        ("GetStopPosition", Some(1)),
        ("GetCurrentPosition", Some(1)),
        ("ConvertTimeFormat", Some(5)),
        ("SetPositions", None),
        ("GetPositions", Some(2)),
        ("GetAvailable", Some(2)),
        ("SetRate", Some(2)),
        ("GetRate", Some(1)),
        ("GetPreroll", Some(1)),
    ];
    let audio = [
        ("GetTypeInfoCount", None),
        ("GetTypeInfo", None),
        ("GetIDsOfNames", None),
        ("Invoke", None),
        ("put_Volume", Some(1)),
        ("get_Volume", Some(1)),
        ("put_Balance", Some(1)),
        ("get_Balance", Some(1)),
    ];
    ComObjectSpec {
        class: RuntimeComClass::DirectShowGraph,
        interfaces: vec![
            ComInterfaceSpec::iunknown(),
            interface(IID_GRAPH_BUILDER, "IGraphBuilder", &graph_methods),
            interface(IID_FILTER_GRAPH, "IFilterGraph", &graph_methods[..8]),
            interface(IID_MEDIA_CONTROL, "IMediaControl", &control),
            interface(IID_MEDIA_EVENT_EX, "IMediaEventEx", &event),
            interface(IID_MEDIA_SEEKING, "IMediaSeeking", &seeking),
            interface(IID_BASIC_AUDIO, "IBasicAudio", &audio),
            interface(
                IID_MEDIA_EVENT_SINK,
                "IMediaEventSink",
                &[("Notify", Some(3))],
            ),
        ],
    }
}
