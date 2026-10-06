typedef int BOOL;
typedef unsigned long DWORD;
typedef void *HANDLE;
typedef DWORD(__stdcall *THREAD_START_ROUTINE)(void *parameter);

enum {
  ERROR_SUCCESS = 0,
  ERROR_INVALID_PARAMETER = 87,
  WAIT_OBJECT_0 = 0,
};

#define FLS_OUT_OF_INDEXES ((DWORD)0xffffffffUL)
#define MAIN_VALUE ((void *)0x12345678UL)
#define WORKER_VALUE ((void *)0x23456789UL)

__declspec(dllimport) BOOL __stdcall CloseHandle(HANDLE object);
__declspec(dllimport) DWORD __stdcall GetCurrentProcessId(void);
__declspec(dllimport) DWORD __stdcall GetCurrentThreadId(void);
__declspec(dllimport) HANDLE __stdcall GetCurrentThread(void);
__declspec(dllimport) HANDLE __stdcall CreateThread(
    void *attributes, DWORD stack_size, THREAD_START_ROUTINE start,
    void *parameter, DWORD creation_flags, DWORD *thread_id);
#ifdef VINE_FIXTURE_EXIT_THREAD
__declspec(dllimport) void __stdcall ExitThread(DWORD exit_code);
#endif
__declspec(dllimport) DWORD __stdcall FlsAlloc(void *callback);
__declspec(dllimport) BOOL __stdcall FlsFree(DWORD index);
__declspec(dllimport) void *__stdcall FlsGetValue(DWORD index);
__declspec(dllimport) BOOL __stdcall FlsSetValue(DWORD index, void *value);
__declspec(dllimport) BOOL __stdcall GetExitCodeThread(HANDLE thread,
                                                       DWORD *exit_code);
__declspec(dllimport) DWORD __stdcall GetLastError(void);
__declspec(dllimport) void __stdcall SetLastError(DWORD error);
__declspec(dllimport) DWORD __stdcall WaitForSingleObject(HANDLE object,
                                                          DWORD milliseconds);

typedef struct {
  DWORD index;
  void *before_value;
  DWORD before_last_error;
  BOOL set_result;
  void *after_value;
  DWORD after_last_error;
  DWORD process_id;
  DWORD thread_id;
  HANDLE pseudo_thread;
} WORKER_OBSERVATION;

static WORKER_OBSERVATION worker_observation;

#ifdef VINE_FIXTURE_EXIT_THREAD
__attribute__((noinline)) static DWORD grow_stack(DWORD depth) {
  volatile unsigned char scratch[3072];
  volatile DWORD index;
  for (index = 0; index < 3072; index += 1024) {
    scratch[index] = (unsigned char)depth;
  }
  if (depth == 0) return (DWORD)(scratch[0] - scratch[2048]);
  return grow_stack(depth - 1) + (DWORD)(scratch[0] - scratch[2048]);
}
#endif

static DWORD __stdcall worker_entry(void *parameter) {
  WORKER_OBSERVATION *observation = (WORKER_OBSERVATION *)parameter;
  SetLastError(88);
  observation->process_id = GetCurrentProcessId();
  observation->thread_id = GetCurrentThreadId();
  observation->pseudo_thread = GetCurrentThread();
  if (observation->pseudo_thread != (HANDLE)0xfffffffeUL ||
      observation->thread_id == 0 || observation->process_id == 0 ||
      GetLastError() != 88) return 7;
  observation->before_value = FlsGetValue(observation->index);
  observation->before_last_error = GetLastError();
  observation->set_result = FlsSetValue(observation->index, WORKER_VALUE);
  SetLastError(89);
  observation->after_value = FlsGetValue(observation->index);
  observation->after_last_error = GetLastError();
  if (observation->before_value != (void *)0) {
    return 1;
  }
  if (observation->before_last_error != ERROR_SUCCESS) {
    return 2;
  }
  if (observation->set_result == 0) {
    return 3;
  }
  if (observation->after_value != WORKER_VALUE) {
    return 4;
  }
  DWORD result = observation->after_last_error == ERROR_SUCCESS ? 0 : 5;
#ifdef VINE_FIXTURE_EXIT_THREAD
  if (grow_stack(3) != 0) result = 6;
  ExitThread(result);
#endif
  return result;
}

int __cdecl fixture_entry(void) {
  DWORD index;
  DWORD unset_index = FLS_OUT_OF_INDEXES;
  DWORD thread_id = 0;
  DWORD worker_exit_code = 0xffffffffUL;
  DWORD process_id = GetCurrentProcessId();
  DWORD main_thread_id = GetCurrentThreadId();
  HANDLE thread;
  int result = 0;

  if (process_id == 0 || main_thread_id == 0 ||
      GetCurrentThread() != (HANDLE)0xfffffffeUL) return 13;
  index = FlsAlloc((void *)0);
  if (index == FLS_OUT_OF_INDEXES || !FlsSetValue(index, MAIN_VALUE)) {
    return 1;
  }
  SetLastError(87);
  if (FlsGetValue(index) != MAIN_VALUE || GetLastError() != ERROR_SUCCESS) {
    result = 2;
    goto cleanup;
  }

  worker_observation.index = index;
  thread = CreateThread((void *)0, 0, worker_entry, &worker_observation, 0,
                        &thread_id);
  if (thread == (HANDLE)0) {
    result = 3;
    goto cleanup;
  }
  if (WaitForSingleObject(thread, 0xffffffffUL) != WAIT_OBJECT_0) {
    result = 4;
  } else if (!GetExitCodeThread(thread, &worker_exit_code)) {
    result = 5;
  } else if (worker_exit_code != 0) {
    result = 20 + (int)worker_exit_code;
  } else if (worker_observation.process_id != process_id ||
             worker_observation.thread_id != thread_id ||
             worker_observation.thread_id == main_thread_id ||
             worker_observation.pseudo_thread != (HANDLE)0xfffffffeUL) {
    result = 14;
  }
  if (!CloseHandle(thread) && result == 0) {
    result = 6;
  }
  if (result != 0) {
    goto cleanup;
  }

  SetLastError(90);
  if (FlsGetValue(index) != MAIN_VALUE || GetLastError() != ERROR_SUCCESS) {
    result = 7;
    goto cleanup;
  }

  unset_index = FlsAlloc((void *)0);
  if (unset_index == FLS_OUT_OF_INDEXES) {
    result = 8;
    goto cleanup;
  }
  SetLastError(122);
  if (FlsGetValue(unset_index) != (void *)0 ||
      GetLastError() != ERROR_SUCCESS) {
    result = 9;
    goto cleanup;
  }

  SetLastError(122);
  if (FlsGetValue(FLS_OUT_OF_INDEXES) != (void *)0 ||
      GetLastError() != ERROR_INVALID_PARAMETER) {
    result = 10;
  }

cleanup:
  if (unset_index != FLS_OUT_OF_INDEXES && !FlsFree(unset_index) && result == 0) {
    result = 11;
  }
  if (!FlsFree(index) && result == 0) {
    result = 12;
  }
  return result;
}
