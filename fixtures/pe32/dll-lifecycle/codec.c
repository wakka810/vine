typedef int BOOL;
typedef unsigned long DWORD;

#define DLL_PROCESS_DETACH 0
#define DLL_PROCESS_ATTACH 1
#define TRUE 1
#define FALSE 0

static volatile int codec_lifecycle_state;

__declspec(dllexport) int __cdecl codec_get_state(void) {
  return codec_lifecycle_state;
}

__declspec(dllexport) int __cdecl codec_note_engine_detached(void) {
  if (codec_lifecycle_state != 1) {
    __builtin_trap();
  }
  codec_lifecycle_state = 2;
  return codec_lifecycle_state;
}

BOOL __stdcall fixture_dll_entry(void *module, DWORD reason, void *reserved) {
  if (module == 0) {
    return FALSE;
  }
  if (reason == DLL_PROCESS_ATTACH) {
    if (reserved == 0 || codec_lifecycle_state != 0) {
      return FALSE;
    }
    codec_lifecycle_state = 1;
    return TRUE;
  }
  if (reason == DLL_PROCESS_DETACH) {
    if (reserved == 0 || codec_lifecycle_state != 2) {
      __builtin_trap();
    }
    codec_lifecycle_state = 3;
  }
  return TRUE;
}
