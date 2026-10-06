use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestRange, MemoryAccess,
    MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection,
};
use crate::runtime::{
    PreparedBrowserAudio, ProcessAddressAllocationClassification, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationReleasePolicy, ProcessAddressAllocationRequest,
    RUNTIME_COM_IUNKNOWN_IID, RUNTIME_COM_MAX_INTERFACES_PER_OBJECT, Runtime, RuntimeAddressRange,
    RuntimeComClass, RuntimeComIid, RuntimeComInterfaceId, RuntimeComInterfaceRegistration,
    RuntimeComMethodRegistration, RuntimeComMethodTarget, RuntimeComObjectId, RuntimeStateError,
};

pub const COM_S_OK: ComHResult = ComHResult(0);
pub const COM_S_FALSE: ComHResult = ComHResult(1);
pub const COM_E_NOINTERFACE: ComHResult = ComHResult(0x8000_4002_u32 as i32);
pub const COM_E_POINTER: ComHResult = ComHResult(0x8000_4003_u32 as i32);
pub const COM_RPC_E_CHANGED_MODE: ComHResult = ComHResult(0x8001_0106_u32 as i32);
pub const RUNTIME_COM_ENDPOINT_SLOT_SIZE: u32 = 16;
pub const RUNTIME_COM_BACKING_BYTES: u32 = GUEST_PAGE_SIZE as u32 * 2;
pub const RUNTIME_COM_MAX_ENDPOINTS_PER_OBJECT: usize =
    GUEST_PAGE_SIZE as usize / RUNTIME_COM_ENDPOINT_SLOT_SIZE as usize;
const RUNTIME_COM_SEARCH_BASE: GuestAddress = GuestAddress(0x0100_0000);
const RUNTIME_COM_SEARCH_BYTES: u64 = 0x0f00_0000;
const RUNTIME_COM_ALLOCATION_ALIGNMENT: u32 = 0x1_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComHResult(pub i32);

impl ComHResult {
    pub const fn bits(self) -> u32 {
        self.0 as u32
    }

    #[cfg(test)]
    pub(crate) const fn succeeded(self) -> bool {
        self.0 >= 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComMethodSpec {
    pub name: String,
    pub explicit_argument_words: Option<u16>,
}

impl ComMethodSpec {
    pub fn query_interface() -> Self {
        Self::frontend("QueryInterface", 2)
    }

    pub fn add_ref() -> Self {
        Self::frontend("AddRef", 0)
    }

    pub fn release() -> Self {
        Self::frontend("Release", 0)
    }

    pub fn unsupported(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            explicit_argument_words: None,
        }
    }

    pub fn frontend(name: impl Into<String>, explicit_argument_words: u16) -> Self {
        Self {
            name: name.into(),
            explicit_argument_words: Some(explicit_argument_words),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComInterfaceSpec {
    pub iid: RuntimeComIid,
    pub name: String,
    pub methods: Vec<ComMethodSpec>,
}

impl ComInterfaceSpec {
    pub fn iunknown() -> Self {
        Self {
            iid: RUNTIME_COM_IUNKNOWN_IID,
            name: "IUnknown".to_owned(),
            methods: vec![
                ComMethodSpec::query_interface(),
                ComMethodSpec::add_ref(),
                ComMethodSpec::release(),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComObjectSpec {
    pub class: RuntimeComClass,
    pub interfaces: Vec<ComInterfaceSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledComObject {
    pub object_id: RuntimeComObjectId,
    pub interfaces: Vec<(RuntimeComIid, GuestAddress)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComCallTrap {
    pub endpoint_address: GuestAddress,
    pub stack_pointer: GuestAddress,
    pub return_address: GuestAddress,
    pub call_site: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComCallFrame {
    pub object_id: crate::runtime::RuntimeComObjectId,
    pub interface_id: RuntimeComInterfaceId,
    pub iid: RuntimeComIid,
    pub vtable_slot: u16,
    pub return_stack_pointer: GuestAddress,
    pub return_address: GuestAddress,
    pub explicit_arguments: Vec<u32>,
}

#[derive(Debug)]
pub struct ComCallCompletion {
    pub created_thread: Option<crate::runtime::RuntimeThreadId>,
    pub presented_device: Option<crate::runtime::RuntimeGraphicsDeviceId>,
    pub instruction_pointer: GuestAddress,
    pub stack_pointer: GuestAddress,
    pub eax: u32,
}

#[derive(Debug)]
pub struct ComFaultTarget {
    pub interface: String,
    pub vtable_slot: u16,
    pub method: String,
}

#[derive(Debug)]
pub struct ComFault {
    pub message: String,
    pub call_site: GuestAddress,
    pub target: Option<ComFaultTarget>,
}

#[derive(Debug)]
pub enum ComDispatchOutcome {
    Completed(ComCallCompletion),
    AudioPending(ComAudioDeferred),
    GuestCallback(crate::api::PreparedGuestCallback),
    MediaPending(crate::directshow::ComMediaDeferred),
    MediaData(crate::directshow::ComMediaData),
    Fault(ComFault),
}

#[derive(Debug)]
pub struct ComAudioDeferred {
    call: Box<ComAudioDeferredCall>,
}

#[derive(Debug)]
struct ComAudioDeferredCall {
    reference_count: u32,
    prepared: PreparedBrowserAudio,
    trap: ComCallTrap,
    target: RuntimeComMethodTarget,
    frame: ComCallFrame,
}

impl ComAudioDeferred {
    pub fn request_id(&self) -> u64 {
        self.call.prepared.request_id
    }
    pub fn command_bytes(&self) -> &[u8] {
        &self.call.prepared.command
    }
}

pub(crate) struct ComFrontendCall<'a> {
    pub(crate) runtime: &'a mut Runtime,
    pub(crate) trap: ComCallTrap,
    pub(crate) target: RuntimeComMethodTarget,
    pub(crate) frame: ComCallFrame,
}

impl ComFrontendCall<'_> {
    pub(crate) fn frame(&self) -> &ComCallFrame {
        &self.frame
    }

    pub(crate) fn class(&self) -> RuntimeComClass {
        self.target.class
    }

    pub(crate) fn fault(self, message: impl Into<String>) -> ComDispatchOutcome {
        ComDispatchOutcome::Fault(fault(message, self.trap, Some(&self.target)))
    }

    pub(crate) fn memory_fault(self, error: GuestMemoryError) -> ComDispatchOutcome {
        ComDispatchOutcome::Fault(fault_from_memory(error, self.trap, Some(&self.target)))
    }

    pub(crate) fn runtime_fault(self, error: RuntimeStateError) -> ComDispatchOutcome {
        ComDispatchOutcome::Fault(fault_from_runtime(error, self.trap, Some(&self.target)))
    }

    pub(crate) fn complete(self, eax: u32) -> ComDispatchOutcome {
        complete_call(self.frame, eax)
    }

    pub(crate) fn defer_audio(self, prepared: PreparedBrowserAudio) -> ComDispatchOutcome {
        let reference_count = match self.runtime.com().object(self.target.object_id) {
            Ok(object) => object.reference_count,
            Err(error) => return self.runtime_fault(error),
        };
        ComDispatchOutcome::AudioPending(ComAudioDeferred {
            call: Box::new(ComAudioDeferredCall {
                reference_count,
                prepared,
                trap: self.trap,
                target: self.target,
                frame: self.frame,
            }),
        })
    }
}

impl Runtime {
    pub(crate) fn install_com_object(
        &mut self,
        spec: ComObjectSpec,
    ) -> Result<InstalledComObject, RuntimeStateError> {
        self.install_com_object_with_guest_writes(None, spec, |_| Ok(Vec::new()))
    }
}

pub(crate) fn initialize_com_backing(
    memory: &mut GuestMemory,
    allocation: &crate::runtime::ProcessAddressAllocationRecord,
    spec: &ComObjectSpec,
) -> Result<(MemoryOwnerId, Vec<RuntimeComInterfaceRegistration>), RuntimeStateError> {
    if allocation.range.byte_length != u64::from(RUNTIME_COM_BACKING_BYTES)
        || allocation.usable_byte_length != u64::from(RUNTIME_COM_BACKING_BYTES)
    {
        return Err(RuntimeStateError::new(
            "COM backing allocation did not preserve its two-page layout",
        ));
    }
    let data_range = GuestRange::new(allocation.range.base, GUEST_PAGE_SIZE)
        .map_err(RuntimeStateError::from_memory)?;
    let endpoint_base = add_address(allocation.range.base, GUEST_PAGE_SIZE as u32)?;
    let endpoint_range =
        GuestRange::new(endpoint_base, GUEST_PAGE_SIZE).map_err(RuntimeStateError::from_memory)?;

    let memory_owner = memory
        .create_owner(MemoryOwnerKind::Runtime)
        .map_err(RuntimeStateError::from_memory)?;
    memory
        .reserve_fixed_for_address_allocation(
            allocation.id.0,
            memory_owner,
            MemoryRegionKind::Runtime,
            allocation.range.base,
            allocation.range.byte_length,
        )
        .map_err(RuntimeStateError::from_memory)?;
    let writable = PageProtection {
        read: true,
        write: true,
        execute: false,
    };
    memory
        .commit(memory_owner, data_range, writable)
        .map_err(RuntimeStateError::from_memory)?;
    memory
        .commit(memory_owner, endpoint_range, writable)
        .map_err(RuntimeStateError::from_memory)?;
    let interfaces = initialize_com_interfaces(memory, allocation.range.base, spec)?;
    memory
        .protect(
            memory_owner,
            data_range,
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .map_err(RuntimeStateError::from_memory)?;
    memory
        .protect(
            memory_owner,
            endpoint_range,
            PageProtection {
                read: true,
                write: false,
                execute: true,
            },
        )
        .map_err(RuntimeStateError::from_memory)?;
    memory
        .seal_owner(memory_owner)
        .map_err(RuntimeStateError::from_memory)?;

    Ok((memory_owner, interfaces))
}

pub(crate) fn com_address_allocation_request() -> ProcessAddressAllocationRequest {
    ProcessAddressAllocationRequest {
        purpose: ProcessAddressAllocationPurpose::ComEndpoint,

        preferred_address: None,
        search_range: RuntimeAddressRange {
            base: RUNTIME_COM_SEARCH_BASE,
            byte_length: RUNTIME_COM_SEARCH_BYTES,
        },
        reserve_size: u64::from(RUNTIME_COM_BACKING_BYTES),
        alignment: RUNTIME_COM_ALLOCATION_ALIGNMENT,
        guard_pages: 0,
        classification: ProcessAddressAllocationClassification::Com,

        release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
        executable: true,
    }
}

impl Runtime {
    pub fn complete_com_audio(
        &mut self,
        pending: ComAudioDeferred,
        accepted: bool,
    ) -> ComDispatchOutcome {
        let pending = *pending.call;
        let live = self.resolve_com_method(pending.trap.endpoint_address);
        match live {
            Ok(Some(target))
                if target.object_id == pending.target.object_id
                    && target.interface_id == pending.target.interface_id
                    && target.guest_pointer == pending.target.guest_pointer
                    && target.iid == pending.target.iid
                    && target.method.slot == pending.target.method.slot => {}
            _ => {
                return ComDispatchOutcome::Fault(fault(
                    "browser audio completion has no matching live COM endpoint",
                    pending.trap,
                    Some(&pending.target),
                ));
            }
        }
        if !self
            .com()
            .object(pending.target.object_id)
            .is_ok_and(|object| object.reference_count == pending.reference_count)
        {
            return ComDispatchOutcome::Fault(fault(
                "COM references changed during browser audio handoff",
                pending.trap,
                Some(&pending.target),
            ));
        }
        match decode_frame(&self.memory, &pending.target, pending.trap) {
            Ok(frame) if frame == pending.frame => {}
            _ => {
                return ComDispatchOutcome::Fault(fault(
                    "COM audio call frame changed during browser handoff",
                    pending.trap,
                    Some(&pending.target),
                ));
            }
        }
        match self.complete_browser_audio(&pending.prepared, accepted) {
            Ok(eax) => complete_call(pending.frame, eax),
            Err(error) => ComDispatchOutcome::Fault(fault_from_runtime(
                error,
                pending.trap,
                Some(&pending.target),
            )),
        }
    }

    pub fn dispatch_com_call(&mut self, trap: ComCallTrap) -> ComDispatchOutcome {
        let target = match self.resolve_com_method(trap.endpoint_address) {
            Ok(Some(target)) => target,
            Ok(None) => return ComDispatchOutcome::Fault(unowned_endpoint_fault(trap)),
            Err(error) => return ComDispatchOutcome::Fault(fault_from_runtime(error, trap, None)),
        };
        if target.method.explicit_argument_words.is_none() {
            return ComDispatchOutcome::Fault(fault(
                format!(
                    "COM interface {} slot {} ({}) is not implemented",
                    target.interface_name, target.method.slot, target.method.name
                ),
                trap,
                Some(&target),
            ));
        }
        let frame = match decode_frame(&self.memory, &target, trap) {
            Ok(frame) => frame,
            Err(fault) => return ComDispatchOutcome::Fault(*fault),
        };
        match target.method.slot {
            0 => dispatch_query_interface(self, trap, target, frame),
            1 => {
                let count = match self.add_ref_com_interface(target.interface_id) {
                    Ok(count) => count,
                    Err(error) => {
                        return ComDispatchOutcome::Fault(fault_from_runtime(
                            error,
                            trap,
                            Some(&target),
                        ));
                    }
                };
                complete_call(frame, count)
            }
            2 if target.class == RuntimeComClass::DirectShowGraph => {
                crate::directshow::dispatch_graph_release(ComFrontendCall {
                    runtime: self,
                    trap,
                    target,
                    frame,
                })
            }
            2 => dispatch_release(self, trap, target, frame),
            _ => {
                let class = target.class;
                let call = ComFrontendCall {
                    runtime: self,
                    trap,
                    target,
                    frame,
                };
                match class {
                    RuntimeComClass::D3d9
                    | RuntimeComClass::D3d9Device
                    | RuntimeComClass::D3d9Texture
                    | RuntimeComClass::D3d9Surface
                    | RuntimeComClass::D3d9VertexBuffer => crate::d3d9::dispatch_d3d9_com(call),
                    RuntimeComClass::DirectInput8A => crate::dinput8::dispatch_dinput8_com(call),
                    RuntimeComClass::DirectSound8 | RuntimeComClass::DirectSoundBuffer8 => {
                        crate::dsound::dispatch_dsound_com(call)
                    }
                    RuntimeComClass::DirectShowGraph
                    | RuntimeComClass::DirectShowAllocator
                    | RuntimeComClass::DirectShowSample
                    | RuntimeComClass::DirectShowOutputPin
                    | RuntimeComClass::DirectShowStream => {
                        crate::directshow::dispatch_directshow_com(call)
                    }
                }
            }
        }
    }
}

fn dispatch_query_interface(
    runtime: &mut Runtime,
    trap: ComCallTrap,
    target: RuntimeComMethodTarget,
    frame: ComCallFrame,
) -> ComDispatchOutcome {
    let iid_pointer = frame.explicit_arguments[0];
    let output_pointer = frame.explicit_arguments[1];
    if iid_pointer == 0 || output_pointer == 0 {
        return complete_call(frame, COM_E_POINTER.bits());
    }
    let iid_bytes = match runtime.memory.read(GuestAddress(iid_pointer), 16) {
        Ok(bytes) => bytes,
        Err(error) => {
            return ComDispatchOutcome::Fault(fault_from_memory(error, trap, Some(&target)));
        }
    };
    if let Err(error) =
        runtime
            .memory
            .validate_access(GuestAddress(output_pointer), 4, MemoryAccess::Write)
    {
        return ComDispatchOutcome::Fault(fault_from_memory(error, trap, Some(&target)));
    }
    let iid = RuntimeComIid::from_windows_bytes(
        iid_bytes
            .try_into()
            .expect("checked COM IID read contains sixteen bytes"),
    );
    let queried = match runtime
        .com()
        .query_interface_target(target.interface_id, iid)
    {
        Ok(interface) => interface,
        Err(error) => {
            return ComDispatchOutcome::Fault(fault_from_runtime(error, trap, Some(&target)));
        }
    };
    if let Some(interface) = queried {
        let queried_id = interface.id;
        match runtime
            .add_ref_interface_with_guest_pointer_write(queried_id, GuestAddress(output_pointer))
        {
            Ok(_) => complete_call(frame, COM_S_OK.bits()),
            Err(error) => ComDispatchOutcome::Fault(fault_from_runtime(error, trap, Some(&target))),
        }
    } else {
        let mut transaction = runtime.memory.begin_transaction();
        match transaction.write(GuestAddress(output_pointer), &0_u32.to_le_bytes()) {
            Ok(()) => {
                transaction.commit_transaction();
                complete_call(frame, COM_E_NOINTERFACE.bits())
            }
            Err(error) => ComDispatchOutcome::Fault(fault_from_memory(error, trap, Some(&target))),
        }
    }
}

fn dispatch_release(
    runtime: &mut Runtime,
    trap: ComCallTrap,
    target: RuntimeComMethodTarget,
    frame: ComCallFrame,
) -> ComDispatchOutcome {
    match runtime.prepare_browser_audio_release(target.interface_id) {
        Ok(Some(prepared)) => {
            let reference_count = runtime
                .com()
                .object(target.object_id)
                .expect("validated release object")
                .reference_count;
            return ComDispatchOutcome::AudioPending(ComAudioDeferred {
                call: Box::new(ComAudioDeferredCall {
                    reference_count,
                    prepared,
                    trap,
                    target,
                    frame,
                }),
            });
        }
        Ok(None) => {}
        Err(error) => {
            return ComDispatchOutcome::Fault(fault_from_runtime(error, trap, Some(&target)));
        }
    }
    match runtime.release_com_interface(target.interface_id) {
        Ok(count) => complete_call(frame, count),
        Err(error) => ComDispatchOutcome::Fault(fault_from_runtime(error, trap, Some(&target))),
    }
}

fn decode_frame(
    memory: &GuestMemory,
    target: &RuntimeComMethodTarget,
    trap: ComCallTrap,
) -> Result<ComCallFrame, Box<ComFault>> {
    // A u16 explicit count plus this and the return address fits u32 bytes.
    let argument_words = target.method.argument_words().ok_or_else(|| {
        Box::new(fault(
            "implemented COM dispatch requires a native argument count",
            trap,
            Some(target),
        ))
    })?;
    let argument_bytes = argument_words * 4;
    let frame_bytes = argument_bytes + 4;
    let return_stack_pointer = trap
        .stack_pointer
        .0
        .checked_add(frame_bytes)
        .map(GuestAddress)
        .ok_or_else(|| {
            Box::new(fault(
                "COM return stack pointer crosses the x86 address limit",
                trap,
                Some(target),
            ))
        })?;
    let bytes = memory
        .read(trap.stack_pointer, frame_bytes as usize)
        .map_err(|error| Box::new(fault_from_memory(error, trap, Some(target))))?;
    let words = bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().expect("x86 COM words are four bytes")))
        .collect::<Vec<_>>();
    if words[0] != trap.return_address.0 {
        return Err(Box::new(fault(
            "guest COM stack return address does not match the trapped CPU state",
            trap,
            Some(target),
        )));
    }
    let this_pointer = GuestAddress(words[1]);
    if this_pointer != target.guest_pointer {
        return Err(Box::new(fault(
            "COM this pointer does not identify the interface that owns the trapped endpoint",
            trap,
            Some(target),
        )));
    }
    Ok(ComCallFrame {
        object_id: target.object_id,
        interface_id: target.interface_id,
        iid: target.iid,
        vtable_slot: target.method.slot,
        return_stack_pointer,
        return_address: trap.return_address,
        explicit_arguments: words[2..].to_vec(),
    })
}

pub(crate) fn complete_call(frame: ComCallFrame, eax: u32) -> ComDispatchOutcome {
    ComDispatchOutcome::Completed(ComCallCompletion {
        created_thread: None,
        instruction_pointer: frame.return_address,
        stack_pointer: frame.return_stack_pointer,
        presented_device: None,
        eax,
    })
}

pub(crate) fn validate_object_spec(spec: &ComObjectSpec) -> Result<(), RuntimeStateError> {
    if spec.interfaces.is_empty() || spec.interfaces.len() > RUNTIME_COM_MAX_INTERFACES_PER_OBJECT {
        return Err(RuntimeStateError::new(
            "COM object interface count is empty or exceeds the bounded foundation",
        ));
    }
    let endpoint_count = spec
        .interfaces
        .iter()
        .try_fold(0_usize, |total, interface| {
            total.checked_add(interface.methods.len())
        })
        .ok_or_else(|| RuntimeStateError::new("COM endpoint count overflowed host accounting"))?;
    if endpoint_count > RUNTIME_COM_MAX_ENDPOINTS_PER_OBJECT {
        return Err(RuntimeStateError::new(
            "COM object exceeds its one-page endpoint capacity",
        ));
    }
    Ok(())
}

fn initialize_com_interfaces(
    memory: &mut GuestMemory,
    base: GuestAddress,
    spec: &ComObjectSpec,
) -> Result<Vec<RuntimeComInterfaceRegistration>, RuntimeStateError> {
    let interface_bytes = u32::try_from(spec.interfaces.len())
        .ok()
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| RuntimeStateError::new("COM interface pointer layout overflowed"))?;
    let mut vtable_cursor = align_up(interface_bytes, 16)
        .ok_or_else(|| RuntimeStateError::new("COM vtable layout overflowed alignment"))?;
    let endpoint_base = add_address(base, GUEST_PAGE_SIZE as u32)?;
    let mut endpoint_index = 0_u32;
    let mut interfaces = Vec::with_capacity(spec.interfaces.len());

    for (interface_index, interface) in spec.interfaces.iter().enumerate() {
        let interface_offset = u32::try_from(interface_index)
            .ok()
            .and_then(|index| index.checked_mul(4))
            .ok_or_else(|| RuntimeStateError::new("COM interface pointer offset overflowed"))?;
        let guest_pointer = add_address(base, interface_offset)?;
        let vtable = add_address(base, vtable_cursor)?;
        let vtable_bytes = u32::try_from(interface.methods.len())
            .ok()
            .and_then(|count| count.checked_mul(4))
            .ok_or_else(|| RuntimeStateError::new("COM vtable byte count overflowed"))?;
        vtable_cursor = vtable_cursor
            .checked_add(vtable_bytes)
            .ok_or_else(|| RuntimeStateError::new("COM vtable layout crossed the data page"))?;
        if vtable_cursor > GUEST_PAGE_SIZE as u32 {
            return Err(RuntimeStateError::new(
                "COM interface pointers and vtables exceed one data page",
            ));
        }
        memory
            .write(guest_pointer, &vtable.0.to_le_bytes())
            .map_err(RuntimeStateError::from_memory)?;
        let mut vtable_image = Vec::with_capacity(vtable_bytes as usize);
        let mut methods = Vec::with_capacity(interface.methods.len());
        for (slot, method) in interface.methods.iter().enumerate() {
            let slot = u16::try_from(slot).expect("bounded COM method index fits u16");
            let endpoint_offset = endpoint_index
                .checked_mul(RUNTIME_COM_ENDPOINT_SLOT_SIZE)
                .ok_or_else(|| RuntimeStateError::new("COM endpoint offset overflowed"))?;
            let endpoint = add_address(endpoint_base, endpoint_offset)?;
            endpoint_index += 1;
            vtable_image.extend_from_slice(&endpoint.0.to_le_bytes());
            methods.push(RuntimeComMethodRegistration {
                slot,
                name: method.name.clone(),
                endpoint,
                explicit_argument_words: method.explicit_argument_words,
            });
            memory
                .write(endpoint, &[0x0f, 0x0b])
                .map_err(RuntimeStateError::from_memory)?;
        }
        memory
            .write(vtable, &vtable_image)
            .map_err(RuntimeStateError::from_memory)?;
        interfaces.push(RuntimeComInterfaceRegistration {
            iid: interface.iid,
            name: interface.name.clone(),
            guest_pointer,
            vtable,
            methods,
        });
    }
    Ok(interfaces)
}

fn align_up(value: u32, alignment: u32) -> Option<u32> {
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
}

fn add_address(base: GuestAddress, offset: u32) -> Result<GuestAddress, RuntimeStateError> {
    base.0
        .checked_add(offset)
        .map(GuestAddress)
        .ok_or_else(|| RuntimeStateError::new("COM object layout crosses the x86 address limit"))
}

fn unowned_endpoint_fault(trap: ComCallTrap) -> ComFault {
    fault(
        format!(
            "COM endpoint {:#010x} is not owned by a live interface",
            trap.endpoint_address.0
        ),
        trap,
        None,
    )
}

fn fault_from_runtime(
    error: RuntimeStateError,
    trap: ComCallTrap,
    target: Option<&RuntimeComMethodTarget>,
) -> ComFault {
    fault(error.message, trap, target)
}

fn fault_from_memory(
    error: GuestMemoryError,
    trap: ComCallTrap,
    target: Option<&RuntimeComMethodTarget>,
) -> ComFault {
    fault(error.message, trap, target)
}

fn fault(
    message: impl Into<String>,
    trap: ComCallTrap,
    target: Option<&RuntimeComMethodTarget>,
) -> ComFault {
    ComFault {
        message: message.into(),
        call_site: trap.call_site,
        target: target.map(|target| ComFaultTarget {
            interface: target.interface_name.clone(),
            vtable_slot: target.method.slot,
            method: target.method.name.clone(),
        }),
    }
}
