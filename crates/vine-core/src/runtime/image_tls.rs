use std::collections::BTreeMap;

use super::{Runtime, RuntimeStateError};
use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestRange, MemoryOwnerId, MemoryOwnerKind,
    MemoryRegionKind, PageProtection,
};
use crate::runtime::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationId,
    ProcessAddressAllocationPurpose, ProcessAddressAllocationRecord,
    ProcessAddressAllocationReleasePolicy, ProcessAddressAllocationRequest,
    ProcessAddressSpaceAllocator, RuntimeAddressRange, RuntimeThreadId,
    RuntimeThreadLifecycleState,
};
use crate::{ImageMaterialization, Pe32LoadPlan};

const TLS_VECTOR_BYTES: u32 = 4096;
const TEB_TLS_POINTER: u32 = 0x2c;

#[derive(Debug, Clone)]
struct ImageTlsModule {
    index: u32,
    alignment: u32,
    // Captured after image relocation, before guest code can mutate the template.
    template: Vec<u8>,
}

#[derive(Debug, Clone)]
struct TlsAllocation {
    address_id: ProcessAddressAllocationId,
    owner: MemoryOwnerId,
    range: GuestRange,
}

#[derive(Debug, Clone)]
pub(super) struct ImageTlsThread {
    vector: TlsAllocation,
    blocks: BTreeMap<u32, TlsAllocation>,
}

#[derive(Debug, Default, Clone)]
pub(super) struct RuntimeImageTls {
    modules: BTreeMap<u32, ImageTlsModule>,
    threads: BTreeMap<RuntimeThreadId, ImageTlsThread>,
}

fn request(
    size: usize,
    alignment: u32,
) -> Result<ProcessAddressAllocationRequest, RuntimeStateError> {
    Ok(ProcessAddressAllocationRequest {
        purpose: ProcessAddressAllocationPurpose::ImageTls,
        preferred_address: None,
        search_range: RuntimeAddressRange::new(GuestAddress(0x6800_0000), 0x0800_0000)?,
        reserve_size: (size.max(1) as u64).div_ceil(GUEST_PAGE_SIZE) * GUEST_PAGE_SIZE,
        alignment: alignment.max(GUEST_PAGE_SIZE as u32),
        guard_pages: 0,
        classification: ProcessAddressAllocationClassification::ProcessEnvironment,
        release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
        executable: false,
    })
}

fn initialize(
    memory: &mut GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
    bytes: &[u8],
) -> Result<TlsAllocation, RuntimeStateError> {
    let range = GuestRange::new(allocation.range.base, allocation.range.byte_length)
        .map_err(RuntimeStateError::from_memory)?;
    let owner = memory
        .create_owner(MemoryOwnerKind::Runtime)
        .map_err(RuntimeStateError::from_memory)?;
    memory
        .reserve_fixed_for_address_allocation(
            allocation.id.0,
            owner,
            MemoryRegionKind::Anonymous,
            range.start(),
            range.byte_length(),
        )
        .map_err(RuntimeStateError::from_memory)?;
    memory
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
    let chunk_size = memory.limits().max_access_bytes;
    for (index, chunk) in bytes.chunks(chunk_size).enumerate() {
        memory
            .write_initialization(
                owner,
                GuestAddress(range.start().0 + (index * chunk_size) as u32),
                chunk,
            )
            .map_err(RuntimeStateError::from_memory)?;
    }
    memory
        .seal_owner(owner)
        .map_err(RuntimeStateError::from_memory)?;
    Ok(TlsAllocation {
        address_id: allocation.id,
        owner,
        range,
    })
}

fn release(memory: &mut GuestMemory, allocation: &TlsAllocation) -> Result<(), RuntimeStateError> {
    memory
        .release_region(allocation.owner, allocation.range.start())
        .map_err(RuntimeStateError::from_memory)?;
    memory
        .destroy_owner(allocation.owner)
        .map_err(RuntimeStateError::from_memory)?;
    Ok(())
}

impl RuntimeImageTls {
    pub(super) fn thread_requests(
        &self,
    ) -> Result<
        Vec<(
            Option<ProcessAddressAllocationId>,
            ProcessAddressAllocationRequest,
        )>,
        RuntimeStateError,
    > {
        if self.modules.is_empty() {
            return Ok(Vec::new());
        }
        let mut requests = vec![(None, request(TLS_VECTOR_BYTES as usize, 4)?)];
        for module in self.modules.values() {
            requests.push((None, request(module.template.len(), module.alignment)?));
        }
        Ok(requests)
    }

    pub(super) fn initialize_thread(
        &self,
        memory: &mut GuestMemory,
        allocations: &[ProcessAddressAllocationRecord],
        teb: GuestAddress,
    ) -> Result<Option<ImageTlsThread>, RuntimeStateError> {
        if self.modules.is_empty() && allocations.is_empty() {
            return Ok(None);
        }
        if allocations.len() != self.modules.len() + 1 {
            return Err(RuntimeStateError::new(
                "TLS thread allocations do not match loaded modules",
            ));
        }
        let vector = initialize(memory, &allocations[0], &[])?;
        let mut blocks = BTreeMap::new();
        for ((handle, module), allocation) in self.modules.iter().zip(&allocations[1..]) {
            let block = initialize(memory, allocation, &module.template)?;
            memory
                .write(
                    GuestAddress(vector.range.start().0 + module.index * 4),
                    &block.range.start().0.to_le_bytes(),
                )
                .map_err(RuntimeStateError::from_memory)?;
            blocks.insert(*handle, block);
        }
        memory
            .write(
                GuestAddress(teb.0 + TEB_TLS_POINTER),
                &vector.range.start().0.to_le_bytes(),
            )
            .map_err(RuntimeStateError::from_memory)?;
        Ok(Some(ImageTlsThread { vector, blocks }))
    }

    pub(super) fn publish_thread(&mut self, id: RuntimeThreadId, state: Option<ImageTlsThread>) {
        if let Some(state) = state {
            self.threads.insert(id, state);
        }
    }

    pub(super) fn register_module(
        &mut self,
        memory: &mut GuestMemory,
        allocator: &mut ProcessAddressSpaceAllocator,
        handle: u32,
        loader: &Pe32LoadPlan,
        image: &ImageMaterialization,
        tebs: &[(RuntimeThreadId, GuestAddress)],
    ) -> Result<(), RuntimeStateError> {
        let Some(tls) = &loader.tls else {
            return Ok(());
        };
        if self.modules.contains_key(&handle) {
            return Err(RuntimeStateError::new("TLS module is already registered"));
        }
        let index = (0..TLS_VECTOR_BYTES / 4)
            .find(|index| self.modules.values().all(|module| module.index != *index))
            .ok_or_else(|| RuntimeStateError::new("PE TLS index capacity is exhausted"))?;
        let base = image.layout.image_range.start().0;
        let mut template = Vec::with_capacity((tls.template_size + tls.zero_fill) as usize);
        let mut offset = 0;
        while offset < tls.template_size {
            let length = (tls.template_size - offset).min(memory.limits().max_access_bytes as u32);
            template.extend_from_slice(
                &memory
                    .read_owned_host(
                        GuestAddress(base + tls.template_rva.0 + offset),
                        length as usize,
                        image.layout.owner,
                    )
                    .map_err(RuntimeStateError::from_memory)?,
            );
            offset += length;
        }
        template.resize((tls.template_size + tls.zero_fill) as usize, 0);
        memory
            .write(GuestAddress(base + tls.index_rva.0), &index.to_le_bytes())
            .map_err(RuntimeStateError::from_memory)?;
        self.modules.insert(
            handle,
            ImageTlsModule {
                index,
                alignment: tls.alignment,
                template,
            },
        );
        for (id, teb) in tebs {
            if let Some(thread) = self.threads.get_mut(id) {
                let module = &self.modules[&handle];
                let allocation =
                    allocator.reserve(request(module.template.len(), module.alignment)?)?;
                let block = initialize(memory, &allocation, &module.template)?;
                let slot = GuestAddress(thread.vector.range.start().0 + index * 4);
                memory
                    .read_owned_host(slot, 4, thread.vector.owner)
                    .map_err(RuntimeStateError::from_memory)?;
                memory
                    .write(slot, &block.range.start().0.to_le_bytes())
                    .map_err(RuntimeStateError::from_memory)?;
                thread.blocks.insert(handle, block);
            } else {
                let requests = self.thread_requests()?;
                let state = allocator.reserve_batch_with(requests, |allocations| {
                    self.initialize_thread(memory, allocations, *teb)
                })?;
                self.publish_thread(*id, state);
            }
        }
        Ok(())
    }

    pub(super) fn remove_module(
        &mut self,
        memory: &mut GuestMemory,
        allocator: &mut ProcessAddressSpaceAllocator,
        handle: u32,
    ) -> Result<(), RuntimeStateError> {
        let Some(module) = self.modules.get(&handle) else {
            return Ok(());
        };
        for thread in self.threads.values_mut() {
            let block = thread
                .blocks
                .get(&handle)
                .ok_or_else(|| RuntimeStateError::new("TLS module lost a thread block"))?;
            allocator.release(block.address_id)?;
            release(memory, block)?;
            let slot = GuestAddress(thread.vector.range.start().0 + module.index * 4);
            memory
                .read_owned_host(slot, 4, thread.vector.owner)
                .map_err(RuntimeStateError::from_memory)?;
            memory
                .write(slot, &0_u32.to_le_bytes())
                .map_err(RuntimeStateError::from_memory)?;
            thread.blocks.remove(&handle);
        }
        self.modules.remove(&handle);
        Ok(())
    }

    pub(super) fn thread_allocation_ids(
        &self,
        id: RuntimeThreadId,
    ) -> Vec<ProcessAddressAllocationId> {
        self.threads
            .get(&id)
            .map(|thread| {
                std::iter::once(thread.vector.address_id)
                    .chain(thread.blocks.values().map(|block| block.address_id))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn release_thread_memory(
        &self,
        memory: &mut GuestMemory,
        id: RuntimeThreadId,
    ) -> Result<(), RuntimeStateError> {
        if let Some(thread) = self.threads.get(&id) {
            for block in thread.blocks.values() {
                release(memory, block)?;
            }
            release(memory, &thread.vector)?;
        }
        Ok(())
    }

    pub(super) fn retire_thread(&mut self, id: RuntimeThreadId) {
        self.threads.remove(&id);
    }
}

impl Runtime {
    pub(super) fn image_tls_tebs(
        &self,
    ) -> Result<Vec<(RuntimeThreadId, GuestAddress)>, RuntimeStateError> {
        let mut result = Vec::new();
        for thread in self.threads.states() {
            if matches!(thread.lifecycle, RuntimeThreadLifecycleState::Exited { .. }) {
                continue;
            }
            let teb = thread
                .environment
                .as_ref()
                .map(|env| env.teb_address)
                .or_else(|| {
                    self.process
                        .environment()
                        .map(|env| env.initial_teb_address)
                });
            if let Some(teb) = teb {
                if result.iter().any(|(_, existing)| *existing == teb) {
                    return Err(RuntimeStateError::new(
                        "live TLS threads must have distinct registered TEBs",
                    ));
                }
                result.push((thread.id, teb));
            }
        }
        Ok(result)
    }
}
