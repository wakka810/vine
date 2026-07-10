typedef unsigned int UINT32;
typedef unsigned int UINTPTR;

enum {
  FIXTURE_READONLY_VALUE = 0x11223344u,
  FIXTURE_WRITABLE_VALUE = 0x11223345u,
  FIXTURE_READONLY_WRITE_VALUE = 0xa1b2c3d4u,
  FIXTURE_WRITABLE_WRITE_VALUE = 0x55667788u,
  FIXTURE_NONPRESENT_ADDRESS = 0xe1000000u,
  FIXTURE_CROSS_PAGE_ADDRESS = 0xe2000ffeu,
  FIXTURE_CROSS_PAGE_WRITE_VALUE = 0x89abcdefu,
};

_Static_assert(sizeof(void *) == 4, "fixture requires i386 pointers");
_Static_assert(sizeof(UINT32) == 4, "fixture requires 32-bit UINT32");
_Static_assert(sizeof(UINTPTR) == sizeof(void *),
               "fixture requires 32-bit UINTPTR");

#pragma section(".rprobe", read)
__declspec(allocate(".rprobe")) __declspec(dllexport) volatile UINT32
    fixture_readonly = FIXTURE_READONLY_VALUE;

__declspec(dllexport) volatile UINT32 fixture_writable;

#pragma section(".nxprobe", read)
__declspec(allocate(".nxprobe")) __declspec(dllexport) const unsigned char
    fixture_nonexec_code[] = {
        0xb8, 0x78, 0x56, 0x34, 0x12, /* mov eax, 0x12345678 */
        0xc3,                         /* ret */
};

__declspec(dllexport) int __cdecl fixture_entry(void) {
  fixture_writable = fixture_readonly + 1u;
  return fixture_writable == FIXTURE_WRITABLE_VALUE ? 0 : 1;
}

__declspec(dllexport) void __cdecl fixture_write_readonly(void) {
  fixture_readonly = FIXTURE_READONLY_WRITE_VALUE;
}

__declspec(dllexport) void __cdecl fixture_write_writable(void) {
  fixture_writable = FIXTURE_WRITABLE_WRITE_VALUE;
}

__declspec(dllexport) UINT32 __cdecl fixture_read_nonpresent(void) {
  volatile const UINT32 *address =
      (volatile const UINT32 *)(UINTPTR)FIXTURE_NONPRESENT_ADDRESS;
  return *address;
}

__declspec(dllexport) void __cdecl fixture_write_nonpresent(void) {
  volatile UINT32 *address =
      (volatile UINT32 *)(UINTPTR)FIXTURE_NONPRESENT_ADDRESS;
  *address = FIXTURE_WRITABLE_WRITE_VALUE;
}

__declspec(dllexport) void __cdecl fixture_cross_page_write(void) {
  volatile UINT32 *address =
      (volatile UINT32 *)(UINTPTR)FIXTURE_CROSS_PAGE_ADDRESS;
  *address = FIXTURE_CROSS_PAGE_WRITE_VALUE;
}

__declspec(dllexport) void __cdecl fixture_return_trap(void) {
  __builtin_trap();
}
