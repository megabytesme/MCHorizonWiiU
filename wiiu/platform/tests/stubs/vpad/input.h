#pragma once

#include <stdint.h>

#define VPAD_CHAN_0 0
#define VPAD_TP_1920X1080 0
#define VPAD_VALID 0

typedef int32_t VPADChan;
typedef enum VPADReadError {
   VPAD_READ_SUCCESS = 0,
   VPAD_READ_NO_SAMPLES = -1,
   VPAD_READ_BUSY = -4,
   VPAD_READ_UNINITIALIZED = -5,
} VPADReadError;
typedef enum VPADButtonProcMode {
   VPAD_BUTTON_PROC_MODE_LOOSE = 0,
   VPAD_BUTTON_PROC_MODE_TIGHT = 1,
} VPADButtonProcMode;
typedef struct VPADVec2D {
   float x, y;
} VPADVec2D;
typedef struct VPADTouchData {
   uint16_t x, y, touched, validity;
} VPADTouchData;
typedef struct VPADStatus {
   uint32_t hold, trigger, release;
   VPADVec2D leftStick, rightStick;
   VPADTouchData tpNormal;
} VPADStatus;

int32_t VPADRead(VPADChan chan, VPADStatus *buffers, uint32_t count,
                 VPADReadError *out_error);
void VPADSetButtonProcMode(VPADChan chan, VPADButtonProcMode mode);
void VPADGetTPCalibratedPointEx(VPADChan chan, int32_t resolution,
                                VPADTouchData *out, const VPADTouchData *in);
