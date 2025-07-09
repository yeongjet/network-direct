use network_direct_sys::{
    IND2Connector, IND2ConnectorVtbl, IND2Overlapped, ND_BUFFER_OVERFLOW, ND_PENDING,
};
use std::{borrow::BorrowMut, net::SocketAddr, ptr};
use windows::{Win32::System::IO::OVERLAPPED, core::Result};

use crate::{
    ND2Overlapped, QueuePair,
    util::{std_addr_to_win, win_addr_to_std_fn},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReadLimits {
    pub inbound_read_limit: u32,
    pub outbound_read_limit: u32,
}

impl Default for ReadLimits {
    fn default() -> Self {
        Self {
            inbound_read_limit: 0,
            outbound_read_limit: 0,
        }
    }
}

pub struct Connector {
    pub(crate) ptr: *mut IND2Connector,
    vtbl: IND2ConnectorVtbl,
}

impl AsRef<IND2Connector> for Connector {
    fn as_ref(&self) -> &IND2Connector {
        unsafe { &*self.ptr }
    }
}

impl AsMut<IND2Connector> for Connector {
    fn as_mut(&mut self) -> &mut IND2Connector {
        unsafe { &mut *self.ptr }
    }
}

impl Connector {
    pub fn from(ptr: *mut IND2Connector) -> Self {
        Self {
            ptr,
            vtbl: unsafe { *((*ptr).lpVtbl) },
        }
    }

    pub fn bind(&self, address: impl Into<SocketAddr>) -> Result<()> {
        let (addr, addr_len) = std_addr_to_win(address.into());

        unsafe { self.vtbl.Bind.unwrap()(self.ptr, &addr as *const _ as *const _, addr_len).ok() }
    }

    pub fn connect(
        &self,
        queue_pair: &QueuePair,
        dest_address: impl Into<SocketAddr>,
        limits: ReadLimits,
        private_data: Option<&[u8]>,
        overlapped: *mut OVERLAPPED,
    ) -> Result<()> {
        let (addr, addr_len) = std_addr_to_win(dest_address.into());
        unsafe {
            let (data_ptr, data_len) = private_data
                .map(|s| (s.as_ptr(), s.len()))
                .unwrap_or_else(|| (ptr::null(), 0));

            let res = self.vtbl.Connect.unwrap()(
                self.ptr,
                queue_pair.ptr as *mut _,
                &addr as *const _ as *const _,
                addr_len,
                limits.inbound_read_limit,
                limits.outbound_read_limit,
                data_ptr as *const _,
                data_len as u32,
                overlapped,
            );

            if res == ND_PENDING {
                self.get_overlapped_result(overlapped, true)
            } else {
                res.ok()
            }
        }
    }

    pub fn complete_connect(&self, overlapped: *mut OVERLAPPED) -> Result<()> {
        unsafe {
            let res = self.vtbl.CompleteConnect.unwrap()(self.ptr, overlapped);
            if res == ND_PENDING {
                self.get_overlapped_result(overlapped, true)
            } else {
                res.ok()
            }
        }
    }

    pub fn accept(
        &self,
        queue_pair: &QueuePair,
        limits: ReadLimits,
        private_data: Option<&[u8]>,
        ov_ptr: *mut OVERLAPPED,
    ) -> Result<()> {
        unsafe {
            let (data_ptr, data_len) = private_data
                .map(|s| (s.as_ptr(), s.len()))
                .unwrap_or_else(|| (ptr::null(), 0));
            println!(
                "{:?},{:?},{},{},{:?},{},{:p}",
                self.ptr,
                queue_pair.ptr as *mut _,
                limits.inbound_read_limit,
                limits.outbound_read_limit,
                data_ptr as *const _,
                data_len as u32,
                ov_ptr
            );
            let res = self.vtbl.Accept.unwrap()(
                self.ptr,
                queue_pair.ptr as *mut _,
                limits.inbound_read_limit,
                limits.outbound_read_limit,
                data_ptr as *const _,
                data_len as u32,
                ov_ptr,
            );
            if res == ND_PENDING {
                self.get_overlapped_result(ov_ptr, false)
            } else {
                res.ok()
            }
        }
    }

    pub fn reject(&self, private_data: Option<&[u8]>) -> Result<()> {
        unsafe {
            let (data_ptr, data_len) = private_data
                .map(|s| (s.as_ptr(), s.len()))
                .unwrap_or_else(|| (ptr::null(), 0));

            self.vtbl.Reject.unwrap()(self.ptr, data_ptr as *const _, data_len as u32).ok()
        }
    }

    pub fn read_limits(&self) -> Result<ReadLimits> {
        unsafe {
            let mut limits = ReadLimits {
                inbound_read_limit: 0,
                outbound_read_limit: 0,
            };
            self.vtbl.GetReadLimits.unwrap()(
                self.ptr,
                &mut limits.inbound_read_limit,
                &mut limits.outbound_read_limit,
            )
            .ok()?;
            Ok(limits)
        }
    }

    pub fn private_data(&self) -> Result<Vec<u8>> {
        unsafe {
            let mut size = 0;
            let res = self.vtbl.GetPrivateData.unwrap()(self.ptr, ptr::null_mut(), &mut size);
            if res != ND_BUFFER_OVERFLOW {
                res.ok()?;
            }

            let mut data = vec![0u8; size as usize];

            if size > 0 {
                self.vtbl.GetPrivateData.unwrap()(self.ptr, data.as_mut_ptr() as *mut _, &mut size)
                    .ok()?;
            }

            Ok(data)
        }
    }

    pub fn local_address(&self) -> Result<SocketAddr> {
        unsafe { win_addr_to_std_fn(self.ptr, self.vtbl.GetLocalAddress.unwrap()) }
    }

    pub fn peer_address(&self) -> Result<SocketAddr> {
        unsafe { win_addr_to_std_fn(self.ptr, self.vtbl.GetPeerAddress.unwrap()) }
    }

    pub fn notify_disconnect(&self, mut overlapped: *mut OVERLAPPED) -> Result<()> {
        unsafe {
            let res = self.vtbl.NotifyDisconnect.unwrap()(self.ptr, overlapped);
            if res == ND_PENDING {
                self.get_overlapped_result(overlapped, false)
            } else {
                res.ok()
            }
        }
    }

    pub fn disconnect(&self, overlapped: *mut OVERLAPPED) -> Result<()> {
        unsafe {
            let res = self.vtbl.Disconnect.unwrap()(self.ptr, overlapped);
            if res == ND_PENDING {
                self.get_overlapped_result(overlapped, true)
            } else {
                res.ok()
            }
        }
    }
}

impl ND2Overlapped for Connector {
    fn as_overlapped_mut(&self) -> &mut IND2Overlapped {
        unsafe { &mut *(self.ptr as *mut IND2Overlapped) }
    }
}

impl Clone for Connector {
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

impl Drop for Connector {
    fn drop(&mut self) {
        unsafe {
            let _n = self.vtbl.Release.unwrap()(self.ptr);
        }
    }
}
