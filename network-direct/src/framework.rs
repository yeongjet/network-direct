use std::{
    collections::HashMap, ffi::{self, c_void}, mem, net::{IpAddr, SocketAddr}, ptr::{self, addr_of}
};

use network_direct_sys::{IID_IND2Provider, IND2Provider, ND_VERSION_2};
// use lazy_static::lazy_static;
// use networkdirect_sys::*;
// use thiserror::Error;
use windows::{
    core::{GUID, HRESULT, PCSTR, PCWSTR, PWSTR}, Win32::{
        Foundation::{HANDLE, MAX_PATH},
        Networking::WinSock::{
            WSCEnumProtocols, WSCGetProviderPath, AF_INET, AF_INET6, WSAENOBUFS, WSAPROTOCOL_INFOW
        },
        System::{
            Environment::ExpandEnvironmentStringsW,
            LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_FLAGS},
        },
    }
};

use crate::{Adapter, Provider, ND_PROVIDER_FLAGS, ND_SERVICE_FLAGS1};

pub type DllGetClassObject = unsafe extern "C" fn(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut ffi::c_void,
) -> HRESULT;

pub type DllCanUnloadNow = unsafe extern "C" fn() -> bool;

pub struct Framework {
    pub providers: Vec<Provider>,
}

impl Framework {
    pub fn new() -> Self {
        let mut providers: Vec<Provider> = vec![];
        let mut protocols_buf_size: u32 = 0;
        let mut error: i32 = 0;
        let enum_protocol_count =
            unsafe { WSCEnumProtocols(None, None, &mut protocols_buf_size, &mut error) };
        if !(error == WSAENOBUFS.0 && enum_protocol_count == -1) {
            panic!("WSCEnumProtocols failed with error: {}", error);
        }
        let protocol_info_size = size_of::<WSAPROTOCOL_INFOW>() as u32;
        if protocols_buf_size % protocol_info_size != 0 {
            panic!(
                "Size of WSAPROTOCOL_INFOW is not a multiple of the size of the protocol info structure"
            );
        }
        let expected_protocol_count = protocols_buf_size / protocol_info_size;
        let mut protocos = vec![WSAPROTOCOL_INFOW::default(); expected_protocol_count as usize];
        let enum_protocol_count = unsafe {
            WSCEnumProtocols(
                None,
                Some(protocos.as_mut_ptr()),
                &mut protocols_buf_size,
                &mut error,
            )
        } as u32;
        if enum_protocol_count != expected_protocol_count {
            panic!(
                "WSCEnumProtocols returned unexpected count: expected {}, got {}",
                expected_protocol_count, enum_protocol_count
            );
        }
        let nd_protocos: Vec<_> = protocos
                .iter()
                .copied()
                .filter(|x| {
                    // x.dwServiceFlags1 &  ND_SERVICE_FLAGS1 == ND_SERVICE_FLAGS1
                    //     && 
                        x.iVersion == ND_VERSION_2 as i32
                        // && x.dwProviderFlags & ND_PROVIDER_FLAGS == ND_PROVIDER_FLAGS
                        // && matches!(x.iAddressFamily, i if i == AF_INET.0.into() || i == AF_INET6.0.into())
                        // && x.iSocketType == -1
                        // && x.iProtocol == 0
                        // && x.iProtocolMaxOffset == 0
                })
                .collect();
        println!("nd_protocos:{:?}", nd_protocos);
        nd_protocos.iter().for_each(|x| {
            // 路径长度不能为0
            let mut dll_path_len = MAX_PATH as i32;
            // 需要预留足够的容量并初始化为0
            let mut raw_dll_path_in_chars = [0u16; MAX_PATH as usize];
            let mut error: i32 = 0;
            unsafe {
                WSCGetProviderPath(
                    &x.ProviderId,
                    PWSTR(raw_dll_path_in_chars.as_mut_ptr()),
                    &mut dll_path_len,
                    &mut error,
                )
            };
            if error != 0 {
                panic!("WSCGetProviderPath failed with error: {}", error);
            }
            // -10并不会影响ExpandEnvironmentStringsW的输出，%SYSTEMROOT%\system32\mlx依然会返回C:\WINDOWS\system32\mlx5nd.dll
            // 因为就算-10，mlx依然是5nd.dll，ExpandEnvironmentStringsW会一直读到0为止
            // let raw_path_in_bytes_sliced = raw_path_in_bytes[..raw_path_length as usize -10].to_vec();
            // path_in_chars = %SYSTEMROOT%\system32\mlx5nd.dll
            // path_in_bytes.truncate(path_length as usize) truncate之后后面还是不为0;
            let mut dll_path_in_chars: [u16; 260] = [0u16; MAX_PATH as usize];
            unsafe {
                ExpandEnvironmentStringsW(
                    PCWSTR::from_raw(raw_dll_path_in_chars.as_ptr()),
                    Some(&mut dll_path_in_chars),
                )
            };
            // path_in_bytes会包括字符串末尾的0，所以要-1
            // let dll_path_len = unsafe {
            //     ExpandEnvironmentStringsW(
            //         PCWSTR::from_raw(raw_dll_path_in_chars.as_ptr()),
            //         Some(&mut dll_path_in_chars),
            //     )
            // } - 1;
            // let dll_path = String::from_utf16(&dll_path_in_chars[..dll_path_len as usize]).unwrap();
            // println!("dll_path:{}", dll_path);
            let hmodule = unsafe {
                LoadLibraryExW(
                    PWSTR(dll_path_in_chars.as_mut_ptr()),
                    None,
                    LOAD_LIBRARY_FLAGS(0),
                )
                .unwrap()
            };
            // let address = GetProcAddress(library, "GetTickCount\0".as_ptr()).unwrap();
            let get_class_object =
                unsafe { GetProcAddress(hmodule, PCSTR("DllGetClassObject".as_ptr())).unwrap() };
            let get_class_object: DllGetClassObject = unsafe { mem::transmute(get_class_object) };
            println!("proc: {:?}", get_class_object);
            let can_unload_now =
                unsafe { GetProcAddress(hmodule, PCSTR("DllCanUnloadNow".as_ptr())).unwrap() };
            let can_unload_now: DllCanUnloadNow = unsafe { mem::transmute(can_unload_now) };
            println!("canUnloadNow: {:?}", can_unload_now);
            // let provider = providers.first().unwrap();
            let mut provider_ptr = ptr::null_mut();
            println!(
                "{:p},{:?},{:p}",
                provider_ptr,
                addr_of!(provider_ptr),
                &provider_ptr
            );
            unsafe {
                (get_class_object)(&x.ProviderId, &IID_IND2Provider, &mut provider_ptr).unwrap();
            };
            println!(
                "{:p},{:?},{:p}",
                provider_ptr,
                addr_of!(provider_ptr),
                &provider_ptr
            );
            // \xd0\x47\x37\x92\xff\x7f\x00\x00\x01
            // 0x6a440feec8(provider_ptr)[0x182a04548d0]
            // 0x182a04548d0[\xd0\x47\x37\x92\xff\x7f\x00\x00\x01]
            let provider = Provider::from(provider_ptr as *mut IND2Provider);
            providers.push(provider);
        });
        return Self { providers };
    }

    pub fn open_adapter(&self, addr: SocketAddr) -> Option<Adapter> {
        for provider in &self.providers {
            let ip_list = provider.query_ip_list().unwrap();
            println!("ip list: {:?}", ip_list);
            if ip_list.contains(&addr.ip()) {
                let adapter_id = provider.resolve_address(addr).unwrap();
                let adapter = provider.open_adapter(adapter_id).unwrap();
                return Some(adapter);
            }
        }
        None
    }
}
