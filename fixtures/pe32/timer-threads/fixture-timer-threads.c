typedef unsigned long HANDLE;
typedef unsigned long (__stdcall *thread_entry)(void *);
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
  return 0;
}
