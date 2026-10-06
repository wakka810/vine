__declspec(dllimport) int __cdecl engine_verify_attached(void);
__declspec(dllimport) unsigned long __stdcall GetCurrentProcessId(void);
__declspec(dllimport) void * __stdcall LoadLibraryA(const char *);

int __cdecl fixture_entry(void) {
  void *engine = LoadLibraryA("lifecycle_engine.dll");
  if (!engine || LoadLibraryA("lifecycle_engine.dll") != engine) return 2;
  return engine_verify_attached() && GetCurrentProcessId() != 0 ? 0 : 1;
}
