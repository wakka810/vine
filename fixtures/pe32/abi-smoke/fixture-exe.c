typedef int(__cdecl *fixture_callback)(int value);

__declspec(dllimport) int __cdecl fixture_add(int left, int right);
__declspec(dllimport) int __cdecl
fixture_call_callback(fixture_callback callback, int value);

volatile int fixture_result;

static int __cdecl add_one(int value) {
  return value + 1;
}

int __cdecl fixture_entry(void) {
  int sum = fixture_add(19, 23);
  fixture_result = fixture_call_callback(add_one, sum);
  return fixture_result == 43 ? 0 : 1;
}
