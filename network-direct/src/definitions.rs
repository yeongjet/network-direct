use windows::Win32::Networking::WinSock::{
    PFL_HIDDEN, PFL_NETWORKDIRECT_PROVIDER, SOCKET_ADDRESS, SOCKET_ADDRESS_LIST, XP1_CONNECT_DATA,
    XP1_GUARANTEED_DELIVERY, XP1_GUARANTEED_ORDER, XP1_MESSAGE_ORIENTED,
};

pub const ND_SERVICE_FLAGS1: u32 =
    XP1_GUARANTEED_DELIVERY | XP1_GUARANTEED_ORDER | XP1_MESSAGE_ORIENTED | XP1_CONNECT_DATA;
pub const ND_PROVIDER_FLAGS: u32 = PFL_HIDDEN | PFL_NETWORKDIRECT_PROVIDER;

pub trait SocketAddressExt {
    fn get_addresses(&self) -> &[SOCKET_ADDRESS];
}

impl SocketAddressExt for SOCKET_ADDRESS_LIST {
    fn get_addresses(&self) -> &[SOCKET_ADDRESS] {
        unsafe { std::slice::from_raw_parts(self.Address.as_ptr(), self.iAddressCount as usize) }
    }
}
