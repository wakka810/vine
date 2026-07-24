use serde::Serialize;

use crate::memory::{GUEST_PAGE_SIZE, GuestAddress, MemoryOwnerId, PageProtection};

use super::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationId,
    ProcessAddressAllocationLifetime, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationReleasePolicy, ProcessAddressAllocationRequest, RuntimeAddressRange,
    RuntimeStateError,
};

pub const RUNTIME_VIRTUAL_MEMORY_ADDRESS_OWNER: &str = "runtime-virtual-memory";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeVirtualMemoryReserveRequest {
    pub preferred_address: Option<GuestAddress>,
    pub byte_length: u32,
    pub minimum_application_address: GuestAddress,
    pub maximum_application_address: GuestAddress,
    pub allocation_granularity: u32,
    pub top_down: bool,
    pub protection: PageProtection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeVirtualMemoryReservation {
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub range: RuntimeAddressRange,
    pub usable_byte_length: u32,
    pub top_down: bool,
    pub protection: PageProtection,
}

impl RuntimeVirtualMemoryReserveRequest {
    pub(crate) fn address_request(
        self,
    ) -> Result<ProcessAddressAllocationRequest, RuntimeStateError> {
        if self.byte_length == 0 {
            return Err(virtual_memory_error(
                "virtual-memory-size-zero",
                "virtual memory reservation size must be nonzero",
            ));
        }
        if self.allocation_granularity < GUEST_PAGE_SIZE as u32
            || !self.allocation_granularity.is_power_of_two()
        {
            return Err(virtual_memory_error(
                "virtual-memory-allocation-granularity-invalid",
                "virtual memory allocation granularity must be a page-sized or larger power of two",
            ));
        }
        let maximum_exclusive = u64::from(self.maximum_application_address.0)
            .checked_add(1)
            .ok_or_else(|| {
                virtual_memory_error(
                    "virtual-memory-search-range-overflow",
                    "virtual memory maximum application address overflowed",
                )
            })?;
        let minimum = u64::from(self.minimum_application_address.0);
        let search_byte_length = maximum_exclusive.checked_sub(minimum).ok_or_else(|| {
            virtual_memory_error(
                "virtual-memory-search-range-invalid",
                "virtual memory application range is reversed or empty",
            )
        })?;
        let search_range =
            RuntimeAddressRange::new(self.minimum_application_address, search_byte_length)?;
        if let Some(preferred_address) = self.preferred_address
            && preferred_address.0 % self.allocation_granularity != 0
        {
            return Err(virtual_memory_error(
                "virtual-memory-preferred-address-unaligned",
                "virtual memory preferred address must satisfy allocation granularity",
            ));
        }
        Ok(ProcessAddressAllocationRequest {
            purpose: ProcessAddressAllocationPurpose::VirtualMemory,
            owner: RUNTIME_VIRTUAL_MEMORY_ADDRESS_OWNER.to_owned(),
            preferred_address: self.preferred_address,
            search_range,
            reserve_size: u64::from(self.byte_length),
            alignment: self.allocation_granularity,
            guard_pages: 0,
            classification: ProcessAddressAllocationClassification::VirtualMemory,
            lifetime: ProcessAddressAllocationLifetime::Process,
            release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
            executable: self.protection.execute,
        })
    }
}

fn virtual_memory_error(code: &'static str, message: impl Into<String>) -> RuntimeStateError {
    RuntimeStateError::invalid_input("runtime-virtual-memory", code, message)
}
