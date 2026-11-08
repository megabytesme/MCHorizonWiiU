use cafe_sys::nn::ac;
use cafe_sys::nsys::net::socket as sys;

use self::netc::{sockaddr, socklen_t};
use super::{getsockopt, setsockopt, socket_addr_from_c, socket_addr_to_c};
use crate::ffi::{c_int, c_long};
use crate::io::{self, BorrowedBuf, BorrowedCursor, IoSlice, IoSliceMut};
use crate::net::{Shutdown, SocketAddr};
use crate::sync::{Arc, Once};
use crate::sys::pal::unsupported;
use crate::time::Duration;
use crate::{cmp, mem, ptr};

/// POSIX names for the shared socket code, backed by `cafe_sys::nsys::net::socket`.
#[expect(non_camel_case_types)]
pub(super) mod netc {
    use cafe_sys::nsys::net::socket as sys;

    use crate::ffi::{c_char, c_int, c_uint, c_void};

    pub type sa_family_t = u16;
    pub type socklen_t = sys::SockLen;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct sockaddr {
        pub sa_family: sa_family_t,
        pub sa_data: [c_char; 14],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct in_addr {
        pub s_addr: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct sockaddr_in {
        pub sin_family: sa_family_t,
        pub sin_port: u16,
        pub sin_addr: in_addr,
        pub sin_zero: [u8; 8],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct ip_mreq {
        pub imr_multiaddr: in_addr,
        pub imr_interface: in_addr,
    }

    #[repr(C)]
    pub struct addrinfo {
        pub ai_flags: c_int,
        pub ai_family: c_int,
        pub ai_socktype: c_int,
        pub ai_protocol: c_int,
        pub ai_addrlen: socklen_t,
        pub ai_canonname: *mut c_char,
        pub ai_addr: *mut sockaddr,
        pub ai_next: *mut addrinfo,
    }

    const _: () = {
        assert!(size_of::<sockaddr>() == size_of::<sys::SockAddr>());
        assert!(size_of::<sockaddr_in>() == size_of::<sys::SockAddrIn>());
        assert!(size_of::<addrinfo>() == size_of::<sys::AddrInfo>());
    };

    pub const AF_INET: c_int = sys::Family::INET.0;
    pub const SOCK_STREAM: c_int = sys::Type::STREAM.0;
    pub const SOCK_DGRAM: c_int = sys::Type::DATAGRAM.0;
    pub const SOL_SOCKET: c_int = sys::Level::Socket as c_int;
    pub const IPPROTO_IP: c_int = sys::Level::Ip as c_int;
    pub const IPPROTO_TCP: c_int = sys::Level::Tcp as c_int;

    pub const SO_REUSEADDR: c_int = sys::SockOpt::Reuseaddr as c_int;
    pub const SO_KEEPALIVE: c_int = sys::SockOpt::Keepalive as c_int;
    pub const SO_BROADCAST: c_int = sys::SockOpt::Broadcast as c_int;
    pub const SO_LINGER: c_int = sys::SockOpt::Linger as c_int;
    pub const SO_ERROR: c_int = sys::SockOpt::Lasterror as c_int;
    pub const SO_NONBLOCK: c_int = sys::SockOpt::Nonblock as c_int;
    pub const IP_TTL: c_int = sys::SockOpt::IP_TTL as c_int;
    pub const TCP_NODELAY: c_int = sys::SockOpt::TcpNodelay as c_int;

    pub const IP_MULTICAST_TTL: c_int = sys::SockOpt::IpMulticastTtl as c_int;
    pub const IP_MULTICAST_LOOP: c_int = sys::SockOpt::IpMulticastLoop as c_int;
    pub const IP_ADD_MEMBERSHIP: c_int = sys::SockOpt::IpAddMembership as c_int;
    pub const IP_DROP_MEMBERSHIP: c_int = sys::SockOpt::IpDropMembership as c_int;

    pub const EMSGSIZE: c_int = sys::Error::EMSGSIZE.0;

    // nsysnet only supports IPv4. The shared socket code still needs the IPv6
    // items, so they use the Linux values, and `Socket::new` rejects AF_INET6.
    pub const AF_INET6: c_int = 10;
    pub const IPPROTO_IPV6: c_int = 41;
    pub const IPV6_V6ONLY: c_int = 26;
    pub const IPV6_MULTICAST_LOOP: c_int = 19;
    pub const IPV6_ADD_MEMBERSHIP: c_int = 20;
    pub const IPV6_DROP_MEMBERSHIP: c_int = 21;

    // nsysnet has no timeout options; the shared code only passes these back to
    // `set_timeout`/`timeout`, which report them as unsupported.
    pub const SO_SNDTIMEO: c_int = 0x1005;
    pub const SO_RCVTIMEO: c_int = 0x1006;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct in6_addr {
        pub s6_addr: [u8; 16],
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct sockaddr_in6 {
        pub sin6_family: sa_family_t,
        pub sin6_port: u16,
        pub sin6_flowinfo: u32,
        pub sin6_addr: in6_addr,
        pub sin6_scope_id: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct ipv6_mreq {
        pub ipv6mr_multiaddr: in6_addr,
        pub ipv6mr_interface: c_uint,
    }

    /// nsysnet only returns IPv4 addresses and rejects any other length in
    /// `accept`, so this has exactly the size of `sockaddr_in`.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct sockaddr_storage {
        pub ss_family: sa_family_t,
        pub ss_data: [u8; 14],
    }

    /// Returned instead of `-1` for options that `sys::SockOpt` cannot express.
    /// `cvt` turns it into `ENOPROTOOPT`, since `socketlasterr` is not set.
    pub const UNSUPPORTED_OPTION: c_int = c_int::MIN;

    fn sockopt(level: c_int, name: c_int) -> Option<(sys::Level, sys::SockOpt)> {
        let level = match level {
            SOL_SOCKET => sys::Level::Socket,
            IPPROTO_IP => sys::Level::Ip,
            IPPROTO_TCP => sys::Level::Tcp,
            _ => return None,
        };
        let name = match name {
            // Also `IP_TTL`.
            SO_REUSEADDR => sys::SockOpt::Reuseaddr,
            SO_KEEPALIVE => sys::SockOpt::Keepalive,
            SO_BROADCAST => sys::SockOpt::Broadcast,
            SO_LINGER => sys::SockOpt::Linger,
            SO_ERROR => sys::SockOpt::Lasterror,
            SO_NONBLOCK => sys::SockOpt::Nonblock,
            TCP_NODELAY => sys::SockOpt::TcpNodelay,
            IP_MULTICAST_TTL => sys::SockOpt::IpMulticastTtl,
            IP_MULTICAST_LOOP => sys::SockOpt::IpMulticastLoop,
            IP_ADD_MEMBERSHIP => sys::SockOpt::IpAddMembership,
            IP_DROP_MEMBERSHIP => sys::SockOpt::IpDropMembership,
            _ => return None,
        };
        Some((level, name))
    }

    pub unsafe fn setsockopt(
        fd: c_int,
        level: c_int,
        name: c_int,
        value: *const c_void,
        len: socklen_t,
    ) -> c_int {
        match sockopt(level, name) {
            Some((level, name)) => unsafe { sys::set_sockopt(fd, level, name, value, len) },
            None => UNSUPPORTED_OPTION,
        }
    }

    pub unsafe fn getsockopt(
        fd: c_int,
        level: c_int,
        name: c_int,
        value: *mut c_void,
        len: *mut socklen_t,
    ) -> c_int {
        match sockopt(level, name) {
            Some((level, name)) => unsafe { sys::get_sockopt(fd, level, name, value, len) },
            None => UNSUPPORTED_OPTION,
        }
    }

    pub unsafe fn connect(fd: c_int, addr: *const sockaddr, len: socklen_t) -> c_int {
        unsafe { sys::connect(fd, addr.cast(), len) }
    }

    pub unsafe fn bind(fd: c_int, addr: *const sockaddr, len: socklen_t) -> c_int {
        unsafe { sys::bind(fd, addr.cast(), len) }
    }

    pub unsafe fn listen(fd: c_int, backlog: c_int) -> c_int {
        unsafe { sys::listen(fd, backlog) }
    }

    pub unsafe fn send(fd: c_int, buf: *const c_void, len: c_int, flags: c_int) -> c_int {
        unsafe { sys::send(fd, buf, len, sys::MsgFlags(flags)) }
    }

    pub unsafe fn sendto(
        fd: c_int,
        buf: *const c_void,
        len: c_int,
        flags: c_int,
        dest: *const sockaddr,
        dest_len: socklen_t,
    ) -> c_int {
        unsafe { sys::sendto(fd, buf, len, sys::MsgFlags(flags), dest.cast(), dest_len) }
    }

    pub unsafe fn getpeername(fd: c_int, addr: *mut sockaddr, len: *mut socklen_t) -> c_int {
        unsafe { sys::get_peername(fd, addr.cast(), len) }
    }

    pub unsafe fn getsockname(fd: c_int, addr: *mut sockaddr, len: *mut socklen_t) -> c_int {
        unsafe { sys::get_sockname(fd, addr.cast(), len) }
    }

    pub unsafe fn getaddrinfo(
        node: *const c_char,
        service: *const c_char,
        hints: *const addrinfo,
        res: *mut *mut addrinfo,
    ) -> c_int {
        unsafe { sys::get_addr_info(node, service, hints.cast(), res.cast()).0 }
    }

    pub unsafe fn freeaddrinfo(res: *mut addrinfo) {
        unsafe { sys::free_addr_info(res.cast()) }
    }
}

#[expect(non_camel_case_types)]
pub type wrlen_t = c_int;

pub fn cvt(ret: c_int) -> io::Result<c_int> {
    match ret {
        netc::UNSUPPORTED_OPTION => Err(io::Error::from_raw_os_error(sys::Error::ENOPROTOOPT.0)),
        ..0 => Err(last_error()),
        _ => Ok(ret),
    }
}

pub fn cvt_r(f: impl FnOnce() -> c_int) -> io::Result<c_int> {
    cvt(f())
}

pub fn cvt_gai(err: c_int) -> io::Result<()> {
    use io::ErrorKind::Uncategorized;
    Err(match sys::AiError(err) {
        sys::AiError::SUCCESS => return Ok(()),
        sys::AiError::AGAIN => {
            io::const_error!(Uncategorized, "temporary failure in name resolution")
        }
        sys::AiError::FAIL => {
            io::const_error!(Uncategorized, "non-recoverable failure in name resolution")
        }
        sys::AiError::MEMORY => io::const_error!(Uncategorized, "memory allocation failure"),
        sys::AiError::NONAME => io::const_error!(Uncategorized, "name or service not known"),
        _ => io::const_error!(Uncategorized, "failed to look up address information"),
    })
}

fn last_error() -> io::Error {
    io::Error::from_raw_os_error(unsafe { sys::last_error() }.0)
}

fn to_timeval(dur: Duration) -> sys::TimeVal {
    // SOSelect rejects components above 999999.
    let mut tv = sys::TimeVal {
        sec: dur.as_secs().min(999_999) as c_long,
        usec: dur.subsec_micros() as c_long,
    };
    if tv.sec == 0 && tv.usec == 0 {
        tv.usec = 1;
    }
    tv
}

pub fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| unsafe {
        sys::init();
        // Brings up the console's configured connection. If that fails, the
        // socket calls report the error.
        ac::init();
        ac::connect();
    });
}

pub struct Socket(Arc<SocketHandle>);

struct SocketHandle(c_int);

impl Socket {
    fn from_raw(fd: c_int) -> Socket {
        Socket(Arc::new(SocketHandle(fd)))
    }
}

impl Socket {
    pub fn new(family: c_int, ty: c_int) -> io::Result<Socket> {
        if family != netc::AF_INET {
            return Err(io::const_error!(io::ErrorKind::Unsupported, "only IPv4 is supported"));
        }
        cvt(unsafe { sys::socket(sys::Family(family), sys::Type(ty), sys::Protocol::IP) })
            .map(Socket::from_raw)
    }

    pub fn connect(&self, addr: &SocketAddr) -> io::Result<()> {
        let (addr, len) = socket_addr_to_c(addr);
        cvt(unsafe { netc::connect(self.as_raw(), addr.as_ptr(), len) }).map(drop)
    }

    pub fn connect_timeout(&self, addr: &SocketAddr, timeout: Duration) -> io::Result<()> {
        if timeout.is_zero() {
            return Err(io::Error::ZERO_TIMEOUT);
        }
        if !(0..u32::BITS as c_int).contains(&self.as_raw()) {
            return Err(io::const_error!(io::ErrorKind::Unsupported, "socket cannot be polled"));
        }

        self.set_nonblocking(true)?;
        let r = self.connect(addr);
        self.set_nonblocking(false)?;
        match r {
            Ok(()) => return Ok(()),
            Err(e) if e.raw_os_error() == Some(sys::Error::EINPROGRESS.0) => {}
            Err(e) => return Err(e),
        }

        let mut writefds = sys::FdSet { bits: 1 << self.as_raw() };
        let mut timeout = to_timeval(timeout);
        let n = cvt(unsafe {
            sys::select(self.as_raw() + 1, ptr::null_mut(), &mut writefds, ptr::null_mut(), &mut timeout)
        })?;
        if n == 0 {
            return Err(io::const_error!(io::ErrorKind::TimedOut, "connection timed out"));
        }
        match self.take_error()? {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    pub fn accept(&self, storage: *mut sockaddr, len: *mut socklen_t) -> io::Result<Socket> {
        cvt(unsafe { sys::accept(self.as_raw(), storage.cast(), len) }).map(Socket::from_raw)
    }

    pub fn duplicate(&self) -> io::Result<Socket> {
        Ok(Socket(Arc::clone(&self.0)))
    }

    fn recv_with_flags(&self, mut buf: BorrowedCursor<'_, u8>, flags: sys::MsgFlags) -> io::Result<()> {
        let len = cmp::min(buf.capacity(), c_int::MAX as usize) as c_int;
        let ret = cvt(unsafe { sys::recv(self.as_raw(), buf.as_mut().as_mut_ptr().cast(), len, flags) })?;
        unsafe { buf.advance(ret as usize) };
        Ok(())
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let mut buf = BorrowedBuf::from(buf);
        self.recv_with_flags(buf.unfilled(), sys::MsgFlags::NONE)?;
        Ok(buf.len())
    }

    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        let mut buf = BorrowedBuf::from(buf);
        self.recv_with_flags(buf.unfilled(), sys::MsgFlags::PEEK)?;
        Ok(buf.len())
    }

    pub fn read_buf(&self, buf: BorrowedCursor<'_, u8>) -> io::Result<()> {
        self.recv_with_flags(buf, sys::MsgFlags::NONE)
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        crate::io::default_read_vectored(|b| self.read(b), bufs)
    }

    #[inline]
    pub fn is_read_vectored(&self) -> bool {
        false
    }

    fn recv_from_with_flags(
        &self,
        buf: &mut [u8],
        flags: sys::MsgFlags,
    ) -> io::Result<(usize, SocketAddr)> {
        let mut storage: netc::sockaddr_storage = unsafe { mem::zeroed() };
        let mut addrlen = size_of_val(&storage) as socklen_t;
        let len = cmp::min(buf.len(), c_int::MAX as usize) as c_int;
        let n = cvt(unsafe {
            sys::recvfrom(
                self.as_raw(),
                buf.as_mut_ptr().cast(),
                len,
                flags,
                (&raw mut storage).cast(),
                &mut addrlen,
            )
        })?;
        Ok((n as usize, unsafe { socket_addr_from_c(&storage, addrlen as usize)? }))
    }

    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.recv_from_with_flags(buf, sys::MsgFlags::NONE)
    }

    pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.recv_from_with_flags(buf, sys::MsgFlags::PEEK)
    }

    fn send(&self, buf: &[u8]) -> io::Result<usize> {
        let len = cmp::min(buf.len(), c_int::MAX as usize) as c_int;
        cvt(unsafe { sys::send(self.as_raw(), buf.as_ptr().cast(), len, sys::MsgFlags::NONE) }).map(|n| n as usize)
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        crate::io::default_write_vectored(|b| self.send(b), bufs)
    }

    #[inline]
    pub fn is_write_vectored(&self) -> bool {
        false
    }

    pub fn set_timeout(&self, _: Option<Duration>, _: c_int) -> io::Result<()> {
        unsupported()
    }

    pub fn timeout(&self, _: c_int) -> io::Result<Option<Duration>> {
        unsupported()
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        let how = match how {
            Shutdown::Write => sys::Shutdown::Write,
            Shutdown::Read => sys::Shutdown::Read,
            Shutdown::Both => sys::Shutdown::ReadWrite,
        };
        cvt(unsafe { sys::shutdown(self.as_raw(), how) }).map(drop)
    }

    pub fn set_linger(&self, linger: Option<Duration>) -> io::Result<()> {
        let linger = sys::Linger {
            onoff: linger.is_some() as c_int,
            linger: linger.unwrap_or_default().as_secs().try_into().unwrap_or(c_int::MAX),
        };
        unsafe { setsockopt(self, netc::SOL_SOCKET, netc::SO_LINGER, linger) }
    }

    pub fn linger(&self) -> io::Result<Option<Duration>> {
        let val: sys::Linger = unsafe { getsockopt(self, netc::SOL_SOCKET, netc::SO_LINGER)? };
        Ok((val.onoff != 0).then(|| Duration::from_secs(val.linger as u64)))
    }

    pub fn set_keepalive(&self, keepalive: bool) -> io::Result<()> {
        unsafe { setsockopt(self, netc::SOL_SOCKET, netc::SO_KEEPALIVE, keepalive as c_int) }
    }

    pub fn keepalive(&self) -> io::Result<bool> {
        let raw: c_int = unsafe { getsockopt(self, netc::SOL_SOCKET, netc::SO_KEEPALIVE)? };
        Ok(raw != 0)
    }

    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        unsafe { setsockopt(self, netc::IPPROTO_TCP, netc::TCP_NODELAY, nodelay as c_int) }
    }

    pub fn nodelay(&self) -> io::Result<bool> {
        let raw: c_int = unsafe { getsockopt(self, netc::IPPROTO_TCP, netc::TCP_NODELAY)? };
        Ok(raw != 0)
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        unsafe { setsockopt(self, netc::SOL_SOCKET, netc::SO_NONBLOCK, nonblocking as c_int) }
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        let raw: c_int = unsafe { getsockopt(self, netc::SOL_SOCKET, netc::SO_ERROR)? };
        Ok((raw != 0).then(|| io::Error::from_raw_os_error(raw)))
    }

    pub fn as_raw(&self) -> c_int {
        self.0.0
    }
}

impl Drop for SocketHandle {
    fn drop(&mut self) {
        unsafe { sys::close(self.0) };
    }
}
