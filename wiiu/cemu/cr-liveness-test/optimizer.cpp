#include <cassert>
#include <cstdint>
#include <cstdio>

using uint32 = uint32_t;
using sint32 = int32_t;
enum {
    PPCREC_IML_TYPE_R_R, PPCREC_IML_TYPE_R_R_R, PPCREC_IML_TYPE_R_R_S32,
    PPCREC_IML_TYPE_R_S32, PPCREC_IML_TYPE_FPR_R_R, PPCREC_IML_TYPE_FPR_R_R_R,
    PPCREC_IML_TYPE_FPR_R_R_R_R, PPCREC_IML_TYPE_FPR_R,
    PPCREC_IML_TYPE_CJUMP, PPCREC_IML_TYPE_CJUMP_CYCLE_CHECK,
    PPCREC_IML_TYPE_CONDITIONAL_R_S32, PPCREC_IML_TYPE_CR, PPCREC_IML_TYPE_MACRO,
    PPCREC_IML_TYPE_STORE, PPCREC_IML_TYPE_STORE_INDEXED,
    PPCREC_IML_OP_MFCR, PPCREC_IML_OP_MTCRF, PPCREC_IML_OP_CR_CLEAR, PPCREC_IML_OP_CR_SET,
    PPCREC_IML_OP_CR_OR, PPCREC_IML_OP_CR_ORC, PPCREC_IML_OP_CR_AND, PPCREC_IML_OP_CR_ANDC,
    PPCREC_JUMP_CONDITION_NONE, PPC_REC_STORE_STWCX_MARKER,
    SIM_COMPARE, SIM_GPR_ONLY,
};

struct PPCRecImlInstruction_t {
    int type, operation;
    int crRegister = -1;
    uint32 crIgnoreMask = 0;
    struct { uint32 condition, crRegisterIndex, crBitIndex, jumpmarkAddress; } op_conditionalJump{};
    struct { uint32 crRegisterIndex, crBitIndex; } op_conditional_r_s32{};
    struct { int immS32; } op_r_immS32{};
    struct { int crD, crA, crB; } op_cr{};
    struct { int copyWidth; } op_storeLoad{};
};
struct PPCRecImlSegment_t {
    PPCRecImlInstruction_t *imlList = nullptr;
    sint32 imlListCount = 0;
    uint32 crBitsInput = 0, crBitsWritten = 0, crBitsRead = 0;
    PPCRecImlSegment_t *nextSegmentBranchTaken = nullptr, *nextSegmentBranchNotTaken = nullptr;
    bool nextSegmentIsUncertain = false;
};
struct ppcImlGenContext_t {
    PPCRecImlSegment_t *segmentList[6];
    sint32 segmentListCount;
};
static void PPCRecompilerIml_setLinkBranchTaken(PPCRecImlSegment_t *a, PPCRecImlSegment_t *b) { a->nextSegmentBranchTaken = b; }
static void PPCRecompilerIml_setLinkBranchNotTaken(PPCRecImlSegment_t *a, PPCRecImlSegment_t *b) { a->nextSegmentBranchNotTaken = b; }
static PPCRecImlSegment_t *PPCRecompiler_getSegmentByPPCJumpAddress(ppcImlGenContext_t *ctx, uint32 pos) {
    return pos < (uint32)ctx->segmentListCount ? ctx->segmentList[pos] : nullptr;
}
static uint32 ppc_MTCRFMaskToCRBitMask(uint32 mask) {
    uint32 result = 0;
    for (unsigned f = 0; f < 8; ++f)
        if (mask & (1 << (7 - f))) result |= 0xfu << (f * 4);
    return result;
}
#define cemu_assert_unimplemented() assert(false)
#include IML_INPUT

struct Result { uint32 ignores, reads, inputs, writes; bool call_boundary; };

static Result run(bool split, bool with_call, bool branch_reads_eq, int segments, bool reads_mfcr = true)
{
    PPCRecImlInstruction_t compare{PPCREC_IML_TYPE_R_S32, SIM_COMPARE, 0};
    PPCRecImlInstruction_t mfcr{PPCREC_IML_TYPE_R_S32, PPCREC_IML_OP_MFCR};
    PPCRecImlInstruction_t mtcrf{PPCREC_IML_TYPE_R_S32, PPCREC_IML_OP_MTCRF};
    mtcrf.op_r_immS32.immS32 = 128;
    PPCRecImlInstruction_t call{PPCREC_IML_TYPE_MACRO, 0};
    PPCRecImlInstruction_t eq_branch{PPCREC_IML_TYPE_CJUMP, 0};
    eq_branch.op_conditionalJump = { 1, 0, 2, 1 };
    PPCRecImlInstruction_t first[] = { compare, branch_reads_eq ? eq_branch : mfcr, call };
    PPCRecImlInstruction_t gpr_only{PPCREC_IML_TYPE_R_S32, SIM_GPR_ONLY};
    PPCRecImlInstruction_t second[] = { reads_mfcr ? mfcr : gpr_only, with_call ? call : mtcrf, eq_branch };
    if (!with_call) second[2].op_conditionalJump.jumpmarkAddress = 2;
    PPCRecImlSegment_t head{first, branch_reads_eq ? 2 : split ? 1 : 3};
    PPCRecImlSegment_t witness{second, split ? with_call ? 2 : 3 : 0};
    PPCRecImlSegment_t unused[4]{};
    ppcImlGenContext_t ctx{{&head, &witness, &unused[0], &unused[1], &unused[2], &unused[3]}, segments};
    PPCRecompilerIML_linkSegments(&ctx);
    PPCRecompiler_removeRedundantCRUpdates(&ctx);
    return {first[0].crIgnoreMask & 0xf, witness.crBitsRead, witness.crBitsInput,
            witness.crBitsWritten, witness.nextSegmentIsUncertain};
}

int main()
{
    const auto call = run(true, true, false, 6);
    const auto inlined = run(true, false, false, 6);
    const auto direct = run(true, true, true, 6);
    const auto same = run(false, true, false, 6);
    const auto short_call = run(true, true, false, 4);
    const auto known_no_read = run(true, false, false, 6, false);
    const auto unknown_no_read = run(true, true, false, 6, false);
    assert(call.call_boundary && call.reads == UINT32_MAX && call.writes == 0);
    assert(!inlined.call_boundary && inlined.reads == UINT32_MAX && inlined.writes == 15);
    assert(call.ignores == EXPECT_CALL && inlined.ignores == EXPECT_INLINE);
    assert(direct.ignores == (EXPECT_CALL & ~4u) && same.ignores == 0 && short_call.ignores == 0);
    assert(call.inputs == EXPECT_MFCR_INPUT && inlined.inputs == EXPECT_MFCR_INPUT);
    assert(known_no_read.ignores == 15 && unknown_no_read.ignores == EXPECT_CALL);
    printf("join-call ignore=%u join-inline ignore=%u branch-read ignore=%u same-block ignore=%u short-call ignore=%u mfcr live-in=%08x known overwrite=%x unknown edge=%x\n",
           call.ignores, inlined.ignores, direct.ignores, same.ignores, short_call.ignores,
           call.inputs, known_no_read.ignores, unknown_no_read.ignores);

    // Local definition satisfies CR0. MFCR still consumes incoming CR1–7.
    PPCRecImlInstruction_t instructions[] = {
        {PPCREC_IML_TYPE_R_S32, SIM_COMPARE, 0},
        {PPCREC_IML_TYPE_R_S32, PPCREC_IML_OP_MFCR},
        {PPCREC_IML_TYPE_MACRO, 0},
    };
    PPCRecImlSegment_t block{instructions, 3};
    ppcImlGenContext_t local{{&block}, 1};
    PPCRecompilerIML_linkSegments(&local);
    PPCRecompiler_removeRedundantCRUpdates(&local);
    assert(block.crBitsInput == (EXPECT_MFCR_INPUT & ~15u));
    assert((instructions[0].crIgnoreMask & 15u) == 0);
}
