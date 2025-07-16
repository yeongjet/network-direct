use crate::ND2Overlapped;
use bitflags::bitflags;
use network_direct_sys::{
    IND2MemoryRegion, IND2MemoryRegionVtbl, IND2Overlapped, ND_MR_FLAG_ALLOW_LOCAL_WRITE,
    ND_MR_FLAG_ALLOW_REMOTE_READ, ND_MR_FLAG_ALLOW_REMOTE_WRITE, ND_MR_FLAG_DO_NOT_SECURE_VM,
    ND_MR_FLAG_RDMA_READ_SINK, ND_PENDING,
};
use windows::Win32::System::IO::OVERLAPPED;
use windows::core::Result;

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

pub trait Buffer {
    fn as_ptr(&self) -> *const u8;
    fn byte_len(&self) -> usize;
}

pub struct MemoryRegion<T: Buffer> {
    ptr: *mut IND2MemoryRegion,
    vtbl: IND2MemoryRegionVtbl,
    pub buffer: T,
}

unsafe impl<T: Buffer> Send for MemoryRegion<T> where T: Send {}

unsafe impl<T: Buffer> Sync for MemoryRegion<T> {}

impl<T: Buffer> MemoryRegion<T> {
    fn as_ref(&self) -> &IND2MemoryRegion {
        unsafe { &*self.ptr }
    }
}

impl<T: Buffer> AsMut<IND2MemoryRegion> for MemoryRegion<T> {
    fn as_mut(&mut self) -> &mut IND2MemoryRegion {
        unsafe { &mut *self.ptr }
    }
}

impl<T: Buffer> MemoryRegion<T> {
    pub fn from(ptr: *mut IND2MemoryRegion, buffer: T) -> Self {
        Self {
            ptr,
            vtbl: unsafe { *((*ptr).lpVtbl) },
            buffer,
        }
    }

    pub fn get_local_token(&self) -> u32 {
        unsafe { self.vtbl.GetLocalToken.unwrap()(self.ptr) }
    }

    pub fn get_remote_token(&self) -> u32 {
        unsafe { self.vtbl.GetRemoteToken.unwrap()(self.ptr) }
    }

    // pub fn buffer_ref(&self) -> Pin<&[u8; 4096]> {
    //     self.buffer.as_ref()
    // }

    // pub fn buffer_mut(&mut self) -> Pin<&mut [u8; 4096]> {
    //     self.buffer.as_mut()
    // }

    pub fn register(&self, flags: RegisterFlags, overlapped: *mut OVERLAPPED) -> Result<()> {
        use std::ffi::c_void;
        unsafe {
            let res = self.vtbl.Register.unwrap()(
                self.ptr,
                self.buffer.as_ptr() as *const c_void,
                self.buffer.byte_len() as u64,
                flags.bits(),
                overlapped,
            );
            if res == ND_PENDING {
                self.get_overlapped_result(overlapped, true)
            } else {
                res.ok()
            }
        }
        // let reg_mem_region = MemoryRegion::from(self.ptr, Some(buffer));

        // Ok(reg_mem_region)
    }

    pub fn deregister(self, overlapped: *mut OVERLAPPED) -> Result<()> {
        let res = unsafe { self.vtbl.Deregister.unwrap()(self.ptr, overlapped) };
        if res == ND_PENDING {
            self.get_overlapped_result(overlapped, true)
        } else {
            res.ok()
        }
        // let parts = (
        //     UnregisteredMemoryRegion::from(self.ptr),
        //     self.buffer.take().unwrap(),
        // );
        // mem::forget(self);
        // Ok(parts)
    }
}

impl<T: Buffer> ND2Overlapped for MemoryRegion<T> {
    fn as_overlapped_mut(&self) -> &mut IND2Overlapped {
        unsafe { &mut *(self.ptr as *mut IND2Overlapped) }
    }
}

impl<T: Buffer> Drop for MemoryRegion<T> {
    fn drop(&mut self) {
        unsafe {
            let _n = self.vtbl.Release.unwrap()(self.ptr);
        }
    }
}
