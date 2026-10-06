#[test]
fn graphics_resource_capacity_keeps_multiple_textures_and_reuses_released_mapping() {
    use crate::d3d9::{D3D9_D3DLOCK_DISCARD, D3D9_D3DUSAGE_DYNAMIC};
    let (mut runtime, device) = texture_discard_fixture();
    let mut roots = runtime
        .address_space_allocator
        .records()
        .filter(|record| record.parent_id.is_none())
        .collect::<Vec<_>>();
    roots.sort_by_key(|record| record.range.base.0);
    for pair in roots.windows(2) {
        assert!(pair[0].range.end_exclusive().unwrap() <= u64::from(pair[1].range.base.0));
    }
    let arena = runtime
        .address_space_allocator
        .allocation_for_purpose(&ProcessAddressAllocationPurpose::GraphicsResourceArena)
        .unwrap();
    assert!(
        arena.range.end_exclusive().unwrap() <= u64::from(RUNTIME_GUEST_THREAD_STACK_ARENA_BASE.0)
    );

    let mut textures = Vec::new();
    let mut backings = Vec::new();
    for value in 1..=6_u8 {
        assert_eq!(
            texture_discard_call(
                &mut runtime,
                &device,
                23,
                &[
                    1024,
                    1024,
                    1,
                    D3D9_D3DUSAGE_DYNAMIC,
                    D3D9_D3DFMT_X8R8G8B8,
                    0,
                    D3D9_FIXTURE_OUTPUT.0,
                    0,
                ]
            ),
            COM_S_OK.bits()
        );
        let texture = texture_discard_output_interface(&runtime);
        assert_eq!(
            texture_discard_call(
                &mut runtime,
                &texture,
                19,
                &[0, D3D9_FIXTURE_OUTPUT.0, 0, D3D9_D3DLOCK_DISCARD]
            ),
            COM_S_OK.bits()
        );
        let backing = runtime
            .graphics_texture_for_com_object(texture.object_id)
            .unwrap()
            .backing
            .unwrap();
        runtime
            .memory
            .write(backing.range.start(), &[value; 4])
            .unwrap();
        runtime
            .memory
            .write(
                GuestAddress(backing.range.start().0 + 4 * 1024 * 1024 - 4),
                &[value; 4],
            )
            .unwrap();
        assert_eq!(
            texture_discard_call(&mut runtime, &texture, 20, &[0]),
            COM_S_OK.bits()
        );
        assert_eq!(
            runtime
                .memory
                .read_owned(backing.range.start(), 4, backing.memory_owner)
                .unwrap(),
            [value; 4]
        );
        assert_eq!(
            runtime
                .memory
                .read_owned(
                    GuestAddress(backing.range.start().0 + 4 * 1024 * 1024 - 4),
                    4,
                    backing.memory_owner
                )
                .unwrap(),
            [value; 4]
        );
        textures.push(texture);
        backings.push(backing);
    }
    assert!(
        backings
            .iter()
            .map(|backing| backing.range.byte_length())
            .sum::<u64>()
            > 16 * 1024 * 1024
    );
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &device,
            26,
            &[
                112,
                D3D9_D3DUSAGE_WRITEONLY,
                0x144,
                D3D9_D3DPOOL_MANAGED,
                D3D9_FIXTURE_OUTPUT.0,
                0,
            ]
        ),
        COM_S_OK.bits()
    );
    let vertex = texture_discard_output_interface(&runtime);
    assert_eq!(
        texture_discard_call(&mut runtime, &vertex, 11, &[0, 0, D3D9_FIXTURE_OUTPUT.0, 0]),
        COM_S_OK.bits()
    );
    let vertex_bits = GuestAddress(u32::from_le_bytes(
        runtime
            .memory
            .read(D3D9_FIXTURE_OUTPUT, 4)
            .unwrap()
            .try_into()
            .unwrap(),
    ));
    runtime.memory.write(vertex_bits, &[0x7b; 112]).unwrap();
    assert_eq!(
        texture_discard_call(&mut runtime, &vertex, 12, &[]),
        COM_S_OK.bits()
    );

    assert_eq!(texture_discard_call(&mut runtime, &textures[0], 2, &[]), 0);
    let retired = backings[0];
    assert!(
        runtime
            .address_space_allocator
            .allocation(retired.address_allocation_id)
            .is_err()
    );
    assert!(
        runtime
            .memory
            .read_owned(retired.range.start(), 4, retired.memory_owner)
            .is_err()
    );
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &device,
            23,
            &[
                1024,
                1024,
                1,
                D3D9_D3DUSAGE_DYNAMIC,
                D3D9_D3DFMT_X8R8G8B8,
                0,
                D3D9_FIXTURE_OUTPUT.0,
                0,
            ]
        ),
        COM_S_OK.bits()
    );
    let replacement = texture_discard_output_interface(&runtime);
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &replacement,
            19,
            &[0, D3D9_FIXTURE_OUTPUT.0, 0, D3D9_D3DLOCK_DISCARD]
        ),
        COM_S_OK.bits()
    );
    let reused = runtime
        .graphics_texture_for_com_object(replacement.object_id)
        .unwrap()
        .backing
        .unwrap();
    assert_eq!(reused.range, retired.range);
    assert_ne!(reused.address_allocation_id, retired.address_allocation_id);
    assert_ne!(reused.memory_owner, retired.memory_owner);
    runtime
        .memory
        .write(reused.range.start(), &[0xa9; 4])
        .unwrap();
    assert_eq!(
        texture_discard_call(&mut runtime, &replacement, 20, &[0]),
        COM_S_OK.bits()
    );
    for (index, backing) in backings.iter().enumerate().skip(1) {
        assert_eq!(
            runtime
                .memory
                .read_owned(backing.range.start(), 4, backing.memory_owner)
                .unwrap(),
            [(index + 1) as u8; 4]
        );
    }
    assert_eq!(runtime.memory.read(vertex_bits, 112).unwrap(), [0x7b; 112]);
    assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
}

#[test]
fn graphics_resource_capacity_exhaustion_keeps_output_and_all_authority_for_retry() {
    use crate::d3d9::{D3D9_D3DLOCK_DISCARD, D3D9_D3DUSAGE_DYNAMIC};
    let (mut runtime, device) = texture_discard_fixture();
    let texture = texture_discard_create(&mut runtime, &device, D3D9_D3DUSAGE_DYNAMIC, 0);
    let arena = runtime
        .address_space_allocator
        .allocation_for_purpose(&ProcessAddressAllocationPurpose::GraphicsResourceArena)
        .unwrap()
        .clone();
    // Reserve address capacity without committing an arena-sized physical buffer.
    let occupied = runtime
        .address_space_allocator
        .try_reserve_batch_with(
            vec![(
                Some(arena.id),
                ProcessAddressAllocationRequest {
                    purpose: ProcessAddressAllocationPurpose::GraphicsResource,
                    preferred_address: Some(arena.range.base),
                    search_range: arena.range,
                    reserve_size: arena.range.byte_length,
                    alignment: GUEST_PAGE_SIZE as u32,
                    guard_pages: 0,
                    classification: ProcessAddressAllocationClassification::Graphics,
                    release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
                    executable: false,
                },
            )],
            |allocations| Ok(Some(allocations[0].clone())),
        )
        .unwrap()
        .unwrap();
    write_d3d9_fixture_words(
        &mut runtime.memory,
        D3D9_FIXTURE_OUTPUT,
        &[0xcccc_cccc, 0xdddd_dddd],
    );
    texture_discard_rejected(
        &mut runtime,
        &texture,
        &[0, D3D9_FIXTURE_OUTPUT.0, 0, D3D9_D3DLOCK_DISCARD],
        true,
    );
    assert!(
        runtime
            .graphics_texture_for_com_object(texture.object_id)
            .unwrap()
            .backing
            .is_none()
    );
    runtime
        .address_space_allocator
        .release(occupied.id)
        .unwrap();
    assert_eq!(
        texture_discard_call(
            &mut runtime,
            &texture,
            19,
            &[0, D3D9_FIXTURE_OUTPUT.0, 0, D3D9_D3DLOCK_DISCARD]
        ),
        COM_S_OK.bits()
    );
    let backing = runtime
        .graphics_texture_for_com_object(texture.object_id)
        .unwrap()
        .backing
        .unwrap();
    assert_eq!(backing.range.start(), arena.range.base);
    runtime
        .memory
        .write(backing.range.start(), &[0x5c; 24])
        .unwrap();
    assert_eq!(
        texture_discard_call(&mut runtime, &texture, 20, &[0]),
        COM_S_OK.bits()
    );
    assert_eq!(
        runtime
            .memory
            .read_owned(backing.range.start(), 24, backing.memory_owner)
            .unwrap(),
        [0x5c; 24]
    );
    assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
}
