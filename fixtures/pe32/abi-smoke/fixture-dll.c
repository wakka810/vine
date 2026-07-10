typedef int(__cdecl *fixture_callback)(int value);

__declspec(dllexport) int __cdecl fixture_add(int left, int right) {
  return left + right;
}

__declspec(dllexport) int __cdecl
fixture_call_callback(fixture_callback callback, int value) {
  return callback(value);
}
