/* Independent test oracle: actual host x87 instructions. This object is never
 * built for Wasm and is referenced only by Rust tests. Preserve host FP/SIMD
 * state around every probe; input and output travel as raw bytes. */
#include <stdint.h>

void vine_x87_native(uint32_t op, uint64_t as, uint32_t ae,
                    uint64_t bs, uint32_t be, uint32_t control, uint64_t out[4]) {
    _Alignas(16) unsigned char saved[512];
    unsigned char a[10], b[10], result[10];
    for (int i=0;i<8;i++) { a[i]=(unsigned char)(as>>(i*8)); b[i]=(unsigned char)(bs>>(i*8)); }
    a[8]=ae; a[9]=ae>>8; b[8]=be; b[9]=be>>8;
    uint16_t cw=(uint16_t)control, sw;
    __asm__ volatile("fxsave64 %0; fninit; fldcw %1; fldt %2" : "=m"(saved) : "m"(cw), "m"(a) : "memory");
    switch(op) {
    case 0: __asm__ volatile("fldt %0; faddp" : : "m"(b) : "memory"); break;
    case 1: __asm__ volatile("fldt %0; fsubrp" : : "m"(b) : "memory"); break;
    case 2: __asm__ volatile("fldt %0; fmulp" : : "m"(b) : "memory"); break;
    case 3: __asm__ volatile("fldt %0; fdivrp" : : "m"(b) : "memory"); break;
    case 4: __asm__ volatile("fsqrt" : : : "memory"); break;
    case 5: __asm__ volatile("frndint" : : : "memory"); break;
    case 6: __asm__ volatile("f2xm1" : : : "memory"); break;
    case 7: __asm__ volatile("fldt %0; fxch; fscale" : : "m"(b) : "memory"); break;
    }
    __asm__ volatile("fnstsw %0; fnclex; fstpt %1; fxrstor64 %2" : "=m"(sw), "=m"(result) : "m"(saved) : "memory");
    uint64_t sig=0;
    for (int i=0;i<8;i++) sig |= (uint64_t)result[i]<<(i*8);
    out[0]=sig; out[1]=(uint16_t)result[8] | (uint16_t)result[9]<<8;
    out[2]=0; out[3]=sw;
}

void vine_x87_native_stack(uint32_t kind, uint32_t control, uint64_t out[4]) {
    _Alignas(16) unsigned char saved[512];
    unsigned char environment[28], value[10] = {1,0,0,0,0,0,0,0x80,0xff,0x3f};
    uint32_t memory = 0x12345678;
    uint16_t cw = control;
    __asm__ volatile("fxsave64 %0; fninit; fldcw %1" : "=m"(saved) : "m"(cw) : "memory");
    switch(kind) {
    case 0: __asm__ volatile("fld %%st(0)" : : : "memory"); break;
    case 1:
        __asm__ volatile("fld1; fld1; fld1; fld1; fld1; fld1; fld1; fld1; fld1" : : : "memory"); break;
    case 2: __asm__ volatile("fstps %0" : "=m"(memory) : : "memory"); break;
    case 3: __asm__ volatile("fld1; fadd %%st(1), %%st" : : : "memory"); break;
    case 4: __asm__ volatile("fld1; fxch %%st(1)" : : : "memory"); break;
    case 5: __asm__ volatile("fldt %1; fstps %0" : "=m"(memory) : "m"(value) : "memory"); break;
    case 6: __asm__ volatile("fld1; fldz; fdivrp" : : : "memory"); break;
    }
    __asm__ volatile("fnstenv %0; fxrstor64 %1" : "=m"(environment) : "m"(saved) : "memory");
    out[0]=(uint16_t)environment[4] | (uint16_t)environment[5]<<8;
    out[1]=(uint16_t)environment[8] | (uint16_t)environment[9]<<8;
    out[2]=memory; out[3]=0;
}

void vine_x87_native_memory(uint32_t op, uint64_t as, uint32_t ae,
                           uint32_t bits, uint32_t control, uint64_t out[4]) {
    _Alignas(16) unsigned char saved[512];
    unsigned char a[10], result[10];
    for (int i=0;i<8;i++) a[i]=(unsigned char)(as>>(i*8));
    a[8]=ae; a[9]=ae>>8;
    uint16_t cw=(uint16_t)control, sw;
    __asm__ volatile("fxsave64 %0; fninit; fldcw %1; fldt %2" : "=m"(saved) : "m"(cw), "m"(a) : "memory");
    switch(op) {
    case 0: __asm__ volatile("fadds %0" : : "m"(bits) : "memory"); break;
    case 1: __asm__ volatile("fsubs %0" : : "m"(bits) : "memory"); break;
    case 2: __asm__ volatile("fmuls %0" : : "m"(bits) : "memory"); break;
    case 3: __asm__ volatile("fdivs %0" : : "m"(bits) : "memory"); break;
    case 4: __asm__ volatile("fcoms %0" : : "m"(bits) : "memory"); break;
    case 5: __asm__ volatile("fcomps %0" : : "m"(bits) : "memory"); break;
    case 6: __asm__ volatile("flds %0" : : "m"(bits) : "memory"); break;
    }
    __asm__ volatile("fnstsw %0; fnclex; fstpt %1; fxrstor64 %2" : "=m"(sw), "=m"(result) : "m"(saved) : "memory");
    uint64_t sig=0;
    for (int i=0;i<8;i++) sig |= (uint64_t)result[i]<<(i*8);
    out[0]=sig; out[1]=(uint16_t)result[8] | (uint16_t)result[9]<<8;
    out[2]=0; out[3]=sw;
}

void vine_x87_native_copies(uint32_t kind, uint64_t as, uint32_t ae, uint64_t out[6]) {
    _Alignas(16) unsigned char saved[512];
    unsigned char a[10], fpu[108];
    for (int i=0;i<8;i++) a[i]=(unsigned char)(as>>(i*8));
    a[8]=ae; a[9]=ae>>8;
    __asm__ volatile("fxsave64 %0; fninit; fldt %1" : "=m"(saved) : "m"(a) : "memory");
    switch(kind) {
    case 0: __asm__ volatile("fld %%st(0)" : : : "memory"); break;
    case 1: __asm__ volatile("fst %%st(1)" : : : "memory"); break;
    case 2: __asm__ volatile("fstp %%st(1)" : : : "memory"); break;
    case 3: __asm__ volatile("fxch %%st(1)" : : : "memory"); break;
    }
    __asm__ volatile("fnsave %0; fxrstor64 %1" : "=m"(fpu) : "m"(saved) : "memory");
    out[0]=(uint16_t)fpu[4] | (uint16_t)fpu[5]<<8;
    out[1]=(uint16_t)fpu[8] | (uint16_t)fpu[9]<<8;
    for(int reg=0;reg<2;reg++) {
        uint64_t sig=0;
        for(int i=0;i<8;i++) sig|=(uint64_t)fpu[28+reg*10+i]<<(i*8);
        out[2+reg*2]=sig; out[3+reg*2]=(uint16_t)fpu[36+reg*10] | (uint16_t)fpu[37+reg*10]<<8;
    }
}

/* Actual packed SSE2 conversion, with every lane and the aggregate exception
 * flags observed as raw bits. All exceptions are masked for this numeric-only
 * test; architectural unmasked handling belongs to the CPU tests. */
void vine_sse_native_cvtdq2ps(const int32_t input[4], uint32_t rounding, uint32_t out[5]) {
    _Alignas(16) unsigned char saved[512];
    uint32_t result[4], flags;
    uint32_t control = 0x1f80 | (rounding << 13);
    __asm__ volatile(
        "fxsave64 %[saved]\n\t"
        "ldmxcsr %[control]\n\t"
        "movdqu %[input], %%xmm0\n\t"
        "cvtdq2ps %%xmm0, %%xmm0\n\t"
        "movdqu %%xmm0, %[result]\n\t"
        "stmxcsr %[flags]\n\t"
        "fxrstor64 %[saved]"
        : [saved] "=m"(saved), [result] "=m"(result), [flags] "=m"(flags)
        : [control] "m"(control), [input] "m"(*(const int32_t (*)[4])input)
        : "xmm0", "memory");
    for (int i = 0; i < 4; i++) out[i] = result[i];
    out[4] = flags & 0x3f;
}

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
extern unsigned char vine_sse_mulps_test_instruction, vine_sse_mulps_test_resume;
static uint32_t *sse_mul_output;
static LONG CALLBACK sse_mul_handler(EXCEPTION_POINTERS *exception) {
    CONTEXT *context = exception->ContextRecord;
    if (!sse_mul_output || context->Rip != (uintptr_t)&vine_sse_mulps_test_instruction)
        return EXCEPTION_CONTINUE_SEARCH;
    sse_mul_output[0] = (uint32_t)context->Xmm0.Low;
    sse_mul_output[1] = (uint32_t)(context->Xmm0.Low >> 32);
    sse_mul_output[2] = (uint32_t)context->Xmm0.High;
    sse_mul_output[3] = (uint32_t)((uint64_t)context->Xmm0.High >> 32);
    sse_mul_output[4] = context->MxCsr;
    sse_mul_output[5] = exception->ExceptionRecord->ExceptionCode;
    /* Preserve the observation, then skip the instruction to the one host
     * state restore below. No faulting operation is retried. */
    context->Rip = (uintptr_t)&vine_sse_mulps_test_resume;
    context->MxCsr = context->FltSave.MxCsr = 0x1f80;
    return EXCEPTION_CONTINUE_EXECUTION;
}
#endif

/* Serialized by the Rust test lock. Windows unmasked cases observe actual
 * exception CONTEXT, including all unchanged destination lanes and new flags.
 * Other hosts use this oracle only with every exception masked. */
void vine_sse_native_mulps(const uint32_t a[4], const uint32_t b[4],
                          uint32_t control, uint32_t out[6]) {
    _Alignas(16) unsigned char saved[512];
    uint32_t result[4] = {0}, flags = 0;
    out[5] = 0;
#ifdef _WIN32
    void *handler = 0;
    if ((control & 0x1f80) != 0x1f80) {
        handler = AddVectoredExceptionHandler(1, sse_mul_handler);
        if (!handler) { out[5] = UINT32_MAX; return; }
    }
    sse_mul_output = out;
#endif
    __asm__ volatile(
        "fxsave64 %[saved]\n\t"
        "ldmxcsr %[control]\n\t"
        "movdqu %[a], %%xmm0\n\t"
        "movdqu %[b], %%xmm1\n\t"
        ".globl vine_sse_mulps_test_instruction\n"
        "vine_sse_mulps_test_instruction:\n\t"
        "mulps %%xmm1, %%xmm0\n\t"
        "movdqu %%xmm0, %[result]\n\t"
        "stmxcsr %[flags]\n\t"
        ".globl vine_sse_mulps_test_resume\n"
        "vine_sse_mulps_test_resume:\n\t"
        "fxrstor64 %[saved]"
        : [saved] "=m"(saved), [result] "+m"(result), [flags] "+m"(flags)
        : [control] "m"(control), [a] "m"(*(const uint32_t (*)[4])a),
          [b] "m"(*(const uint32_t (*)[4])b)
        : "xmm0", "xmm1", "memory");
#ifdef _WIN32
    sse_mul_output = 0;
    if (handler) RemoveVectoredExceptionHandler(handler);
#endif
    if (!out[5]) {
        for (int i=0;i<4;i++) out[i] = result[i];
        out[4] = flags;
    }
}

#ifdef _WIN32
extern unsigned char vine_sse_addps_test_instruction, vine_sse_addps_test_resume;
extern unsigned char vine_sse_subps_test_instruction, vine_sse_subps_test_resume;
static uint32_t *sse_add_output;
static LONG CALLBACK sse_add_handler(EXCEPTION_POINTERS *exception) {
    CONTEXT *context = exception->ContextRecord;
    if (!sse_add_output) return EXCEPTION_CONTINUE_SEARCH;
    uintptr_t resume;
    if (context->Rip == (uintptr_t)&vine_sse_addps_test_instruction)
        resume = (uintptr_t)&vine_sse_addps_test_resume;
    else if (context->Rip == (uintptr_t)&vine_sse_subps_test_instruction)
        resume = (uintptr_t)&vine_sse_subps_test_resume;
    else return EXCEPTION_CONTINUE_SEARCH;
    sse_add_output[0] = (uint32_t)context->Xmm0.Low;
    sse_add_output[1] = (uint32_t)(context->Xmm0.Low >> 32);
    sse_add_output[2] = (uint32_t)context->Xmm0.High;
    sse_add_output[3] = (uint32_t)((uint64_t)context->Xmm0.High >> 32);
    sse_add_output[4] = context->MxCsr;
    sse_add_output[5] = exception->ExceptionRecord->ExceptionCode;
    context->Rip = resume;
    context->MxCsr = context->FltSave.MxCsr = 0x1f80;
    return EXCEPTION_CONTINUE_EXECUTION;
}
#endif

/* Two exact packed instructions, with the same state preservation and actual
 * Windows fault CONTEXT contract as the multiplication oracle above. */
void vine_sse_native_add_sub(const uint32_t a[4], const uint32_t b[4],
                            uint32_t control, uint32_t subtract, uint32_t out[6]) {
    _Alignas(16) unsigned char saved[512];
    uint32_t result[4] = {0}, flags = 0;
    out[5] = 0;
#ifdef _WIN32
    void *handler = 0;
    if ((control & 0x1f80) != 0x1f80) {
        handler = AddVectoredExceptionHandler(1, sse_add_handler);
        if (!handler) { out[5] = UINT32_MAX; return; }
    }
    sse_add_output = out;
#endif
#define SSE_ADD_SUB_PROBE(instruction) \
    __asm__ volatile( \
        "fxsave64 %[saved]\n\t" \
        "ldmxcsr %[control]\n\t" \
        "movdqu %[a], %%xmm0\n\t" \
        "movdqu %[b], %%xmm1\n\t" \
        ".globl vine_sse_" #instruction "_test_instruction\n" \
        "vine_sse_" #instruction "_test_instruction:\n\t" \
        #instruction " %%xmm1, %%xmm0\n\t" \
        "movdqu %%xmm0, %[result]\n\t" \
        "stmxcsr %[flags]\n\t" \
        ".globl vine_sse_" #instruction "_test_resume\n" \
        "vine_sse_" #instruction "_test_resume:\n\t" \
        "fxrstor64 %[saved]" \
        : [saved] "=m"(saved), [result] "+m"(result), [flags] "+m"(flags) \
        : [control] "m"(control), [a] "m"(*(const uint32_t (*)[4])a), \
          [b] "m"(*(const uint32_t (*)[4])b) \
        : "xmm0", "xmm1", "memory")
    if (subtract) {
        SSE_ADD_SUB_PROBE(subps);
    } else {
        SSE_ADD_SUB_PROBE(addps);
    }
#undef SSE_ADD_SUB_PROBE
#ifdef _WIN32
    sse_add_output = 0;
    if (handler) RemoveVectoredExceptionHandler(handler);
#endif
    if (!out[5]) {
        for (int i=0;i<4;i++) out[i] = result[i];
        out[4] = flags;
    }
}

#ifdef _WIN32
extern unsigned char vine_sse_addpd_test_instruction, vine_sse_addpd_test_resume;
extern unsigned char vine_sse_subpd_test_instruction, vine_sse_subpd_test_resume;
extern unsigned char vine_sse_mulpd_test_instruction, vine_sse_mulpd_test_resume;
static uint32_t *sse_double_output;
static LONG CALLBACK sse_double_handler(EXCEPTION_POINTERS *exception) {
    CONTEXT *context = exception->ContextRecord;
    if (!sse_double_output) return EXCEPTION_CONTINUE_SEARCH;
    uintptr_t resume;
    if (context->Rip == (uintptr_t)&vine_sse_addpd_test_instruction)
        resume = (uintptr_t)&vine_sse_addpd_test_resume;
    else if (context->Rip == (uintptr_t)&vine_sse_subpd_test_instruction)
        resume = (uintptr_t)&vine_sse_subpd_test_resume;
    else if (context->Rip == (uintptr_t)&vine_sse_mulpd_test_instruction)
        resume = (uintptr_t)&vine_sse_mulpd_test_resume;
    else return EXCEPTION_CONTINUE_SEARCH;
    sse_double_output[0] = (uint32_t)context->Xmm0.Low;
    sse_double_output[1] = (uint32_t)(context->Xmm0.Low >> 32);
    sse_double_output[2] = (uint32_t)context->Xmm0.High;
    sse_double_output[3] = (uint32_t)((uint64_t)context->Xmm0.High >> 32);
    sse_double_output[4] = context->MxCsr;
    sse_double_output[5] = exception->ExceptionRecord->ExceptionCode;
    context->Rip = resume;
    context->MxCsr = context->FltSave.MxCsr = 0x1f80;
    return EXCEPTION_CONTINUE_EXECUTION;
}
#endif

/* Three exact binary64 packed instructions, with the same state preservation and actual
 * Windows fault CONTEXT contract as the multiplication oracle above. */
void vine_sse_native_double(const uint32_t a[4], const uint32_t b[4],
                            uint32_t control, uint32_t operation, uint32_t out[6]) {
    _Alignas(16) unsigned char saved[512];
    uint32_t result[4] = {0}, flags = 0;
    out[5] = 0;
#ifdef _WIN32
    void *handler = 0;
    if ((control & 0x1f80) != 0x1f80) {
        handler = AddVectoredExceptionHandler(1, sse_double_handler);
        if (!handler) { out[5] = UINT32_MAX; return; }
    }
    sse_double_output = out;
#endif
#define SSE_DOUBLE_PROBE(instruction) \
    __asm__ volatile( \
        "fxsave64 %[saved]\n\t" \
        "ldmxcsr %[control]\n\t" \
        "movdqu %[a], %%xmm0\n\t" \
        "movdqu %[b], %%xmm1\n\t" \
        ".globl vine_sse_" #instruction "_test_instruction\n" \
        "vine_sse_" #instruction "_test_instruction:\n\t" \
        #instruction " %%xmm1, %%xmm0\n\t" \
        "movdqu %%xmm0, %[result]\n\t" \
        "stmxcsr %[flags]\n\t" \
        ".globl vine_sse_" #instruction "_test_resume\n" \
        "vine_sse_" #instruction "_test_resume:\n\t" \
        "fxrstor64 %[saved]" \
        : [saved] "=m"(saved), [result] "+m"(result), [flags] "+m"(flags) \
        : [control] "m"(control), [a] "m"(*(const uint32_t (*)[4])a), \
          [b] "m"(*(const uint32_t (*)[4])b) \
        : "xmm0", "xmm1", "memory")
    if (operation == 2) {
        SSE_DOUBLE_PROBE(mulpd);
    } else if (operation == 1) {
        SSE_DOUBLE_PROBE(subpd);
    } else {
        SSE_DOUBLE_PROBE(addpd);
    }
#undef SSE_DOUBLE_PROBE
#ifdef _WIN32
    sse_double_output = 0;
    if (handler) RemoveVectoredExceptionHandler(handler);
#endif
    if (!out[5]) {
        for (int i=0;i<4;i++) out[i] = result[i];
        out[4] = flags;
    }
}
