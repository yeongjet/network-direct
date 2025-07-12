use generic_array::ArrayLength;
use network_direct_sys::{
    IID_IND2CompletionQueue, IID_IND2Connector, IID_IND2Listener, IID_IND2MemoryRegion,
    IID_IND2MemoryWindow, IID_IND2QueuePair, IND2Adapter, IND2AdapterVtbl, IND2CompletionQueue,
    IND2Connector, IND2Listener, IND2MemoryRegion, IND2MemoryWindow, IND2QueuePair, KAFFINITY,
    ND_VERSION_2, ND2_ADAPTER_INFO,
};
use std::{
    fs::File,
    mem,
    os::windows::io::{AsRawHandle, FromRawHandle},
    ptr,
};
use windows::{Win32::Foundation::HANDLE, core::Result};

use crate::{Buffer, CompletionQueue, Connector, Listener, MemoryRegion, MemoryWindow, QueuePair};

pub struct Adapter {
    ptr: *mut IND2Adapter,
    vtbl: IND2AdapterVtbl,
}

impl Adapter {
    pub fn from(ptr: *mut IND2Adapter) -> Self {
        Self {
            ptr,
            vtbl: unsafe { *((*ptr).lpVtbl) },
        }
    }

    pub fn query(&self) -> Result<ND2_ADAPTER_INFO> {
        let mut info = ND2_ADAPTER_INFO {
            InfoVersion: ND_VERSION_2,
            ..Default::default()
        };
        let mut info_size = mem::size_of::<ND2_ADAPTER_INFO>() as u32;
        unsafe { self.vtbl.Query.unwrap()(self.ptr, &mut info, &mut info_size).ok() }?;
        Ok(info)
    }

    pub fn create_adapter_file(&self) -> Result<File> {
        let mut file = HANDLE::default();
        unsafe {
            self.vtbl.CreateOverlappedFile.unwrap()(self.ptr, &mut file).ok()?;
            Ok(File::from_raw_handle(file.0))
        }
    }

    pub fn create_memory_region<T, N: ArrayLength>(
        &self,
        file: &File,
        buffer: Buffer<T, N>,
    ) -> Result<MemoryRegion<T, N>> {
        let mut memory_region = ptr::null_mut();
        unsafe {
            self.vtbl.CreateMemoryRegion.unwrap()(
                self.ptr,
                &IID_IND2MemoryRegion,
                HANDLE(file.as_raw_handle()),
                &mut memory_region,
            )
            .ok()
        }?;
        Ok(MemoryRegion::from(
            memory_region as *mut IND2MemoryRegion,
            buffer,
        ))
    }

    pub fn create_memory_window(&self) -> Result<MemoryWindow> {
        let mut memory_window = ptr::null_mut();
        unsafe {
            self.vtbl.CreateMemoryWindow.unwrap()(
                self.ptr,
                &IID_IND2MemoryWindow,
                &mut memory_window,
            )
            .ok()
        }?;
        Ok(unsafe { MemoryWindow::from(memory_window as *mut IND2MemoryWindow) })
    }

    pub fn create_completion_queue(
        &self,
        adapter_file: &impl AsRawHandle,
        queue_depth: u32,
        group: u16,
        affinity: KAFFINITY,
    ) -> Result<CompletionQueue> {
        let fd = HANDLE(adapter_file.as_raw_handle() as _);
        let mut cq = ptr::null_mut();
        unsafe {
            self.vtbl.CreateCompletionQueue.unwrap()(
                self.ptr,
                &IID_IND2CompletionQueue,
                fd,
                queue_depth,
                group,
                affinity,
                &mut cq,
            )
            .ok()
        }?;
        Ok(CompletionQueue::from(cq as *mut IND2CompletionQueue))
    }

    pub fn create_listener(&self, adapter_file: &impl AsRawHandle) -> Result<Listener> {
        let fd = HANDLE(adapter_file.as_raw_handle() as _);
        let mut listener = ptr::null_mut();
        unsafe {
            self.vtbl.CreateListener.unwrap()(self.ptr, &IID_IND2Listener, fd, &mut listener).ok()
        }?;
        Ok(Listener::from(listener as *mut IND2Listener))
    }

    pub fn create_connector(&self, adapter_file: &impl AsRawHandle) -> Result<Connector> {
        unsafe {
            let fd = HANDLE(adapter_file.as_raw_handle() as _);
            let mut conn = ptr::null_mut();
            self.vtbl.CreateConnector.unwrap()(self.ptr, &IID_IND2Connector, fd, &mut conn).ok()?;
            Ok(Connector::from(conn as *mut IND2Connector))
        }
    }

    pub fn create_queue_pair(
        &self,
        receive_completion_queue: &impl AsRef<IND2CompletionQueue>,
        initiator_completion_queue: &impl AsRef<IND2CompletionQueue>,
        receive_queue_depth: u32,
        initiator_queue_depth: u32,
        max_receive_request_sge: u32,
        max_initiator_request_sge: u32,
        inline_data_size: u32,
    ) -> Result<QueuePair> {
        unsafe {
            let mut qp = ptr::null_mut();
            self.vtbl.CreateQueuePair.unwrap()(
                self.ptr,
                &IID_IND2QueuePair,
                receive_completion_queue.as_ref() as *const _ as *mut _,
                initiator_completion_queue.as_ref() as *const _ as *mut _,
                ptr::null_mut(),
                receive_queue_depth,
                initiator_queue_depth,
                max_receive_request_sge,
                max_initiator_request_sge,
                inline_data_size,
                &mut qp,
            )
            .ok()?;
            Ok(QueuePair::from(qp as *mut IND2QueuePair))
        }
    }
}

impl Clone for Adapter {
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

impl Drop for Adapter {
    fn drop(&mut self) {
        unsafe {
            let _n = self.vtbl.Release.unwrap()(self.ptr);
        }
    }
}
