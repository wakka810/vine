use crate::memory::{GuestAddress, GuestMemory};
use crate::runtime::{RuntimeHeapInitialization, RuntimeStateError};

pub(crate) fn initialize_allocation(
    memory: &mut GuestMemory,
    address: GuestAddress,
    byte_length: u32,
    initialization: RuntimeHeapInitialization,
) -> Result<(), RuntimeStateError> {
    let byte_length = usize::try_from(byte_length).map_err(|_| {
        RuntimeStateError::new("private heap allocation size does not fit host accounting")
    })?;
    if initialization == RuntimeHeapInitialization::Uninitialized {
        return memory
            .validate_host_write(address, byte_length)
            .map_err(RuntimeStateError::from_memory);
    }
    let bytes = vec![0; byte_length];
    memory
        .write(address, &bytes)
        .map_err(RuntimeStateError::from_memory)?;
    Ok(())
}

#[cfg(test)]
#[path = "heap_backing_tests.rs"]
mod backing_tests;

#[cfg(test)]
#[path = "heap_materialization_tests.rs"]
mod materialization_tests;
