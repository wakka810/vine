mod support;
use support::{Fixture, OUTPUT, RETURN};
use vine_core::{
    DispatchOutcome, GuestAddress, ProcessAddressAllocationPurpose, complete_user32_message_wait,
    install_runtime_return_sentinels,
};

const K: &str = "kernel32.dll";
const U: &str = "user32.dll";

#[test]
fn message_wait_consumes_auto_reset_events_and_finishes_a_host_timeout() {
    let mut f = Fixture::new(&[
        (K, "CreateEventA"),
        (K, "WaitForSingleObject"),
        (U, "MsgWaitForMultipleObjects"),
    ]);
    let event = f.call(K, "CreateEventA", &[0, 0, 1, 0]).eax.unwrap();
    f.words(OUTPUT, &[event]);
    assert_eq!(
        f.call(U, "MsgWaitForMultipleObjects", &[1, OUTPUT.0, 0, 0, 0xbf])
            .eax,
        Some(0)
    );
    assert_eq!(f.call(K, "WaitForSingleObject", &[event, 0]).eax, Some(258));
    let DispatchOutcome::Deferred(call) =
        f.dispatch(U, "MsgWaitForMultipleObjects", &[1, OUTPUT.0, 0, 5, 0xbf])
    else {
        panic!("expected host wait")
    };
    assert!(matches!(
        complete_user32_message_wait(&f.dispatcher, &call, 0, &mut f.runtime),
        DispatchOutcome::Fault(_)
    ));
    f.runtime
        .synchronize_scheduled_clock_for_host_execution(0, 100_000)
        .unwrap();
    let outcome = complete_user32_message_wait(&f.dispatcher, &call, 1, &mut f.runtime);
    let DispatchOutcome::Immediate(done) = outcome else {
        panic!("{outcome:?}")
    };
    assert_eq!(done.eax, Some(258));
    assert!(matches!(
        complete_user32_message_wait(&f.dispatcher, &call, 1, &mut f.runtime),
        DispatchOutcome::Fault(_)
    ));
}

#[test]
fn multimedia_timer_owns_an_executable_worker_and_cancel_is_idempotent_failure() {
    let mut f = Fixture::new(&[
        ("winmm.dll", "timeSetEvent"),
        ("winmm.dll", "timeKillEvent"),
        (K, "Sleep"),
    ]);
    let allocation = f
        .runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&ProcessAddressAllocationPurpose::CallbackReturns)
        .unwrap()
        .clone();
    let sentinels = install_runtime_return_sentinels(f.runtime.memory_mut(), &allocation).unwrap();
    f.runtime
        .install_initial_thread_stack(&sentinels.process_entry_return, 0x100000, 0x10000)
        .unwrap();
    f.runtime
        .install_process_environment("fixture.exe")
        .unwrap();
    f.ack();
    let result = f.call("winmm.dll", "timeSetEvent", &[10, 1, RETURN.0, 0x1234, 1]);
    let id = result.eax.unwrap();
    assert_ne!(id, 0);
    assert_eq!(f.call("winmm.dll", "timeKillEvent", &[id]).eax, Some(0));
    assert_eq!(f.call("winmm.dll", "timeKillEvent", &[id]).eax, Some(97));
    assert_eq!(
        f.call("winmm.dll", "timeKillEvent", &[id + 1]).eax,
        Some(97)
    );
    assert_eq!(f.read(GuestAddress(RETURN.0), 1), [0xc3]);
}
