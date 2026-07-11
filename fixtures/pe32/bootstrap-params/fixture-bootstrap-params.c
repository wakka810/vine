typedef unsigned int UINT32;
typedef unsigned int UINTPTR;
typedef unsigned long ULONG;
typedef unsigned short UINT16;

extern ULONG __readfsdword(ULONG offset);
#pragma intrinsic(__readfsdword)

enum {
  PEB_PROCESS_PARAMETERS_OFFSET = 0x10u,
  PROCESS_PARAMETERS_COMMAND_LINE_OFFSET = 0x40u,
  UNICODE_STRING_BUFFER_OFFSET = 0x04u,
  EXPECTED_NAME_UNITS = 20u,
  MAX_COMMAND_LINE_BYTES = 32768u,
  NO_MATCH = 0xffffffffu,
};

_Static_assert(sizeof(void *) == 4, "fixture requires i386 pointers");
_Static_assert(sizeof(UINT32) == 4, "fixture requires 32-bit UINT32");
_Static_assert(sizeof(UINT16) == 2, "fixture requires 16-bit UINT16");

static const UINT16 expected_name[EXPECTED_NAME_UNITS] = {
    'b', 'o', 'o', 't', 's', 't', 'r', 'a', 'p', '-',
    'p', 'a', 'r', 'a', 'm', 's', '.', 'e', 'x', 'e',
};

__declspec(dllexport) volatile UINT32 fixture_peb;
__declspec(dllexport) volatile UINT32 fixture_process_parameters;
__declspec(dllexport) volatile UINT32 fixture_command_line_buffer;
__declspec(dllexport) volatile UINT32 fixture_command_line_length;
__declspec(dllexport) volatile UINT32 fixture_command_line_maximum_length;
__declspec(dllexport) volatile UINT32 fixture_command_line_match_index;

static UINT16 fold_ascii(UINT16 value) {
  if (value >= 'A' && value <= 'Z') {
    return (UINT16)(value + ('a' - 'A'));
  }
  return value;
}

static UINT32 find_expected_name(volatile const UINT16 *text, UINT32 units) {
  UINT32 index;
  if (units < EXPECTED_NAME_UNITS) {
    return NO_MATCH;
  }
  for (index = 0; index <= units - EXPECTED_NAME_UNITS; ++index) {
    UINT32 expected_index;
    for (expected_index = 0; expected_index < EXPECTED_NAME_UNITS;
         ++expected_index) {
      if (fold_ascii(text[index + expected_index]) !=
          expected_name[expected_index]) {
        break;
      }
    }
    if (expected_index == EXPECTED_NAME_UNITS) {
      return index;
    }
  }
  return NO_MATCH;
}

__declspec(dllexport) int __cdecl fixture_entry(void) {
  UINT32 peb = (UINT32)__readfsdword(0x30u);
  UINT32 process_parameters;
  UINT32 command_line_address;
  UINT16 length;
  UINT16 maximum_length;
  UINT32 buffer;
  UINT32 match_index;

  fixture_peb = peb;
  if (peb == 0u) {
    return 1;
  }
  process_parameters =
      *(volatile const UINT32 *)(UINTPTR)(peb + PEB_PROCESS_PARAMETERS_OFFSET);
  fixture_process_parameters = process_parameters;
  if (process_parameters == 0u) {
    return 2;
  }

  command_line_address =
      process_parameters + PROCESS_PARAMETERS_COMMAND_LINE_OFFSET;
  length = *(volatile const UINT16 *)(UINTPTR)command_line_address;
  maximum_length =
      *(volatile const UINT16 *)(UINTPTR)(command_line_address + 2u);
  buffer = *(volatile const UINT32 *)(UINTPTR)(
      command_line_address + UNICODE_STRING_BUFFER_OFFSET);
  fixture_command_line_length = length;
  fixture_command_line_maximum_length = maximum_length;
  fixture_command_line_buffer = buffer;
  if (length == 0u || (length & 1u) != 0u || length > MAX_COMMAND_LINE_BYTES ||
      maximum_length < length) {
    return 3;
  }
  if (buffer == 0u) {
    return 4;
  }

  match_index = find_expected_name(
      (volatile const UINT16 *)(UINTPTR)buffer, (UINT32)length / 2u);
  fixture_command_line_match_index = match_index;
  return match_index == (UINT32)NO_MATCH ? 5 : 0;
}
