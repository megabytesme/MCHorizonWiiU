#include "vpad_bridge.h"
#include <coreinit/debug.h>
#include <vpad/input.h>
#include "swkbd_bridge.h"

static int32_t swkbd_input_occupied;
static int32_t trace_first_poll = 1;
#ifdef HORIZON_WIIU_TRACE_VPAD_INPUT
static int32_t trace_have_last_input;
static uint32_t trace_last_hold;
static int32_t trace_last_touched;
static uint16_t trace_last_touch_validity;
#endif

static void
update_swkbd(const VPADStatus *status, int32_t trace)
{
   if (status == NULL) {
      if (trace) OSReport("Horizon Cafe input trace: swkbd IsOpen begin\n");
      int32_t open = horizon_swkbd_is_open();
      if (trace) OSReport("Horizon Cafe input trace: swkbd IsOpen complete\n");
      swkbd_input_occupied = open != 0;
      return;
   }

   if (trace) OSReport("Horizon Cafe input trace: swkbd Calc begin\n");
   int32_t result = horizon_swkbd_calc(status);
   if (trace) OSReport("Horizon Cafe input trace: swkbd Calc complete\n");
   if (result != 0) {
      swkbd_input_occupied = 1;
      return;
   }
   if (trace) OSReport("Horizon Cafe input trace: swkbd IsOpen begin\n");
   int32_t open = horizon_swkbd_is_open();
   if (trace) OSReport("Horizon Cafe input trace: swkbd IsOpen complete\n");
   swkbd_input_occupied = open != 0;
}

int32_t
horizon_wiiu_swkbd_input_occupied(void)
{
   return swkbd_input_occupied;
}

int32_t
horizon_wiiu_vpad_read(HorizonVpadSample *sample)
{
   int32_t trace = trace_first_poll;
   trace_first_poll = 0;

   VPADStatus status;
   VPADReadError error = VPAD_READ_SUCCESS;
   if (trace) OSReport("Horizon Cafe input trace: VPADRead begin\n");
   int32_t count = VPADRead(VPAD_CHAN_0, &status, 1, &error);
   if (trace) {
      OSReport("Horizon Cafe input trace: VPADRead complete count=%d error=%d\n",
               count, (int32_t)error);
   }
   if (error != VPAD_READ_SUCCESS) {
      update_swkbd(NULL, trace);
      return (int32_t)error;
   }
   if (count <= 0) {
      update_swkbd(NULL, trace);
      return VPAD_READ_NO_SAMPLES;
   }
#ifdef HORIZON_WIIU_TRACE_VPAD_INPUT
   const int32_t touched = status.tpNormal.touched != 0;
   const int32_t input_changed = !trace_have_last_input ||
      status.hold != trace_last_hold || status.trigger != 0 || status.release != 0 ||
      touched != trace_last_touched ||
      (touched && status.tpNormal.validity != trace_last_touch_validity);
   if (input_changed) {
      OSReport("Horizon Cafe VPAD input: hold=%08x trigger=%08x release=%08x "
               "raw_touch=%u validity=%u x=%u y=%u\n",
               (unsigned)status.hold, (unsigned)status.trigger,
               (unsigned)status.release,
               (unsigned)status.tpNormal.touched,
               (unsigned)status.tpNormal.validity,
               (unsigned)status.tpNormal.x, (unsigned)status.tpNormal.y);
   }
   trace_last_hold = status.hold;
   trace_last_touched = touched;
   trace_last_touch_validity = status.tpNormal.validity;
   trace_have_last_input = 1;
#endif
   update_swkbd(&status, trace);
   sample->buttons = status.hold;
   sample->left_x = status.leftStick.x;
   sample->left_y = status.leftStick.y;
   sample->right_x = status.rightStick.x;
   sample->right_y = status.rightStick.y;
   sample->touch_x = 0;
   sample->touch_y = 0;
   sample->touch_active = status.tpNormal.touched != 0;
   sample->touch_valid = 0;
   if (status.tpNormal.touched != 0) {
      VPADTouchData calibrated;
      if (trace) OSReport("Horizon Cafe input trace: touch calibration begin\n");
      VPADGetTPCalibratedPointEx(VPAD_CHAN_0, VPAD_TP_1920X1080,
                                 &calibrated, &status.tpNormal);
      if (trace) OSReport("Horizon Cafe input trace: touch calibration complete\n");
      if (calibrated.validity == VPAD_VALID && calibrated.touched != 0 &&
          calibrated.x < 1920 && calibrated.y < 1080) {
         sample->touch_x = calibrated.x;
         sample->touch_y = calibrated.y;
         sample->touch_valid = 1;
      }
   }
   if (trace) OSReport("Horizon Cafe input trace: VPAD sample conversion complete\n");
   return 1;
}
