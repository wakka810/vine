typedef unsigned long HANDLE;
typedef unsigned long (__stdcall *thread_entry)(void *);
__declspec(dllimport) HANDLE __stdcall GetCurrentThread(void);
__declspec(dllimport) unsigned long __stdcall SuspendThread(HANDLE thread);
__declspec(dllimport) unsigned long __stdcall ResumeThread(HANDLE thread);
__declspec(dllimport) HANDLE __stdcall CreateThread(void *security, unsigned long stack_size,
    thread_entry entry, void *parameter, unsigned long flags, unsigned long *thread_id);
__declspec(dllimport) HANDLE __stdcall CreateWaitableTimerA(void *security, int manual_reset, const char *name);
__declspec(dllimport) unsigned long __stdcall WaitForSingleObjectEx(HANDLE handle, unsigned long timeout, int alertable);
__declspec(dllimport) unsigned long __stdcall WaitForSingleObject(HANDLE handle, unsigned long timeout);
__declspec(dllimport) HANDLE __stdcall CreateEventA(void *security, int manual_reset, int initial_state, const char *name);
__declspec(dllimport) int __stdcall SetEvent(HANDLE event);
__declspec(dllimport) int __stdcall SetWaitableTimer(HANDLE timer, const long long *due_time,
    long period, void *routine, void *context, int resume);
__declspec(dllimport) void __stdcall Sleep(unsigned long milliseconds);
__declspec(dllimport) int __stdcall CloseHandle(HANDLE object);
static volatile unsigned long started, main_ran, completed, worker_error, phase;
static HANDLE timer;
static volatile unsigned long idle_started, idle_completed;
static HANDLE idle_event;
static HANDLE finite_event, finite_gate;
static volatile unsigned long finite_started;

static unsigned long __stdcall finite_worker(void *unused) {
  (void)unused;
  finite_started = 1;
  if (WaitForSingleObject(finite_gate, ~0ul) != 0) return 41;
  Sleep(5);
  if (!SetEvent(finite_event)) return 42;
  Sleep(5);
  return 0;
}

static int finite_object_waits(int manual) {
  unsigned long id, attempts = 0;
  finite_started = 0;
  finite_event = CreateEventA(0, manual, 0, 0);
  finite_gate = CreateEventA(0, 1, 0, 0);
  if (!finite_event || !finite_gate) return 40;
  /* No signal exists: this tests a real browser timeout without consuming an
     event or reporting success merely because the host timer completed. */
  if (WaitForSingleObject(finite_event, 1) != 258u) return 43;
  HANDLE thread = CreateThread(0, 0, finite_worker, 0, 0, &id);
  if (!thread || !id) return 44;
  while (!finite_started && ++attempts < 100000) Sleep(0);
  if (!finite_started) return 45;
  /* The gate keeps the thread alive regardless of browser scheduling speed. */
  if (WaitForSingleObject(thread, 1) != 258u) return 46;
  if (!SetEvent(finite_gate)) return 47;
  if (WaitForSingleObject(finite_event, 1000) != 0) return 48;
  if (WaitForSingleObject(finite_event, 0) != (manual ? 0u : 258u)) return 49;
  if (WaitForSingleObject(thread, 1000) != 0 ||
      WaitForSingleObject(thread, 1) != 0) return 50;
  if (!CloseHandle(thread) || !CloseHandle(finite_event) || !CloseHandle(finite_gate)) return 51;
  return 0;
}

static unsigned long __stdcall idle_worker(void *parameter) {
  const unsigned long bit = (unsigned long)parameter;
  idle_started |= bit;
  Sleep(bit == 1 ? 200 : 300);
  idle_completed |= bit;
  if (bit == 1 && idle_event) {
    if (!SetEvent(idle_event)) return 1;
    Sleep(50);
  }
  return 0;
}

static int wait_with_idle_workers(int event_wait) {
  HANDLE threads[2];
  unsigned long id, attempts = 0;
  idle_started = 0; idle_completed = 0;
  idle_event = event_wait ? CreateEventA(0, 0, 0, 0) : 0;
  if (event_wait && !idle_event) return 30;
  threads[0] = CreateThread(0, 0, idle_worker, (void *)1, 0, &id);
  threads[1] = CreateThread(0, 0, idle_worker, (void *)2, 0, &id);
  if (!threads[0] || !threads[1]) return 31;
  while (idle_started != 3 && ++attempts < 100000) Sleep(0);
  if (idle_started != 3 || idle_completed != 0) return 32;
  /* Both workers are waiting for real browser Sleep completions. The main
     thread must park its post-call CPU context until this object is signaled. */
  if (WaitForSingleObject(event_wait ? idle_event : threads[0], ~0ul) != 0 ||
      !(idle_completed & 1)) return 33;
  if (WaitForSingleObject(threads[0], ~0ul) != 0 ||
      WaitForSingleObject(threads[1], ~0ul) != 0 || idle_completed != 3) return 34;
  if (!CloseHandle(threads[0]) || !CloseHandle(threads[1]) ||
      (event_wait && !CloseHandle(idle_event))) return 35;
  return 0;
}

static unsigned long __stdcall worker(void *unused) {
  (void)unused;
  started = 1;
  if (phase == 0) Sleep(5);
  if (phase == 0 && !main_ran) worker_error = 3;
  if (WaitForSingleObjectEx(timer, phase == 0 ? 20 : 100, 1) != (phase == 0 ? 258u : 0u)) worker_error = 1;
  if (!main_ran) worker_error = 2;
  completed = phase + 1;
  return 0;
}

static volatile unsigned long suspension_stage;
static HANDLE suspension_event;
static unsigned long __stdcall suspension_worker(void *parameter) {
  const unsigned long mode = (unsigned long)parameter;
  suspension_stage = 1;
  if (mode == 1) {
    if (SuspendThread(GetCurrentThread()) != 0) return 1;
  } else if (mode == 2) {
    if (WaitForSingleObject(suspension_event, ~0ul) != 0) return 2;
  } else {
    Sleep(30);
  }
  suspension_stage = 2;
  return 0;
}

static int suspension_round_trip(int mode) {
  unsigned long id, attempts = 0;
  suspension_stage = 0;
  suspension_event = mode == 2 ? CreateEventA(0, 0, 0, 0) : 0;
  if (mode == 2 && !suspension_event) return 69;
  HANDLE thread = CreateThread(0, 0, suspension_worker, (void *)(unsigned long)mode, 0, &id);
  if (!thread) return 60;
  while (!suspension_stage && ++attempts < 100000) Sleep(0);
  if (suspension_stage != 1) return 61;
  if (mode != 1 && (SuspendThread(thread) != 0 || SuspendThread(thread) != 1)) return 62;
  if (mode == 2 && !SetEvent(suspension_event)) return 70;
  /* The browser Sleep completion must not execute a suspended continuation. */
  Sleep(60);
  if (suspension_stage != 1) return 63;
  if (mode != 1) {
    if (ResumeThread(thread) != 2) return 64;
    Sleep(5);
    if (suspension_stage != 1) return 65;
  }
  if (ResumeThread(thread) != 1) return 66;
  if (WaitForSingleObject(thread, 1000) != 0 || suspension_stage != 2) return 67;
  if (!CloseHandle(thread) || (suspension_event && !CloseHandle(suspension_event))) return 68;
  return 0;
}

int __cdecl fixture_entry(void) {
  HANDLE thread;
  unsigned long id = 0;
  unsigned long attempts = 0;
  timer = CreateWaitableTimerA(0, 1, 0);
  if (!timer) return 1;
  thread = CreateThread(0, 0, worker, 0, 0, &id);
  if (!thread || !id) return 2;
  while (!started && ++attempts < 10000) Sleep(0);
  if (!started) return 3;
  main_ran = 1;
  /* The worker's Sleep completes while this thread is waiting on the same timer. */
  if (WaitForSingleObjectEx(timer, 20, 1) != 258u) return 8;
  attempts = 0;
  while (completed != 1 && ++attempts < 100000) Sleep(0);
  if (completed != 1 || worker_error) return 4 + (int)worker_error;
  if (!CloseHandle(thread)) return 7;
  /* An armed manual-reset timer must satisfy two concurrent host-paced waits.
     Completion of the first request must not invalidate the other's arm identity. */
  const long long due_time = -500000;
  phase = 1; started = 0; main_ran = 0; completed = 0;
  if (!SetWaitableTimer(timer, &due_time, 0, 0, 0, 0)) return 9;
  thread = CreateThread(0, 0, worker, 0, 0, &id);
  if (!thread || !id) return 10;
  while (!started && ++attempts < 110000) Sleep(0);
  if (!started) return 11;
  main_ran = 1;
  Sleep(1);
  if (WaitForSingleObjectEx(timer, 100, 1) != 0) return 12;
  attempts = 0;
  while (completed != 2 && ++attempts < 100000) Sleep(0);
  if (completed != 2) return 13;
  if (worker_error) return 20 + (int)worker_error;
  if (WaitForSingleObjectEx(timer, 0, 1) != 0) return 14;
  if (!CloseHandle(thread) || !CloseHandle(timer)) return 14;
  const int join_result = wait_with_idle_workers(0);
  if (join_result) return join_result;
  const int event_result = wait_with_idle_workers(1);
  if (event_result) return event_result;
  const int finite_auto = finite_object_waits(0);
  if (finite_auto) return finite_auto;
  const int finite_manual = finite_object_waits(1);
  if (finite_manual) return finite_manual;
  const int suspended_wait = suspension_round_trip(0);
  if (suspended_wait) return suspended_wait;
  const int suspended_self = suspension_round_trip(1);
  if (suspended_self) return suspended_self;
  const int suspended_event = suspension_round_trip(2);
  if (suspended_event) return suspended_event;
  return 0;
}
