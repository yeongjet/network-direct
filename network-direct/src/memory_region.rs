use std::marker::PhantomData;
use std::pin::Pin;

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

// pub struct UnregisteredMemoryRegion {
//     ptr: *mut IND2MemoryRegion,
//     vtbl: IND2MemoryRegionVtbl,
// }

// impl AsRef<IND2MemoryRegion> for UnregisteredMemoryRegion {
//     fn as_ref(&self) -> &IND2MemoryRegion {
//         unsafe { &*self.ptr }
//     }
// }

// impl AsMut<IND2MemoryRegion> for UnregisteredMemoryRegion {
//     fn as_mut(&mut self) -> &mut IND2MemoryRegion {
//         unsafe { &mut *self.ptr }
//     }
// }

// impl UnregisteredMemoryRegion {
//     pub fn from(ptr: *mut IND2MemoryRegion) -> Self {
//         Self {
//             ptr,
//             vtbl: unsafe { *((*ptr).lpVtbl) },
//         }
//     }

//     pub fn get_local_token(&self) -> LocalToken {
//         LocalToken(unsafe { self.vtbl.GetLocalToken.unwrap()(self.ptr) })
//     }

//     pub fn get_remote_token(&self) -> RemoteToken {
//         RemoteToken(unsafe { self.vtbl.GetRemoteToken.unwrap()(self.ptr) })
//     }

// }

// impl ND2Overlapped for UnregisteredMemoryRegion {
//     fn as_overlapped_mut(&self) -> &mut IND2Overlapped {
//         unsafe { &mut *(self.ptr as *mut IND2Overlapped) }
//     }
// }

// impl Drop for UnregisteredMemoryRegion {
//     fn drop(&mut self) {
//         unsafe {
//             let _n = self.vtbl.Release.unwrap()(self.ptr);
//         }
//     }
// }

// pub type Buffer = Pin<Box<[u8; 4096]>>;
// pub type Buffer<T, N> = Pin<Box<GenericArray<T, N>>>;

// pub struct MemoryRegion<T, U>
// where
//     T: AsRef<[U]>,
// {
//     ptr: *mut IND2MemoryRegion,
//     vtbl: IND2MemoryRegionVtbl,
//     pub buffer: T,
// }

pub struct MemoryRegion<T, P>
where
    T: AsRef<[P]>,
{
    ptr: *mut IND2MemoryRegion,
    vtbl: IND2MemoryRegionVtbl,
    pub buffer: Pin<Box<T>>,
    _marker: PhantomData<P>,
}
// unsafe impl<T> Send for MemoryRegion<T> where T: Send {}

// unsafe impl<T> Sync for MemoryRegion<T> {}

impl<T, P> AsRef<IND2MemoryRegion> for MemoryRegion<T, P>
where
    T: AsRef<[P]>,
{
    fn as_ref(&self) -> &IND2MemoryRegion {
        unsafe { &*self.ptr }
    }
}

impl<T, P> AsMut<IND2MemoryRegion> for MemoryRegion<T, P>
where
    T: AsRef<[P]>,
{
    fn as_mut(&mut self) -> &mut IND2MemoryRegion {
        unsafe { &mut *self.ptr }
    }
}

impl<T, P> MemoryRegion<T, P>
where
    T: AsRef<[P]>,
{
    pub fn from(ptr: *mut IND2MemoryRegion, buffer: Pin<Box<T>>) -> Self {
        Self {
            ptr,
            vtbl: unsafe { *((*ptr).lpVtbl) },
            buffer,
            _marker: PhantomData,
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
        let buffer = (*self.buffer).as_ref();
        unsafe {
            let res = self.vtbl.Register.unwrap()(
                self.ptr,
                buffer as *const _ as *const c_void,
                buffer.len() as u64,
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

impl<T, P> ND2Overlapped for MemoryRegion<T, P>
where
    T: AsRef<[P]>,
{
    fn as_overlapped_mut(&self) -> &mut IND2Overlapped {
        unsafe { &mut *(self.ptr as *mut IND2Overlapped) }
    }
}

impl<T, P> Drop for MemoryRegion<T, P>
where
    T: AsRef<[P]>,
{
    fn drop(&mut self) {
        unsafe {
            let _n = self.vtbl.Release.unwrap()(self.ptr);
        }
    }
}
