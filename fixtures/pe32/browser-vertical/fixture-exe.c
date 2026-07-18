typedef int(__cdecl *fixture_callback)(int value);

typedef struct fixture_idirect3d9 fixture_idirect3d9;
typedef struct fixture_d3d_adapter_identifier {
  char driver[512];
  char description[512];
  char device_name[32];
  unsigned int driver_version_low;
  unsigned int driver_version_high;
  unsigned int vendor_id;
  unsigned int device_id;
  unsigned int sub_sys_id;
  unsigned int revision;
  unsigned char device_identifier[16];
  unsigned int whql_level;
} fixture_d3d_adapter_identifier;

typedef char fixture_d3d_adapter_identifier_size[
    sizeof(fixture_d3d_adapter_identifier) == 1100 ? 1 : -1];

typedef struct fixture_d3d_display_mode {
  unsigned int width;
  unsigned int height;
  unsigned int refresh_rate;
  unsigned int format;
} fixture_d3d_display_mode;

typedef struct fixture_idirect3d9_vtable {
  int(__stdcall *query_interface)(fixture_idirect3d9 *self, const void *iid,
                                  void **object);
  unsigned int(__stdcall *add_ref)(fixture_idirect3d9 *self);
  unsigned int(__stdcall *release)(fixture_idirect3d9 *self);
  void *register_software_device;
  void *get_adapter_count;
  int(__stdcall *get_adapter_identifier)(
      fixture_idirect3d9 *self, unsigned int adapter, unsigned int flags,
      fixture_d3d_adapter_identifier *identifier);
  void *get_adapter_mode_count;
  void *enum_adapter_modes;
  int(__stdcall *get_adapter_display_mode)(fixture_idirect3d9 *self,
                                            unsigned int adapter,
                                            fixture_d3d_display_mode *mode);
} fixture_idirect3d9_vtable;

struct fixture_idirect3d9 {
  fixture_idirect3d9_vtable *vtable;
};

__declspec(dllimport) int __cdecl fixture_deferred_add(int left, int right);
__declspec(dllimport) int __cdecl
fixture_call_callback(fixture_callback callback, int value);
__declspec(dllimport) int __cdecl fixture_commit_outputs(unsigned int rgba,
                                                          unsigned int tone_hz,
                                                          unsigned int byte);
__declspec(dllimport) fixture_idirect3d9 *__stdcall
Direct3DCreate9(unsigned int sdk_version);

volatile int fixture_sum;
volatile int fixture_nested_result;
volatile int fixture_commit_result;
__declspec(dllexport) volatile unsigned int fixture_thread_marker;

static int fixture_fixed_string_is(const char *actual, unsigned int capacity,
                                   const char *expected) {
  unsigned int index = 0;
  while (expected[index] != 0) {
    if (index >= capacity || actual[index] != expected[index]) {
      return 0;
    }
    ++index;
  }
  if (index >= capacity || actual[index] != 0) {
    return 0;
  }
  return 1;
}

static int fixture_fixed_string_is_present(const char *actual,
                                           unsigned int capacity) {
  if (capacity == 0 || actual[0] == 0) {
    return 0;
  }
  for (unsigned int index = 1; index < capacity; ++index) {
    if (actual[index] == 0) {
      return 1;
    }
  }
  return 0;
}

static int fixture_d3d9_failure(fixture_idirect3d9 *direct3d, int result) {
  direct3d->vtable->release(direct3d);
  return result;
}

__declspec(dllexport) unsigned int __cdecl fixture_thread_entry(void) {
  fixture_thread_marker = 0x54485244u;
  return 7u;
}

static int __cdecl inner_callback(int value) {
  return value + 1;
}

static int __cdecl outer_callback(int value) {
  return fixture_call_callback(inner_callback, value) + 1;
}

int __cdecl fixture_entry(void) {
  fixture_sum = fixture_deferred_add(19, 23);
  if (fixture_sum != 42) {
    return 1;
  }

  fixture_nested_result = fixture_call_callback(outer_callback, fixture_sum);
  if (fixture_nested_result != 44) {
    return 2;
  }

  if (Direct3DCreate9(0) != 0) {
    return 3;
  }
  fixture_idirect3d9 *direct3d = Direct3DCreate9(32);
  if (direct3d == 0) {
    return 4;
  }
  fixture_d3d_display_mode mode = {0xccccccccu, 0xccccccccu, 0xccccccccu,
                                   0xccccccccu};
  if (direct3d->vtable->get_adapter_display_mode(direct3d, 0, &mode) != 0) {
    return fixture_d3d9_failure(direct3d, 5);
  }
  if (mode.width != 1920 || mode.height != 1080 || mode.refresh_rate != 60 ||
      mode.format != 22) {
    return fixture_d3d9_failure(direct3d, 6);
  }
  fixture_d3d_adapter_identifier identifier;
  if (direct3d->vtable->get_adapter_identifier(direct3d, 0, 0,
                                                &identifier) != 0) {
    return fixture_d3d9_failure(direct3d, 7);
  }
  const unsigned char expected_device_identifier[16] = {
      0x45, 0x4e, 0x49, 0x56, 0x33, 0x44, 0x39, 0x44,
      0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01};
  int has_vine_device_identifier = 1;
  for (unsigned int index = 0; index < 16; ++index) {
    if (identifier.device_identifier[index] != expected_device_identifier[index]) {
      has_vine_device_identifier = 0;
      break;
    }
  }
  const int has_vine_driver =
      fixture_fixed_string_is(identifier.driver, 512, "vine-d3d9.dll");
  const int has_vine_description =
      fixture_fixed_string_is(identifier.description, 512,
                              "Vine Virtual Direct3D9 Adapter");
  const int has_vine_identity =
      has_vine_driver && has_vine_description &&
      fixture_fixed_string_is(identifier.device_name, 32,
                              "\\\\.\\DISPLAY1") &&
      identifier.driver_version_low == 0 &&
      identifier.driver_version_high == 0x00010000u &&
      identifier.vendor_id == 0 && identifier.device_id == 0 &&
      identifier.sub_sys_id == 0 && identifier.revision == 0 &&
      identifier.whql_level == 0 && has_vine_device_identifier;
  if (!has_vine_identity &&
      (has_vine_driver || has_vine_description ||
       !fixture_fixed_string_is_present(identifier.driver, 512) ||
       !fixture_fixed_string_is_present(identifier.description, 512) ||
       !fixture_fixed_string_is_present(identifier.device_name, 32))) {
    return fixture_d3d9_failure(direct3d, 8);
  }
  if (direct3d->vtable->add_ref(direct3d) != 2) {
    return 11;
  }
  if (direct3d->vtable->release(direct3d) != 1) {
    return 12;
  }
  if (direct3d->vtable->release(direct3d) != 0) {
    return 13;
  }

  fixture_commit_result = fixture_commit_outputs(0xff0000ffu, 880u, 0x5au);
  return fixture_commit_result == 1 ? 0 : 14;
}
