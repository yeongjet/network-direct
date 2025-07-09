use std::{borrow::BorrowMut, mem};

use bitflags::bitflags;

use network_direct_sys::{
    IND2MemoryRegion, IND2MemoryRegionVtbl, IND2Overlapped, ND_MR_FLAG_ALLOW_LOCAL_WRITE,
    ND_MR_FLAG_ALLOW_REMOTE_READ, ND_MR_FLAG_ALLOW_REMOTE_WRITE, ND_MR_FLAG_DO_NOT_SECURE_VM,
    ND_MR_FLAG_RDMA_READ_SINK, ND_PENDING,
};
use windows::Win32::System::IO::OVERLAPPED;
use windows::core::Result;

use crate::ND2Overlapped;

pub struct RemoteToken(pub u32);

pub struct LocalToken(pub u32);

bitflags! {
    pub struct RegisterFlags: u32 {
        const ALLOW_LOCAL_WRITE = ND_MR_FLAG_ALLOW_LOCAL_WRITE;
        const ALLOW_REMOTE_READ = ND_MR_FLAG_ALLOW_REMOTE_READ;
        const ALLOW_REMOTE_WRITE = ND_MR_FLAG_ALLOW_REMOTE_WRITE;
        const RDMA_READ_SINK = ND_MR_FLAG_RDMA_READ_SINK;
        const DO_NOT_SECURE_VM = ND_MR_FLAG_DO_NOT_SECURE_VM;
    }
}

pub struct UnregisteredMemoryRegion {
    ptr: *mut IND2MemoryRegion,
    vtbl: IND2MemoryRegionVtbl,
}

impl AsRef<IND2MemoryRegion> for UnregisteredMemoryRegion {
    fn as_ref(&self) -> &IND2MemoryRegion {
        unsafe { &*self.ptr }
    }
}

impl AsMut<IND2MemoryRegion> for UnregisteredMemoryRegion {
    fn as_mut(&mut self) -> &mut IND2MemoryRegion {
        unsafe { &mut *self.ptr }
    }
}

impl UnregisteredMemoryRegion {
    pub fn from(ptr: *mut IND2MemoryRegion) -> Self {
        Self {
            ptr,
            vtbl: unsafe { *((*ptr).lpVtbl) },
        }
    }

    pub fn get_local_token(&self) -> LocalToken {
        LocalToken(unsafe { self.vtbl.GetLocalToken.unwrap()(self.ptr) })
    }

    pub fn get_remote_token(&self) -> RemoteToken {
        RemoteToken(unsafe { self.vtbl.GetRemoteToken.unwrap()(self.ptr) })
    }

    pub fn register<T, U>(
        &self,
        buffer: T,
        flags: RegisterFlags,
        overlapped: *mut OVERLAPPED,
    ) -> Result<MemoryRegion<T>>
    where
        T: AsRef<[U]>,
    {
        let slice = buffer.as_ref();
        unsafe {
            let res = self.vtbl.Register.unwrap()(
                self.ptr,
                slice.as_ptr() as _,
                slice.len() as u64,
                flags.bits(),
                overlapped,
            );
            if res == ND_PENDING {
                self.get_overlapped_result(overlapped, true)?;
            } else {
                res.ok()?;
            }
        }
        let reg_mem_region = MemoryRegion::from(self.ptr, Some(buffer));
        mem::forget(self);
        Ok(reg_mem_region)
    }
}

impl ND2Overlapped for UnregisteredMemoryRegion {
    fn as_overlapped_mut(&self) -> &mut IND2Overlapped {
        unsafe { &mut *(self.ptr as *mut IND2Overlapped) }
    }
}

impl Drop for UnregisteredMemoryRegion {
    fn drop(&mut self) {
        unsafe {
            let _n = self.vtbl.Release.unwrap()(self.ptr);
        }
    }
}

pub struct MemoryRegion<T> {
    ptr: *mut IND2MemoryRegion,
    vtbl: IND2MemoryRegionVtbl,
    pub buffer: Option<T>,
}
unsafe impl<T> Send for MemoryRegion<T> where T: Send {}

// unsafe impl<T> Sync for MemoryRegion<T> {}

impl<T> AsRef<IND2MemoryRegion> for MemoryRegion<T> {
    fn as_ref(&self) -> &IND2MemoryRegion {
        unsafe { &*self.ptr }
    }
}

impl<T> AsMut<IND2MemoryRegion> for MemoryRegion<T> {
    fn as_mut(&mut self) -> &mut IND2MemoryRegion {
        unsafe { &mut *self.ptr }
    }
}

impl<T> MemoryRegion<T> {
    pub fn from(ptr: *mut IND2MemoryRegion, buffer: Option<T>) -> Self {
        Self {
            ptr,
            vtbl: unsafe { *((*ptr).lpVtbl) },
            buffer,
        }
    }

    pub fn get_local_token(&self) -> LocalToken {
        LocalToken(unsafe { self.vtbl.GetLocalToken.unwrap()(self.ptr) })
    }

    pub fn get_remote_token(&self) -> RemoteToken {
        RemoteToken(unsafe { self.vtbl.GetRemoteToken.unwrap()(self.ptr) })
    }

    pub fn buffer(&self) -> &T {
        self.buffer.as_ref().unwrap()
    }

    pub fn buffer_mut(&mut self) -> &mut T {
        self.buffer.as_mut().unwrap()
    }

    pub fn deregister(
        mut self,
        overlapped: *mut OVERLAPPED,
    ) -> Result<(UnregisteredMemoryRegion, T)> {
        let res = unsafe { self.vtbl.Deregister.unwrap()(self.ptr, overlapped) };
        if res == ND_PENDING {
            self.get_overlapped_result(overlapped, true)?;
        } else {
            res.ok()?;
        }
        let parts = (
            UnregisteredMemoryRegion::from(self.ptr),
            self.buffer.take().unwrap(),
        );
        mem::forget(self);
        Ok(parts)
    }
}

impl<T> ND2Overlapped for MemoryRegion<T> {
    fn as_overlapped_mut(&self) -> &mut IND2Overlapped {
        unsafe { &mut *(self.ptr as *mut IND2Overlapped) }
    }
}

impl<T> Drop for MemoryRegion<T> {
    fn drop(&mut self) {
        unsafe {
            let _n = self.vtbl.Release.unwrap()(self.ptr);
        }
    }
}
