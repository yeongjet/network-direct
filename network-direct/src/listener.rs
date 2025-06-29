use std::{borrow::BorrowMut, net::SocketAddr};

use network_direct_sys::{IND2Listener, IND2ListenerVtbl, IND2Overlapped, ND_PENDING};
use windows::{Win32::System::IO::OVERLAPPED, core::Result};

use crate::{
    Connector, ND2Overlapped,
    util::{std_addr_to_win, win_addr_to_std_fn},
};

pub struct Listener {
    ptr: *mut IND2Listener,
    vtbl: IND2ListenerVtbl,
}

impl AsRef<IND2Listener> for Listener {
    fn as_ref(&self) -> &IND2Listener {
        unsafe { &*self.ptr }
    }
}

impl AsMut<IND2Listener> for Listener {
    fn as_mut(&mut self) -> &mut IND2Listener {
        unsafe { &mut *self.ptr }
    }
}

impl Listener {
    pub fn from(ptr: *mut IND2Listener) -> Self {
        Self {
            ptr,
            vtbl: unsafe { *((*ptr).lpVtbl) },
        }
    }

    pub fn bind(&self, address: impl Into<SocketAddr>) -> Result<()> {
        let (addr, addr_len) = std_addr_to_win(address.into());

        unsafe { self.vtbl.Bind.unwrap()(self.ptr, &addr as *const _ as *const _, addr_len).ok() }
    }

    pub fn listen(&self, backlog: u32) -> Result<()> {
        unsafe { self.vtbl.Listen.unwrap()(self.ptr, backlog).ok() }
    }

    pub fn get_local_address(&self) -> Result<SocketAddr> {
        unsafe { win_addr_to_std_fn(self.ptr, self.vtbl.GetLocalAddress.unwrap()) }
    }

    pub fn get_connection_request(
        &self,
        connector: &mut Connector,
        mut overlapped: impl BorrowMut<OVERLAPPED>,
    ) -> Result<()> {
        unsafe {
            let res = self.vtbl.GetConnectionRequest.unwrap()(
                self.ptr,
                connector.ptr as *mut _,
                overlapped.borrow_mut(),
            );

            if res == ND_PENDING {
                self.get_overlapped_result(overlapped, true)
            } else {
                res.ok()
            }
        }
    }
}

impl ND2Overlapped for Listener {
    fn as_overlapped_mut(&self) -> &mut IND2Overlapped {
        unsafe { &mut *(self.ptr as *mut IND2Overlapped) }
    }
}

unsafe impl Send for Listener {}

impl Clone for Listener {
    fn clone(&self) -> Self {
        unsafe {
            let _n = self.vtbl.AddRef.unwrap()(self.ptr);
        }
        Self {
            ptr: self.ptr,
            vtbl: self.vtbl,
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        unsafe {
            let _n = self.vtbl.Release.unwrap()(self.ptr);
        }
    }
}
