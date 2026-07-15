typedef int(__cdecl *fixture_callback)(int value);

__declspec(dllimport) int __cdecl fixture_deferred_add(int left, int right);
__declspec(dllimport) int __cdecl
fixture_call_callback(fixture_callback callback, int value);
__declspec(dllimport) int __cdecl fixture_commit_outputs(unsigned int rgba,
                                                          unsigned int tone_hz,
                                                          unsigned int byte);

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

  fixture_commit_result = fixture_commit_outputs(0xff0000ffu, 880u, 0x5au);
  return fixture_commit_result == 1 ? 0 : 3;
}
