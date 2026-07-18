typedef int(__cdecl *fixture_callback)(int value);

typedef struct fixture_idirect3d9 fixture_idirect3d9;
typedef struct fixture_idirect3d9_vtable {
  int(__stdcall *query_interface)(fixture_idirect3d9 *self, const void *iid,
                                  void **object);
  unsigned int(__stdcall *add_ref)(fixture_idirect3d9 *self);
  unsigned int(__stdcall *release)(fixture_idirect3d9 *self);
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
  if (direct3d->vtable->add_ref(direct3d) != 2) {
    return 5;
  }
  if (direct3d->vtable->release(direct3d) != 1) {
    return 6;
  }
  if (direct3d->vtable->release(direct3d) != 0) {
    return 7;
  }

  fixture_commit_result = fixture_commit_outputs(0xff0000ffu, 880u, 0x5au);
  return fixture_commit_result == 1 ? 0 : 8;
}
