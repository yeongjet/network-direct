use std::{
    net::{IpAddr, SocketAddr},
    ptr,
};
use network_direct_sys::{IID_IND2Adapter, IND2Adapter, IND2ConnectorVtbl, IND2Provider, IND2ProviderVtbl, ND_BUFFER_OVERFLOW};
use windows::{
    Win32::
        Networking::WinSock::SOCKET_ADDRESS_LIST
    ,
    core::Result,
};

use crate::{definitions::SocketAddressExt, util::{std_addr_to_win, win_addr_to_std}, Adapter};

// pub struct Provider {
//     pub inner: *mut IND2Provider,
//     pub guid: GUID,
//     pub dll_path: String,
//     pub hmodule: HMODULE,
//     // pub get_class_object: unsafe fn(GUID,GUID,&IClassFactory) -> isize,
//     pub get_class_object: DllGetClassObject,
//     pub can_unload_now: DllCanUnloadNow,
// }
#[derive(Debug)]
pub struct Provider {
    ptr: *mut IND2Provider,
    vtbl: IND2ProviderVtbl
}

impl Provider {
    pub fn from(ptr: *mut IND2Provider) -> Self {
        Self { ptr, vtbl: unsafe { *((*ptr).lpVtbl) } }
    }

    pub fn query_address_list(&self) -> Result<Vec<IpAddr>> {
        let mut size = 0;
        let ptr = ptr::null_mut();
        let result = unsafe {
            self.vtbl.QueryAddressList.unwrap()(self.ptr, ptr, &mut size)
        };
        if result != ND_BUFFER_OVERFLOW {
            result.ok()?;
        }
        let mut data = vec![0u8; size as usize];
        unsafe {
            self.vtbl.QueryAddressList.unwrap()(
                self.ptr,
                data.as_mut_ptr() as *mut SOCKET_ADDRESS_LIST,
                &mut size,
            )
            .ok()
        }?;
        let win_addr: &SOCKET_ADDRESS_LIST = unsafe { &*(data.as_ptr() as *const _) };
        let mut result = Vec::with_capacity(win_addr.iAddressCount as usize);
        for addr in win_addr.get_addresses() {
            if let Some(addr) = win_addr_to_std(unsafe { &*addr.lpSockaddr }) {
                result.push(addr.ip());
            }
        }
        Ok(result)
    }

	pub fn resolve_address(&self, addr: SocketAddr) -> Result<u64> {
        let (addr, addr_len) = std_addr_to_win(addr);
        let mut adapter_id = 0;
        unsafe { self.vtbl.ResolveAddress.unwrap()(self.ptr, &addr as *const _ as *const _, addr_len, &mut adapter_id).ok() }?;
        Ok(adapter_id)
	}

    pub fn open_adapter(&self, adapter_id: u64) -> Result<Adapter> {
        let mut adapter = ptr::null_mut();
        unsafe { self.vtbl.OpenAdapter.unwrap()(self.ptr, &IID_IND2Adapter, adapter_id, &mut adapter).ok() }?;
        Ok(Adapter::from(adapter as *mut IND2Adapter))
	}
}
