use std::ffi::c_void;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestContext(pub u128);

impl RequestContext {
	pub fn as_ptr(&self) -> *mut c_void {
		self.0 as *mut c_void
	}
}
