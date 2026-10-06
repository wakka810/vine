typedef unsigned long HANDLE;
typedef unsigned long (__stdcall *thread_entry)(void *);
__declspec(dllimport) HANDLE __stdcall CreateThread(void *, unsigned long, thread_entry, void *, unsigned long, unsigned long *);
__declspec(dllimport) void __stdcall Sleep(unsigned long);
__declspec(dllimport) unsigned long __stdcall WaitForSingleObject(HANDLE, unsigned long);
__declspec(dllimport) int __stdcall CloseHandle(HANDLE);
__declspec(dllimport) int __stdcall MessageBoxA(void *, const char *, const char *, unsigned long);
__declspec(dllimport) short __stdcall GetAsyncKeyState(int);
static volatile unsigned long woke;
static unsigned long __stdcall background(void *unused) {
  (void)unused;
  Sleep(50);
  woke = 1;
  return 0;
}
int __cdecl fixture_entry(void) {
  unsigned long id = 0;
  unsigned long attempts = 0;
  while (!(GetAsyncKeyState(0x11) & 0x8000) && ++attempts < 200) Sleep(5);
  if (!(GetAsyncKeyState(0x11) & 0x8000)) return 6;
  HANDLE thread = CreateThread(0, 0, background, 0, 0, &id);
  if (!thread || !id) return 1;
  /* Actual browser UI must wait while another guest thread wakes from Sleep. */
  if (MessageBoxA(0, "\x82\xa0\r\nMessage fixture", "Vine message fixture", 0x1010) != 1) return 2;
  if (GetAsyncKeyState(0x11) & 0x8000) return 7;
  if (!woke) return 3;
  if (WaitForSingleObject(thread, ~0ul) != 0 || !CloseHandle(thread)) return 4;
  if (MessageBoxA(0, "Choose a button", "Vine button fixture", 0x103) != 2) return 5;
  return 0;
}
