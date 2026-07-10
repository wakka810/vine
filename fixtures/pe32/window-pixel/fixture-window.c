typedef int BOOL;
typedef unsigned char BYTE;
typedef unsigned short ATOM;
typedef unsigned int UINT;
typedef long LONG;
typedef unsigned long DWORD;
typedef unsigned long WPARAM;
typedef long LPARAM;
typedef long LRESULT;
typedef unsigned long COLORREF;
typedef void *HANDLE;
typedef HANDLE HBRUSH;
typedef HANDLE HCURSOR;
typedef HANDLE HDC;
typedef HANDLE HICON;
typedef HANDLE HINSTANCE;
typedef HANDLE HMENU;
typedef HANDLE HMODULE;
typedef HANDLE HWND;
typedef const char *LPCSTR;

typedef struct {
  LONG x;
  LONG y;
} POINT;

typedef struct {
  LONG left;
  LONG top;
  LONG right;
  LONG bottom;
} RECT;

typedef struct {
  HWND hwnd;
  UINT message;
  WPARAM wParam;
  LPARAM lParam;
  DWORD time;
  POINT pt;
} MSG;

typedef struct {
  HDC hdc;
  BOOL fErase;
  RECT rcPaint;
  BOOL fRestore;
  BOOL fIncUpdate;
  BYTE rgbReserved[32];
} PAINTSTRUCT;

typedef LRESULT(__stdcall *WNDPROC)(HWND hwnd, UINT message, WPARAM wParam,
                                    LPARAM lParam);

typedef struct {
  UINT cbSize;
  UINT style;
  WNDPROC lpfnWndProc;
  int cbClsExtra;
  int cbWndExtra;
  HINSTANCE hInstance;
  HICON hIcon;
  HCURSOR hCursor;
  HBRUSH hbrBackground;
  LPCSTR lpszMenuName;
  LPCSTR lpszClassName;
  HICON hIconSm;
} WNDCLASSEXA;

__declspec(dllimport) HMODULE __stdcall GetModuleHandleA(LPCSTR module_name);
__declspec(dllimport) ATOM __stdcall
RegisterClassExA(const WNDCLASSEXA *window_class);
__declspec(dllimport) HWND __stdcall CreateWindowExA(
    DWORD extended_style, LPCSTR class_name, LPCSTR window_name, DWORD style,
    int x, int y, int width, int height, HWND parent, HMENU menu,
    HINSTANCE instance, void *parameter);
__declspec(dllimport) BOOL __stdcall ShowWindow(HWND hwnd, int command);
__declspec(dllimport) BOOL __stdcall InvalidateRect(HWND hwnd,
                                                    const RECT *rectangle,
                                                    BOOL erase);
__declspec(dllimport) BOOL __stdcall GetMessageA(MSG *message, HWND hwnd,
                                                 UINT minimum, UINT maximum);
__declspec(dllimport) LRESULT __stdcall
DispatchMessageA(const MSG *message);
__declspec(dllimport) LRESULT __stdcall DefWindowProcA(
    HWND hwnd, UINT message, WPARAM wParam, LPARAM lParam);
__declspec(dllimport) HDC __stdcall BeginPaint(HWND hwnd,
                                               PAINTSTRUCT *paint);
__declspec(dllimport) BOOL __stdcall EndPaint(HWND hwnd,
                                              const PAINTSTRUCT *paint);
__declspec(dllimport) BOOL __stdcall DestroyWindow(HWND hwnd);
__declspec(dllimport) void __stdcall PostQuitMessage(int exit_code);
__declspec(dllimport) COLORREF __stdcall SetPixel(HDC dc, int x, int y,
                                                  COLORREF color);

enum {
  WM_CREATE = 0x0001,
  WM_DESTROY = 0x0002,
  WM_PAINT = 0x000f,
  WM_QUIT = 0x0012,
  WS_OVERLAPPEDWINDOW = 0x00cf0000,
  SW_SHOW = 5,
};

#define CW_USEDEFAULT ((int)0x80000000u)
#define CLR_INVALID ((COLORREF)0xffffffffu)
#define FIXTURE_PIXEL_COLOR ((COLORREF)0x000000ffu)

_Static_assert(sizeof(void *) == 4, "fixture requires i386 pointers");
_Static_assert(sizeof(BOOL) == 4, "unexpected BOOL size");
_Static_assert(sizeof(UINT) == 4, "unexpected UINT size");
_Static_assert(sizeof(LONG) == 4, "fixture requires 32-bit LONG");
_Static_assert(sizeof(DWORD) == 4, "unexpected DWORD size");
_Static_assert(sizeof(WPARAM) == 4, "unexpected WPARAM size");
_Static_assert(sizeof(LPARAM) == 4, "unexpected LPARAM size");
_Static_assert(sizeof(LRESULT) == 4, "unexpected LRESULT size");
_Static_assert(sizeof(POINT) == 8, "unexpected POINT layout");
_Static_assert(sizeof(RECT) == 16, "unexpected RECT layout");
_Static_assert(sizeof(WNDCLASSEXA) == 48, "unexpected WNDCLASSEXA layout");
_Static_assert(__builtin_offsetof(WNDCLASSEXA, lpfnWndProc) == 8,
               "unexpected WNDCLASSEXA WndProc offset");
_Static_assert(__builtin_offsetof(WNDCLASSEXA, hInstance) == 20,
               "unexpected WNDCLASSEXA instance offset");
_Static_assert(__builtin_offsetof(WNDCLASSEXA, lpszClassName) == 40,
               "unexpected WNDCLASSEXA class name offset");
_Static_assert(__builtin_offsetof(WNDCLASSEXA, hIconSm) == 44,
               "unexpected WNDCLASSEXA small icon offset");
_Static_assert(sizeof(MSG) == 28, "unexpected MSG layout");
_Static_assert(__builtin_offsetof(MSG, message) == 4,
               "unexpected MSG message offset");
_Static_assert(__builtin_offsetof(MSG, lParam) == 12,
               "unexpected MSG lParam offset");
_Static_assert(__builtin_offsetof(MSG, time) == 16,
               "unexpected MSG time offset");
_Static_assert(__builtin_offsetof(MSG, pt) == 20,
               "unexpected MSG point offset");
_Static_assert(sizeof(PAINTSTRUCT) == 64, "unexpected PAINTSTRUCT layout");
_Static_assert(__builtin_offsetof(PAINTSTRUCT, rcPaint) == 8,
               "unexpected PAINTSTRUCT rect offset");
_Static_assert(__builtin_offsetof(PAINTSTRUCT, fRestore) == 24,
               "unexpected PAINTSTRUCT restore offset");
_Static_assert(__builtin_offsetof(PAINTSTRUCT, rgbReserved) == 32,
               "unexpected PAINTSTRUCT reserved offset");

static const char fixture_class_name[] = "VineWindowPixelFixture";
static WNDCLASSEXA fixture_class;
static MSG fixture_message;
static PAINTSTRUCT fixture_paint;

static volatile UINT event_trace[3];
static volatile UINT event_count;
static volatile UINT create_count;
static volatile UINT paint_count;
static volatile UINT destroy_count;
static volatile UINT begin_paint_count;
static volatile UINT end_paint_count;
static volatile UINT set_pixel_count;
static volatile UINT destroy_success_count;
static volatile UINT post_quit_count;
static volatile UINT dispatch_count;
static volatile UINT dispatched_paint_count;
static volatile UINT failure_code;
static volatile COLORREF pixel_result = CLR_INVALID;

static void set_failure(UINT code) {
  if (failure_code == 0) {
    failure_code = code;
  }
}

static void record_event(UINT message) {
  if (event_count < 3) {
    event_trace[event_count] = message;
  } else {
    set_failure(30);
  }
  event_count += 1;
}

static LRESULT __stdcall fixture_wnd_proc(HWND hwnd, UINT message,
                                          WPARAM wParam, LPARAM lParam) {
  if (message == WM_CREATE) {
    create_count += 1;
    record_event(message);
    return 0;
  }

  if (message == WM_PAINT) {
    HDC dc;
    BOOL destroyed;

    paint_count += 1;
    record_event(message);
    begin_paint_count += 1;
    dc = BeginPaint(hwnd, &fixture_paint);
    if (dc == (HDC)0) {
      set_failure(31);
    } else {
      set_pixel_count += 1;
      pixel_result = SetPixel(dc, 1, 1, FIXTURE_PIXEL_COLOR);
      if (pixel_result != FIXTURE_PIXEL_COLOR) {
        set_failure(32);
      }
      end_paint_count += 1;
      if (!EndPaint(hwnd, &fixture_paint)) {
        set_failure(33);
      }
    }

    destroyed = DestroyWindow(hwnd);
    if (destroyed) {
      destroy_success_count += 1;
    } else {
      set_failure(34);
      PostQuitMessage(34);
    }
    return 0;
  }

  if (message == WM_DESTROY) {
    destroy_count += 1;
    record_event(message);
    post_quit_count += 1;
    PostQuitMessage(0);
    return 0;
  }

  return DefWindowProcA(hwnd, message, wParam, lParam);
}

int __cdecl fixture_entry(void) {
  HINSTANCE instance;
  HWND hwnd;
  BOOL message_result;

  instance = (HINSTANCE)GetModuleHandleA((LPCSTR)0);
  if (instance == (HINSTANCE)0) {
    return 1;
  }

  fixture_class.cbSize = sizeof(fixture_class);
  fixture_class.lpfnWndProc = fixture_wnd_proc;
  fixture_class.hInstance = instance;
  fixture_class.lpszClassName = fixture_class_name;
  if (RegisterClassExA(&fixture_class) == 0) {
    return 2;
  }

  hwnd = CreateWindowExA(0, fixture_class_name, fixture_class_name,
                         WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, CW_USEDEFAULT,
                         160, 120, (HWND)0, (HMENU)0, instance, (void *)0);
  if (hwnd == (HWND)0) {
    return 3;
  }

  ShowWindow(hwnd, SW_SHOW);
  if (!InvalidateRect(hwnd, (const RECT *)0, 0)) {
    DestroyWindow(hwnd);
    return 4;
  }

  while ((message_result = GetMessageA(&fixture_message, (HWND)0, 0, 0)) >
         0) {
    dispatch_count += 1;
    if (fixture_message.message == WM_PAINT) {
      dispatched_paint_count += 1;
    }
    DispatchMessageA(&fixture_message);
  }

  if (message_result < 0) {
    return 5;
  }
  if (failure_code != 0) {
    return (int)failure_code;
  }
  if (fixture_message.message != WM_QUIT) {
    return 12;
  }
  if (fixture_message.wParam != 0) {
    return 6;
  }
  if (dispatch_count == 0 || dispatched_paint_count != 1) {
    return 7;
  }
  if (create_count != 1 || paint_count != 1 || destroy_count != 1) {
    return 8;
  }
  if (event_count != 3 || event_trace[0] != WM_CREATE ||
      event_trace[1] != WM_PAINT || event_trace[2] != WM_DESTROY) {
    return 9;
  }
  if (begin_paint_count != 1 || end_paint_count != 1 ||
      set_pixel_count != 1 || pixel_result != FIXTURE_PIXEL_COLOR) {
    return 10;
  }
  if (destroy_success_count != 1 || post_quit_count != 1) {
    return 11;
  }
  return 0;
}
