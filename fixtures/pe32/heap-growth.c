typedef unsigned int UINT32;
typedef unsigned int UINTPTR;
typedef void *HANDLE;
_Static_assert(sizeof(void *) == 4, "fixture requires PE32 pointers");
__declspec(dllimport) HANDLE __stdcall HeapCreate(UINT32 flags, UINT32 initial_size, UINT32 maximum_size);
__declspec(dllimport) void *__stdcall HeapAlloc(HANDLE heap, UINT32 flags, UINT32 bytes);
__declspec(dllimport) UINT32 __stdcall HeapSize(HANDLE heap, UINT32 flags, const void *block);
__declspec(dllimport) int __stdcall HeapFree(HANDLE heap, UINT32 flags, void *block);

static UINT32 page_offset(UINT32 bytes, UINT32 page) {
  if (page == 0) return 0;
  if (page == 1) return (bytes / 2u) & ~4095u;
  return (bytes - 1u) & ~4095u;
}

static int zero_and_mark(volatile unsigned char *block, UINT32 bytes,
                         unsigned char marker) {
  UINT32 page;
  for (page = 0; page < 3; ++page) {
    UINT32 offset = page_offset(bytes, page);
    UINT32 length = bytes - offset < 4096u ? bytes - offset : 4096u;
    UINT32 i;
    for (i = 0; i < length; ++i) if (block[offset + i] != 0) return 0;
    block[offset] = (unsigned char)(marker + page);
    block[offset + length - 1u] = (unsigned char)(marker + page + 8u);
  }
  return 1;
}

static int marks_match(volatile unsigned char *block, UINT32 bytes,
                       unsigned char marker) {
  UINT32 page;
  for (page = 0; page < 3; ++page) {
    UINT32 offset = page_offset(bytes, page);
    UINT32 length = bytes - offset < 4096u ? bytes - offset : 4096u;
    if (block[offset] != (unsigned char)(marker + page) ||
        block[offset + length - 1u] != (unsigned char)(marker + page + 8u)) return 0;
  }
  return 1;
}

static int ranges_distinct(void *const *blocks, const UINT32 *sizes) {
  UINT32 i;
  for (i = 0; i < 3; ++i) {
    UINTPTR start = (UINTPTR)blocks[i];
    UINT32 j;
    if (!start || start > 0xffffffffu - sizes[i]) return 0;
    for (j = 0; j < i; ++j) {
      UINTPTR other = (UINTPTR)blocks[j];
      if (start < other + sizes[j] && other < start + sizes[i]) return 0;
    }
  }
  return 1;
}

int __cdecl fixture_entry(void) {
  const UINT32 sizes[3] = {33177632u, 47u * 1024u * 1024u, 64u * 1024u * 1024u + 32u};
  unsigned char markers[3] = {0x21u, 0x41u, 0x61u};
  void *blocks[3];
  HANDLE heap = HeapCreate(0, 0, 0);
  UINT32 i;
  UINT32 replacement_size;
  if (!heap) return 1;
  for (i = 0; i < 3; ++i) {
    UINT32 size;
    blocks[i] = HeapAlloc(heap, 8u, sizes[i]);
    if (!blocks[i]) return 2 + (int)i;
    size = HeapSize(heap, 0, blocks[i]);
    if (size == 0xffffffffu || size < sizes[i]) return 5 + (int)i;
    if (!zero_and_mark((volatile unsigned char *)blocks[i], sizes[i], markers[i])) return 8 + (int)i;
  }
  if (!ranges_distinct(blocks, sizes)) return 11;
  for (i = 0; i < 3; ++i) {
    if (!marks_match((volatile unsigned char *)blocks[i], sizes[i], markers[i])) return 12 + (int)i;
  }
  if (!HeapFree(heap, 0, blocks[1])) return 15;
  blocks[1] = HeapAlloc(heap, 8u, sizes[1]);
  if (!blocks[1]) return 16;
  replacement_size = HeapSize(heap, 0, blocks[1]);
  if (replacement_size < sizes[1] || replacement_size == 0xffffffffu) return 17;
  markers[1] = 0x81u;
  if (!zero_and_mark((volatile unsigned char *)blocks[1], sizes[1], markers[1])) return 18;
  if (!ranges_distinct(blocks, sizes)) return 19;
  for (i = 0; i < 3; ++i) {
    if (!marks_match((volatile unsigned char *)blocks[i], sizes[i], markers[i])) return 20 + (int)i;
    if (!HeapFree(heap, 0, blocks[i])) return 23 + (int)i;
  }
  return 0;
}
