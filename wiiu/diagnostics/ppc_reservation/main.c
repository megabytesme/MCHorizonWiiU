#include <coreinit/debug.h>
#include <stdint.h>
#include <whb/proc.h>

extern uint32_t preserve_store_success(uint32_t *base, uint32_t input);
extern uint32_t preserve_store_mismatch(uint32_t *base, uint32_t input);
extern uint32_t preserve_mirrored_load(uint32_t *base);
extern uint32_t preserve_mirrored_store(uint32_t *base, uint32_t input);

static uint32_t failures[6];

static void check(unsigned slot, unsigned iteration, uint32_t value, uint32_t expected)
{
    if (value != expected && failures[slot] == 0) {
        failures[slot] = iteration;
        OSReport("ALLOC_COUNTER first_bad mode=%u iteration=%u actual=%08x expected=%08x\n",
                 slot, iteration, value, expected);
    }
}

int main(void)
{
    static uint32_t values[33];
    uint32_t half = (uint32_t)(uintptr_t)values >> 1;
    WHBProcInit();
    for (unsigned i = 1; i <= 1000000; ++i) {
        values[0] = i;
        uint32_t input = i + 1;
        check(0, i, preserve_store_success(values, input), i);
        check(1, i, values[1], input);
        check(2, i, preserve_store_mismatch(values, input), i);
        check(3, i, preserve_mirrored_load(values), half);
        check(4, i, preserve_mirrored_store(values, input), half);
        check(5, i, values[0], input);
    }
    unsigned fail = 0;
    for (unsigned n = 0; n < 6; ++n) fail |= failures[n];
    OSReport("ALLOC_COUNTER result=%s iterations=1000000\n", fail ? "FAIL" : "PASS");
    WHBProcShutdown();
    return fail != 0;
}
