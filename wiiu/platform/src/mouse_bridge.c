#include "mouse_bridge.h"

#include <nsyshid/hid.h>
#include <stdatomic.h>
#include <stddef.h>
#include <stdint.h>

#define MOUSE_QUEUE_CAPACITY 64u
#define MOUSE_QUEUE_MASK (MOUSE_QUEUE_CAPACITY - 1u)

_Static_assert((MOUSE_QUEUE_CAPACITY & MOUSE_QUEUE_MASK) == 0,
               "mouse queue capacity must be a power of two");

typedef struct MouseQueueSlot {
   _Atomic uint32_t sequence;
   HorizonMouseEvent event;
} MouseQueueSlot;

static MouseQueueSlot queue[MOUSE_QUEUE_CAPACITY];
static _Atomic uint32_t enqueue_position;
static _Atomic uint32_t dequeue_position;
static _Atomic uint32_t dropped_events;
static _Atomic int queue_state;
static _Atomic int callbacks_enabled;
static _Atomic int device_active;
static _Atomic uint32_t device_handle;
static _Atomic uint32_t device_generation;
static uint8_t report_buffer[8];
static HIDClient hid_client;
static int client_registered;

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

   for (uint32_t i = 0; i < MOUSE_QUEUE_CAPACITY; ++i) {
      atomic_init(&queue[i].sequence, i);
   }
   atomic_store_explicit(&enqueue_position, 0, memory_order_relaxed);
   atomic_store_explicit(&dequeue_position, 0, memory_order_relaxed);
   atomic_store_explicit(&dropped_events, 0, memory_order_relaxed);
   atomic_store_explicit(&queue_state, 2, memory_order_release);
}

static void
enqueue_event(HorizonMouseEvent event)
{
   uint32_t position = atomic_load_explicit(&enqueue_position, memory_order_relaxed);
   MouseQueueSlot *slot;
   for (;;) {
      slot = &queue[position & MOUSE_QUEUE_MASK];
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
dequeue_event(HorizonMouseEvent *event)
{
   uint32_t position = atomic_load_explicit(&dequeue_position, memory_order_relaxed);
   MouseQueueSlot *slot;
   for (;;) {
      slot = &queue[position & MOUSE_QUEUE_MASK];
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
                         position + MOUSE_QUEUE_CAPACITY,
                         memory_order_release);
   return 1;
}

static void
queue_detached(void)
{
   HorizonMouseEvent event = {0};
   event.kind = HORIZON_MOUSE_DETACHED;
   enqueue_event(event);
}

static void
read_report(uint32_t handle, int32_t error, uint8_t *buffer,
            uint32_t bytes_transferred, void *user_context)
{
   uint32_t generation = (uint32_t)(uintptr_t)user_context;
   if (!atomic_load_explicit(&callbacks_enabled, memory_order_acquire) ||
       !atomic_load_explicit(&device_active, memory_order_acquire) ||
       atomic_load_explicit(&device_handle, memory_order_relaxed) != handle ||
       atomic_load_explicit(&device_generation, memory_order_relaxed) != generation) {
      return;
   }

   if (error != 0) {
      atomic_store_explicit(&device_active, 0, memory_order_release);
      queue_detached();
      return;
   }

   if (buffer != NULL && bytes_transferred >= 3) {
      HorizonMouseEvent event = {0};
      event.kind = HORIZON_MOUSE_REPORT;
      event.buttons = buffer[0] & 0x07;
      event.x = (int8_t)buffer[1];
      event.y = (int8_t)buffer[2];
      event.wheel = bytes_transferred >= 4 ? (int8_t)buffer[3] : 0;
      enqueue_event(event);
   }

   if (atomic_load_explicit(&callbacks_enabled, memory_order_acquire) &&
       atomic_load_explicit(&device_active, memory_order_acquire) &&
       atomic_load_explicit(&device_generation, memory_order_relaxed) == generation) {
      int32_t result = HIDRead(handle, report_buffer, sizeof(report_buffer),
                               read_report, user_context);
      if (result != 0) {
         atomic_store_explicit(&device_active, 0, memory_order_release);
         queue_detached();
      }
   }
}

static void
protocol_set(uint32_t handle, int32_t error, uint8_t *buffer,
             uint32_t bytes_transferred, void *user_context)
{
   (void)buffer;
   (void)bytes_transferred;
   uint32_t generation = (uint32_t)(uintptr_t)user_context;
   if (!atomic_load_explicit(&callbacks_enabled, memory_order_acquire) ||
       !atomic_load_explicit(&device_active, memory_order_acquire) ||
       atomic_load_explicit(&device_handle, memory_order_relaxed) != handle ||
       atomic_load_explicit(&device_generation, memory_order_relaxed) != generation) {
      return;
   }
   if (error != 0) {
      atomic_store_explicit(&device_active, 0, memory_order_release);
      queue_detached();
      return;
   }

   int32_t result = HIDRead(handle, report_buffer, sizeof(report_buffer),
                            read_report, user_context);
   if (result != 0) {
      atomic_store_explicit(&device_active, 0, memory_order_release);
      queue_detached();
   }
}

static int32_t
device_attached(HIDClient *client, HIDDevice *device, HIDAttachEvent attach)
{
   (void)client;
   if (device == NULL ||
       !atomic_load_explicit(&callbacks_enabled, memory_order_acquire)) {
      return 0;
   }

   if (attach == HID_DEVICE_DETACH) {
      if (atomic_load_explicit(&device_active, memory_order_acquire) &&
          atomic_load_explicit(&device_handle, memory_order_relaxed) == device->handle) {
         atomic_store_explicit(&device_active, 0, memory_order_release);
         atomic_fetch_add_explicit(&device_generation, 1, memory_order_relaxed);
         queue_detached();
      }
      return 0;
   }

   /* HID boot subclass and mouse protocol; ignore report-protocol devices. */
   if (device->subClass != 1 || device->protocol != 2 ||
       atomic_load_explicit(&device_active, memory_order_acquire)) {
      return 0;
   }

   uint32_t generation = atomic_fetch_add_explicit(&device_generation, 1,
                                                    memory_order_relaxed) + 1u;
   atomic_store_explicit(&device_handle, device->handle, memory_order_relaxed);
   atomic_store_explicit(&device_active, 1, memory_order_release);
   int32_t result = HIDSetProtocol(device->handle, device->interfaceIndex, 0,
                                   protocol_set, (void *)(uintptr_t)generation);
   if (result != 0) {
      atomic_store_explicit(&device_active, 0, memory_order_release);
      queue_detached();
   }
   return 0;
}

int32_t
horizon_wiiu_mouse_init(void)
{
   initialize_queue();
   if (client_registered) {
      return 0;
   }
   atomic_store_explicit(&callbacks_enabled, 1, memory_order_release);
   int32_t result = HIDSetup();
   if (result != 0) {
      atomic_store_explicit(&callbacks_enabled, 0, memory_order_release);
      return result;
   }
   result = HIDAddClient(&hid_client, device_attached);
   if (result != 0) {
      atomic_store_explicit(&callbacks_enabled, 0, memory_order_release);
      HIDTeardown();
      return result;
   }
   client_registered = 1;
   return 0;
}

int32_t
horizon_wiiu_mouse_teardown(void)
{
   atomic_store_explicit(&callbacks_enabled, 0, memory_order_release);
   if (atomic_exchange_explicit(&device_active, 0, memory_order_acq_rel)) {
      atomic_fetch_add_explicit(&device_generation, 1, memory_order_relaxed);
      queue_detached();
   }
   int32_t result = 0;
   if (client_registered) {
      result = HIDDelClient(&hid_client);
      client_registered = 0;
      int32_t teardown_result = HIDTeardown();
      if (result == 0) {
         result = teardown_result;
      }
   }
   return result;
}

int32_t
horizon_wiiu_mouse_pop(HorizonMouseEvent *event)
{
   if (event == NULL || atomic_load_explicit(&queue_state, memory_order_acquire) != 2) {
      return 0;
   }
   return dequeue_event(event);
}

uint32_t
horizon_wiiu_mouse_dropped(void)
{
   return atomic_load_explicit(&dropped_events, memory_order_relaxed);
}
