#include <coreinit/debug.h>
#include <netinet/tcp.h>
#include <nsysnet/_socket.h>
#include <stdint.h>
#include <whb/proc.h>

/* Cafe socketlasterr values, not the WUT libc errno numbering. */
enum { CAFE_EINVAL = 11, CAFE_ENOTSOCK = 24, CAFE_EFAULT = 29 };

static int option(const char *name, int fd, const void *value, socklen_t length,
                  int expected_result, int expected_error)
{
    const int rc = RPLWRAP(setsockopt)(fd, IPPROTO_TCP, TCP_NODELAY, value, length);
    const int error = RPLWRAP(socketlasterr)();
    OSReport("TCP_NODELAY_PROBE %s rc=%d cafe_error=%d expected=%d/%d\n",
             name, rc, error, expected_result, expected_error);
    return rc != expected_result || error != expected_error;
}

int main(void)
{
    WHBProcInit();
    socket_lib_init();
    const int tcp = RPLWRAP(socket)(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    const int udp = RPLWRAP(socket)(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (tcp < 0 || udp < 0) {
        OSReport("TCP_NODELAY_PROBE result=FAIL sockets=%d/%d\n", tcp, udp);
        socket_lib_finish();
        WHBProcShutdown();
        return 1;
    }
    const int on = 1;
    static const int off = 0;
    int fail = option("tcp_enable", tcp, &on, sizeof(on), 0, 0);
    /* WinSock rejects TCP_NODELAY on a datagram with WSAEINVAL. */
    fail |= option("udp_host_rejection", udp, &on, sizeof(on), -1, CAFE_EINVAL);
    fail |= option("tcp_disable", tcp, &off, sizeof(off), 0, 0);
    fail |= option("invalid_descriptor", -1, &on, sizeof(on), -1, CAFE_ENOTSOCK);
    fail |= option("invalid_option_length", tcp, &on, 1, -1, CAFE_EINVAL);
    fail |= option("null_option", tcp, NULL, sizeof(on), -1, CAFE_EFAULT);
    fail |= option("tcp_reenable", tcp, &on, sizeof(on), 0, 0);

    RPLWRAP(socketclose)(tcp);
    RPLWRAP(socketclose)(udp);
    socket_lib_finish();
    OSReport("TCP_NODELAY_PROBE result=%s\n", fail ? "FAIL" : "PASS");
    WHBProcShutdown();
    return fail;
}
