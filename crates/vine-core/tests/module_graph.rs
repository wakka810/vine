use vine_core::{
    RuntimeModuleClass, RuntimeModuleGraph, RuntimeModuleId, RuntimeModuleLoadState,
    RuntimeModuleReferenceOwner, RuntimeModuleRegistration,
};

#[test]
fn module_graph_orders_dependencies_and_owns_reference_lifecycle() {
    let mut graph = RuntimeModuleGraph::default();
    let main = register(
        &mut graph,
        0x4d01_0001,
        "GAME.EXE",
        RuntimeModuleClass::MainExecutable,
    );
    let engine = register(
        &mut graph,
        0x4d01_0002,
        "ENGINE.DLL",
        RuntimeModuleClass::GuestDynamicLibrary,
    );
    let kernel32 = register(
        &mut graph,
        0x4d01_0003,
        "KERNEL32.DLL",
        RuntimeModuleClass::RuntimeSystemModule,
    );
    let codec = register(
        &mut graph,
        0x4d01_0004,
        "CODEC.DLL",
        RuntimeModuleClass::GuestDynamicLibrary,
    );

    assert_eq!(graph.main_module(), Some(main));
    assert_eq!(
        graph
            .module_by_name("kernel32.DLL")
            .expect("case-insensitive lookup")
            .id,
        kernel32
    );
    assert_eq!(
        graph
            .register_module(RuntimeModuleRegistration {
                handle: 0x4d01_0005,
                requested_name: "Kernel32.dll".to_owned(),
                canonical_name: "Kernel32.dll".to_owned(),
                class: RuntimeModuleClass::RuntimeSystemModule,
            })
            .expect_err("canonical module names are unique")
            .code,
        "module-identity-duplicate"
    );
    assert_eq!(
        graph
            .register_module(RuntimeModuleRegistration {
                handle: 0x4d01_0005,
                requested_name: "wrong-class.dll".to_owned(),
                canonical_name: "wrong-class.dll".to_owned(),
                class: RuntimeModuleClass::MainExecutable,
            })
            .expect_err("main executable identity requires an exe basename")
            .code,
        "module-canonical-extension-invalid"
    );
    assert_eq!(
        graph
            .module_by_name("C:\\Windows\\kernel32.dll")
            .expect_err("canonical lookup rejects paths")
            .code,
        "module-canonical-name-invalid"
    );

    graph.add_dependency(main, engine).expect("main -> engine");
    graph
        .add_dependency(main, kernel32)
        .expect("main -> kernel32");
    graph
        .add_dependency(engine, codec)
        .expect("engine -> codec");
    graph
        .add_dependency(engine, kernel32)
        .expect("engine -> kernel32");
    assert_eq!(
        graph
            .module(engine)
            .expect("engine node")
            .reference_count_for(&RuntimeModuleReferenceOwner::StaticImport { requester: main }),
        1
    );
    assert_eq!(graph.module(kernel32).unwrap().reference_count(), 3);
    assert_eq!(graph.module(codec).unwrap().reference_count(), 1);

    let edges_before = graph
        .nodes()
        .iter()
        .map(|(id, node)| (*id, node.dependencies.clone(), node.reference_count()))
        .collect::<Vec<_>>();
    assert_eq!(
        graph
            .add_dependency(codec, main)
            .expect_err("cycle is rejected")
            .code,
        "module-dependency-cycle"
    );
    assert_eq!(
        graph
            .nodes()
            .iter()
            .map(|(id, node)| (*id, node.dependencies.clone(), node.reference_count()))
            .collect::<Vec<_>>(),
        edges_before
    );

    assert_eq!(graph.add_dynamic_reference(codec, 8192).unwrap(), 1);
    assert_eq!(graph.add_dynamic_reference(codec, 8192).unwrap(), 2);
    assert_eq!(graph.release_dynamic_reference(codec, 8192).unwrap(), 1);
    assert_eq!(graph.release_dynamic_reference(codec, 8192).unwrap(), 0);
    assert_eq!(
        graph
            .release_dynamic_reference(codec, 8192)
            .expect_err("released dynamic owner cannot underflow")
            .code,
        "module-reference-unknown"
    );
    assert_eq!(graph.module(codec).unwrap().reference_count(), 1);

    for id in [engine, kernel32, codec] {
        graph.mark_ready(id).expect("dependency ready");
    }
    let states_before_failed_attach = graph
        .nodes()
        .iter()
        .map(|(id, node)| (*id, node.state))
        .collect::<Vec<_>>();
    assert_eq!(
        graph.attach_all().expect_err("main is not ready").code,
        "module-attach-state-invalid"
    );
    assert_eq!(
        graph
            .nodes()
            .iter()
            .map(|(id, node)| (*id, node.state))
            .collect::<Vec<_>>(),
        states_before_failed_attach
    );
    graph.mark_ready(main).expect("main ready");

    let attach = graph.attach_all().expect("dependency-first attach order");
    assert_eq!(attach, vec![codec, kernel32, engine, main]);
    for (index, id) in attach.iter().enumerate() {
        assert_eq!(
            graph.module(*id).unwrap().state,
            RuntimeModuleLoadState::ProcessAttached {
                sequence: u32::try_from(index + 1).unwrap()
            }
        );
    }
    let detach = graph.detach_all().expect("reverse detach order");
    assert_eq!(detach, vec![main, engine, kernel32, codec]);
    for (index, id) in detach.iter().enumerate() {
        assert_eq!(
            graph.module(*id).unwrap().state,
            RuntimeModuleLoadState::ProcessDetached {
                sequence: u32::try_from(index + 1).unwrap()
            }
        );
    }

    let mut rollback = RuntimeModuleGraph::default();
    let orphan = register(
        &mut rollback,
        0x4d01_0001,
        "ORPHAN.DLL",
        RuntimeModuleClass::GuestDynamicLibrary,
    );
    rollback
        .rollback_registration(orphan)
        .expect("unlinked registration rollback");
    assert_eq!(
        register(
            &mut rollback,
            0x4d01_0001,
            "RETRY.DLL",
            RuntimeModuleClass::GuestDynamicLibrary,
        ),
        orphan
    );
    assert_eq!(
        rollback
            .module_by_name("orphan.dll")
            .expect_err("rolled-back module is absent")
            .code,
        "module-name-unknown"
    );
}

fn register(
    graph: &mut RuntimeModuleGraph,
    handle: u32,
    name: &str,
    class: RuntimeModuleClass,
) -> RuntimeModuleId {
    graph
        .register_module(RuntimeModuleRegistration {
            handle,
            requested_name: name.to_owned(),
            canonical_name: name.to_owned(),
            class,
        })
        .expect("register module")
}
