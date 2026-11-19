#pragma once

#include <stdint.h>
#include <stddef.h>

/* Raw nsyskbd events copied by its callbacks and drained by the Cafe main loop. */
typedef struct HorizonKeyboardEvent {
   uint8_t kind;
   uint8_t channel;
   uint8_t hid_code;
   uint8_t is_pressed_down;
   uint16_t utf16_character;
   uint16_t reserved;
} HorizonKeyboardEvent;

_Static_assert(sizeof(HorizonKeyboardEvent) == 8,
               "unexpected keyboard event ABI size");
_Static_assert(offsetof(HorizonKeyboardEvent, utf16_character) == 4,
               "unexpected keyboard event UTF-16 offset");
_Static_assert(offsetof(HorizonKeyboardEvent, reserved) == 6,
               "unexpected keyboard event reserved offset");

enum HorizonKeyboardEventKind {
   HORIZON_KEYBOARD_ATTACHED = 1,
   HORIZON_KEYBOARD_DETACHED = 2,
   HORIZON_KEYBOARD_KEY = 3,
};

/* Initialize/tear down nsyskbd. Return the KDBError value from WUT. */
int32_t horizon_wiiu_keyboard_init(void);
int32_t horizon_wiiu_keyboard_teardown(void);

/* Return 1 and copy one queued record, or 0 when the queue is empty. */
int32_t horizon_wiiu_keyboard_pop(HorizonKeyboardEvent *event);

/* Number of callback records dropped because the bounded queue was full. */
uint32_t horizon_wiiu_keyboard_dropped(void);
