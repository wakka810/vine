typedef unsigned int UINT32;
typedef unsigned long ULONG;

extern ULONG __readfsdword(ULONG offset);
#pragma intrinsic(__readfsdword)

_Static_assert(sizeof(void *) == 4, "fixture requires i386 pointers");
_Static_assert(sizeof(UINT32) == 4, "fixture requires 32-bit UINT32");
_Static_assert(sizeof(ULONG) == 4, "fixture requires 32-bit ULONG");

__declspec(dllexport) volatile UINT32 fixture_teb;
__declspec(dllexport) volatile UINT32 fixture_peb;
__declspec(dllexport) volatile UINT32 fixture_teb_self;
__declspec(dllexport) volatile UINT32 fixture_teb_peb;

__declspec(dllexport) int __cdecl fixture_entry(void) {
  UINT32 teb = (UINT32)__readfsdword(0x18u);
  UINT32 peb = (UINT32)__readfsdword(0x30u);

  fixture_teb = teb;
  fixture_peb = peb;
  if (teb == 0u) {
    return 1;
  }
  if (peb == 0u) {
    return 2;
  }

  fixture_teb_self = *(volatile const UINT32 *)(teb + 0x18u);
  fixture_teb_peb = *(volatile const UINT32 *)(teb + 0x30u);
  if (fixture_teb_self != teb) {
    return 3;
  }
  if (fixture_teb_peb != peb) {
    return 4;
  }
  return 0;
}
