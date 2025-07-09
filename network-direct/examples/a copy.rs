use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    os::windows::io::{AsHandle, AsRawHandle},
};

use clap::Parser;

use network_direct::{
    Adapter, Framework, NotifyType, Overlapped, Provider, ReadLimits, RegisterFlags,
    RequestContext, get_local_addr,
};
use network_direct_sys::ND2_SGE;
use windows::Win32::{
    Foundation::HANDLE,
    Networking::WinSock::IPPROTO_TCP,
    System::{
        IO::{CreateIoCompletionPort, GetQueuedCompletionStatus},
        Threading::INFINITE,
    },
};

/// Network Direct Test Program
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run mode: server(-s) or client(-c)
    #[arg(
        short = 's',
        long = "server",
        group = "mode",
        help = "Run in server mode"
    )]
    is_server: bool,

    #[arg(
        short = 'c',
        long = "client",
        group = "mode",
        help = "Run in client mode"
    )]
    is_client: bool,

    /// IP address (listening address for server mode, target address for client mode)
    #[arg(help = "IP address - listening address in server mode, target address in client mode")]
    ip: IpAddr,
    // #[arg(skip)]
    // addr: Option<SocketAddr>,
}

impl Args {
    // fn with_addr(mut self) -> Self {
    //     self.addr = Some(SocketAddr::new(self.ip, 0));
    //     self
    // }

    fn validate() -> Self {
        let args = Self::parse();
        if !args.is_server && !args.is_client {
            eprintln!("Error: Must specify running mode, use -s for server or -c for client");
            std::process::exit(1);
        }
        args
        // args.with_addr()
    }
}

#[derive(Clone, Copy)]
struct People {
    age: u32,
}

// struct Ov {
//     overlapped: Overlapped,
//     success_callback: Box<dyn Fn() + Send + Sync>,
//     failure_callback: Box<dyn Fn() + Send + Sync>,
// }

fn run_client(framework: &Framework, remote_ip: &IpAddr) {
    let remote_addr = SocketAddr::new(*remote_ip, 0);
    let local_addr = get_local_addr(remote_addr);
    println!("local_addr:{}", local_addr);
    let adapter = framework.open_adapter(local_addr).unwrap();
    let mut adapter_file = adapter.create_adapter_file().unwrap();
    let unregistered_memory_region = adapter.create_memory_region(&mut adapter_file).unwrap();
    let x_XferLen = 4096;
    let mut buffer: [People; 1024] = [People { age: 0 }; 1024];
    let mut overlapped = Overlapped::default();
    let mut send_overlapped = Overlapped::default();
    let mut recv_overlapped = Overlapped::default();
    let memory_region = unregistered_memory_region
        .register(buffer, RegisterFlags::ALLOW_LOCAL_WRITE, &mut overlapped)
        .unwrap();
    let adapter_info = adapter.query().unwrap();
    let queue_depth = std::cmp::min(
        adapter_info.MaxCompletionQueueDepth,
        adapter_info.MaxReceiveQueueDepth,
    );
    let send_cq = adapter
        .create_completion_queue(&adapter_file, queue_depth, 0, 0)
        .unwrap();
    let recv_cq = adapter
        .create_completion_queue(&adapter_file, queue_depth, 0, 0)
        .unwrap();
    let m_hIocp = unsafe {
        CreateIoCompletionPort(HANDLE(adapter_file.as_raw_handle()), None, 0, 0).unwrap()
    };
    send_cq
        .notify(NotifyType::Any, &mut send_overlapped)
        .unwrap();
    recv_cq
        .notify(NotifyType::Any, &mut recv_overlapped)
        .unwrap();
    let connector = adapter.create_connector(&adapter_file).unwrap();
    let queue_pair = adapter
        .create_queue_pair(&recv_cq, &send_cq, 1, 1, 1, 1, 0)
        .unwrap();
    let local_token = memory_region.get_local_token();
    let sge = [ND2_SGE {
        Buffer: buffer.as_mut_ptr() as *mut std::ffi::c_void,
        BufferLength: 0,
        MemoryRegionToken: memory_region.get_local_token().0,
    }];
    queue_pair.receive(RequestContext(0), &sge).unwrap();
    connector.bind(local_addr).unwrap();
    let read_limits = ReadLimits {
        inbound_read_limit: IPPROTO_TCP.0 as u32,
        outbound_read_limit: 0,
    };
    
    let mut connect_ov = Overlapped::from(
        Some(Box::new(|| println!("Connection established!"))),
        Some(Box::new(|| println!("Connection failed!"))),
    ).unwrap();
    
    println!("Initiating connection to {}", remote_addr);
    connector
        .connect(&queue_pair, remote_addr, read_limits, None, &mut connect_ov)
        .unwrap();
    
    println!("Client waiting for connection completion...");
    
    loop {
        let mut bytes_ret = 0u32;
        let mut key = 0usize;
        let mut rov_ptr = std::ptr::null_mut();
        println!("Waiting for completion...");
        
        let result = unsafe {
            GetQueuedCompletionStatus(m_hIocp, &mut bytes_ret, &mut key, &mut rov_ptr, INFINITE)
        };
        
        match result {
            Ok(_) => {
                println!("IOCP completion received - bytes: {}, key: {}", bytes_ret, key);
                
                // 检查是否是发送或接收完成队列的通知
                if rov_ptr == &mut send_overlapped.ptr as *mut _ {
                    println!("Send completion queue notification");
                    if let Some(success_fn) = &send_overlapped.success_fn {
                        success_fn();
                    }
                    // 重新设置通知
                    send_cq.notify(NotifyType::Any, &mut send_overlapped).unwrap();
                } else if rov_ptr == &mut recv_overlapped.ptr as *mut _ {
                    println!("Receive completion queue notification");
                    if let Some(success_fn) = &recv_overlapped.success_fn {
                        success_fn();
                    }
                    // 重新设置通知
                    recv_cq.notify(NotifyType::Any, &mut recv_overlapped).unwrap();
                } else if rov_ptr == &mut connect_ov.ptr as *mut _ {
                    println!("Connection completed!");
                    if let Some(success_fn) = &connect_ov.success_fn {
                        success_fn();
                    }
                    
                    // 连接成功，可以开始发送/接收数据
                    println!("Connection established successfully!");
                    break;
                }
            }
            Err(e) => {
                eprintln!("GetQueuedCompletionStatus failed: {:?}", e);
                break;
            }
        }
    }
    // if m_hIocp.is_null()
    // {
    //     eprintln!("Failed to bind adapter to IOCP, error {}", GetLastError());
    //     std::process::exit(1);
    // }
}

fn run_server(framework: &Framework, local_ip: &IpAddr) {
    // let fSuccess = unsafe { GetQueuedCompletionStatus(pTest->m_hIocp, &bytesRet, &key, &pOv, INFINITE).unwrap() };
    let local_addr = SocketAddr::new(*local_ip, 0);
    let adapter = framework.open_adapter(local_addr).unwrap();
    let mut adapter_file = adapter.create_adapter_file().unwrap();
    let unregistered_memory_region = adapter.create_memory_region(&mut adapter_file).unwrap();
    let mut buffer: [People; 1024] = [People { age: 0 }; 1024];
    let mut ov = Overlapped::default();
    let _memory_region = unregistered_memory_region
        .register(&mut buffer, RegisterFlags::ALLOW_REMOTE_WRITE, &mut ov)
        .unwrap();
    let adapter_info = adapter.query().unwrap();
    let queue_depth = std::cmp::min(
        adapter_info.MaxCompletionQueueDepth,
        adapter_info.MaxReceiveQueueDepth,
    );
    let send_cq = adapter
        .create_completion_queue(&adapter_file, queue_depth, 0, 0)
        .unwrap();
    let recv_cq = adapter
        .create_completion_queue(&adapter_file, queue_depth, 0, 0)
        .unwrap();
    let listener = adapter.create_listener(&adapter_file).unwrap();
    listener.bind(local_addr).unwrap();
    listener.listen(0).unwrap();
    let iocp = unsafe {
        CreateIoCompletionPort(HANDLE(adapter_file.as_raw_handle()), None, 0, 0).unwrap()
    };
    let mut send_ov = Overlapped::from(None, None).unwrap();
    let mut recv_ov = Overlapped::from(None, None).unwrap();
    send_cq.notify(NotifyType::Any, &mut send_ov).unwrap();
    recv_cq.notify(NotifyType::Any, &mut recv_ov).unwrap();
    let mut connector = adapter.create_connector(&adapter_file).unwrap();
    let mut ov2 = Overlapped::from(
        Some(Box::new(|| {
            println!("Connection request received!");
        })),
        Some(Box::new(|| {
            println!("Connection request failed!");
        })),
    )
    .unwrap();
    
    // 启动异步连接请求监听
    listener
        .get_connection_request(&mut connector, &mut ov2)
        .unwrap();
    
    println!("Server waiting for connections...");
    
    loop {
        let mut bytes_ret = 0u32;
        let mut key = 0usize;
        let mut rov_ptr = std::ptr::null_mut();
        println!("Waiting for completion...");
        
        let result = unsafe {
            GetQueuedCompletionStatus(iocp, &mut bytes_ret, &mut key, &mut rov_ptr, INFINITE)
        };
        
        match result {
            Ok(_) => {
                println!("IOCP completion received - bytes: {}, key: {}", bytes_ret, key);
                
                // 检查是否是发送或接收完成队列的通知
                if rov_ptr == &mut send_ov.ptr as *mut _ {
                    println!("Send completion queue notification");
                    if let Some(success_fn) = &send_ov.success_fn {
                        success_fn();
                    }
                    // 重新设置通知
                    send_cq.notify(NotifyType::Any, &mut send_ov).unwrap();
                } else if rov_ptr == &mut recv_ov.ptr as *mut _ {
                    println!("Receive completion queue notification");
                    if let Some(success_fn) = &recv_ov.success_fn {
                        success_fn();
                    }
                    // 重新设置通知
                    recv_cq.notify(NotifyType::Any, &mut recv_ov).unwrap();
                } else if rov_ptr == &mut ov2.ptr as *mut _ {
                    println!("Connection request completed!");
                    if let Some(success_fn) = &ov2.success_fn {
                        success_fn();
                    }
                    
                    // 处理连接请求
                    // 这里你需要创建 queue pair 并接受连接
                    let queue_pair = adapter
                        .create_queue_pair(&recv_cq, &send_cq, 1, 1, 1, 1, 0)
                        .unwrap();
                    
                    let read_limits = ReadLimits {
                        inbound_read_limit: IPPROTO_TCP.0 as u32,
                        outbound_read_limit: 0,
                    };
                    
                    let mut accept_ov = Overlapped::from(
                        Some(Box::new(|| println!("Connection accepted!"))),
                        Some(Box::new(|| println!("Connection accept failed!"))),
                    ).unwrap();
                    
                    // 接受连接
                    connector.accept(&queue_pair, read_limits, None, &mut accept_ov).unwrap();
                }
            }
            Err(e) => {
                eprintln!("GetQueuedCompletionStatus failed: {:?}", e);
                break;
            }
        }
    }
}
// cargo run --example a -- -s 192.168.1.1
// cargo run --example a -- --server 192.168.1.1

// cargo run --example a -- -c 192.168.1.1
// cargo run --example a -- --client 192.168.1.1
fn main() {
    let args = Args::validate();
    let framework = Framework::new();
    if args.is_server {
        println!("Running in server mode, listening on: {}", args.ip);
        run_server(&framework, &args.ip);
    } else {
        println!("Running in client mode, remote ip: {}", args.ip);
        run_client(&framework, &args.ip)
    }

    // Example pointer, replace with actual pointer as needed
    // let ptr: *mut IND2Provider = std::ptr::null_mut();
    // let ip = "192.168.1.1:0".parse().unwrap();
    // let provider = unsafe { Provider::from_ptr(ptr) };
    // let adapter_id = provider.resolve_address(ip).unwrap();
    // let adapter = provider.open_adapter(adapter_id).unwrap();
    // let adapter_info = adapter.query().unwrap();

    // println!("{:?}", adapter_info);
}
