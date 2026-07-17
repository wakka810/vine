typedef unsigned long fixture_handle;

__declspec(dllimport) fixture_handle __stdcall
CreateFileA(const char *file_name, unsigned long desired_access,
            unsigned long share_mode, void *security_attributes,
            unsigned long creation_disposition,
            unsigned long flags_and_attributes, fixture_handle template_file);
__declspec(dllimport) unsigned long __stdcall
GetFileSize(fixture_handle file, unsigned long *file_size_high);
__declspec(dllimport) int __stdcall
ReadFile(fixture_handle file, void *buffer, unsigned long bytes_to_read,
         unsigned long *bytes_read, void *overlapped);
__declspec(dllimport) unsigned long __stdcall
SetFilePointer(fixture_handle file, long distance_low, long *distance_high,
               unsigned long move_method);
__declspec(dllimport) int __stdcall CloseHandle(fixture_handle object);

static const char fixture_source_path[] =
    "fixtures\\storage\\source-read-v1.dat";
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
  unsigned long transferred = 0;
  fixture_handle file =
      CreateFileA(fixture_source_path, 0x80000000u, 1u, 0, 3u,
                  0x08000020u, 0u);
  int result = 0;

  if (file == 0xffffffffu) {
    return 10;
  }
  if (GetFileSize(file, 0) != 94u) {
    result = 11;
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
  return result;
}
