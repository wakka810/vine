typedef int(__cdecl *fixture_callback)(int value);

__declspec(dllexport) int __cdecl fixture_deferred_add(int left, int right) {
  return left + right;
}

__declspec(dllexport) int __cdecl
fixture_call_callback(fixture_callback callback, int value) {
  return callback(value);
}

__declspec(dllexport) int __cdecl fixture_commit_outputs(unsigned int rgba,
                                                          unsigned int tone_hz,
                                                          unsigned int byte) {
  return rgba == 0xff0000ffu && tone_hz == 880u && byte == 0x5au;
}
