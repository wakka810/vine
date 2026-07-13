typedef int BOOL;
typedef unsigned long DWORD;
typedef void *HANDLE;
typedef DWORD(__stdcall *THREAD_START_ROUTINE)(void *parameter);

enum {
  ERROR_SUCCESS = 0,
  ERROR_INVALID_PARAMETER = 87,
  WAIT_OBJECT_0 = 0,
};

#define TLS_OUT_OF_INDEXES ((DWORD)0xffffffffUL)
#define MAIN_VALUE ((void *)0x12345678UL)
#define WORKER_VALUE ((void *)0x23456789UL)

__declspec(dllimport) BOOL __stdcall CloseHandle(HANDLE object);
__declspec(dllimport) HANDLE __stdcall CreateThread(
    void *attributes, DWORD stack_size, THREAD_START_ROUTINE start,
    void *parameter, DWORD creation_flags, DWORD *thread_id);
__declspec(dllimport) BOOL __stdcall GetExitCodeThread(HANDLE thread,
                                                       DWORD *exit_code);
__declspec(dllimport) DWORD __stdcall GetLastError(void);
__declspec(dllimport) void __stdcall SetLastError(DWORD error);
__declspec(dllimport) DWORD __stdcall TlsAlloc(void);
__declspec(dllimport) BOOL __stdcall TlsFree(DWORD index);
__declspec(dllimport) void *__stdcall TlsGetValue(DWORD index);
__declspec(dllimport) BOOL __stdcall TlsSetValue(DWORD index, void *value);
__declspec(dllimport) DWORD __stdcall WaitForSingleObject(HANDLE object,
                                                          DWORD milliseconds);

typedef struct {
  DWORD index;
  void *before_value;
  DWORD before_last_error;
  BOOL set_result;
  void *after_value;
  DWORD after_last_error;
} WORKER_OBSERVATION;

static WORKER_OBSERVATION worker_observation;

static DWORD __stdcall worker_entry(void *parameter) {
  WORKER_OBSERVATION *observation = (WORKER_OBSERVATION *)parameter;
  SetLastError(88);
  observation->before_value = TlsGetValue(observation->index);
  observation->before_last_error = GetLastError();
  observation->set_result = TlsSetValue(observation->index, WORKER_VALUE);
  SetLastError(89);
  observation->after_value = TlsGetValue(observation->index);
  observation->after_last_error = GetLastError();
  return observation->before_value == (void *)0 &&
                 observation->before_last_error == ERROR_SUCCESS &&
                 observation->set_result != 0 &&
                 observation->after_value == WORKER_VALUE &&
                 observation->after_last_error == ERROR_SUCCESS
             ? 0
             : 1;
}

int __cdecl fixture_entry(void) {
  DWORD index = TlsAlloc();
  DWORD unset_index = TLS_OUT_OF_INDEXES;
  DWORD thread_id = 0;
  DWORD worker_exit_code = 0xffffffffUL;
  HANDLE thread;
  int result = 0;

  if (index == TLS_OUT_OF_INDEXES || !TlsSetValue(index, MAIN_VALUE)) {
    return 1;
  }
  SetLastError(87);
  if (TlsGetValue(index) != MAIN_VALUE || GetLastError() != ERROR_SUCCESS) {
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
  if (WaitForSingleObject(thread, 5000) != WAIT_OBJECT_0 ||
      !GetExitCodeThread(thread, &worker_exit_code) || worker_exit_code != 0) {
    result = 4;
  }
  if (!CloseHandle(thread) && result == 0) {
    result = 5;
  }
  if (result != 0) {
    goto cleanup;
  }

  SetLastError(90);
  if (TlsGetValue(index) != MAIN_VALUE || GetLastError() != ERROR_SUCCESS) {
    result = 6;
    goto cleanup;
  }

  unset_index = TlsAlloc();
  if (unset_index == TLS_OUT_OF_INDEXES) {
    result = 7;
    goto cleanup;
  }
  SetLastError(122);
  if (TlsGetValue(unset_index) != (void *)0 ||
      GetLastError() != ERROR_SUCCESS) {
    result = 8;
    goto cleanup;
  }

  SetLastError(122);
  if (TlsGetValue(TLS_OUT_OF_INDEXES) != (void *)0 ||
      GetLastError() != ERROR_INVALID_PARAMETER) {
    result = 9;
  }

cleanup:
  if (unset_index != TLS_OUT_OF_INDEXES && !TlsFree(unset_index) && result == 0) {
    result = 10;
  }
  if (!TlsFree(index) && result == 0) {
    result = 11;
  }
  return result;
}
