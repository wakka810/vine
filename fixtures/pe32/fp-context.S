// Raw x87 values, per-thread control words and XMM bits survive real Runtime
// waits and CPU context switches. Link the timer-threads-kernel32 import library.
.def @feat.00
.scl 3
.type 0
.endef
.set @feat.00, 1
.text
.globl _fixture_entry
_fixture_entry:
    pushl %ebx
    fldcw _main_control
    fld1
    fld1
    faddp %st, %st(1)
    movups _main_vector, %xmm0
    pushl $_thread_id
    pushl $0
    pushl $0
    pushl $_worker
    pushl $0
    pushl $0
    calll *"__imp__CreateThread@24"
    testl %eax, %eax
    jz main_failure
    movl %eax, %ebx
    pushl $-1
    pushl %ebx
    calll *"__imp__WaitForSingleObject@8"
    testl %eax, %eax
    jne main_failure
    cmpl $0, _child_error
    jne main_failure
    fnstcw _observed_control
    cmpw $0x037f, _observed_control
    jne main_failure
    fstpt _observed_float
    cmpl $0, _observed_float
    jne main_failure
    cmpl $0x80000000, _observed_float+4
    jne main_failure
    cmpw $0x4000, _observed_float+8
    jne main_failure
    movups %xmm0, _observed_vector
    cmpl $0x01234567, _observed_vector
    jne main_failure
    cmpl $0x89abcdef, _observed_vector+4
    jne main_failure
    cmpl $0x7f800001, _observed_vector+8
    jne main_failure
    cmpl $0x80000000, _observed_vector+12
    jne main_failure
    pushl %ebx
    calll *"__imp__CloseHandle@4"
    testl %eax, %eax
    jz main_failure
    xorl %eax, %eax
    popl %ebx
    ret
main_failure:
    movl $1, %eax
    popl %ebx
    ret

_worker:
    fldcw _child_control
    fldl _two
    fld1
    faddp %st, %st(1)
    movups _child_vector, %xmm0
    pushl $1
    calll *"__imp__Sleep@4"
    fnstcw _observed_control
    cmpw $0x027f, _observed_control
    jne child_failure
    fstpt _observed_float
    cmpl $0, _observed_float
    jne child_failure
    cmpl $0xc0000000, _observed_float+4
    jne child_failure
    cmpw $0x4000, _observed_float+8
    jne child_failure
    movups %xmm0, _observed_vector
    cmpl $0xfedcba98, _observed_vector
    jne child_failure
    cmpl $0x76543210, _observed_vector+4
    jne child_failure
    cmpl $0xff800001, _observed_vector+8
    jne child_failure
    cmpl $0x00000001, _observed_vector+12
    jne child_failure
    movl $0, _child_error
    xorl %eax, %eax
    retl $4
child_failure:
    movl $2, %eax
    retl $4
.data
.p2align 4
_main_vector: .long 0x01234567,0x89abcdef,0x7f800001,0x80000000
_child_vector: .long 0xfedcba98,0x76543210,0xff800001,0x00000001
_observed_vector: .space 16
_two: .long 0,0x40000000
_observed_float: .space 10
_main_control: .word 0x037f
_child_control: .word 0x027f
_observed_control: .word 0
_thread_id: .long 0
_child_error: .long 1
