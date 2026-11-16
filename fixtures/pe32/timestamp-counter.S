// Compare real-clock RDTSC against Runtime GetTickCount, then cross a Sleep.
// No CRT; link with the existing sleep-kernel32 import library.
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
    calll *"__imp__GetTickCount@0"
    movl %eax, %ebp
    rdtsc
    movl %eax, %ebx
    movl %edx, %esi
    calll *"__imp__GetTickCount@0"
    movl %eax, %edi
wait_tick:
    calll *"__imp__GetTickCount@0"
    subl %edi, %eax
    cmpl $20, %eax
    jb wait_tick
    rdtsc
    // Store the second observation before forming the elapsed 64-bit value.
    movl %eax, _second_low
    movl %edx, _second_high
    subl %ebx, %eax
    sbbl %esi, %edx
    movl %eax, %ebx
    movl %edx, %esi
    testl %edx, %edx
    jne check_upper
    cmpl $19000000, %eax
    jb failure
check_upper:
    calll *"__imp__GetTickCount@0"
    subl %ebp, %eax
    incl %eax
    // Form the exact 64-bit upper bound using ordinary supported integer ops.
    movl %eax, %ebp
    xorl %ecx, %ecx
    xorl %eax, %eax
    xorl %edx, %edx
    movl $1000000, %edi
multiply_bound:
    testl $1, %edi
    jz shift_bound
    addl %ebp, %eax
    adcl %ecx, %edx
shift_bound:
    addl %ebp, %ebp
    adcl %ecx, %ecx
    shrl $1, %edi
    jnz multiply_bound
    cmpl %esi, %edx
    jb failure
    ja check_sleep
    cmpl %ebx, %eax
    jb failure
check_sleep:
    pushl $1
    calll *"__imp__Sleep@4"
    rdtsc
    subl _second_low, %eax
    sbbl _second_high, %edx
    jb failure
    orl %edx, %eax
    jz failure
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
.data
.p2align 2
_second_low: .long 0
_second_high: .long 0
