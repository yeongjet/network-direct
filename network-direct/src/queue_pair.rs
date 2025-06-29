use bitflags::bitflags;
use network_direct_sys::{IND2MemoryRegion, IND2MemoryWindow, IND2QueuePair, IND2QueuePairVtbl, ND2_SGE, ND_OP_FLAG_ALLOW_READ, ND_OP_FLAG_ALLOW_WRITE, ND_OP_FLAG_INLINE, ND_OP_FLAG_READ_FENCE, ND_OP_FLAG_SEND_AND_SOLICIT_EVENT, ND_OP_FLAG_SILENT_SUCCESS};
use windows::core::Result;

use crate::{RemoteToken, RequestContext};

bitflags! {
	pub struct SendFlags: u32 {
		const SILENT_SUCCESS = ND_OP_FLAG_SILENT_SUCCESS;
		const READ_FENCE = ND_OP_FLAG_READ_FENCE;
		const SEND_AND_SOLICIT_EVENT = ND_OP_FLAG_SEND_AND_SOLICIT_EVENT;
		const INLINE = ND_OP_FLAG_INLINE;
	}

	pub struct BindFlags: u32 {
		const SILENT_SUCCESS = ND_OP_FLAG_SILENT_SUCCESS;
		const READ_FENCE = ND_OP_FLAG_READ_FENCE;
		const ALLOW_READ = ND_OP_FLAG_ALLOW_READ;
		const ALLOW_WRITE = ND_OP_FLAG_ALLOW_WRITE;
	}

	pub struct InvalidateFlags: u32 {
		const SILENT_SUCCESS = ND_OP_FLAG_SILENT_SUCCESS;
		const READ_FENCE = ND_OP_FLAG_READ_FENCE;
	}

	pub struct ReadFlags: u32 {
		const SILENT_SUCCESS = ND_OP_FLAG_SILENT_SUCCESS;
		const READ_FENCE = ND_OP_FLAG_READ_FENCE;
	}

	pub struct WriteFlags: u32 {
		const SILENT_SUCCESS = ND_OP_FLAG_SILENT_SUCCESS;
		const READ_FENCE = ND_OP_FLAG_READ_FENCE;
		const INLINE = ND_OP_FLAG_INLINE;
	}
}

pub struct QueuePair {
	pub(crate) ptr: *mut IND2QueuePair,
	vtbl: IND2QueuePairVtbl
}

impl AsRef<IND2QueuePair> for QueuePair {
	fn as_ref(&self) -> &IND2QueuePair {
		unsafe { &*self.ptr }
	}
}

impl AsMut<IND2QueuePair> for QueuePair {
	fn as_mut(&mut self) -> &mut IND2QueuePair {
		unsafe { &mut *self.ptr }
	}
}

impl QueuePair {
	pub fn from(ptr: *mut IND2QueuePair) -> Self {
        Self { ptr, vtbl: unsafe { *((*ptr).lpVtbl) } }
    }

	pub fn flush(&self) -> Result<()> {
		unsafe { self.vtbl.Flush.unwrap()(self.ptr).ok() }
	}

	pub fn send(&self, request_context: RequestContext, sge: &[ND2_SGE], flags: SendFlags) -> Result<()> {
		unsafe {
			self.vtbl.Send.unwrap()(
				self.ptr,
				request_context.as_ptr(),
				sge.as_ptr(),
				sge.len() as u32,
				flags.bits(),
			)
			.ok()
		}
	}

	pub fn receive(&self, request_context: RequestContext, sge: &[ND2_SGE]) -> Result<()> {
		unsafe { self.vtbl.Receive.unwrap()(self.ptr, request_context.as_ptr(), sge.as_ptr(), sge.len() as u32).ok() }
	}

	pub fn bind<T>(
		&mut self,
		request_context: RequestContext,
		memory_region: &impl AsRef<IND2MemoryRegion>,
		memory_window: &impl AsRef<IND2MemoryWindow>,
		buffer: &[T],
		flags: BindFlags,
	) -> Result<()> {
		unsafe {
			self.vtbl.Bind.unwrap()(
				self.ptr,
				request_context.as_ptr(),
				memory_region.as_ref() as *const _ as *mut _,
				memory_window.as_ref() as *const _ as *mut _,
				buffer.as_ptr() as _,
				buffer.len() as _,
				flags.bits(),
			)
			.ok()
		}
	}

	pub fn invalidate<T>(
		&mut self,
		request_context: RequestContext,
		memory_window: &impl AsRef<IND2MemoryWindow>,
		flags: InvalidateFlags,
	) -> Result<()> {
		unsafe {
			self.vtbl.Invalidate.unwrap()(
				self.ptr,
				request_context.as_ptr(),
				memory_window.as_ref() as *const _ as *mut _,
				flags.bits(),
			)
			.ok()
		}
	}

	pub fn read(
		&mut self,
		request_context: RequestContext,
		sge: &[ND2_SGE],
		remote_address: u64,
		remote_token: RemoteToken,
		flags: ReadFlags,
	) -> Result<()> {
		unsafe {
			self.vtbl.Read.unwrap()(
				self.ptr,
				request_context.as_ptr(),
				sge.as_ptr(),
				sge.len() as u32,
				remote_address,
				remote_token.0,
				flags.bits(),
			)
			.ok()
		}
	}

	pub fn write(
		&mut self,
		request_context: RequestContext,
		sge: &[ND2_SGE],
		remote_address: u64,
		remote_token: RemoteToken,
		flags: WriteFlags,
	) -> Result<()> {
		unsafe {
			self.vtbl.Write.unwrap()(
				self.ptr,
				request_context.as_ptr(),
				sge.as_ptr(),
				sge.len() as u32,
				remote_address,
				remote_token.0,
				flags.bits(),
			)
			.ok()
		}
	}
}

unsafe impl Send for QueuePair {}

impl Clone for QueuePair {
	fn clone(&self) -> Self {
		unsafe {
			let _n = self.vtbl.AddRef.unwrap()(self.ptr);
		}

		Self { ptr: self.ptr, vtbl: self.vtbl }
	}
}

impl Drop for QueuePair {
	fn drop(&mut self) {
		unsafe {
			let _n = self.vtbl.Release.unwrap()(self.ptr);
		}
	}
}
