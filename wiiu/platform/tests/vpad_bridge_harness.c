#include "../src/vpad_bridge.h"
#include <assert.h>
#include <stdarg.h>
#include <stdint.h>
#include <string.h>
#include <vpad/input.h>

static VPADStatus scripted_statuses[HORIZON_VPAD_SAMPLE_CAPACITY];
static uint32_t scripted_count;
static VPADReadError scripted_error;
static uint32_t read_capacity;
static VPADButtonProcMode active_mode = VPAD_BUTTON_PROC_MODE_LOOSE;
static uint32_t mode_set_calls;
static int32_t swkbd_open;
static int32_t swkbd_calc_result;
static uint32_t swkbd_calc_calls;
static uint32_t swkbd_is_open_calls;
static uint32_t swkbd_last_hold;

void
OSReport(const char *format, ...)
{
   (void)format;
}

int32_t
VPADRead(VPADChan chan, VPADStatus *buffers, uint32_t count,
         VPADReadError *out_error)
{
   (void)chan;
   read_capacity = count;
   *out_error = scripted_error;
   if (scripted_error != VPAD_READ_SUCCESS) return 0;
   uint32_t copied = scripted_count < count ? scripted_count : count;
   memcpy(buffers, scripted_statuses, copied * sizeof(*buffers));
   return (int32_t)copied;
}

void
VPADSetButtonProcMode(VPADChan chan, VPADButtonProcMode mode)
{
   (void)chan;
   active_mode = mode;
   ++mode_set_calls;
}

void
VPADGetTPCalibratedPointEx(VPADChan chan, int32_t resolution,
                           VPADTouchData *out, const VPADTouchData *in)
{
   (void)chan;
   (void)resolution;
   *out = *in;
}

int32_t
horizon_swkbd_is_open(void)
{
   ++swkbd_is_open_calls;
   return swkbd_open;
}

int32_t
horizon_swkbd_calc(const void *vpad_status)
{
   const VPADStatus *status = vpad_status;
   ++swkbd_calc_calls;
   swkbd_last_hold = status->hold;
   return swkbd_calc_result;
}

static void
script(const VPADStatus *statuses_newest_first, uint32_t count,
       VPADReadError error)
{
   assert(count <= HORIZON_VPAD_SAMPLE_CAPACITY);
   scripted_count = count;
   scripted_error = error;
   if (count != 0) memcpy(scripted_statuses, statuses_newest_first,
                          count * sizeof(*scripted_statuses));
}

static void
expect_transitions(const HorizonVpadBatch *batch, const uint32_t *expected,
                   uint32_t expected_count)
{
   assert(batch->transition_count == expected_count);
   for (uint32_t i = 0; i < expected_count; ++i) {
      assert(batch->transitions[i] == expected[i]);
   }
}

static void
test_tight_ordered_button_history(void)
{
   HorizonVpadBatch batch = {0};
   const VPADStatus neutral[] = {{.hold = 0}};
   script(neutral, 1, VPAD_READ_SUCCESS);
   assert(horizon_wiiu_vpad_read(&batch) == 1);
   assert(active_mode == VPAD_BUTTON_PROC_MODE_TIGHT);
   assert(mode_set_calls == 1);
   assert(read_capacity == HORIZON_VPAD_SAMPLE_CAPACITY);
   expect_transitions(&batch, NULL, 0);

   /* WUT order is newest first; chronologically this is press/release/press. */
   const VPADStatus up_down_up[] = {
      {.hold = 1, .trigger = 1},
      {.hold = 0, .release = 1},
      {.hold = 1, .trigger = 1},
   };
   const uint32_t press_release_press[] = {1, 0, 1};
   script(up_down_up, 3, VPAD_READ_SUCCESS);
   assert(horizon_wiiu_vpad_read(&batch) == 1);
   expect_transitions(&batch, press_release_press, 3);
   assert(batch.latest.buttons == 1);

   /* Starting held, a release/repress cannot collapse to the final held bit. */
   const VPADStatus release_repress[] = {
      {.hold = 1, .trigger = 1},
      {.hold = 0, .release = 1},
      {.hold = 1},
   };
   const uint32_t released_pressed[] = {0, 1};
   script(release_repress, 3, VPAD_READ_SUCCESS);
   assert(horizon_wiiu_vpad_read(&batch) == 1);
   expect_transitions(&batch, released_pressed, 2);

   const VPADStatus repeated[] = {{.hold = 1}, {.hold = 1}, {.hold = 1}};
   script(repeated, 3, VPAD_READ_SUCCESS);
   assert(horizon_wiiu_vpad_read(&batch) == 1);
   expect_transitions(&batch, NULL, 0);
   assert(active_mode == VPAD_BUTTON_PROC_MODE_TIGHT);
   assert(mode_set_calls == 1);
}

static void
test_loose_keyboard_ownership_and_restore(void)
{
   HorizonVpadBatch batch = {0};
   const VPADStatus statuses[] = {
      {.hold = 0, .release = 1},
      {.hold = 1, .trigger = 1},
   };
   uint32_t prior_calc_calls = swkbd_calc_calls;
   swkbd_open = 1;
   swkbd_calc_result = 1;
   script(statuses, 2, VPAD_READ_SUCCESS);
   assert(horizon_wiiu_vpad_read(&batch) == 1);
   assert(active_mode == VPAD_BUTTON_PROC_MODE_LOOSE);
   assert(mode_set_calls == 2);
   assert(swkbd_calc_calls == prior_calc_calls + 1);
   assert(swkbd_last_hold == statuses[0].hold);
   assert(horizon_wiiu_swkbd_input_occupied() == 1);
   const uint32_t keyboard_poll_release[] = {0};
   expect_transitions(&batch, keyboard_poll_release, 1);

   /* The final occupied poll stays LOOSE even once IsOpen has gone false. */
   swkbd_open = 0;
   swkbd_calc_result = 0;
   script(statuses, 2, VPAD_READ_SUCCESS);
   assert(horizon_wiiu_vpad_read(&batch) == 1);
   assert(active_mode == VPAD_BUTTON_PROC_MODE_LOOSE);
   assert(horizon_wiiu_swkbd_input_occupied() == 0);

   /* The next real poll restores TIGHT. Its history is still physical data. */
   const VPADStatus neutral[] = {{.hold = 0}};
   script(neutral, 1, VPAD_READ_SUCCESS);
   assert(horizon_wiiu_vpad_read(&batch) == 1);
   assert(active_mode == VPAD_BUTTON_PROC_MODE_TIGHT);
   assert(mode_set_calls == 3);
   assert(swkbd_calc_calls == prior_calc_calls + 3);
   expect_transitions(&batch, NULL, 0);
}

static void
test_touch_latest_sample_and_poll_errors(void)
{
   HorizonVpadBatch batch = {0};
   VPADStatus contact = {
      .hold = 0,
      .tpNormal = {.x = 777, .y = 333, .touched = 1, .validity = VPAD_VALID},
   };
   script(&contact, 1, VPAD_READ_SUCCESS);
   assert(horizon_wiiu_vpad_read(&batch) == 1);
   assert(batch.latest.touch_active == 1);
   assert(batch.latest.touch_valid == 1);
   assert(batch.latest.touch_x == 777 && batch.latest.touch_y == 333);

   VPADStatus invalid_contact = {
      .tpNormal = {.x = 777, .y = 333, .touched = 1, .validity = 1},
   };
   script(&invalid_contact, 1, VPAD_READ_SUCCESS);
   assert(horizon_wiiu_vpad_read(&batch) == 1);
   assert(batch.latest.touch_active == 1);
   assert(batch.latest.touch_valid == 0);

   VPADStatus release = {0};
   script(&release, 1, VPAD_READ_SUCCESS);
   assert(horizon_wiiu_vpad_read(&batch) == 1);
   assert(batch.latest.touch_active == 0);
   assert(batch.latest.touch_valid == 0);

   const VPADStatus sentinel = {.hold = 0xdeadbeef};
   batch.latest.buttons = sentinel.hold;
   uint32_t calc_calls = swkbd_calc_calls;
   /* A no-sample poll still refreshes keyboard ownership from IsOpen. */
   swkbd_open = 1;
   script(NULL, 0, VPAD_READ_NO_SAMPLES);
   assert(horizon_wiiu_vpad_read(&batch) == VPAD_READ_NO_SAMPLES);
   assert(batch.latest.buttons == sentinel.hold);
   assert(swkbd_calc_calls == calc_calls);
   assert(horizon_wiiu_swkbd_input_occupied() == 1);
   assert(active_mode == VPAD_BUTTON_PROC_MODE_LOOSE);

   /* Busy/error polls also refresh occupancy, but do not call Calc. */
   swkbd_open = 0;
   script(NULL, 0, VPAD_READ_BUSY);
   assert(horizon_wiiu_vpad_read(&batch) == VPAD_READ_BUSY);
   assert(swkbd_calc_calls == calc_calls);
   assert(batch.latest.buttons == sentinel.hold);
   assert(horizon_wiiu_swkbd_input_occupied() == 0);
   script(NULL, 0, VPAD_READ_UNINITIALIZED);
   assert(horizon_wiiu_vpad_read(&batch) == VPAD_READ_UNINITIALIZED);
   assert(swkbd_calc_calls == calc_calls);
   assert(batch.latest.buttons == sentinel.hold);
   assert(horizon_wiiu_swkbd_input_occupied() == 0);
}

int
main(void)
{
   test_tight_ordered_button_history();
   test_loose_keyboard_ownership_and_restore();
   test_touch_latest_sample_and_poll_errors();
   return 0;
}
