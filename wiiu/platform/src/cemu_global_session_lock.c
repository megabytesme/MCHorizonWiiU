/*
 * Scratch Cemu diagnostic only. This process-global TCP endpoint serializes
 * all worlds; it is not a per-world lock and is not verified on Wii U hardware.
 * The source is linked only when HORIZON_WIIU_CEMU_GLOBAL_SESSION_LOCK=ON.
 */
#include <arpa/inet.h>
#include <coreinit/thread.h>
#include <errno.h>
#include <netinet/in.h>
#include <nsysnet/_socket.h>
#include <stdatomic.h>
#include <stdint.h>
#include <sys/socket.h>
#include <unistd.h>

#define HORIZON_CEMU_GLOBAL_SESSION_LOCK_PORT 48263u

/* socket_lib_init has no result value. Initialize it once and leave it alive
 * for the process; closing one SessionLock must not tear down other sockets. */
static atomic_int s_socket_library_state;

static void ensure_socket_library(void)
{
   int state = atomic_load_explicit(&s_socket_library_state, memory_order_acquire);
   if (state == 2) {
      return;
   }

   int uninitialized = 0;
   if (atomic_compare_exchange_strong_explicit(&s_socket_library_state,
                                               &uninitialized, 1,
                                               memory_order_acq_rel,
                                               memory_order_acquire)) {
      socket_lib_init();
      atomic_store_explicit(&s_socket_library_state, 2, memory_order_release);
      return;
   }

   while (atomic_load_explicit(&s_socket_library_state, memory_order_acquire) != 2) {
      OSSleepTicks(1);
   }
}

static int32_t close_with_error(int fd)
{
   int error = errno;
   (void)__rplwrap_socketclose(fd);
   return -(error > 0 ? error : EIO);
}

int32_t horizon_wiiu_cemu_global_lock_open(void)
{
   ensure_socket_library();

   const int fd = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
   if (fd < 0) {
      return -(errno > 0 ? errno : EIO);
   }

   /* Reuse must stay disabled so another process cannot share this endpoint. */
   const int reuse_address = 0;
   if (setsockopt(fd, SOL_SOCKET, SO_REUSEADDR,
                  &reuse_address, sizeof(reuse_address)) != 0) {
      return close_with_error(fd);
   }

   struct sockaddr_in address = {0};
   address.sin_family = AF_INET;
   address.sin_port = htons((uint16_t)HORIZON_CEMU_GLOBAL_SESSION_LOCK_PORT);
   address.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
   if (bind(fd, (struct sockaddr *)&address, sizeof(address)) != 0) {
      return close_with_error(fd);
   }
   if (listen(fd, 1) != 0) {
      return close_with_error(fd);
   }

   return fd;
}

void horizon_wiiu_cemu_global_lock_close(int32_t fd)
{
   if (fd >= 0) {
      (void)__rplwrap_socketclose(fd);
   }
}
