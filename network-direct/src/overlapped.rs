use std::{
    borrow::{Borrow, BorrowMut},
    ptr,
};

use network_direct_sys::IND2Overlapped;
use windows::{
    Win32::{
        Foundation::CloseHandle,
        System::{IO::OVERLAPPED, Threading::CreateEventA},
    },
    core::{PCSTR, Result},
};

pub struct Overlapped {
    ptr: OVERLAPPED,
}

impl Overlapped {
    pub fn new() -> Result<Self> {
        let ptr = OVERLAPPED {
            hEvent: unsafe { CreateEventA(None, false, false, PCSTR(ptr::null()))? },
            ..Default::default()
        };

        Ok(Self { ptr })
    }
}

impl Borrow<OVERLAPPED> for &mut Overlapped {
    fn borrow(&self) -> &OVERLAPPED {
        &self.ptr
    }
}

impl BorrowMut<OVERLAPPED> for &mut Overlapped {
    fn borrow_mut(&mut self) -> &mut OVERLAPPED {
        &mut self.ptr
    }
}

impl Drop for Overlapped {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.ptr.hEvent).unwrap();
        }
    }
}

pub trait ND2Overlapped {
    fn as_overlapped_mut(&self) -> &mut IND2Overlapped;

    fn cancel_overlapped_requests(&self) -> Result<()> {
        let this = self.as_overlapped_mut();
        unsafe { (*this.lpVtbl).CancelOverlappedRequests.unwrap()(this).ok() }
    }

    fn get_overlapped_result(
        &self,
        mut overlapped: impl BorrowMut<OVERLAPPED>,
        wait: bool,
    ) -> Result<()> {
        let this = self.as_overlapped_mut();
        unsafe {
            (*this.lpVtbl).GetOverlappedResult.unwrap()(this, overlapped.borrow_mut(), wait.into())
                .ok()
        }
    }
}
