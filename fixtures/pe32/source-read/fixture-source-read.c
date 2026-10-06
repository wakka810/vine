typedef unsigned long fixture_handle;

__declspec(dllimport) fixture_handle __stdcall
CreateFileA(const char *file_name, unsigned long desired_access,
            unsigned long share_mode, void *security_attributes,
            unsigned long creation_disposition,
            unsigned long flags_and_attributes, fixture_handle template_file);
__declspec(dllimport) unsigned long __stdcall GetFileType(fixture_handle file);
__declspec(dllimport) unsigned long __stdcall
GetFileSize(fixture_handle file, unsigned long *file_size_high);
__declspec(dllimport) unsigned long __stdcall GetFileAttributesA(const char *path);
__declspec(dllimport) fixture_handle __stdcall FindFirstFileA(const char *pattern, void *find_data);
__declspec(dllimport) int __stdcall FindNextFileA(fixture_handle search, void *find_data);
__declspec(dllimport) int __stdcall FindClose(fixture_handle search);
__declspec(dllimport) void __stdcall SetLastError(unsigned long error);
__declspec(dllimport) int __stdcall
ReadFile(fixture_handle file, void *buffer, unsigned long bytes_to_read,
         unsigned long *bytes_read, void *overlapped);
__declspec(dllimport) unsigned long __stdcall
SetFilePointer(fixture_handle file, long distance_low, long *distance_high,
               unsigned long move_method);
__declspec(dllimport) int __stdcall
WriteFile(fixture_handle file, const void *buffer, unsigned long bytes_to_write,
          unsigned long *bytes_written, void *overlapped);
__declspec(dllimport) int __stdcall FlushFileBuffers(fixture_handle file);
__declspec(dllimport) int __stdcall CloseHandle(fixture_handle object);
__declspec(dllimport) fixture_handle __stdcall LoadLibraryA(const char *module_name);
__declspec(dllimport) unsigned long __stdcall GetLastError(void);
__declspec(dllimport) int __stdcall CreateDirectoryA(const char *path, void *security_attributes);
__declspec(dllimport) int __stdcall DeleteFileA(const char *path);
__declspec(dllimport) int __stdcall
WideCharToMultiByte(unsigned long code_page, unsigned long flags,
                    const unsigned short *source, int source_count,
                    char *destination, int capacity, const char *default_char,
                    int *used_default);
__declspec(dllimport) int __stdcall
MultiByteToWideChar(unsigned long code_page, unsigned long flags,
                    const char *source, int source_count,
                    unsigned short *destination, int capacity);
__declspec(dllimport) int __stdcall
GetStringTypeW(unsigned long info_type, const unsigned short *source,
               int source_count, unsigned short *types);
__declspec(dllimport) int __stdcall
LCMapStringW(unsigned long locale, unsigned long flags,
             const unsigned short *source, int source_count,
             unsigned short *destination, int capacity);

__declspec(dllimport) fixture_handle __stdcall
HeapCreate(unsigned long flags, unsigned long initial_size, unsigned long maximum_size);
__declspec(dllimport) void *__stdcall
HeapAlloc(fixture_handle heap, unsigned long flags, unsigned long size);
__declspec(dllimport) void *__stdcall
HeapReAlloc(fixture_handle heap, unsigned long flags, void *allocation, unsigned long size);
__declspec(dllimport) unsigned long __stdcall
HeapSize(fixture_handle heap, unsigned long flags, const void *allocation);
__declspec(dllimport) int __stdcall
HeapFree(fixture_handle heap, unsigned long flags, void *allocation);
__declspec(dllimport) void *__stdcall
VirtualAlloc(void *address, unsigned long size, unsigned long allocation_type,
             unsigned long protection);
__declspec(dllimport) int __stdcall
VirtualFree(void *address, unsigned long size, unsigned long free_type);

static const char fixture_source_path[] =
    "fixtures\\storage\\source-read-v1.dat";
static const char fixture_write_probe_path[] =
    "fixtures\\storage\\write-probe.dat";
static const unsigned char fixture_prefix[] = {'V', 'i', 'n', 'e', ' ', 'i', 'm', 'm'};
static const unsigned char fixture_seek_target[] = {
    's', 'e', 'e', 'k', '-', 't', 'a', 'r', 'g', 'e', 't'};

static int bytes_equal(const unsigned char *left, const unsigned char *right,
                       unsigned long length) {
  unsigned long index;
  for (index = 0; index < length; index += 1) {
    if (left[index] != right[index]) {
      return 0;
    }
  }
  return 1;
}

int __cdecl fixture_entry(void) {
  unsigned char buffer[11];
  unsigned long transferred = 0xffffffffu;
  unsigned long size_high = 0xffffffffu;
  fixture_handle file;
  fixture_handle search;
  union {
    unsigned long words[80];
    unsigned char bytes[320];
  } find_data;
  int result = 0;

  // Fixed Japanese locale calls cross the real PE32/Wasm/CPU memory boundary.
  {
    const unsigned short wide[3] = {0x0041, 0x3042, 0};
    const unsigned char encoded[4] = {0x41, 0x82, 0xa0, 0};
    unsigned short decoded[3] = {0xffff, 0xffff, 0xffff};
    unsigned short mapped[3] = {0xffff, 0xffff, 0xffff};
    unsigned short types[3] = {0xffff, 0xffff, 0xffff};
    unsigned char bytes[4] = {0xff, 0xff, 0xff, 0xff};
    int used_default = -1;
    SetLastError(0x5678u);
    if (WideCharToMultiByte(0u, 0u, wide, -1, 0, 0, 0, 0) != 4 ||
        GetLastError() != 0x5678u) return 100;
    if (WideCharToMultiByte(932u, 0u, wide, -1, (char *)bytes, 4, 0,
                            &used_default) != 4 || used_default != 0 ||
        !bytes_equal(bytes, encoded, 4u) || GetLastError() != 0x5678u) return 101;
    if (MultiByteToWideChar(0u, 0u, (const char *)encoded, -1, 0, 0) != 3 ||
        GetLastError() != 0x5678u) return 102;
    if (MultiByteToWideChar(932u, 1u, (const char *)encoded, 4, decoded, 3) != 3 ||
        !bytes_equal((const unsigned char *)decoded,
                     (const unsigned char *)wide, 6u) ||
        GetLastError() != 0x5678u) return 103;
    if (!GetStringTypeW(1u, wide, 3, types) || types[0] != 897u ||
        types[1] != 768u || types[2] != 544u || GetLastError() != 0x5678u) return 104;
    if (LCMapStringW(0x0411u, 0x0100u, wide, 3, mapped, 3) != 3 ||
        mapped[0] != 0x0061u || mapped[1] != 0x3042u || mapped[2] != 0u ||
        GetLastError() != 0x5678u) return 105;
    if (LCMapStringW(0x0411u, 0x0200u, mapped, 3, decoded, 3) != 3 ||
        !bytes_equal((const unsigned char *)decoded,
                     (const unsigned char *)wide, 6u)) return 106;
    if (MultiByteToWideChar(932u, 0u, (const char *)encoded, 4, decoded, 1) != 0 ||
        GetLastError() != 122u || decoded[0] != 0x0041u) return 107;
    // PE32/x86 byte-addressed UTF-16 input is checked for bounds and read
    // protection, not rejected by an unrelated opaque-pointer alignment rule.
    {
      const unsigned short shifted[4] = {0x4100, 0x4200, 0x0030, 0};
      const unsigned short *unaligned =
          (const unsigned short *)((const unsigned char *)shifted + 1);
      SetLastError(0x5678u);
      if (WideCharToMultiByte(932u, 0u, unaligned, 3, (char *)bytes, 4, 0, 0) != 4 ||
          !bytes_equal(bytes, encoded, 4u) || GetLastError() != 0x5678u) return 136;
      if (!GetStringTypeW(1u, unaligned, 3, types) || types[0] != 897u ||
          types[1] != 768u || types[2] != 544u || GetLastError() != 0x5678u) return 137;
      if (LCMapStringW(0x0411u, 0x0100u, unaligned, 3, mapped, 3) != 3 ||
          mapped[0] != 0x0061u || mapped[1] != 0x3042u || mapped[2] != 0u ||
          GetLastError() != 0x5678u) return 138;
    }
  }

  // Ordinary non-profiled heap sizes, guest writes, growth, and retirement
  // exercise both authoritative Rust memory and the CPU mirror.
  {
    fixture_handle heap = HeapCreate(0u, 4096u, 0u);
    unsigned char *allocation;
    unsigned char *grown;
    unsigned long index;
    if (!heap) return 108;
    SetLastError(0x6789u);
    allocation = (unsigned char *)HeapAlloc(heap, 8u, 17u);
    if (!allocation || HeapSize(heap, 1u, allocation) != 17u ||
        GetLastError() != 0x6789u) return 109;
    for (index = 0; index < 17u; index += 1) {
      if (allocation[index] != 0u) return 110;
      allocation[index] = (unsigned char)(index + 1u);
    }
    if (HeapFree(heap, 0u, allocation + 1u) || GetLastError() != 87u) return 200;
    if (HeapReAlloc(heap, 8u, allocation + 1u, 37u) || GetLastError() != 87u) return 201;
    SetLastError(0x6789u);
    grown = (unsigned char *)HeapReAlloc(heap, 8u, allocation, 37u);
    if (!grown || HeapSize(heap, 0u, grown) != 37u ||
        GetLastError() != 0x6789u) return 111;
    for (index = 0; index < 37u; index += 1) {
      if (grown[index] != (index < 17u ? (unsigned char)(index + 1u) : 0u)) return 112;
    }
    if (HeapAlloc(heap, 0u, 0x7ffffff8u) != 0 || GetLastError() != 0x6789u ||
        HeapSize(heap, 0u, grown) != 37u) return 113;
    if (!HeapFree(heap, 0u, grown) || GetLastError() != 0x6789u) return 114;
    if (HeapSize(heap, 0u, grown) != 0xffffffffu || GetLastError() != 87u) return 115;

    // Valid lazy mappings are bounded by Rust's physical memory quota, not a
    // browser lifetime count of previously serviced stack/heap page faults.
    SetLastError(0x6789u);
    allocation = (unsigned char *)HeapAlloc(heap, 8u, 17u * 1024u * 1024u);
    if (!allocation) return 132;
    for (index = 0; index < 1025u; index += 1) {
      if (allocation[index * 4096u] != 0u) return 133;
      allocation[index * 4096u] = (unsigned char)(index + 1u);
    }
    for (index = 0; index < 1025u; index += 1) {
      if (allocation[index * 4096u] != (unsigned char)(index + 1u)) return 134;
    }
    if (HeapSize(heap, 0u, allocation) != 17u * 1024u * 1024u ||
        GetLastError() != 0x6789u || !HeapFree(heap, 0u, allocation) ||
        GetLastError() != 0x6789u) return 135;
  }

  // Reservation, mapping, CPU writes, and decommit/recommit cross both memory
  // authorities. No historical exact-size allowlist selects this 7-page range.
  {
    unsigned char *reserved;
    unsigned char *committed;
    unsigned long index;
    SetLastError(0x789au);
    reserved = (unsigned char *)VirtualAlloc(0, 7u * 4096u, 0x00102000u, 4u);
    if (!reserved || GetLastError() != 0x789au) return 116;
    committed = (unsigned char *)VirtualAlloc(reserved, 4097u, 0x1000u, 4u);
    if (committed != reserved || GetLastError() != 0x789au) return 117;
    for (index = 0; index < 8192u; index += 1) {
      if (committed[index] != 0u) return 118;
      committed[index] = (unsigned char)(index + 1u);
    }
    if (!VirtualFree(reserved, 4097u, 0x4000u) || GetLastError() != 0x789au) return 119;
    committed = (unsigned char *)VirtualAlloc(reserved, 4096u, 0x1000u, 4u);
    if (committed != reserved || GetLastError() != 0x789au) return 120;
    for (index = 0; index < 4096u; index += 1) {
      if (committed[index] != 0u) return 121;
    }
    committed[4095] = 42u;
    if (VirtualAlloc(reserved + 7u * 4096u, 4096u, 0x1000u, 4u) != 0 ||
        GetLastError() != 487u || committed[4095] != 42u) return 122;
    if (VirtualFree(reserved + 1u, 4096u, 0x4000u) ||
        GetLastError() != 87u || committed[4095] != 42u) return 123;
  }

  // An attached statically imported DLL must be reusable without loading or
  // modifying any physical source DLL, regardless of its graph dependents.
  {
    fixture_handle first;
    SetLastError(0x89abu);
    first = LoadLibraryA("KERNEL32.DLL");
    if (!first || GetLastError() != 0x89abu) return 124;
    if (LoadLibraryA("kernel32.dll") != first || GetLastError() != 0x89abu) return 125;
  }

  // This asset is not registered yet: a write open must discover it as
  // immutable without creating a writable handle or touching the source.
  if (CreateFileA(fixture_write_probe_path, 0x40000000u, 1u, 0, 3u,
                  0x08000020u, 0u) != 0xffffffffu || GetLastError() != 5u) return 39;
  if (CreateFileA(fixture_write_probe_path, 0x80000000u, 1u, 0, 2u,
                  0x08000020u, 0u) != 0xffffffffu || GetLastError() != 5u) return 40;
  // Guest-originated OPFS directories survive a fresh worker/runtime launch.
  if (!CreateDirectoryA("GuestState", 0) && GetLastError() != 183u) return 42;
  if (CreateDirectoryA("GUESTSTATE", 0) || GetLastError() != 183u) return 43;
  SetLastError(0x2345u);
  if (CreateDirectoryA("GuestState\\Slot01", 0)) {
    if (GetLastError() != 0x2345u) return 49;
  } else if (GetLastError() != 183u) return 44;
  if (CreateDirectoryA("MissingParent\\Child", 0) || GetLastError() != 3u) return 45;
  if (CreateDirectoryA("C:\\Outside\\Unsafe", 0) || GetLastError() != 3u) return 46;
  if (CreateDirectoryA("fixtures\\empty", 0) || GetLastError() != 183u) return 47;
  if (CreateDirectoryA(fixture_write_probe_path, 0) || GetLastError() != 183u) return 48;
  if (CreateFileA("fixtures\\storage\\absent-save.dat", 0x80000000u, 1u,
                  0, 3u, 0x08000020u, 0) != 0xffffffffu || GetLastError() != 2u) return 76;
  if (CreateFileA("UnknownParent\\absent-save.dat", 0x40000000u, 0u,
                  0, 4u, 0x08000020u, 0) != 0xffffffffu || GetLastError() != 3u) return 77;
  // A source-only directory can contain a guest-created OPFS file. This tests
  // open/close/reopen, not yet a successful guest WriteFile or save/load.
  {
    const char *save_path = "fixtures\\storage\\empty-save.dat";
    unsigned long existed = GetFileAttributesA(save_path) != 0xffffffffu;
    SetLastError(0x3456u);
    file = CreateFileA(save_path, 0x40000000u, 0u, 0, 4u, 0x08000020u, 0u);
    if (file == 0xffffffffu || GetLastError() != (existed ? 183u : 0u)) return 70;
    SetLastError(0x3456u);
    if (GetFileType(file) != 1u || GetLastError() != 0x3456u) return 128;
    if (GetFileSize(file, 0) != 0u) return 71;
    if (CreateFileA(save_path, 0x80000000u, 1u, 0, 3u,
                    0x08000020u, 0u) != 0xffffffffu || GetLastError() != 32u) return 72;
    if (!CloseHandle(file)) return 73;
    if (GetFileType(file) != 0u || GetLastError() != 6u) return 129;
    SetLastError(0x4567u);
    file = CreateFileA(save_path, 0x80000000u, 1u, 0, 3u, 0x08000020u, 0u);
    if (file == 0xffffffffu || GetLastError() != 0x4567u ||
        GetFileType(file) != 1u || GetFileSize(file, 0) != 0u ||
        GetLastError() != 0x4567u) return 74;
    transferred = 0xdeadbeefu;
    if (!ReadFile(file, buffer, 4u, &transferred, 0) || transferred != 0u ||
        GetLastError() != 0u || !CloseHandle(file)) return 78;
    if (GetFileAttributesA(save_path) != 0x20u) return 75;
  }
  // A real guest save must survive close and a fresh worker/runtime launch.
  {
    const char *save_path = "fixtures\\storage\\guest-save.dat";
    const unsigned char initial_bytes[5] = {0x31, 0x00, 0x00, 0x42, 0x07};
    const unsigned char save_bytes[5] = {0x31, 0x00, 0xff, 0x42, 0x07};
    unsigned long existed = GetFileAttributesA(save_path) == 0x20u;
    if (existed) {
      file = CreateFileA(save_path, 0x80000000u, 1u, 0, 3u, 0x08000020u, 0u);
      if (file == 0xffffffffu || GetFileSize(file, 0) != 5u) return 79;
      transferred = 0u;
      if (!ReadFile(file, buffer, 5u, &transferred, 0) || transferred != 5u ||
          !bytes_equal(buffer, save_bytes, 5u) || !CloseHandle(file)) return 80;
    }
    file = CreateFileA(save_path, 0xc0000000u, 0u, 0, 2u, 0x08000020u, 0u);
    if (file == 0xffffffffu || GetLastError() != (existed ? 183u : 0u)) return 81;
    transferred = 0xdeadbeefu;
    if (!WriteFile(file, initial_bytes, 5u, &transferred, 0)) return 82;
    if (transferred != 5u) return 87;
    if (GetLastError() != 0u) return 88;
    if (GetFileSize(file, 0) != 5u || SetFilePointer(file, 2, 0, 0u) != 2u) return 83;
    transferred = 0xdeadbeefu;
    if (!WriteFile(file, &save_bytes[2], 1u, &transferred, 0) ||
        transferred != 1u || GetLastError() != 0u ||
        GetFileSize(file, 0) != 5u || SetFilePointer(file, 0, 0, 0u) != 0u) return 89;
    transferred = 0u;
    if (!ReadFile(file, buffer, 5u, &transferred, 0) || transferred != 5u ||
        !bytes_equal(buffer, save_bytes, 5u) || SetFilePointer(file, 0, 0, 2u) != 5u) return 90;
    SetLastError(0x3456u);
    if (!FlushFileBuffers(file) || GetLastError() != 0x3456u || !CloseHandle(file)) return 84;
    file = CreateFileA(save_path, 0x80000000u, 1u, 0, 3u, 0x08000020u, 0u);
    if (file == 0xffffffffu || GetFileSize(file, 0) != 5u) return 85;
    transferred = 0;
    if (!ReadFile(file, buffer, 5u, &transferred, 0) || transferred != 5u ||
        !bytes_equal(buffer, save_bytes, 5u) || !CloseHandle(file)) return 86;
  }
  // 0x5c is both a path separator and the Windows-932 trail byte of ソ.
  // Verify the guest sees it as one component on each fresh worker launch.
  {
    const char *directory = "GuestState\\\x83\x5c";
    const char *save_path = "GuestState\\\x83\x5c\\slot.dat";
    const unsigned char expected[3] = {0x31, 0x00, 0xff};
    if (!CreateDirectoryA(directory, 0) && GetLastError() != 183u) return 92;
    if (GetFileAttributesA(directory) != 0x30u) return 93;
    if (GetFileAttributesA(save_path) == 0x20u) {
      file = CreateFileA(save_path, 0x80000000u, 1u, 0, 3u, 0x08000020u, 0u);
      if (file == 0xffffffffu || GetFileSize(file, 0) != 3u) return 94;
      transferred = 0;
      if (!ReadFile(file, buffer, 3u, &transferred, 0) || transferred != 3u ||
          !bytes_equal(buffer, expected, 3u) || !CloseHandle(file)) return 95;
    }
    file = CreateFileA(save_path, 0xc0000000u, 0u, 0, 2u, 0x08000020u, 0u);
    if (file == 0xffffffffu) return 96;
    transferred = 0;
    if (!WriteFile(file, expected, 3u, &transferred, 0) || transferred != 3u ||
        !CloseHandle(file)) return 97;
  }
  if (CreateFileA("fixtures\\storage\\source-read-v1.dat\\child", 0xc0000000u,
                  0u, 0, 2u, 0x08000020u, 0u) != 0xffffffffu ||
      GetLastError() != 3u) return 98;
  // These parents exist only in the read-only source tree at first launch.
  SetLastError(0x3456u);
  if (CreateDirectoryA("fixtures\\storage\\GuestOverlay", 0)) {
    if (GetLastError() != 0x3456u) return 64;
  } else if (GetLastError() != 183u) return 65;
  if (GetFileAttributesA("fixtures\\storage\\GuestOverlay") != 0x30u) return 66;
  if (CreateDirectoryA("fixtures\\storage\\GuestOverlay\\Slot", 0)) {
    if (GetLastError() != 0x3456u) return 67;
  } else if (GetLastError() != 183u) return 68;
  if (GetFileAttributesA("fixtures\\storage\\GuestOverlay\\Slot") != 0x30u) return 69;
  SetLastError(0x1234u);
  if (GetFileAttributesA("GuestState") != 0x30u || GetLastError() != 0x1234u) return 57;
  if (GetFileAttributesA("GuestState\\Slot01") != 0x30u || GetLastError() != 0x1234u) return 58;
  if (GetFileAttributesA("GuestState\\retire.dat") == 0xffffffffu) {
    if (GetLastError() != 2u) return 59;
  } else if (GetFileAttributesA("GuestState\\retire.dat") != 0x20u ||
             GetLastError() != 0x1234u) return 60;
  SetLastError(0x6789u);
  if (DeleteFileA("GuestState\\retire.dat")) {
    if (GetLastError() != 0x6789u) return 50;
  } else if (GetLastError() != 2u) return 51;
  if (DeleteFileA("GuestState\\retire.dat") || GetLastError() != 2u) return 52;
  if (GetFileAttributesA("GuestState\\retire.dat") != 0xffffffffu || GetLastError() != 2u) return 61;
  if (DeleteFileA("UnknownParent\\retire.dat") || GetLastError() != 3u) return 53;
  // OPFS may lack this parent even though the immutable source view has it.
  if (DeleteFileA("fixtures\\storage\\missing-delete.dat") || GetLastError() != 2u) return 62;
  if (DeleteFileA("fixtures\\storage\\unknown\\missing-delete.dat") || GetLastError() != 3u) return 63;
  if (DeleteFileA("GuestState\\Slot01") || GetLastError() != 5u) return 54;
  if (DeleteFileA(fixture_write_probe_path) || GetLastError() != 5u) return 55;
  if (DeleteFileA("D:\\Outside\\retire.dat") || GetLastError() != 3u) return 56;
  SetLastError(0x4321u);
  if (GetFileAttributesA("fixtures\\empty") != 0x30u ||
      GetLastError() != 0x4321u) return 26;
  search = FindFirstFileA("fixtures\\empty", find_data.bytes);
  if (search == 0xffffffffu || find_data.words[0] != 0x30u ||
      !bytes_equal(find_data.bytes + 44u, (const unsigned char *)"empty", 5u) ||
      !FindClose(search)) return 36;
  // The literal search must discover its immutable file itself; no prior
  // GetFileAttributesA or CreateFileA may register the file in Rust.
  search = FindFirstFileA("fixtures\\storage\\wildcard-*.dat", find_data.bytes);
  if (search == 0xffffffffu || find_data.words[0] != 0x20u ||
      find_data.words[8] != 94u ||
      !bytes_equal(find_data.bytes + 44u, (const unsigned char *)"wildcard-only.dat", 17u) ||
      !FindClose(search)) return 41;
  search = FindFirstFileA("fixtures\\storage\\source-read-v1.dat", find_data.bytes);
  if (search == 0xffffffffu || find_data.words[0] != 0x20u ||
      find_data.words[8] != 94u ||
      !bytes_equal(find_data.bytes + 44u, (const unsigned char *)"source-read-v1.dat", 18u))
    return 29;
  if (FindNextFileA(search, find_data.bytes) != 0 || GetLastError() != 18u)
    return 30;
  if (!FindClose(search)) return 31;
  if (FindClose(search) != 0 || GetLastError() != 6u) return 32;
  if (FindFirstFileA("fixtures\\storage\\absent.dat", find_data.bytes) != 0xffffffffu ||
      GetLastError() != 2u) return 34;
  if (FindFirstFileA("missing\\absent.dat", find_data.bytes) != 0xffffffffu ||
      GetLastError() != 3u) return 35;
  if (FindFirstFileA("D:\\outside\\absent.dat", find_data.bytes) != 0xffffffffu ||
      GetLastError() != 3u) return 126;
  if (FindFirstFileA("D:\\outside\\*", find_data.bytes) != 0xffffffffu ||
      GetLastError() != 3u) return 127;
  search = FindFirstFileA("fixtures\\*", find_data.bytes);
  if (search == 0xffffffffu || find_data.words[0] != 0x30u ||
      !FindClose(search)) return 33;
  SetLastError(0x4321u);
  if (GetFileAttributesA(fixture_source_path) != 0x20u) return 19;
  if (GetLastError() != 0x4321u) return 23;
  if (GetFileAttributesA("fixtures\\storage") != 0x30u ||
      GetLastError() != 0x4321u) return 20;
  if (GetFileAttributesA("fixtures\\storage\\absent.dat") != 0xffffffffu ||
      GetLastError() != 2u) return 21;
  if (GetFileAttributesA("outside\\absent.dat") != 0xffffffffu ||
      GetLastError() != 3u) return 22;
  if (GetFileAttributesA(".") != 0x30u || GetLastError() != 3u) return 24;
  if (GetFileAttributesA("D:\\Vine\\fixtures\\storage\\source-read-v1.dat") != 0xffffffffu ||
      GetLastError() != 3u) return 25;
  // Neither a write open nor CREATE_ALWAYS may mutate the selected source.
  if (CreateFileA(fixture_source_path, 0x40000000u, 1u, 0, 3u,
                  0x08000020u, 0u) != 0xffffffffu || GetLastError() != 5u) return 37;
  if (CreateFileA(fixture_source_path, 0xc0000000u, 0u, 0, 2u,
                  0x08000020u, 0u) != 0xffffffffu || GetLastError() != 5u) return 91;
  if (CreateFileA(fixture_source_path, 0x80000000u, 1u, 0, 2u,
                  0x08000020u, 0u) != 0xffffffffu || GetLastError() != 5u) return 38;
  file = CreateFileA(fixture_source_path, 0x80000000u, 1u, 0, 3u,
                     0x08000020u, 0u);

  if (file == 0xffffffffu) {
    return 10;
  }
  SetLastError(0x4321u);
  if (GetFileType(file) != 1u || GetLastError() != 0x4321u) return 130;
  transferred = 0xdeadbeefu;
  if (WriteFile(file, fixture_prefix, 1u, &transferred, 0) != 0 ||
      GetLastError() != 5u || transferred != 0xdeadbeefu) result = 27;
  if (FlushFileBuffers(file) != 0 || GetLastError() != 5u) result = 28;
  SetLastError(0x4321u);
  if (result == 0 && GetFileSize(file, 0) != 94u) {
    result = 11;
  } else if (result == 0 &&
             (GetFileSize(file, &size_high) != 94u || size_high != 0u)) {
    result = 17;
  } else if (!ReadFile(file, 0, 0u, &transferred, 0) || transferred != 0u) {
    result = 18;
  } else if (!ReadFile(file, buffer, 8u, &transferred, 0) ||
             transferred != 8u ||
             !bytes_equal(buffer, fixture_prefix, 8u)) {
    result = 12;
  } else if (SetFilePointer(file, 50, 0, 0u) != 50u) {
    result = 13;
  } else if (!ReadFile(file, buffer, 11u, &transferred, 0) ||
             transferred != 11u ||
             !bytes_equal(buffer, fixture_seek_target, 11u)) {
    result = 14;
  }
  if (!CloseHandle(file) && result == 0) {
    result = 15;
  }
  if (result == 0 && (GetFileType(file) != 0u || GetLastError() != 6u)) {
    result = 131;
  }
  if (result == 0 &&
      (LoadLibraryA("missing-dependency.dll") != 0 || GetLastError() != 126u)) {
    result = 16;
  }
  return result;
}
