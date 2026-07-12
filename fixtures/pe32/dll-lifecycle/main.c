__declspec(dllimport) int __cdecl engine_verify_attached(void);
__declspec(dllimport) unsigned long __stdcall GetCurrentProcessId(void);

int __cdecl fixture_entry(void) {
  return engine_verify_attached() && GetCurrentProcessId() != 0 ? 0 : 1;
}
