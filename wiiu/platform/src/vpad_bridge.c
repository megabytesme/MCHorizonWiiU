#include "vpad_bridge.h"
#include <vpad/input.h>
#include "swkbd_bridge.h"

static int32_t swkbd_input_occupied;

static void
update_swkbd(const VPADStatus *status)
{
   int32_t result = horizon_swkbd_calc(status);
   swkbd_input_occupied = result != 0 || horizon_swkbd_is_open() != 0;
}

int32_t
horizon_wiiu_swkbd_input_occupied(void)
{
   return swkbd_input_occupied;
}

int32_t
horizon_wiiu_vpad_read(HorizonVpadSample *sample)
{
   VPADStatus status;
   VPADReadError error = VPAD_READ_SUCCESS;
   int32_t count = VPADRead(VPAD_CHAN_0, &status, 1, &error);
   if (error != VPAD_READ_SUCCESS) {
      update_swkbd(NULL);
      return (int32_t)error;
   }
   if (count <= 0) {
      update_swkbd(NULL);
      return VPAD_READ_NO_SAMPLES;
   }
   update_swkbd(&status);
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
      VPADGetTPCalibratedPointEx(VPAD_CHAN_0, VPAD_TP_1920X1080,
                                 &calibrated, &status.tpNormal);
      if (calibrated.validity == VPAD_VALID && calibrated.touched != 0 &&
          calibrated.x < 1920 && calibrated.y < 1080) {
         sample->touch_x = calibrated.x;
         sample->touch_y = calibrated.y;
         sample->touch_valid = 1;
      }
   }
   return 1;
}
