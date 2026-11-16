typedef unsigned int DWORD;
int _fltused = 0;
__declspec(dllimport) void *__stdcall VirtualAlloc(void *, DWORD, DWORD, DWORD);
__declspec(dllimport) int __stdcall VirtualFree(void *, DWORD, DWORD);
__declspec(dllimport) void __stdcall SetLastError(DWORD);
__declspec(dllimport) DWORD __stdcall GetLastError(void);
#ifndef VINE_FAULT_CASE
#define VINE_FAULT_CASE 0
#endif
__declspec(allocate(".testcode")) volatile unsigned char test_code[4096];
__declspec(allocate(".testro")) const volatile unsigned char read_only[4096] = {0x5a};
static const double two = 2.0, three = 3.0;

static void code(volatile unsigned char *p, unsigned char value) {
    p[0] = 0xb8; p[1] = value; p[2] = 0; p[3] = 0; p[4] = 0; p[5] = 0xc3;
}

int __cdecl fixture_entry(void) {
    volatile unsigned char *p = VirtualAlloc((void *)0, 8192, 0x102000, 4);
    if (!p || VirtualAlloc((void *)p, 8192, 0x1000, 4) != (void *)p) return 1;
    for (DWORD i = 0; i < 20000; i++) {
        *(volatile DWORD *)(p + 32) = i;
        if (*(volatile DWORD *)(p + 32) != i) return 2;
    }
    if (*(volatile DWORD *)(p + 32) != 19999) return 4;
    for (DWORD i = 0; i < 12000; i++) {
        __asm__ volatile("fldl %1; fmull %2; fstpl %0"
            : "=m"(*(volatile double *)(p + 64)) : "m"(two), "m"(three) : "st");
        if (*(volatile double *)(p + 64) != 6.0) return 20;
    }
    code(test_code, 1);
    int (__cdecl *f)(void) = (int (__cdecl *)(void))test_code;
    for (DWORD i = 0; i < 12000; i++) if (f() != 1) return 6;
    test_code[1] = 2;
    for (DWORD i = 0; i < 12000; i++) if (f() != 2) return 7;
    if (!VirtualFree((void *)p, 8192, 0x4000)) return 8;
    if (VirtualAlloc((void *)p, 8192, 0x1000, 4) != (void *)p) return 9;
    if (*(volatile DWORD *)(p + 32) != 0 || p[4096] != 0) return 10;
    code(test_code, 3);
    if (f() != 3) return 12;
    volatile DWORD *teb;
    __asm__ volatile("movl %%fs:0x18, %0" : "=r"(teb));
    for (DWORD i = 0; i < 1024; i++) {
        SetLastError(i + 0x7000);
        if (GetLastError() != i + 0x7000) return 15;
        // This CPU store is above the signed 2 GiB physical-address boundary.
        teb[0x3e0] = i + 0x8000;
        if (teb[0x3e0] != i + 0x8000) return 19;
    }
    p[0] = 0x5a;
    if (VINE_FAULT_CASE == 1) {
        *(volatile unsigned char *)read_only = 0xa5;
    } else if (VINE_FAULT_CASE == 2) {
        if (!VirtualFree((void *)p, 8192, 0x4000)) return 17;
        p[0] = 0xa5;
    } else if (VINE_FAULT_CASE == 3 || VINE_FAULT_CASE == 5) {
        p[4094] = 0x33; p[4095] = 0x22; p[4096] = 0x11; p[4097] = 0x88;
        if (!VirtualFree((void *)(p + 4096), 4096, 0x4000)) return 18;
        if (VINE_FAULT_CASE == 5) {
            __asm__ volatile("fld1; fstps %0" : "=m"(*(volatile float *)(p + 4094)) : : "st");
        } else *(volatile DWORD *)(p + 4094) = 0x99887766;
    } else if (VINE_FAULT_CASE == 4) {
        *(volatile DWORD *)0xfffff000 = 0xaabbccdd;
    }
    return 0;
}
