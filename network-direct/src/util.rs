use std::{
    mem,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    ptr,
};

use network_direct_sys::ND_BUFFER_OVERFLOW;
use windows::{
    Win32::{
        Foundation::{E_NOTIMPL, STATUS_BUFFER_TOO_SMALL},
        Networking::WinSock::{
            AF_INET, AF_INET6, INVALID_SOCKET, SIO_ROUTING_INTERFACE_QUERY, SOCK_STREAM, SOCKADDR,
            SOCKADDR_IN, SOCKADDR_IN6, SOCKADDR_INET, SOCKADDR_STORAGE, WSA_FLAG_OVERLAPPED,
            WSACleanup, WSADATA, WSAGetLastError, WSAIoctl, WSASocketW, WSAStartup,
        },
    },
    core::{HRESULT, Result},
};

pub(crate) type GetAddressFn<T> = unsafe extern "C" fn(*mut T, *mut SOCKADDR, *mut u32) -> HRESULT;

pub(crate) unsafe fn win_addr_to_std_fn<T>(this: *mut T, f: GetAddressFn<T>) -> Result<SocketAddr> {
    let mut size = 0;
    let res = unsafe { f(this, ptr::null_mut(), &mut size) };
    if res != ND_BUFFER_OVERFLOW && res.0 != STATUS_BUFFER_TOO_SMALL.0 {
        res.ok()?;
    }

    let mut data = vec![0u8; size as usize];
    unsafe { f(this, data.as_mut_ptr() as *mut _, &mut size).ok() }?;

    let addr: &SOCKADDR = unsafe { &*(data.as_ptr() as *const _) };
    win_addr_to_std(addr).ok_or_else(|| E_NOTIMPL.into())
}

pub(crate) fn win_addr_to_std(addr: &SOCKADDR) -> Option<SocketAddr> {
    unsafe {
        if addr.sa_family == AF_INET {
            let addr: &SOCKADDR_IN = mem::transmute(addr);
            let ip = Ipv4Addr::from(u32::from_be(addr.sin_addr.S_un.S_addr));
            let port = u16::from_be(addr.sin_port);
            Some(SocketAddr::V4(SocketAddrV4::new(ip, port)))
        } else if addr.sa_family == AF_INET6 {
            let addr: &SOCKADDR_IN6 = mem::transmute(addr);
            let ip = Ipv6Addr::from(addr.sin6_addr.u.Byte);
            let port = u16::from_be(addr.sin6_port);
            Some(SocketAddr::V6(SocketAddrV6::new(
                ip,
                port,
                addr.sin6_flowinfo,
                addr.Anonymous.sin6_scope_id,
            )))
        } else {
            None
        }
    }
}

pub(crate) fn std_addr_to_win(addr: SocketAddr) -> (SOCKADDR_INET, u32) {
    unsafe {
        let addr: SOCKADDR_INET = addr.into();
        let addr_len = if addr.si_family == AF_INET {
            mem::size_of::<SOCKADDR_IN>()
        } else if addr.si_family == AF_INET6 {
            mem::size_of::<SOCKADDR_IN6>()
        } else {
            0
        };
        (addr, addr_len as u32)
    }
}

pub fn get_local_addr(remote_addr: SocketAddr) -> SocketAddr {
    unsafe {
        let mut wsa_data = WSADATA::default();
        let ret = WSAStartup(0x0202, &mut wsa_data);
        if ret != 0 {
            panic!("WSAStartup failed: {}", ret);
        }
    }
    let (remote_addr, remote_addr_len) = std_addr_to_win(remote_addr);
    let mut local_addr = SOCKADDR_STORAGE::default();
    let local_addr_len = mem::size_of::<SOCKADDR_STORAGE>() as u32;
    let mut out_len = 0;
    let socket = unsafe {
        WSASocketW(
            AF_INET.0 as i32,
            SOCK_STREAM.0,
            0,
            None,
            0,
            WSA_FLAG_OVERLAPPED,
        )
        .unwrap()
    };
    if socket == INVALID_SOCKET {
        unsafe { WSACleanup() };
        panic!("Failed to create socket: {}", unsafe {
            WSAGetLastError().0
        });
    }
    let ret = unsafe {
        WSAIoctl(
            socket,
            SIO_ROUTING_INTERFACE_QUERY,
            Some(&remote_addr as *const _ as *const _),
            remote_addr_len,
            Some(&mut local_addr as *mut _ as *mut _),
            local_addr_len,
            &mut out_len,
            None,
            None,
        )
    };
    if ret < 0 {
        println!("Failed to get local address: {}", unsafe {
            WSAGetLastError().0
        });
    }

    win_addr_to_std(unsafe { mem::transmute(&local_addr) }).unwrap()
}
