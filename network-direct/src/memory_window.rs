use network_direct_sys::*;

use crate::RemoteToken;

pub struct MemoryWindow {
    pub(crate) ptr: *mut IND2MemoryWindow,
    vtbl: IND2MemoryWindowVtbl,
}

impl AsRef<IND2MemoryWindow> for MemoryWindow {
    fn as_ref(&self) -> &IND2MemoryWindow {
        unsafe { &*self.ptr }
    }
}

impl AsMut<IND2MemoryWindow> for MemoryWindow {
    fn as_mut(&mut self) -> &mut IND2MemoryWindow {
        unsafe { &mut *self.ptr }
    }
}

impl MemoryWindow {
    pub unsafe fn from(ptr: *mut IND2MemoryWindow) -> MemoryWindow {
        Self {
            ptr,
            vtbl: unsafe { *((*ptr).lpVtbl) },
        }
    }

    pub fn remote_token(&self) -> RemoteToken {
        RemoteToken(unsafe { self.vtbl.GetRemoteToken.unwrap()(self.ptr) })
    }
}

unsafe impl Send for MemoryWindow {}

impl Clone for MemoryWindow {
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

impl Drop for MemoryWindow {
    fn drop(&mut self) {
        unsafe {
            let _n = self.vtbl.Release.unwrap()(self.ptr);
        }
    }
}
