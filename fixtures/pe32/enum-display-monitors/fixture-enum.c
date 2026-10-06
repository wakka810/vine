typedef struct Rect { int left, top, right, bottom; } Rect;
typedef int (__stdcall *MonitorCallback)(void *, void *, const Rect *, unsigned long);
__declspec(dllimport) int __stdcall EnumDisplayMonitors(
    void *hdc, const Rect *clip, MonitorCallback callback, unsigned long data);
__declspec(dllimport) int __stdcall GetSystemMetrics(int index);

static int __stdcall on_monitor(void *monitor, void *hdc, const Rect *rect,
                                unsigned long data) {
    if (!monitor || hdc || !rect || rect->left != 0 || rect->top != 0 ||
        rect->right != 1920 || rect->bottom != 1080 || data != 0x12345678UL)
        return 0;
    return 1;
}

int fixture_entry(void) {
    if (EnumDisplayMonitors(0, 0, on_monitor, 0x12345678UL) != 1)
        return 1;
    if (GetSystemMetrics(0) != 1920 || GetSystemMetrics(1) != 1080 ||
        GetSystemMetrics(4) != 23 || GetSystemMetrics(7) != 3 ||
        GetSystemMetrics(8) != 3 || GetSystemMetrics(49) != 16 ||
        GetSystemMetrics(50) != 16)
        return 2;
    return 0;
}
