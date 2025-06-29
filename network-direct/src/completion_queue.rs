use std::borrow::BorrowMut;

use network_direct_sys::{
    IND2CompletionQueue, IND2CompletionQueueVtbl, IND2Overlapped, KAFFINITY, ND_CQ_NOTIFY_ANY,
    ND_CQ_NOTIFY_ERRORS, ND_CQ_NOTIFY_SOLICITED, ND_PENDING, ND2_RESULT,
};
use windows::{Win32::System::IO::OVERLAPPED, core::Result};

use crate::ND2Overlapped;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum NotifyType {
    Errors,
    Any,
    Solicited,
}

impl NotifyType {
    fn to_u32(self) -> u32 {
        match self {
            NotifyType::Errors => ND_CQ_NOTIFY_ERRORS,
            NotifyType::Any => ND_CQ_NOTIFY_ANY,
            NotifyType::Solicited => ND_CQ_NOTIFY_SOLICITED,
        }
    }
}

pub struct CompletionQueue {
    ptr: *mut IND2CompletionQueue,
    vtbl: IND2CompletionQueueVtbl,
}

impl AsRef<IND2CompletionQueue> for CompletionQueue {
    fn as_ref(&self) -> &IND2CompletionQueue {
        unsafe { &*self.ptr }
    }
}

impl AsMut<IND2CompletionQueue> for CompletionQueue {
    fn as_mut(&mut self) -> &mut IND2CompletionQueue {
        unsafe { &mut *self.ptr }
    }
}

impl CompletionQueue {
    pub fn from(ptr: *mut IND2CompletionQueue) -> Self {
        Self {
            ptr,
            vtbl: unsafe { *((*ptr).lpVtbl) },
        }
    }

    pub fn notify_affinity(&self) -> Result<(u16, KAFFINITY)> {
        unsafe {
            let mut group = 0;
            let mut affinity = 0;
            self.vtbl.GetNotifyAffinity.unwrap()(self.ptr, &mut group, &mut affinity).ok()?;
            Ok((group, affinity))
        }
    }

    pub fn resize(&self, queue_depth: u32) -> Result<()> {
        unsafe { self.vtbl.Resize.unwrap()(self.ptr, queue_depth).ok() }
    }

    pub fn notify(
        &self,
        type_: NotifyType,
        mut overlapped: impl BorrowMut<OVERLAPPED>,
    ) -> Result<()> {
        unsafe {
            let res = self.vtbl.Notify.unwrap()(self.ptr, type_.to_u32(), overlapped.borrow_mut());

            if res == ND_PENDING {
                self.get_overlapped_result(overlapped.borrow_mut(), false)
            } else {
                res.ok()
            }
        }
    }

    pub fn results(&self, results: &mut [ND2_RESULT]) -> u32 {
        unsafe {
            self.vtbl.GetResults.unwrap()(self.ptr, results.as_mut_ptr(), results.len() as u32)
        }
    }

    pub fn poll(
        &self,
        type_: NotifyType,
        mut overlapped: impl BorrowMut<OVERLAPPED>,
    ) -> Result<ND2_RESULT> {
        let mut temp = [ND2_RESULT::default(); 1];
        while self.results(&mut temp) == 0 {
            self.notify(type_, overlapped.borrow_mut())?;
        }

        let [res] = temp;
        Ok(res)
    }
}

impl ND2Overlapped for CompletionQueue {
    fn as_overlapped_mut(&self) -> &mut IND2Overlapped {
        unsafe { &mut *(self.ptr as *mut IND2Overlapped) }
    }
}

unsafe impl Send for CompletionQueue {}

impl Clone for CompletionQueue {
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

impl Drop for CompletionQueue {
    fn drop(&mut self) {
        unsafe {
            let _n = self.vtbl.Release.unwrap()(self.ptr);
        }
    }
}
