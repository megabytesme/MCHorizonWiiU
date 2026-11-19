#include "keyboard_bridge.h"

#include <nsyskbd/nsyskbd.h>
#include <stdatomic.h>
#include <stddef.h>

/*
 * Bounded MPMC queue: nsyskbd does not document which Cafe thread invokes its
 * callbacks, so callback records are copied here and consumed on the host loop.
 * Callback code performs no allocation, logging, or game/UI dispatch.
 */
#define KEYBOARD_QUEUE_CAPACITY 128u
#define KEYBOARD_QUEUE_MASK (KEYBOARD_QUEUE_CAPACITY - 1u)

_Static_assert((KEYBOARD_QUEUE_CAPACITY & KEYBOARD_QUEUE_MASK) == 0,
               "keyboard queue capacity must be a power of two");

typedef struct KeyboardQueueSlot {
   _Atomic uint32_t sequence;
   HorizonKeyboardEvent event;
} KeyboardQueueSlot;

static KeyboardQueueSlot queue[KEYBOARD_QUEUE_CAPACITY];
static _Atomic uint32_t enqueue_position;
static _Atomic uint32_t dequeue_position;
static _Atomic uint32_t dropped_events;
static _Atomic int callbacks_enabled;
/* 0 = uninitialized, 1 = initializing, 2 = ready. */
static _Atomic int queue_state;

static void
initialize_queue(void)
{
   int state = atomic_load_explicit(&queue_state, memory_order_acquire);
   if (state == 2) {
      return;
   }

   int uninitialized = 0;
   if (!atomic_compare_exchange_strong_explicit(&queue_state, &uninitialized, 1,
                                               memory_order_acq_rel,
                                               memory_order_acquire)) {
      while (atomic_load_explicit(&queue_state, memory_order_acquire) != 2) {
      }
      return;
   }

   for (uint32_t i = 0; i < KEYBOARD_QUEUE_CAPACITY; ++i) {
      atomic_init(&queue[i].sequence, i);
   }
   atomic_store_explicit(&enqueue_position, 0, memory_order_relaxed);
   atomic_store_explicit(&dequeue_position, 0, memory_order_relaxed);
   atomic_store_explicit(&dropped_events, 0, memory_order_relaxed);
   atomic_store_explicit(&queue_state, 2, memory_order_release);
}

static void
enqueue_event(HorizonKeyboardEvent event)
{
   uint32_t position = atomic_load_explicit(&enqueue_position, memory_order_relaxed);
   KeyboardQueueSlot *slot;

   for (;;) {
      slot = &queue[position & KEYBOARD_QUEUE_MASK];
      uint32_t sequence = atomic_load_explicit(&slot->sequence, memory_order_acquire);
      int32_t difference = (int32_t)(sequence - position);

      if (difference == 0) {
         if (atomic_compare_exchange_weak_explicit(&enqueue_position, &position,
                                                   position + 1u,
                                                   memory_order_relaxed,
                                                   memory_order_relaxed)) {
            break;
         }
      } else if (difference < 0) {
         atomic_fetch_add_explicit(&dropped_events, 1, memory_order_relaxed);
         return;
      } else {
         position = atomic_load_explicit(&enqueue_position, memory_order_relaxed);
      }
   }

   slot->event = event;
   atomic_store_explicit(&slot->sequence, position + 1u, memory_order_release);
}

static int
dequeue_event(HorizonKeyboardEvent *event)
{
   uint32_t position = atomic_load_explicit(&dequeue_position, memory_order_relaxed);
   KeyboardQueueSlot *slot;

   for (;;) {
      slot = &queue[position & KEYBOARD_QUEUE_MASK];
      uint32_t sequence = atomic_load_explicit(&slot->sequence, memory_order_acquire);
      int32_t difference = (int32_t)(sequence - (position + 1u));

      if (difference == 0) {
         if (atomic_compare_exchange_weak_explicit(&dequeue_position, &position,
                                                   position + 1u,
                                                   memory_order_relaxed,
                                                   memory_order_relaxed)) {
            break;
         }
      } else if (difference < 0) {
         return 0;
      } else {
         position = atomic_load_explicit(&dequeue_position, memory_order_relaxed);
      }
   }

   *event = slot->event;
   atomic_store_explicit(&slot->sequence,
                         position + KEYBOARD_QUEUE_CAPACITY,
                         memory_order_release);
   return 1;
}

static void
keyboard_attached(KBDAttachEvent *event)
{
   if (event == NULL || !atomic_load_explicit(&callbacks_enabled, memory_order_relaxed)) {
      return;
   }

   HorizonKeyboardEvent record = {0};
   record.kind = HORIZON_KEYBOARD_ATTACHED;
   record.channel = event->channel;
   enqueue_event(record);
}

static void
keyboard_detached(KBDAttachEvent *event)
{
   if (event == NULL || !atomic_load_explicit(&callbacks_enabled, memory_order_relaxed)) {
      return;
   }

   HorizonKeyboardEvent record = {0};
   record.kind = HORIZON_KEYBOARD_DETACHED;
   record.channel = event->channel;
   enqueue_event(record);
}

static void
keyboard_key(KBDKeyEvent *event)
{
   if (event == NULL || !atomic_load_explicit(&callbacks_enabled, memory_order_relaxed)) {
      return;
   }

   HorizonKeyboardEvent record = {0};
   record.kind = HORIZON_KEYBOARD_KEY;
   record.channel = event->channel;
   record.hid_code = event->hidCode;
   record.is_pressed_down = event->isPressedDown != 0;
   record.utf16_character = event->asUTF16Character;
   enqueue_event(record);
}

int32_t
horizon_wiiu_keyboard_init(void)
{
   initialize_queue();
   atomic_store_explicit(&callbacks_enabled, 1, memory_order_release);
   KDBError result = KBDSetup(keyboard_attached, keyboard_detached, keyboard_key);
   if (result != KDB_ERROR_NONE) {
      atomic_store_explicit(&callbacks_enabled, 0, memory_order_release);
   }
   return (int32_t)result;
}

int32_t
horizon_wiiu_keyboard_teardown(void)
{
   atomic_store_explicit(&callbacks_enabled, 0, memory_order_release);
   return (int32_t)KBDTeardown();
}

int32_t
horizon_wiiu_keyboard_pop(HorizonKeyboardEvent *event)
{
   if (event == NULL || atomic_load_explicit(&queue_state, memory_order_acquire) != 2) {
      return 0;
   }
   return dequeue_event(event);
}

uint32_t
horizon_wiiu_keyboard_dropped(void)
{
   return atomic_load_explicit(&dropped_events, memory_order_relaxed);
}
