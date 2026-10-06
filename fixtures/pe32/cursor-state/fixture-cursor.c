typedef struct { long x, y; } POINT;
typedef struct { unsigned long size, flags, cursor; POINT position; } CURSORINFO;
__declspec(dllimport) int __stdcall GetCursorPos(POINT *point);
__declspec(dllimport) int __stdcall SetCursorPos(int x, int y);
__declspec(dllimport) int __stdcall GetCursorInfo(CURSORINFO *info);
__declspec(dllimport) int __stdcall ShowCursor(int show);
__declspec(dllimport) void __stdcall SetLastError(unsigned long error);
__declspec(dllimport) unsigned long __stdcall GetLastError(void);

int __cdecl fixture_entry(void) {
  POINT point;
  CURSORINFO info = { 20, 0, 0, { 0, 0 } };
  SetLastError(0x4321);
  if (!GetCursorPos(&point) || point.x != 960 || point.y != 540) return 1;
  if (!SetCursorPos(100, 200) || !GetCursorPos(&point) || point.x != 100 || point.y != 200) return 2;
  if (ShowCursor(0) != -1 || ShowCursor(1) != 0) return 3;
  if (!GetCursorInfo(&info) || info.size != 20 || info.cursor != 0 || info.flags != 0 ||
      info.position.x != 100 || info.position.y != 200) return 4;
  if (GetLastError() != 0x4321) return 5;
  info.size = 16;
  if (GetCursorInfo(&info) || GetLastError() != 87 || info.size != 16) return 6;
  if (GetCursorPos((POINT *)0) || GetLastError() != 998) return 7;
  return 0;
}
