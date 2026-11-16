// Generic PE32 fixture: incomplete comparisons survive real Runtime scheduling.
// Link the timer-threads-kernel32 import library; no CRT or title code is used.
// The runner also checks that an active private REP continuation is parked and
// restored while another thread runs. The guest checks the architectural state.
.def @feat.00
.scl 3
.type 0
.endef
.set @feat.00, 1
.text
.globl _fixture_entry
_fixture_entry:
    pushl %ebp
    movl %esp, %ebp
    pushl %ebx
    pushl %esi
    pushl %edi
    subl $4, %esp
    movl $0x13579bdf, -16(%ebp)
    movl %fs:0x18, %esi
    movl %esi, _main_teb
    movl %fs:0x24, %eax
    movl %eax, _main_thread_id
    pushl $_child_thread_id
    pushl $0
    pushl $0x2468ace0
    pushl $_worker
    pushl $0
    pushl $0
    calll *"__imp__CreateThread@24"
    testl %eax, %eax
    jz main_failure
    movl %eax, %ebx
    cld
    movl $0x98765455, %eax
    movl $12426, %ecx
    movl $_main_bytes, %edi
    xorl %edx, %edx
    movl $0xa55a1234, %edx
    // 12,288 equal bytes precede a mismatch. Three incomplete executions
    // preserve progress; the final comparison exits with 137 bytes remaining.
    repe scasb
    jnc main_failure
    jz main_failure
    jns main_failure
    jnp main_failure
    jo main_failure
    cmpl $0x98765455, %eax
    jne main_failure
    cmpl $0xa55a1234, %edx
    jne main_failure
    cmpl $137, %ecx
    jne main_failure
    cmpl $_main_bytes+12289, %edi
    jne main_failure
    cmpl $0x13579bdf, -16(%ebp)
    jne main_failure
    leal -16(%ebp), %eax
    cmpl %eax, %esp
    jne main_failure
    movl %fs:0x18, %eax
    cmpl %esi, %eax
    jne main_failure
    movl %fs:0x24, %eax
    cmpl _main_thread_id, %eax
    jne main_failure
    pushl $-1
    pushl %ebx
    calll *"__imp__WaitForSingleObject@8"
    testl %eax, %eax
    jne main_failure
    cmpl $0, _child_error
    jne main_failure
    pushl %ebx
    calll *"__imp__CloseHandle@4"
    testl %eax, %eax
    jz main_failure
    xorl %eax, %eax
    jmp main_done
main_failure:
    movl $1, %eax
main_done:
    cld
    addl $4, %esp
    popl %edi
    popl %esi
    popl %ebx
    popl %ebp
    ret

_worker:
    pushl %ebp
    movl %esp, %ebp
    pushl %ebx
    pushl %esi
    pushl %edi
    subl $4, %esp
    movl $0xfdb97531, -16(%ebp)
    cmpl $0x2468ace0, 8(%ebp)
    jne child_failure
    movl %fs:0x18, %esi
    cmpl _main_teb, %esi
    je child_failure
    movl %fs:0x24, %eax
    cmpl _child_thread_id, %eax
    jne child_failure
    // Yield through a real Win32 API while the other CPU has a partial SCAS.
    pushl $0
    calll *"__imp__Sleep@4"
    movl $0x13579b71, %eax
    movl $12426, %ecx
    movl $_child_bytes+12288, %edi
    movl $0x5aa54321, %edx
    movl $0x2468ace0, %ebx
    std
    cmpl %eax, %eax
    // Initial ZF=1 must not suppress REPNE's first comparison. Its final
    // equal byte is at the beginning of the array and advances EDI below it.
    repne scasb
    jnz child_failure
    jc child_failure
    js child_failure
    jnp child_failure
    jo child_failure
    cld
    cmpl $0x13579b71, %eax
    jne child_failure
    cmpl $0x5aa54321, %edx
    jne child_failure
    cmpl $0x2468ace0, %ebx
    jne child_failure
    cmpl $137, %ecx
    jne child_failure
    cmpl $_child_bytes-1, %edi
    jne child_failure
    cmpl $0xfdb97531, -16(%ebp)
    jne child_failure
    leal -16(%ebp), %eax
    cmpl %eax, %esp
    jne child_failure
    movl %fs:0x18, %eax
    cmpl %esi, %eax
    jne child_failure
    movl %fs:0x24, %eax
    cmpl _child_thread_id, %eax
    jne child_failure
    pushl $1
    calll *"__imp__Sleep@4"
    movl %fs:0x18, %eax
    cmpl %esi, %eax
    jne child_failure
    movl $0, _child_error
    xorl %eax, %eax
    jmp child_done
child_failure:
    movl $2, %eax
child_done:
    cld
    addl $4, %esp
    popl %edi
    popl %esi
    popl %ebx
    popl %ebp
    retl $4
.data
.p2align 4
_main_bytes:
    .fill 12288, 1, 0x55
    .byte 0x56
_child_bytes:
    .byte 0x71
    .fill 12288, 1, 0x72
_main_teb: .long 0
_main_thread_id: .long 0
_child_thread_id: .long 0
_child_error: .long 1
