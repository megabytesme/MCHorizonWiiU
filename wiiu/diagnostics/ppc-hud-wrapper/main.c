#include <coreinit/debug.h>
#include <coreinit/thread.h>
#include <coreinit/time.h>
#include <stdbool.h>
#include <stdint.h>

typedef int (*Draw)(void *, uint32_t, uint32_t, uint32_t);
typedef bool (*PrepareDrc)(void *, uint64_t);
typedef struct { uint32_t active, seal_calls; } Arena;

int probe_original(Draw, void *, const float *, uint64_t *, Arena *, bool, PrepareDrc);
int probe_repaired(Draw, void *, const float *, uint64_t *, Arena *, bool, PrepareDrc);

uint32_t last_outputs;
uint64_t last_wait_ns;
static volatile bool foreground = true;
static unsigned plain_calls, grouped_calls, unexpected_calls;

int ProcUIInForeground(void) { return foreground; }

int present_with_clear(Draw draw, void *ctx, const float *rgba, const uint32_t *viewport, bool defer,
                       uint64_t *ticket, bool *verified, bool color_only)
{
    (void)draw;
    (void)ctx;
    (void)rgba;
    (void)viewport;
    (void)defer;
    (void)color_only;
    ++plain_calls;
    *ticket = 9001;
    if (verified) *verified = true;
    last_outputs = 3;
    // Explicit CR0 clobber. The wrapper must restore its saved comparison.
    const int zero = 0;
    __asm__ volatile("cmpwi 0,%0,0" :: "r"(zero) : "cr0");
    return 0;
}

int present_grouped(Draw draw, void *ctx, const float *rgba, const uint32_t *viewport,
                    uint64_t *ticket, bool *verified, PrepareDrc prepare)
{
    (void)draw;
    (void)ctx;
    (void)rgba;
    (void)viewport;
    (void)prepare;
    ++grouped_calls;
    *ticket = 9002;
    if (verified) *verified = true;
    last_outputs = 3;
    return 0;
}

bool horizon_gx2_hud_view_lanes_seal(Arena *arena, uint64_t ticket)
{
    ++arena->seal_calls;
    if (!arena->active) return false;
    if (ticket != 9001 && ticket != 9002 && ticket != 0) return false;
    arena->active = 0;
    return true;
}
void horizon_gx2_hud_view_lanes_finish_verified(Arena *arena) { (void)arena; ++unexpected_calls; }
int64_t drain_guarded_commands_through(int64_t floor) { (void)floor; ++unexpected_calls; return 0; }

int main(void)
{
    enum { EPOCHS = 192, ITERATIONS = 256 };
    unsigned errors[2][4] = {{0}};
    uint32_t measurements[2][4] = {{0}};
    for (unsigned epoch = 0; epoch < EPOCHS; ++epoch) {
        for (unsigned i = 0; i < ITERATIONS; ++i) {
            for (unsigned arm = 0; arm < 2; ++arm) {
                // Preserve both incoming blocks of the original shared plain path.
                const unsigned test = (i + arm) & 3u;
                const bool active = test != 2;
                const bool grouped = test != 0;
                foreground = test != 3;
                Arena arena = { active, 0 };
                uint64_t ticket = UINT64_MAX;
                const float clear[4] = {0};
                const int result = arm == 0
                    ? probe_original(0, 0, clear, &ticket, active ? &arena : 0, grouped, 0)
                    : probe_repaired(0, 0, clear, &ticket, active ? &arena : 0, grouped, 0);
                const int expected_result = foreground ? 0 : -5;
                const uint64_t expected_ticket = foreground ? (active && grouped ? 9002u : 9001u) : 0u;
                const uint32_t expected_outputs = foreground ? 3 : 0;
                const bool bad = result != expected_result || ticket != expected_ticket ||
                    last_outputs != expected_outputs || arena.active != 0 ||
                    arena.seal_calls != (uint32_t)active;
                if (bad && !errors[arm][test]) {
                    OSReport("HUD CPU first arm=%u case=%u epoch=%u status=%d ticket=%llu active=%u seals=%u outputs=%u\n",
                             arm, test, epoch, result, (unsigned long long)ticket,
                             arena.active, arena.seal_calls, last_outputs);
                }
                errors[arm][test] += bad;
                ++measurements[arm][test];
            }
        }
        // The first CPU fixture ran in a few milliseconds. Permit asynchronous
        // recompilation to settle and cover scheduling transitions here.
        OSSleepTicks(OSMillisecondsToTicks(16));
    }
    unsigned failed_cases = unexpected_calls;
    for (unsigned arm = 0; arm < 2; ++arm) {
        for (unsigned test = 0; test < 4; ++test) {
            failed_cases += errors[arm][test] != 0;
            OSReport("HUD CPU result arm=%u case=%u checks=%u errors=%u\n",
                     arm, test, measurements[arm][test], errors[arm][test]);
        }
    }
    OSReport("HUD CPU terminal failed=%u plain=%u grouped=%u unexpected=%u\n",
             failed_cases, plain_calls, grouped_calls, unexpected_calls);
    return (int)failed_cases;
}
