typedef int(__cdecl *fixture_callback)(int value);
typedef struct { int left, top, right, bottom; } monitor_rect;
typedef int(__stdcall *monitor_callback)(void *, void *, const monitor_rect *, int);
__declspec(dllimport) int __stdcall EnumDisplayMonitors(void *, const monitor_rect *, monitor_callback, int);
static volatile int monitor_callback_seen;

static int __stdcall visit_monitor(void *monitor, void *hdc, const monitor_rect *rect, int data) {
  if (monitor && !hdc && rect && rect->right > rect->left && rect->bottom > rect->top && data == 1) {
    monitor_callback_seen = 1;
    return 1;
  }
  return 0;
}

int __stdcall fixture_dll_entry(void *module, unsigned long reason, void *reserved) {
  if (reason == 1) {
    return module && reserved && EnumDisplayMonitors(0, 0, visit_monitor, 1) && monitor_callback_seen;
  }
  return 1;
}

__declspec(dllexport) int __cdecl fixture_deferred_add(int left, int right) {
  return left + right;
}

__declspec(dllexport) int __cdecl
fixture_call_callback(fixture_callback callback, int value) {
  return callback(value);
}

__declspec(dllexport) int __cdecl fixture_commit_outputs(unsigned int rgba,
                                                          unsigned int tone_hz,
                                                          unsigned int byte) {
  return rgba == 0xff0000ffu && tone_hz == 880u && byte == 0x5au;
}
