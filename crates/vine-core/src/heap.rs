use crate::memory::{GuestAddress, GuestMemory};
use crate::runtime::RuntimeStateError;

pub(crate) fn fill_allocation(
    memory: &mut GuestMemory,
    address: GuestAddress,
    byte_length: u32,
    fill_byte: Option<u8>,
) -> Result<(), RuntimeStateError> {
    let byte_length = usize::try_from(byte_length).map_err(|_| {
        RuntimeStateError::new("private heap allocation size does not fit host accounting")
    })?;
    let Some(fill_byte) = fill_byte else {
        return memory
            .validate_host_write(address, byte_length)
            .map_err(RuntimeStateError::from_memory);
    };
    let bytes = vec![fill_byte; byte_length];
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
