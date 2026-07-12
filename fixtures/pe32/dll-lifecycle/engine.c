typedef int BOOL;
typedef unsigned long DWORD;

#define DLL_PROCESS_DETACH 0
#define DLL_PROCESS_ATTACH 1
#define TRUE 1
#define FALSE 0

__declspec(dllimport) int __cdecl codec_get_state(void);
__declspec(dllimport) int __cdecl codec_note_engine_detached(void);

static volatile int engine_lifecycle_state;

__declspec(dllexport) int __cdecl engine_verify_attached(void) {
  return engine_lifecycle_state == 1 && codec_get_state() == 1;
}

BOOL __stdcall fixture_dll_entry(void *module, DWORD reason, void *reserved) {
  if (module == 0) {
    return FALSE;
  }
  if (reason == DLL_PROCESS_ATTACH) {
    if (reserved == 0 || engine_lifecycle_state != 0 ||
        codec_get_state() != 1) {
      return FALSE;
    }
    engine_lifecycle_state = 1;
    return TRUE;
  }
  if (reason == DLL_PROCESS_DETACH) {
    if (reserved == 0 || engine_lifecycle_state != 1 ||
        codec_get_state() != 1) {
      __builtin_trap();
    }
    engine_lifecycle_state = 2;
    if (codec_note_engine_detached() != 2) {
      __builtin_trap();
    }
  }
  return TRUE;
}
