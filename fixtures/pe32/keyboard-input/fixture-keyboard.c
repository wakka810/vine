typedef unsigned long U32;
typedef struct WindowClass {
  U32 size, style;
  void *procedure;
  U32 class_extra, window_extra;
  void *instance, *icon, *cursor, *brush;
  const char *menu, *name;
  void *small_icon;
} WindowClass;
typedef struct Message {
  void *window;
  U32 message, w_param, l_param, time;
  long x, y;
} Message;

extern char __ImageBase;
__declspec(dllimport) void * __stdcall LoadIconA(void *, U32);
__declspec(dllimport) void * __stdcall LoadCursorA(void *, U32);
__declspec(dllimport) U32 __stdcall RegisterClassExA(WindowClass *);
__declspec(dllimport) void * __stdcall CreateWindowExA(U32, const char *, const char *, U32,
    long, long, U32, U32, void *, void *, void *, void *);
__declspec(dllimport) long __stdcall DefWindowProcA(void *, U32, U32, U32);
__declspec(dllimport) U32 __stdcall SendMessageA(void *, U32, U32, U32);
__declspec(dllimport) int __stdcall SetWindowPos(void *, void *, long, long, U32, U32, U32);
__declspec(dllimport) void * __stdcall SetFocus(void *);
__declspec(dllimport) int __stdcall PeekMessageA(Message *, void *, U32, U32, U32);
__declspec(dllimport) int __stdcall WaitMessage(void);
__declspec(dllimport) int __stdcall TranslateMessage(const Message *);
__declspec(dllimport) long __stdcall DispatchMessageA(const Message *);
__declspec(dllimport) short __stdcall GetAsyncKeyState(int);
__declspec(dllimport) U32 __stdcall GetQueueStatus(U32);
__declspec(dllimport) void * __stdcall CreateFileA(const char *, U32, U32, void *, U32, U32, void *);
__declspec(dllimport) int __stdcall WriteFile(void *, const void *, U32, U32 *, void *);
__declspec(dllimport) int __stdcall CloseHandle(void *);

_Static_assert(sizeof(WindowClass) == 48, "fixture requires WNDCLASSEXA32");
_Static_assert(sizeof(Message) == 28, "fixture requires MSG32");

static void *keyboard_window;
static U32 received, characters, translated_downs, app_received, failure;

static long __stdcall procedure(void *window, U32 message, U32 w_param, U32 l_param) {
  if (message == 0x8000 || message == 0x0400) {
    if (window != keyboard_window || app_received >= 2 ||
        message != (app_received == 0 ? 0x8000u : 0x0400u) ||
        w_param != 0x11223344u + app_received || l_param != 0x87654321u + app_received)
      failure = 20;
    if (DefWindowProcA(window, message, w_param, l_param) != 0) failure = 29;
    app_received++;
    return (long)(0x13570000u + app_received);
  }
  if (message == 0x0100 || message == 0x0101) {
    /* A down, repeated down, and up: repeat count 1, scan 0x1e, prior/transition bits. */
    static const U32 expected_messages[3] = {0x0100, 0x0100, 0x0101};
    static const U32 expected_l_params[3] = {0x001e0001, 0x401e0001, 0xc01e0001};
    if (window != keyboard_window) failure = 10;
    else if (received >= 3) failure = 11;
    else if (message != expected_messages[received]) failure = 12;
    else if (w_param != 0x41) failure = 13;
    else if (l_param != expected_l_params[received]) failure = 14;
    received++;
    return (long)(0x27180000 + received);
  }
  if (message == 0x0102) {
    static const U32 expected_l_params[2] = {0x001e0001, 0x401e0001};
    if (window != keyboard_window) failure = 15;
    else if (characters >= 2 || characters >= translated_downs) failure = 16;
    else if (w_param != 'a') failure = 17;
    else if (l_param != expected_l_params[characters]) failure = 18;
    characters++;
    return (long)(0x31410000 + characters);
  }
  return DefWindowProcA(window, message, w_param, l_param);
}

int __cdecl fixture_entry(void) {
  WindowClass wc = {sizeof(WindowClass), 0, (void *)procedure, 0, 0, &__ImageBase,
      LoadIconA(&__ImageBase, 101), LoadCursorA(0, 32512), 0, 0, "keyboard-fixture", 0};
  if (!RegisterClassExA(&wc)) return 2;
  keyboard_window = CreateWindowExA(0, wc.name, "Keyboard fixture", 0x80ca0000,
      0, 0, 206, 129, 0, 0, &__ImageBase, 0);
  if (!keyboard_window) return 3;
  if (SendMessageA(keyboard_window, 0x80, 0, (U32)wc.icon) != 0 ||
      !SetWindowPos(keyboard_window, (void *)-2, 20, 30, 0, 0, 0x0559) ||
      !SetWindowPos(keyboard_window, (void *)-2, 0, 0, 206, 129, 0x010a)) return 4;
  if (SetFocus(keyboard_window) != keyboard_window) return 5;
  /* Application notifications are not keyboard translations; dispatch keeps their ABI. */
  for (U32 i = 0; i < 2; ++i) {
    Message message = {keyboard_window, i == 0 ? 0x8000u : 0x0400u,
        0x11223344u + i, 0x87654321u + i, 0xabcdef01u, -17, 214};
    Message unchanged = message;
    if (TranslateMessage(&message) != 0 || message.window != unchanged.window ||
        message.message != unchanged.message || message.w_param != unchanged.w_param ||
        message.l_param != unchanged.l_param || message.time != unchanged.time ||
        message.x != unchanged.x || message.y != unchanged.y) return 21;
    if ((U32)DispatchMessageA(&message) != 0x13570001u + i || failure) return 22;
  }
  if (app_received != 2) return 23;
  /* Native key ingress is separate from the posted-message queue class. */
  if (GetQueueStatus(0x0008) != 0) return 24;

  /* Publish readiness through the same durable Win32 file path as a save.
     The browser test sends one triplet only after observing this byte. */
  void *ready = CreateFileA("keyboard-ready.dat", 0x40000000u, 0u, 0, 2u, 0x08000020u, 0);
  if (ready == (void *)~0u) return 30;
  const unsigned char ready_byte = 0x4b;
  U32 ready_written = 0;
  int ready_ok = WriteFile(ready, &ready_byte, 1u, &ready_written, 0);
  int ready_closed = CloseHandle(ready);
  if (!ready_ok || ready_written != 1u) return 31;
  if (!ready_closed) return 32;

  U32 drained = 0;
  for (U32 attempts = 0; attempts < 1000000u; attempts++) {
    Message message;
    if (!PeekMessageA(&message, 0, 0, 0, 1)) {
      if (received == 3 && characters == 2) { drained = 1; break; }
      if (!WaitMessage()) return 33;
      continue;
    }
    if (message.message == 0x0100 || message.message == 0x0101) {
      /* Showing the window can overlap the browser's first triplet. Start at a fresh down. */
      if (received == 0 &&
          (message.message == 0x0101 || (message.l_param & 0x40000000) != 0)) {
        U32 expected_l_param = message.message == 0x0101 ? 0xc01e0001 : 0x401e0001;
        if (message.window != keyboard_window || message.w_param != 0x41 ||
            message.l_param != expected_l_param) return 8;
        continue;
      }
      U32 expected_result = 0x27180000 + received + 1;
      if (TranslateMessage(&message) != 1) return 9;
      if (message.message == 0x0100) {
        Message peek;
        translated_downs++;
        if (translated_downs == 1) {
          /* A masked query consumes only that class's new bit, not its message. */
          if (GetQueueStatus(0x0008) != 0x00080008 ||
              GetQueueStatus(0x0008) != 0x00080000 ||
              GetQueueStatus(0x0100) != 0x01000100) return 25;
        }
        if (!PeekMessageA(&peek, 0, 0, 0, 0) || peek.window != keyboard_window ||
            peek.message != 0x0102 || peek.w_param != 'a') return 26;
        /* An unfiltered no-remove peek marks posted messages old but retains them. */
        if (GetQueueStatus(0x0108) != 0x01080000) return 27;
      }
      if ((U32)DispatchMessageA(&message) != expected_result) return 6;
      if (failure) return (int)failure;
    } else if (message.message == 0x0102) {
      U32 expected_result = 0x31410000 + characters + 1;
      if (TranslateMessage(&message) != 0 ||
          (U32)DispatchMessageA(&message) != expected_result) return 19;
      if (failure) return (int)failure;
      if (GetQueueStatus(0x0108) != 0) return 28;
    }
  }
  /* Draining to an empty queue also detects CHAR generated by ingress or by key-up. */
  if (!drained || received != 3 || characters != 2 || translated_downs != 2) return 1;
  if (GetQueueStatus(0x0108) != 0) return 29;
  /* Query after the dispatched key-up so host event timing cannot race the down bit. */
  short first = GetAsyncKeyState(0x41);
  short second = GetAsyncKeyState(0x41);
  return ((first & 0xffff) == 1 && second == 0) ? 0 : 7;
}
