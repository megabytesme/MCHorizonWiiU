#pragma once

#include <stdint.h>

/* Boot-protocol mouse reports copied from asynchronous nsyshid callbacks. */
typedef struct HorizonMouseEvent {
   uint8_t kind;
   uint8_t buttons;
   int8_t x;
   int8_t y;
   int8_t wheel;
   uint8_t reserved[3];
} HorizonMouseEvent;

_Static_assert(sizeof(HorizonMouseEvent) == 8,
               "unexpected mouse event ABI size");

enum HorizonMouseEventKind {
   HORIZON_MOUSE_REPORT = 1,
   HORIZON_MOUSE_DETACHED = 2,
};

/* Initialize/tear down nsyshid. Return the HID API result. */
int32_t horizon_wiiu_mouse_init(void);
int32_t horizon_wiiu_mouse_teardown(void);

/* Return 1 and copy one queued record, or 0 when the queue is empty. */
int32_t horizon_wiiu_mouse_pop(HorizonMouseEvent *event);

/* Number of callback records dropped because the bounded queue was full. */
uint32_t horizon_wiiu_mouse_dropped(void);
