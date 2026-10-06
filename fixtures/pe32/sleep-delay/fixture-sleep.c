__declspec(dllimport) void __stdcall Sleep(unsigned long milliseconds);
__declspec(dllimport) unsigned long __stdcall GetTickCount(void);
__declspec(dllimport) void __stdcall SetLastError(unsigned long error);
__declspec(dllimport) unsigned long __stdcall GetLastError(void);

int __cdecl fixture_entry(void) {
  unsigned long before;
  Sleep(0);
  SetLastError(0x4321);
  before = GetTickCount();
  Sleep(30);
  if (GetTickCount() - before < 15u) return 1;
  if (GetLastError() != 0x4321u) return 2;
  return 0;
}
