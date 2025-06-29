#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use windows::core::BOOL;
use windows::core::GUID;
pub type REFIID = *const GUID;
use windows::core::IUnknown;
use windows::core::HRESULT;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::Networking::WinSock::SOCKADDR as sockaddr;
use windows::Win32::Networking::WinSock::SOCKET_ADDRESS_LIST;
use windows::Win32::System::IO::OVERLAPPED;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

mod status;
pub use status::*;

mod guids;
pub use guids::*;
