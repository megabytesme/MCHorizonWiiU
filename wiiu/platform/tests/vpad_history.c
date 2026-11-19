#include "../src/vpad_bridge.h"
#include <assert.h>
#include <stddef.h>

static void
expect_history(const HorizonVpadButtonSample *samples_newest_first, uint32_t sample_count,
               uint32_t previous, const uint32_t *expected, uint32_t expected_count)
{
   uint32_t output[HORIZON_VPAD_HISTORY_CAPACITY] = {0};
   uint32_t count = horizon_vpad_collect_button_history(
      samples_newest_first, sample_count, previous, output,
      HORIZON_VPAD_HISTORY_CAPACITY);
   assert(count == expected_count);
   for (uint32_t i = 0; i < count; ++i) assert(output[i] == expected[i]);
}

int
main(void)
{
   /* Native ownership preserves LOOSE across open and final occupied poll. */
   assert(horizon_vpad_uses_tight_button_mode(0, 0) == 1);
   assert(horizon_vpad_uses_tight_button_mode(1, 0) == 0);
   assert(horizon_vpad_uses_tight_button_mode(0, 1) == 0);
   assert(horizon_vpad_uses_tight_button_mode(0, 0) == 1);

   /* Prior up; a physical press/release arrives newest-first as up, down. */
   const HorizonVpadButtonSample press_release_newest[] = {
      {.hold = 0, .release = 1},
      {.hold = 1, .trigger = 1},
   };
   const uint32_t press_release[] = {1, 0};
   expect_history(press_release_newest, 2, 0, press_release, 2);

   /* Starting held, release then repress preserves both changed states. */
   const HorizonVpadButtonSample held_release_repress_newest[] = {
      {.hold = 1, .trigger = 1},
      {.hold = 0, .release = 1},
      {.hold = 1},
   };
   const uint32_t release_repress[] = {0, 1};
   expect_history(held_release_repress_newest, 3, 1, release_repress, 2);

   /* Identical samples do not create synthetic edges. */
   const HorizonVpadButtonSample repeated[] = {
      {.hold = 1}, {.hold = 1}, {.hold = 1}, {.hold = 1},
   };
   expect_history(repeated, 4, 1, NULL, 0);

   /* One status can report a complete tap even though hold is up. */
   const HorizonVpadButtonSample same_sample_tap[] = {
      {.hold = 0, .trigger = 1, .release = 1},
   };
   expect_history(same_sample_tap, 1, 0, press_release, 2);
   return 0;
}
