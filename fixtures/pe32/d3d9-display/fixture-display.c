typedef unsigned long U32;
typedef long HRESULT;
typedef struct DisplayMode { U32 width, height, refresh, format; } DisplayMode;
__declspec(dllimport) void * __stdcall Direct3DCreate9(U32 sdk_version);

typedef U32 (__stdcall *AdapterCount)(void *);
typedef HRESULT (__stdcall *DisplayModeQuery)(void *, U32, DisplayMode *);
typedef HRESULT (__stdcall *AdapterIdentifierQuery)(void *, U32, U32, unsigned char *);
typedef HRESULT (__stdcall *DeviceCapsQuery)(void *, U32, U32, U32 *);
typedef HRESULT (__stdcall *DeviceTypeCheck)(void *, U32, U32, U32, U32, U32);
typedef U32 (__stdcall *Release)(void *);
typedef U32 (__stdcall *AddRef)(void *);
typedef HRESULT (__stdcall *QueryInterface)(void *, const void *, void **);
static const unsigned char iunknown_iid[16] = {
    0, 0, 0, 0, 0, 0, 0, 0, 0xc0, 0, 0, 0, 0, 0, 0, 0x46
};

typedef struct WindowClass {
    U32 size, style;
    void *procedure;
    U32 class_extra, window_extra;
    void *instance, *icon, *cursor, *brush;
    const char *menu, *name;
    void *small_icon;
} WindowClass;
typedef struct Rect { long left, top, right, bottom; } Rect;
typedef struct MonitorInfo { U32 size; Rect monitor, work; U32 flags; } MonitorInfo;
extern char __ImageBase;
__declspec(dllimport) void * __stdcall LoadIconA(void *, U32);
__declspec(dllimport) void * __stdcall LoadCursorA(void *, U32);
__declspec(dllimport) U32 __stdcall RegisterClassExA(WindowClass *);
__declspec(dllimport) void * __stdcall CreateWindowExA(U32, const char *, const char *, U32,
    long, long, U32, U32, void *, void *, void *, void *);
__declspec(dllimport) long __stdcall DefWindowProcA(void *, U32, U32, U32);
__declspec(dllimport) void * __stdcall SetFocus(void *);
__declspec(dllimport) int __stdcall IsIconic(void *);
__declspec(dllimport) U32 __stdcall SetWindowLongA(void *, int, U32);
__declspec(dllimport) void * __stdcall MonitorFromWindow(void *, U32);
__declspec(dllimport) int __stdcall GetMonitorInfoA(void *, MonitorInfo *);
__declspec(dllimport) int __stdcall GetClientRect(void *, Rect *);
__declspec(dllimport) void * __stdcall GetDC(void *);
__declspec(dllimport) int __stdcall ReleaseDC(void *, void *);
__declspec(dllimport) int __stdcall FillRect(void *, Rect *, void *);
__declspec(dllimport) void * __stdcall GetStockObject(long);
__declspec(dllimport) U32 __stdcall SetTimer(void *, U32, U32, void *);
__declspec(dllimport) void * __stdcall GetSystemMenu(void *, int);
__declspec(dllimport) U32 __stdcall EnableMenuItem(void *, U32, U32);
__declspec(dllimport) U32 __stdcall GetLastError(void);
__declspec(dllimport) void __stdcall GlobalMemoryStatus(U32 *);
__declspec(dllimport) void __stdcall SetLastError(U32);
__declspec(dllimport) void * __stdcall VirtualAlloc(void *, U32, U32, U32);
__declspec(dllimport) int __stdcall VirtualFree(void *, U32, U32);
__declspec(dllimport) void __stdcall InitializeCriticalSection(void *);
__declspec(dllimport) int __stdcall InitializeCriticalSectionAndSpinCount(void *, U32);
__declspec(dllimport) void __stdcall EnterCriticalSection(void *);
__declspec(dllimport) void __stdcall LeaveCriticalSection(void *);
__declspec(dllimport) void __stdcall DeleteCriticalSection(void *);
__declspec(dllimport) void * __stdcall CreateMutexA(void *, int, const char *);
__declspec(dllimport) int __stdcall CloseHandle(void *);
__declspec(dllimport) void * __stdcall CreateEventA(void *, int, int, const char *);
__declspec(dllimport) U32 __stdcall WaitForMultipleObjects(U32, void **, int, U32);
__declspec(dllimport) void * __stdcall ImmGetDefaultIMEWnd(void *);
__declspec(dllimport) void __stdcall DragAcceptFiles(void *, int);
__declspec(dllimport) U32 __stdcall SendMessageA(void *, U32, U32, U32);
__declspec(dllimport) void * __stdcall ImmAssociateContext(void *, void *);
__declspec(dllimport) U32 __stdcall timeGetDevCaps(U32 *, U32);
__declspec(dllimport) U32 __stdcall timeBeginPeriod(U32);
__declspec(dllimport) U32 __stdcall timeEndPeriod(U32);
__declspec(dllimport) U32 __stdcall timeGetTime(void);
__declspec(dllimport) U32 __stdcall GdiplusStartup(U32 *, U32 *, void *);
__declspec(dllimport) void __stdcall GdiplusShutdown(U32);
#ifdef VINE_POINTER_FIXTURE
typedef struct Message { void *window; U32 message, w_param, l_param, time; long x, y; } Message;
__declspec(dllimport) int __stdcall SetWindowPos(void *, void *, long, long, U32, U32, U32);
__declspec(dllimport) int __stdcall PeekMessageA(Message *, void *, U32, U32, U32);
__declspec(dllimport) int __stdcall TranslateMessage(const Message *);
__declspec(dllimport) long __stdcall DispatchMessageA(const Message *);
__declspec(dllimport) void __stdcall Sleep(U32);
#endif
__declspec(dllimport) HRESULT __stdcall DirectInput8Create(void *, U32, const void *, void **, void *);
__declspec(dllimport) HRESULT __stdcall DirectSoundCreate8(const void *, void **, void *);
typedef HRESULT (__stdcall *SoundCooperative)(void *, void *, U32);
typedef HRESULT (__stdcall *SoundBufferCreate)(void *, const U32 *, void **, void *);
typedef HRESULT (__stdcall *SoundNotify)(void *, U32, const U32 *);
typedef HRESULT (__stdcall *SoundLock)(void *, U32, U32, void **, U32 *, void **, U32 *, U32);
typedef HRESULT (__stdcall *SoundUnlock)(void *, void *, U32, void *, U32);
typedef HRESULT (__stdcall *SoundPlay)(void *, U32, U32, U32);
typedef HRESULT (__stdcall *SoundStop)(void *);
static const unsigned char sound_notify_iid[16] = {
    0x83, 0x07, 0x21, 0xb0, 0xcd, 0x89, 0xd0, 0x11,
    0xaf, 0x08, 0x00, 0xa0, 0xc9, 0x25, 0xcd, 0x16
};
__declspec(dllimport) HRESULT __stdcall CoInitialize(void *);
__declspec(dllimport) U32 __stdcall EnumFontFamiliesExA(void *, const void *, void *, void *, U32);
__declspec(dllimport) void * __stdcall CreateFontA(long, long, long, long, long,
    U32, U32, U32, U32, U32, U32, U32, U32, const char *);
__declspec(dllimport) int __stdcall DeleteObject(void *);
__declspec(dllimport) void * __stdcall SelectObject(void *, void *);
__declspec(dllimport) U32 __stdcall GetOutlineTextMetricsA(void *, U32, void *);
__declspec(dllimport) int __stdcall TextOutA(void *, int, int, const char *, int);
__declspec(dllimport) void * __stdcall CreateCompatibleDC(void *);
__declspec(dllimport) int __stdcall DeleteDC(void *);
__declspec(dllimport) void * __stdcall CreateDIBSection(void *, const void *, U32,
    void **, void *, U32);
static const unsigned char gothic_face[] = {
    0x82, 0x6c, 0x82, 0x72, 0x20, 0x83, 0x53, 0x83, 0x56,
    0x83, 0x62, 0x83, 0x4e, 0
};
static U32 font_callback_count;
static int __stdcall font_callback(const unsigned char *face, const void *metric, U32 type,
                                   void *reference) {
    ++font_callback_count;
    (void)metric;
    return type == 4 && face[23] == 128 && face[28] == gothic_face[0] &&
        reference == (void *)0x1234 ? 0 : 1;
}
typedef HRESULT (__stdcall *EnumInputDevices)(void *, U32, void *, void *, U32);
static const unsigned char input_iid[16] = {
    0x30, 0x80, 0x79, 0xbf, 0x3a, 0x48, 0xa2, 0x4d,
    0xaa, 0x99, 0x5d, 0x64, 0xed, 0x36, 0x97, 0x00
};
static int __stdcall input_device_callback(void *device, void *reference) {
    (void)device; (void)reference;
    return 1;
}
typedef HRESULT (__stdcall *CreateDevice)(void *, U32, U32, void *, U32, U32 *, void **);
typedef HRESULT (__stdcall *CreateTexture)(void *, U32, U32, U32, U32, U32, U32, void **, void *);
/* The x86 stdcall depth float occupies one DWORD; use its IEEE-754 bits without a CRT. */
typedef HRESULT (__stdcall *Clear)(void *, U32, const void *, U32, U32, U32, U32);
typedef HRESULT (__stdcall *SurfaceLevel)(void *, U32, void **);
typedef HRESULT (__stdcall *AddDirtyRect)(void *, const Rect *);
typedef HRESULT (__stdcall *UpdateTexture)(void *, void *, void *);
typedef HRESULT (__stdcall *Scene)(void *);
typedef HRESULT (__stdcall *Present)(void *, const void *, const void *, void *, const void *);
typedef HRESULT (__stdcall *DeviceStatus)(void *);
typedef HRESULT (__stdcall *VertexLock)(void *, U32, U32, void **, U32);
typedef HRESULT (__stdcall *VertexUnlock)(void *);
typedef HRESULT (__stdcall *TextureLock)(void *, U32, U32 *, const Rect *, U32);
typedef HRESULT (__stdcall *TextureUnlock)(void *, U32);
typedef HRESULT (__stdcall *DrawPrimitive)(void *, U32, U32, U32);
typedef HRESULT (__stdcall *SetState)(void *, U32, U32, U32);
typedef HRESULT (__stdcall *SetRenderState)(void *, U32, U32);
typedef HRESULT (__stdcall *SetTexture)(void *, U32, void *);
typedef HRESULT (__stdcall *SetFVF)(void *, U32);
typedef HRESULT (__stdcall *CreateVertexBuffer)(void *, U32, U32, U32, U32, void **, void *);
typedef HRESULT (__stdcall *SetStreamSource)(void *, U32, void *, U32, U32);
typedef HRESULT (__stdcall *SurfaceDC)(void *, void **);
typedef HRESULT (__stdcall *ReleaseSurfaceDC)(void *, void *);
static unsigned char identifier[1100];
static U32 caps[76];
static long __stdcall procedure(void *window, U32 message, U32 wparam, U32 lparam) {
#ifdef VINE_POINTER_FIXTURE
    if (message == 0x0113) return 0;
#endif
    return DefWindowProcA(window, message, wparam, lparam);
}

int fixture_entry(void) {
    U32 startup_input[4] = {2, 0, 0, 0}, first_token = 0xcccccccc, second_token = 0;
    SetLastError(0x12345678);
    U32 memory_status[10] = {0xa5a5a5a5UL};
    memory_status[9] = 0x5a5a5a5aUL;
    GlobalMemoryStatus(memory_status + 1);
    if (memory_status[0] != 0xa5a5a5a5UL || memory_status[9] != 0x5a5a5a5aUL ||
        memory_status[1] != 32 || memory_status[2] > 100 ||
        memory_status[3] < memory_status[4] ||
        memory_status[5] != memory_status[3] || memory_status[6] != memory_status[4] ||
        memory_status[7] != 0x7ffe0000UL || memory_status[8] > memory_status[7] ||
        GetLastError() != 0x12345678) return 78;
    if (CoInitialize(0) != 0 || GetLastError() != 0x12345678) return 72;
    if (CoInitialize(0) != 1 || GetLastError() != 0x12345678) return 73;
    U32 section[6] = {0};
    InitializeCriticalSection(section);
    if (section[2] || section[3] || section[5] != 0x020007d0 ||
        GetLastError() != 0x12345678) return 65;
    EnterCriticalSection(section);
    U32 owning_thread = section[3];
    EnterCriticalSection(section);
    if (section[2] != 2 || !owning_thread || section[3] != owning_thread ||
        GetLastError() != 0x12345678) return 66;
    LeaveCriticalSection(section);
    if (section[2] != 1 || section[3] != owning_thread) return 67;
    LeaveCriticalSection(section);
    if (section[2] || section[3]) return 68;
    DeleteCriticalSection(section);
    for (U32 i = 0; i < 6; ++i) if (section[i]) return 69;
    if (!InitializeCriticalSectionAndSpinCount(section, 137) || section[5] != 137 ||
        GetLastError() != 0x12345678) return 70;
    EnterCriticalSection(section);
    LeaveCriticalSection(section);
    DeleteCriticalSection(section);
    if (GetLastError() != 0x12345678) return 71;
    void *first_mutex = CreateMutexA(0, 1, "fixture mutex");
    if (!first_mutex || GetLastError() != 0) return 74;
    void *duplicate_mutex = CreateMutexA(0, 0, "fixture mutex");
    if (!duplicate_mutex || duplicate_mutex == first_mutex || GetLastError() != 183) return 75;
    SetLastError(0x12345678);
    if (!CloseHandle(first_mutex) || !CloseHandle(duplicate_mutex) ||
        GetLastError() != 0x12345678) return 76;
    void *new_mutex = CreateMutexA(0, 0, "fixture mutex");
    if (!new_mutex || new_mutex == first_mutex || new_mutex == duplicate_mutex ||
        GetLastError() != 0) return 77;
    if (CloseHandle(first_mutex) || GetLastError() != 6 || !CloseHandle(new_mutex) ||
        GetLastError() != 6) return 78;
    SetLastError(0x12345678);
    if (GdiplusStartup(&first_token, startup_input, 0) != 17 || first_token != 0xcccccccc ||
        GetLastError() != 0x12345678) return 29;
    startup_input[0] = 1;
    if (GdiplusStartup(&first_token, startup_input, 0) != 0 || !first_token) return 30;
    if (GdiplusStartup(&second_token, startup_input, 0) != 0 || !second_token ||
        second_token == first_token || GetLastError() != 0x12345678) return 31;
    GdiplusShutdown(first_token);
    GdiplusShutdown(second_token);
    if (GetLastError() != 0x12345678) return 32;
    void *input = (void *)0xcccccccc;
    if (DirectInput8Create(&__ImageBase, 0x700, input_iid, &input, 0) !=
        (HRESULT)0x80070481UL || input != 0 || GetLastError() != 0x12345678) return 33;
    if (DirectInput8Create(&__ImageBase, 0x800, input_iid, &input, 0) != 0 ||
        !input || GetLastError() != 0x12345678) return 34;
    void **input_methods = *(void ***)input;
    if (((EnumInputDevices)input_methods[4])(input, 4, (void *)input_device_callback, 0, 1) != 0 ||
        GetLastError() != 0x12345678) return 35;
    if (((Release)input_methods[2])(input) != 0) return 36;
    void *sound = 0;
    if (DirectSoundCreate8(0, &sound, 0) != 0 || !sound ||
        GetLastError() != 0x12345678) return 37;
    if (((Release)(*(void ***)sound)[2])(sound) != 0) return 38;
    unsigned char font_filter[60] = {0};
    font_filter[23] = 128;
    for (U32 i = 0; i < sizeof(gothic_face); ++i) font_filter[28 + i] = gothic_face[i];
    void *screen_dc = GetDC(0);
    if (!screen_dc) return 40;
    if (EnumFontFamiliesExA(screen_dc, font_filter, (void *)font_callback, (void *)0x1234, 0) != 0 ||
        font_callback_count != 1 || GetLastError() != 0x12345678 ||
        !ReleaseDC(0, screen_dc)) return 41;
    void *font = CreateFontA(37, 19, 0, 0, 100, 0, 0, 0, 128, 4, 0, 2, 0,
        (const char *)gothic_face);
    if (!font || GetLastError() != 0x12345678) return 42;
    void *memory_dc = CreateCompatibleDC(0);
    if (!memory_dc || GetLastError() != 0x12345678) return 43;
    void *default_font = SelectObject(memory_dc, font);
    if (!default_font || GetOutlineTextMetricsA(memory_dc, 0, 0) != 269) return 46;
    static U32 outline[68];
    if (GetOutlineTextMetricsA(memory_dc, 269, outline) != 269 ||
        outline[0] != 269 || outline[1] != 37 || outline[6] != 19 ||
        outline[49] != 212 || ((unsigned char *)outline)[212] != gothic_face[0] ||
        GetLastError() != 0x12345678) return 47;
    static U32 bitmap_info[266];
    bitmap_info[0] = 40; bitmap_info[1] = 37; bitmap_info[2] = (U32)-19;
    bitmap_info[3] = 1 | (8 << 16); bitmap_info[8] = 256;
    for (U32 index = 0; index < 256; ++index) {
        U32 gray = 255 - index;
        bitmap_info[10 + index] = gray | (gray << 8) | (gray << 16);
    }
    void *pixels = 0;
    void *bitmap = CreateDIBSection(memory_dc, bitmap_info, 0, &pixels, 0, 0);
    if (!bitmap || !pixels || GetLastError() != 0x12345678) return 44;
    void *default_bitmap = SelectObject(memory_dc, bitmap);
    if (!default_bitmap || !TextOutA(memory_dc, 0, 0, "a", 1) ||
        GetLastError() != 0x12345678) return 49;
    U32 lit = 0;
    for (U32 index = 0; index < 40 * 19; ++index) lit += ((unsigned char *)pixels)[index];
    if (!lit || SelectObject(memory_dc, default_bitmap) != bitmap ||
        SelectObject(memory_dc, default_font) != font ||
        !DeleteObject(bitmap) || !DeleteObject(font) || !DeleteDC(memory_dc)) return 50;
    unsigned char *reservation = VirtualAlloc(0, 7 * 4096, 0x102000, 4);
    if (!reservation || VirtualAlloc(reservation, 4096, 0x1000, 4) != reservation ||
        GetLastError() != 0x12345678) return 51;
    reservation[100] = 0x5a;
    if (reservation[100] != 0x5a || !VirtualFree(reservation, 4096, 0x4000) ||
        GetLastError() != 0x12345678) return 52;
    void *object = Direct3DCreate9(32);
    if (!object) return 1;
    void **methods = *(void ***)object;
    SetLastError(0x12345678);
    void *unknown = (void *)0xcccccccc, *canonical = 0, *canonical_again = 0;
    if (((QueryInterface)methods[0])(object, input_iid, &unknown) !=
        (HRESULT)0x80004002UL || unknown || GetLastError() != 0x12345678) return 59;
    if (((QueryInterface)methods[0])(object, iunknown_iid, &canonical) != 0 || !canonical)
        return 60;
    void **canonical_methods = *(void ***)canonical;
    if (((QueryInterface)canonical_methods[0])(canonical, iunknown_iid, &canonical_again) != 0 ||
        canonical_again != canonical) return 61;
    if (((Release)canonical_methods[2])(canonical_again) != 2 ||
        ((AddRef)methods[1])(object) != 3 || ((Release)methods[2])(object) != 2 ||
        ((Release)canonical_methods[2])(canonical) != 1 || GetLastError() != 0x12345678)
        return 62;
    DisplayMode mode = { 0, 0, 0, 0 };
    if (((AdapterCount)methods[4])(object) != 1) return 2;
    if (((DisplayModeQuery)methods[8])(object, 0, &mode) != 0 ||
        mode.width != 1920 || mode.height != 1080 || mode.refresh != 60 || mode.format != 22)
        return 3;
    if (((AdapterIdentifierQuery)methods[5])(object, 0, 0, identifier) != 0 ||
        identifier[0] != 'v' || identifier[512] != 'V') return 4;
    if (((DeviceCapsQuery)methods[14])(object, 0, 1, caps) != 0 ||
        caps[0] != 1 || caps[1] != 0) return 5;
    if (((DeviceTypeCheck)methods[9])(object, 0, 1, 22, 22, 1) != 0) return 6;
    if (((DisplayModeQuery)methods[8])(object, 1, &mode) != (HRESULT)0x8876086cUL)
        return 7;
    WindowClass wc = { sizeof(WindowClass), 0, (void *)procedure, 0, 0, &__ImageBase,
        LoadIconA(&__ImageBase, 101), LoadCursorA(0, 32512), 0, 0, "graphics-fixture", 0 };
    if (!RegisterClassExA(&wc)) return 8;
    void *window = CreateWindowExA(0, wc.name, "Graphics fixture", 0x80ca0000,
        20, 30, 206, 129, 0, 0, &__ImageBase, 0);
    if (!window) return 9;
    /* Exercise both stopped and playing notification waits through the Chrome host timer. */
    void *audio = 0, *buffer = 0, *notify = 0;
    if (DirectSoundCreate8(0, &audio, 0) != 0 || !audio) return 120;
    void **audio_methods = *(void ***)audio;
    if (((SoundCooperative)audio_methods[6])(audio, window, 2) != 0) return 121;
    U32 pcm[4] = {1 | (2 << 16), 44100, 176400, 4 | (16 << 16)};
    U32 sound_descriptor[9] = {36, 0x81a8, 3528, 0, (U32)pcm, 0, 0, 0, 0};
    if (((SoundBufferCreate)audio_methods[3])(audio, sound_descriptor, &buffer, 0) != 0 ||
        !buffer) return 122;
    void **buffer_methods = *(void ***)buffer;
    if (((QueryInterface)buffer_methods[0])(buffer, sound_notify_iid, &notify) != 0 ||
        !notify) return 123;
    void *events[2] = {CreateEventA(0, 0, 0, 0), CreateEventA(0, 0, 0, 0)};
    if (!events[0] || !events[1]) return 124;
    U32 positions[4] = {1764, (U32)events[0], 0xffffffffUL, (U32)events[1]};
    if (((SoundNotify)(*(void ***)notify)[3])(notify, 2, positions) != 0) return 125;
    if (WaitForMultipleObjects(2, events, 0, 20) != 0x102) return 126;
    void *first = 0, *second = 0;
    U32 first_bytes = 0, second_bytes = 0;
    if (((SoundLock)buffer_methods[11])(buffer, 0, 3528, &first, &first_bytes,
            &second, &second_bytes, 0) != 0 || !first || first_bytes != 3528 ||
        second || second_bytes) return 127;
    for (U32 index = 0; index < first_bytes; ++index) ((unsigned char *)first)[index] = 0;
    if (((SoundUnlock)buffer_methods[19])(buffer, first, first_bytes, second, second_bytes) != 0 ||
        ((SoundPlay)buffer_methods[12])(buffer, 0, 0, 1) != 0) return 128;
    if (WaitForMultipleObjects(2, events, 0, 100) != 0) return 129;
    if (((SoundStop)buffer_methods[18])(buffer) != 0 ||
        WaitForMultipleObjects(2, events, 0, 0) != 1) return 130;
    if (!CloseHandle(events[0]) || !CloseHandle(events[1]) ||
        ((Release)(*(void ***)notify)[2])(notify) != 1 ||
        ((Release)buffer_methods[2])(buffer) != 0 ||
        ((Release)audio_methods[2])(audio) != 0) return 131;
#ifdef VINE_POINTER_FIXTURE
    if (SendMessageA(window, 0x80, 0, (U32)wc.icon) != 0 ||
        !SetWindowPos(window, (void *)-2, 317, 154, 0, 0, 0x0559) ||
        !SetWindowPos(window, (void *)-2, 0, 0, 206, 129, 0x010a)) return 108;
    SetLastError(0x24681357);
    if (SetFocus(window) != window || GetLastError() != 0x24681357) return 109;
    if (IsIconic(window) || GetLastError() != 0x24681357) return 110;
    SetWindowLongA(window, -16, 0x90ca0000);
    if (SetWindowLongA(window, -16, 0xb0ca0000) != 0x90ca0000 || !IsIconic(window)) return 111;
    if (SetWindowLongA(window, -16, 0x90ca0000) != 0xb0ca0000 || IsIconic(window) ||
        GetLastError() != 0x24681357) return 112;
    void *monitor = MonitorFromWindow(window, 1);
    if (!monitor || GetLastError() != 0x24681357) return 113;
    MonitorInfo info = { sizeof(info), {0, 0, 0, 0}, {0, 0, 0, 0}, 0 };
    if (!GetMonitorInfoA(monitor, &info) || info.monitor.right != 1920 ||
        info.monitor.bottom != 1080 || info.work.bottom != 1032 || info.flags != 1 ||
        GetLastError() != 0x24681357) return 114;
    if (GetMonitorInfoA((void *)0x1234, &info) || GetLastError() != 1461) return 115;
    if (MonitorFromWindow((void *)0x1234, 1) || GetLastError() != 1400) return 116;
#endif
    SetLastError(0x12345678);
    void *menu = GetSystemMenu(window, 0);
    if (!menu || GetLastError() != 0x12345678) return 54;
    if (GetSystemMenu(window, 0) != menu || GetLastError() != 0x12345678) return 55;
    if (EnableMenuItem(menu, 0xf060, 0) != 0 || GetLastError() != 0x12345678) return 56;
    if (EnableMenuItem((void *)0x1234, 0xf060, 0) != 0xffffffffUL ||
        GetLastError() != 1401) return 57;
    if (GetSystemMenu((void *)0x1234, 0) != 0 || GetLastError() != 1400) return 58;
    DragAcceptFiles(window, 0);
    SetLastError(0x12345678);
    if (SetTimer(window, 7, 23, (void *)procedure) != 7 ||
        GetLastError() != 0x12345678) return 39;
    SetLastError(0xa5a55a5a);
    if (ImmGetDefaultIMEWnd(0) != 0 || GetLastError() != 0) return 19;
    if (ImmGetDefaultIMEWnd((void *)0x1234) != 0 || GetLastError() != 1400) return 20;
    void *ime = ImmGetDefaultIMEWnd(window);
    if (!ime || ime == window || ImmGetDefaultIMEWnd(window) != ime) return 21;
    if (SendMessageA(ime, 0x0283, 0x21, 0) != 0 ||
        SendMessageA(ime, 0x0283, 0x22, 0) != 0) return 53;
    SetLastError(0x12345678);
    void *context = ImmAssociateContext(window, 0);
    if (!context || context == ime || GetLastError() != 0x12345678) return 22;
    if (ImmGetDefaultIMEWnd(window) != ime) return 23;
    SetLastError(0x87654321);
    if (ImmAssociateContext(window, context) != 0 || GetLastError() != 0x87654321) return 24;
    U32 time_caps[4] = {0xa5a5a5a5, 0xa5a5a5a5, 0xa5a5a5a5, 0xa5a5a5a5};
    if (timeGetDevCaps(time_caps, 7) != 97 || time_caps[0] != 0xa5a5a5a5) return 25;
    if (timeGetDevCaps(time_caps, 16) != 0 || time_caps[0] != 1 || time_caps[1] != 1000000 ||
        time_caps[2] != 0xa5a5a5a5 || time_caps[3] != 0xa5a5a5a5) return 26;
    if (timeBeginPeriod(0) != 97 || timeBeginPeriod(1) != 0 || timeBeginPeriod(2) != 0 ||
        timeEndPeriod(1) != 0 || timeEndPeriod(2) != 0 || timeEndPeriod(2) != 97) return 27;
    U32 tick = timeGetTime();
    if ((timeGetTime() - tick) > 0x7fffffffUL || GetLastError() != 0x87654321) return 28;
    Rect rect;
    if (!GetClientRect(window, &rect) || rect.right != 200 || rect.bottom != 100) return 10;
    void *hdc = GetDC(window);
    void *brush = GetStockObject(4);
    if (!hdc || !brush || !FillRect(hdc, &rect, brush) || !ReleaseDC(window, hdc)) return 11;
    U32 present[14] = {0, 0, 0, 1, 0, 0, 3, (U32)window, 1, 0, 80, 0, 0, 0};
    void *device = 0;
    if (((CreateDevice)methods[16])(object, 0, 1, window, 0x44, present, &device) != 0 ||
        !device || present[0] != 200 || present[1] != 100 || present[2] != 22) return 12;
    void **device_methods = *(void ***)device;
    SetLastError(0x42424242UL);
    if (((DeviceStatus)device_methods[3])(device) != 0 ||
        GetLastError() != 0x42424242UL) return 101;
    if (((Clear)device_methods[43])(device, 0, 0, 1, 0xff113355UL, 0x3f800000UL, 0) != 0 ||
        ((Clear)device_methods[43])(device, 1, 0, 1, 0xff000000UL, 0x3f800000UL, 0) !=
            (HRESULT)0x8876086cUL) return 79;
    if (((Present)device_methods[17])(device, 0, 0, 0, 0) != 0 ||
        ((Present)device_methods[17])(device, 0, 0, 0, 0) != 0) return 86;
    void *texture = 0, *surface = 0;
    if (((CreateTexture)device_methods[23])(device, 37, 19, 1, 0, 22, 2, &texture, 0) != 0)
        return 13;
    void **texture_methods = *(void ***)texture;
    Rect dirty = {1, 2, 15, 16}, outside = {1, 2, 38, 16};
    if (((AddDirtyRect)texture_methods[21])(texture, &dirty) != 0 ||
        ((AddDirtyRect)texture_methods[21])(texture, &outside) !=
            (HRESULT)0x8876086cUL ||
        ((AddDirtyRect)texture_methods[21])(texture, 0) != 0) return 80;
    if (((SurfaceLevel)texture_methods[18])(texture, 0, &surface) != 0) return 14;
    void **surface_methods = *(void ***)surface;
    void *surface_again = 0;
    if (((SurfaceLevel)texture_methods[18])(texture, 0, &surface_again) != 0 ||
        surface_again != surface || ((Release)surface_methods[2])(surface_again) != 1)
        return 63;
    if (((SurfaceDC)surface_methods[15])(surface, 0) != (HRESULT)0x8876086cUL) return 207;
    if (((SurfaceDC)surface_methods[15])(surface, &hdc) != 0) return 15;
    void *duplicate_dc = 0;
    if (((SurfaceDC)surface_methods[15])(surface, &duplicate_dc) != (HRESULT)0x8876086cUL ||
        duplicate_dc) return 205;
    rect.left = 0; rect.top = 0; rect.right = 37; rect.bottom = 19;
    if (!FillRect(hdc, &rect, brush)) return 16;
    if (((ReleaseSurfaceDC)surface_methods[16])(surface, 0) != (HRESULT)0x8876086cUL)
        return 206;
    if (((ReleaseSurfaceDC)surface_methods[16])(surface, hdc) != 0 ||
        ((ReleaseSurfaceDC)surface_methods[16])(surface, hdc) !=
            (HRESULT)0x8876086cUL) return 17;
    void *destination = 0;
    if (((CreateTexture)device_methods[23])(device, 37, 19, 1, 0, 22, 0, &destination, 0) != 0 ||
        !destination ||
        ((UpdateTexture)device_methods[31])(device, texture, destination) != 0 ||
        ((UpdateTexture)device_methods[31])(device, destination, texture) !=
            (HRESULT)0x8876086cUL ||
        ((SetTexture)device_methods[65])(device, 0, texture) !=
            (HRESULT)0x8876086aUL ||
        ((SetTexture)device_methods[65])(device, 0, destination) != 0 ||
        ((SetTexture)device_methods[65])(device, 1, destination) !=
            (HRESULT)0x8876086aUL ||
        ((SetTexture)device_methods[65])(device, 0, (void *)0x12345UL) !=
            (HRESULT)0x8876086cUL ||
        ((SetTexture)device_methods[65])(device, 0, 0) != 0) return 81;
    if (((Scene)device_methods[41])(device) != 0 ||
        ((Scene)device_methods[41])(device) != (HRESULT)0x8876086cUL ||
        ((Present)device_methods[17])(device, 0, 0, 0, 0) != (HRESULT)0x8876086cUL ||
        ((Scene)device_methods[42])(device) != 0 ||
        ((Scene)device_methods[42])(device) != (HRESULT)0x8876086cUL) return 82;
    if (((SetState)device_methods[69])(device, 0, 5, 2) != 0 ||
        ((SetState)device_methods[69])(device, 0, 6, 2) != 0 ||
        ((SetState)device_methods[69])(device, 1, 5, 2) !=
            (HRESULT)0x8876086aUL ||
        ((SetState)device_methods[67])(device, 0, 1, 4) != 0 ||
        ((SetState)device_methods[67])(device, 0, 2, 2) != 0 ||
        ((SetState)device_methods[67])(device, 0, 3, 0) != 0 ||
        ((SetState)device_methods[67])(device, 0, 4, 2) != 0 ||
        ((SetState)device_methods[67])(device, 0, 5, 2) != 0 ||
        ((SetState)device_methods[67])(device, 0, 6, 0) != 0 ||
        ((SetState)device_methods[67])(device, 1, 1, 4) !=
            (HRESULT)0x8876086aUL) return 83;
    if (((SetFVF)device_methods[89])(device, 0x144) != 0 ||
        ((SetFVF)device_methods[89])(device, 0) != (HRESULT)0x8876086aUL) return 84;
    void *vertex_buffer = 0;
    if (((CreateVertexBuffer)device_methods[26])(device, 112, 8, 0x144, 1,
            &vertex_buffer, 0) != 0 || !vertex_buffer ||
        ((SetStreamSource)device_methods[100])(device, 0, vertex_buffer, 0, 28) != 0 ||
        ((SetStreamSource)device_methods[100])(device, 1, vertex_buffer, 0, 28) !=
            (HRESULT)0x8876086aUL) return 85;
    void **vertex_methods = *(void ***)vertex_buffer;
    U32 *vertices = 0;
    const U32 quad[28] = {
        0x41a00000, 0x41200000, 0, 0x3f800000, 0xffffffff, 0, 0,
        0x43340000, 0x41200000, 0, 0x3f800000, 0xffffffff, 0x3f800000, 0,
        0x41a00000, 0x42b40000, 0, 0x3f800000, 0xffffffff, 0, 0x3f800000,
        0x43340000, 0x42b40000, 0, 0x3f800000, 0xffffffff, 0x3f800000, 0x3f800000
    };
    if (((VertexLock)vertex_methods[11])(vertex_buffer, 0, 0, (void **)&vertices, 0) != 0 || !vertices) return 87;
    void *duplicate_vertex_lock = 0;
    if (((VertexLock)vertex_methods[11])(vertex_buffer, 0, 0, &duplicate_vertex_lock, 0) !=
            (HRESULT)0x8876086cUL || duplicate_vertex_lock) return 207;
    for (U32 i = 0; i < 28; ++i) vertices[i] = quad[i];
    if (((VertexUnlock)vertex_methods[12])(vertex_buffer) != 0 ||
        ((VertexUnlock)vertex_methods[12])(vertex_buffer) != (HRESULT)0x8876086cUL ||
        ((SetRenderState)device_methods[57])(device, 7, 0) != 0 ||
        ((SetRenderState)device_methods[57])(device, 137, 0) != 0 ||
        ((SetRenderState)device_methods[57])(device, 22, 1) != 0 ||
        ((SetRenderState)device_methods[57])(device, 27, 1) != 0 ||
        ((SetRenderState)device_methods[57])(device, 19, 5) != 0 ||
        ((SetRenderState)device_methods[57])(device, 20, 6) != 0 ||
        ((SetRenderState)device_methods[57])(device, 171, 1) != 0 ||
        ((SetTexture)device_methods[65])(device, 0, destination) != 0 ||
        ((Clear)device_methods[43])(device, 0, 0, 1, 0xff113355UL, 0, 0) != 0 ||
        ((Scene)device_methods[41])(device) != 0 ||
        ((DrawPrimitive)device_methods[81])(device, 5, 0, 2) != 0 ||
        ((Scene)device_methods[42])(device) != 0 ||
        ((Present)device_methods[17])(device, 0, 0, 0, 0) != 0) return 88;
    U32 locked[2] = {0, 0};
    if (((TextureLock)texture_methods[19])(texture, 0, locked, 0, 0x8000) != 0 ||
        locked[0] < 37 * 4 || !locked[1] ||
        ((TextureLock)texture_methods[19])(texture, 0, locked, 0, 0x8000) != (HRESULT)0x8876086cUL) return 90;
    for (U32 y = 0; y < 19; ++y) {
        U32 *row = (U32 *)(locked[1] + y * locked[0]);
        for (U32 x = 0; x < 37; ++x) row[x] = 0x00557799;
    }
    if (((TextureUnlock)texture_methods[20])(texture, 0) != 0) return 94;
    if (((TextureUnlock)texture_methods[20])(texture, 0) != (HRESULT)0x8876086cUL) return 95;
    Rect patch = {2, 3, 3, 4}, invalid_patch = {37, 0, 38, 1};
    U32 full_pitch = locked[0], full_bits = locked[1];
    if (((TextureLock)texture_methods[19])(texture, 0, locked, &patch, 0x8000) != 0 ||
        locked[0] != full_pitch || locked[1] != full_bits + 3 * full_pitch + 2 * 4 ||
        ((TextureUnlock)texture_methods[20])(texture, 0) != 0 ||
        ((TextureLock)texture_methods[19])(texture, 0, locked, &invalid_patch, 0x8000) !=
            (HRESULT)0x8876086cUL) return 102;
    if (((AddDirtyRect)texture_methods[21])(texture, 0) != 0) return 96;
    if (((UpdateTexture)device_methods[31])(device, texture, destination) != 0) return 97;
    /* Dynamic default-pool textures accept only a full-level DISCARD lock. */
    U32 discard_output[2] = {0x12345678, 0x87654321};
    if (((TextureLock)texture_methods[19])(texture, 0, discard_output, 0, 0x2000) !=
            (HRESULT)0x8876086cUL || discard_output[0] != 0x12345678 ||
            discard_output[1] != 0x87654321) return 108;
    void *dynamic_texture = 0;
    if (((CreateTexture)device_methods[23])(device, 37, 19, 1, 0x200, 22, 0,
            &dynamic_texture, 0) != 0 || !dynamic_texture) return 109;
    void **dynamic_methods = *(void ***)dynamic_texture;
    if (((TextureLock)dynamic_methods[19])(dynamic_texture, 0, discard_output, &patch, 0x2000) !=
            (HRESULT)0x8876086cUL || discard_output[0] != 0x12345678 ||
            discard_output[1] != 0x87654321 ||
        ((TextureLock)dynamic_methods[19])(dynamic_texture, 0, discard_output, 0, 0x2010) !=
            (HRESULT)0x8876086cUL || discard_output[0] != 0x12345678 ||
            discard_output[1] != 0x87654321) return 110;
    if (((TextureLock)dynamic_methods[19])(dynamic_texture, 0, locked, 0, 0x2000) != 0 ||
        locked[0] < 37 * 4 || !locked[1] ||
        ((TextureLock)dynamic_methods[19])(dynamic_texture, 0, discard_output, 0, 0x2000) !=
            (HRESULT)0x8876086cUL || discard_output[0] != 0x12345678 ||
            discard_output[1] != 0x87654321) return 111;
    for (U32 y = 0; y < 19; ++y) {
        volatile U32 *row = (volatile U32 *)(locked[1] + y * locked[0]);
        for (U32 x = 0; x < 37; ++x) row[x] = 0x00557799;
    }
    if (((TextureUnlock)dynamic_methods[20])(dynamic_texture, 0) != 0 ||
        ((SetTexture)device_methods[65])(device, 0, dynamic_texture) != 0) return 112;
    /* Two live 8 MiB textures must coexist with the other graphics resources. */
    void *large_textures[2] = {0, 0};
    U32 large_bits[2] = {0, 0};
    for (U32 i = 0; i < 2; ++i) {
        if (((CreateTexture)device_methods[23])(device, 2048, 1024, 1, 0x200, 22, 0,
                &large_textures[i], 0) != 0 || !large_textures[i]) return 114;
        void **large_methods = *(void ***)large_textures[i];
        if (((TextureLock)large_methods[19])(large_textures[i], 0, locked, 0, 0x2000) != 0 ||
            locked[0] != 8192 || !locked[1]) return 115;
        large_bits[i] = locked[1];
        volatile U32 *first = (volatile U32 *)locked[1];
        volatile U32 *last = (volatile U32 *)(locked[1] + 8192 * 1024 - 4);
        *first = 0x12345678 + i; *last = 0x87654321 + i;
        if (*first != 0x12345678 + i || *last != 0x87654321 + i ||
            ((TextureUnlock)large_methods[20])(large_textures[i], 0) != 0) return 116;
    }
    if (large_bits[0] == large_bits[1] ||
        *(volatile U32 *)large_bits[0] != 0x12345678 ||
        *(volatile U32 *)(large_bits[0] + 8192 * 1024 - 4) != 0x87654321) return 117;
    for (U32 i = 0; i < 2; ++i) {
        if (((Release)(*(void ***)large_textures[i])[2])(large_textures[i]) != 0) return 118;
    }
    if (((Clear)device_methods[43])(device, 0, 0, 1, 0xff113355UL, 0, 0) != 0) return 98;
    if (((Scene)device_methods[41])(device) != 0) return 99;
    if (((DrawPrimitive)device_methods[81])(device, 5, 0, 2) != 0) return 100;
    /* Re-lock and replace source bytes after DrawPrimitive. Present must use the snapshot. */
    if (((TextureLock)texture_methods[19])(texture, 0, locked, 0, 0x8000) != 0) return 92;
    for (U32 y = 0; y < 19; ++y) {
        U32 *row = (U32 *)(locked[1] + y * locked[0]);
        for (U32 x = 0; x < 37; ++x) row[x] = 0;
    }
    if (((TextureUnlock)texture_methods[20])(texture, 0) != 0) return 93;
    /* Re-DISCARD after submission must not alter the draw's copied texels. */
    if (((TextureLock)dynamic_methods[19])(dynamic_texture, 0, locked, 0, 0x2000) != 0) return 113;
    for (U32 y = 0; y < 19; ++y) {
        volatile U32 *row = (volatile U32 *)(locked[1] + y * locked[0]);
        for (U32 x = 0; x < 37; ++x) row[x] = 0;
    }
    if (((TextureUnlock)dynamic_methods[20])(dynamic_texture, 0) != 0 ||
        ((Scene)device_methods[42])(device) != 0 ||
        ((Present)device_methods[17])(device, 0, 0, 0, 0) != 0) return 93;
#ifdef VINE_POINTER_FIXTURE
    /* After actual Present, allow browser input to reach the logical foreground window.
       Exercise both Peek/Dispatch and the guest window procedure, not only Rust's cursor state. */
    U32 down = 0, up = 0;
    for (U32 attempt = 0; attempt < 20000 && (!down || !up); ++attempt) {
        Message msg;
        if (!PeekMessageA(&msg, 0, 0, 0, 1)) { Sleep(1); continue; }
        if (msg.message == 0x0201 || msg.message == 0x0202) {
            if (msg.window != window || (msg.l_param & 0xffff) >= 206 ||
                (msg.l_param >> 16) >= 129) return 103;
            if (msg.message == 0x0201) { if (!(msg.w_param & 1)) return 104; down = 1; }
            else { if (msg.w_param & 1) return 105; up = 1; }
        }
        if (TranslateMessage(&msg) != 0 || DispatchMessageA(&msg) != 0) return 106;
    }
    if (!down || !up) return 107;
#endif
    if (((Release)dynamic_methods[2])(dynamic_texture) != 0 ||
        ((Release)vertex_methods[2])(vertex_buffer) != 0 ||
        ((Release)(*(void ***)destination)[2])(destination) != 0) return 89;
    if (((Release)surface_methods[2])(surface) != 0 ||
        ((Release)texture_methods[2])(texture) != 0 ||
        ((Release)device_methods[2])(device) != 0 ||
        ((Release)methods[2])(object) != 0) return 18;
    return 0;
}
