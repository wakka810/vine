// Generic PE32 fixture: bounded REP yields and lazy heap page-fault restart.
// Build with clang --target=i686-pc-windows-msvc -c, then link fixture_entry
// against the existing heap-growth-kernel32 import library. No CRT is used.
.def @feat.00
.scl 3
.type 0
.endef
.set @feat.00, 1
.text
.globl _fixture_entry
_fixture_entry:
    pushl %ebp
    pushl %ebx
    pushl %esi
    pushl %edi
    pushl $0
    pushl $0
    pushl $0
    calll *"__imp__HeapCreate@12"
    testl %eax, %eax
    jz failure
    movl %eax, %ebp
    // A large logical allocation is backed lazily by the Rust Runtime.
    pushl $41943040
    pushl $8
    pushl %ebp
    calll *"__imp__HeapAlloc@12"
    testl %eax, %eax
    jz failure
    movl %eax, %ebx
    movl $12288, %ecx
    movl $0x5a, %eax
    movl $_source, %edi
    rep stosb
    movl $12288, %ecx
    movl $_source, %esi
    movl %ebx, %edi
    rep movsb
    testl %ecx, %ecx
    jne failure
    leal 12288(%ebx), %eax
    cmpl %eax, %edi
    jne failure
    cmpl $_source+12288, %esi
    jne failure
    // Check every copied byte, including prefixes before each fault/yield.
    movl $12288, %ecx
    movl %ebx, %edi
verify:
    cmpb $0x5a, (%edi)
    jne failure
    incl %edi
    decl %ecx
    jne verify
    xorl %eax, %eax
    jmp done
failure:
    movl $1, %eax
done:
    popl %edi
    popl %esi
    popl %ebx
    popl %ebp
    ret
.bss
.p2align 12
_source:
    .space 12288
