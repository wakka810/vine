typedef unsigned long HANDLE;
typedef unsigned long (__stdcall *thread_entry)(void *);
__declspec(dllimport) HANDLE __stdcall CreateThread(void *security, unsigned long stack_size,
    thread_entry entry, void *parameter, unsigned long flags, unsigned long *thread_id);
__declspec(dllimport) unsigned long __stdcall WaitForSingleObject(HANDLE handle, unsigned long timeout);
__declspec(dllimport) HANDLE __stdcall CreateWaitableTimerA(void *security, int manual_reset, const char *name);
__declspec(dllimport) int __stdcall SetWaitableTimer(HANDLE timer, const long long *due_time,
    long period, void *routine, void *context, int resume);
__declspec(dllimport) unsigned long __stdcall WaitForSingleObjectEx(HANDLE timer,
    unsigned long timeout, int alertable);
__declspec(dllimport) unsigned long __stdcall GetLastError(void);
__declspec(dllimport) unsigned long __stdcall GetTickCount(void);
__declspec(dllimport) void __stdcall SetLastError(unsigned long value);
__declspec(dllimport) int __stdcall CloseHandle(HANDLE object);
__declspec(dllimport) void __stdcall Sleep(unsigned long milliseconds);

static HANDLE shared_timer;
static volatile unsigned long waiter_ready, waiter_finished, waiter_error;
static unsigned long __stdcall timer_waiter(void *unused) {
  (void)unused;
  SetLastError(0x13579u);
  waiter_ready = 1;
  if (WaitForSingleObjectEx(shared_timer, 2000, 1) != 0 || GetLastError() != 0x13579u)
    waiter_error = 1;
  waiter_finished = 1;
  return 0;
}

int __cdecl fixture_entry(void) {
  const long long due_time = -200000; /* one-shot relative 20 ms */
  HANDLE timer = CreateWaitableTimerA(0, 1, 0);
  HANDLE unarmed = CreateWaitableTimerA(0, 1, 0);
  int result = 0;
  unsigned long started;
  if (!timer || !unarmed) return 1;
  Sleep(1); /* request ID differs from the later wait registration ID */
  if (WaitForSingleObjectEx(unarmed, 0, 1) != 258u) result = 2;
  if (!result && !SetWaitableTimer(timer, &due_time, 0, 0, 0, 0)) result = 3;
  SetLastError(0x4321u);
  started = GetTickCount();
  if (!result && WaitForSingleObjectEx(timer, 40, 1) != 0) result = 4;
  if (!result && GetTickCount() - started < 18u) result = 9;
  if (!result && (GetLastError() != 0x4321u || WaitForSingleObjectEx(timer, 0, 1) != 0)) result = 5;
  if (!result && (WaitForSingleObjectEx(unarmed, 2, 1) != 258u || GetLastError() != 0x4321u)) result = 6;
  if (!result && (WaitForSingleObjectEx(0, 0, 1) != 0xffffffffu || GetLastError() != 6u)) result = 7;
  /* The original real browser wake must not complete a rearmed pending wait. */
  if (!result) {
    const long long first_due = -2000000, rearmed_due = -4000000;
    unsigned long thread_id = 0, attempts = 0;
    shared_timer = timer;
    if (!SetWaitableTimer(timer, &first_due, 0, 0, 0, 0)) return 10;
    HANDLE thread = CreateThread(0, 0, timer_waiter, 0, 0, &thread_id);
    if (!thread || !thread_id) return 11;
    while (!waiter_ready && ++attempts < 1000) Sleep(1);
    Sleep(10);
    if (!waiter_ready || waiter_finished) return 12;
    started = GetTickCount();
    if (!SetWaitableTimer(timer, &rearmed_due, 0, 0, 0, 0)) return 13;
    Sleep(250);
    if (waiter_finished || WaitForSingleObject(thread, 0) != 258u) return 14;
    if (WaitForSingleObject(thread, ~0ul) != 0 || !waiter_finished || waiter_error ||
        GetTickCount() - started < 390u) return 15;
    if (!CloseHandle(thread)) return 16;
  }
  if (!CloseHandle(timer) || !CloseHandle(unarmed)) result = 8;
  return result;
}
