use std::{
    collections::HashMap,
    mem,
    net::{IpAddr, SocketAddr},
    os::windows::io::{AsHandle, AsRawHandle},
};

use clap::Parser;

use network_direct::{
    Adapter, CompletionQueue, Connector, Framework, MemoryRegion, NotifyType, Overlapped, Provider,
    ReadLimits, RegisterFlags, RequestContext, get_local_addr,
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
#[derive(Debug)]
enum Event {
    ConSuccess,
    ConFailed,
    AcceptSuccess,
    AcceptFailed,
    ASuccess,
    AFailed,
    BSuccess,
    BFailed,
    CSuccess,
    CFailed,
    DSuccess,
    DFailed,
}
// struct Ov {
//     overlapped: Overlapped,
//     success_callback: Box<dyn Fn() + Send + Sync>,
//     failure_callback: Box<dyn Fn() + Send + Sync>,
// }

// fn run_client(framework: &Framework, remote_ip: &IpAddr) {
//     let remote_addr = SocketAddr::new(*remote_ip, 0);
//     let local_addr = get_local_addr(remote_addr);
//     println!("local_addr:{}", local_addr);
//     let adapter = framework.open_adapter(local_addr).unwrap();
//     let adapter_file = adapter.create_adapter_file().unwrap();
//     let unregistered_memory_region = adapter.create_memory_region(&mut adapter_file).unwrap();
//     let x_XferLen = 4096;
//     let mut buffer: [People; 1024] = [People { age: 0 }; 1024];
//     let mut overlapped = Overlapped::default();
//     let mut send_overlapped = Overlapped::default();
//     let mut recv_overlapped = Overlapped::default();
//     let memory_region = unregistered_memory_region
//         .register(buffer, RegisterFlags::ALLOW_LOCAL_WRITE, &mut overlapped)
//         .unwrap();
//     let adapter_info = adapter.query().unwrap();
//     let queue_depth = std::cmp::min(
//         adapter_info.MaxCompletionQueueDepth,
//         adapter_info.MaxReceiveQueueDepth,
//     );
//     let send_cq = adapter
//         .create_completion_queue(&adapter_file, queue_depth, 0, 0)
//         .unwrap();
//     let recv_cq = adapter
//         .create_completion_queue(&adapter_file, queue_depth, 0, 0)
//         .unwrap();
//     let m_hIocp = unsafe {
//         CreateIoCompletionPort(HANDLE(adapter_file.as_raw_handle()), None, 0, 0).unwrap()
//     };
//     send_cq
//         .notify(NotifyType::Any, &mut send_overlapped)
//         .unwrap();
//     recv_cq
//         .notify(NotifyType::Any, &mut recv_overlapped)
//         .unwrap();
//     let connector = adapter.create_connector(&adapter_file).unwrap();
//     let queue_pair = adapter
//         .create_queue_pair(&recv_cq, &send_cq, 1, 1, 1, 1, 0)
//         .unwrap();
//     let local_token = memory_region.get_local_token();
//     let sge = [ND2_SGE {
//         Buffer: buffer.as_mut_ptr() as *mut std::ffi::c_void,
//         BufferLength: 0,
//         MemoryRegionToken: memory_region.get_local_token().0,
//     }];
//     queue_pair.receive(RequestContext(0), &sge).unwrap();
//     connector.bind(local_addr).unwrap();
//     let read_limits = ReadLimits {
//         inbound_read_limit: IPPROTO_TCP.0 as u32,
//         outbound_read_limit: 0,
//     };
//     connector
//         .connect(&queue_pair, remote_addr, read_limits, None, &mut overlapped)
//         .unwrap();
//     // if m_hIocp.is_null()
//     // {
//     //     eprintln!("Failed to bind adapter to IOCP, error {}", GetLastError());
//     //     std::process::exit(1);
//     // }
// }

fn connect_success_callback<T>(
    adapter: &Adapter,
    connector: &Connector,
    recv_cq: &CompletionQueue,
    send_cq: &CompletionQueue,
    memory_region: &MemoryRegion<T>,
) {
    println!("connect success callback");
    let queue_pair = adapter
        .create_queue_pair(&recv_cq, &send_cq, 1, 1, 1, 1, 0)
        .unwrap();
    let mut buffer: [People; 1024] = [People { age: 0 }; 1024];
    let sge = [ND2_SGE {
        Buffer: buffer.as_mut_ptr() as *mut std::ffi::c_void,
        BufferLength: mem::size_of_val(&buffer) as u32,
        MemoryRegionToken: memory_region.get_local_token().0,
    }];
    queue_pair.receive(RequestContext(0), &sge).unwrap();
    let read_limits = ReadLimits {
        inbound_read_limit: 0,
        outbound_read_limit: 0,
    };
    let mut accept_ov = Overlapped::new(Event::AcceptSuccess, Event::AcceptFailed);
    let result = connector.accept(&queue_pair, read_limits.clone(), None, &mut accept_ov);
    println!("result {:?}", result);
}

fn run_server(framework: &Framework, local_ip: &IpAddr) {
    let local_addr = SocketAddr::new(*local_ip, 54321);
    let adapter = framework.open_adapter(local_addr).unwrap();
    let mut adapter_file = adapter.create_adapter_file().unwrap();
    let unregistered_memory_region = adapter.create_memory_region(&mut adapter_file).unwrap();
    let mut buffer: [People; 1024] = [People { age: 0 }; 1024];
    let mut ov = Overlapped::new(Event::ASuccess, Event::BFailed);
    let memory_region = unregistered_memory_region
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
    let mut send_ov = Overlapped::new(Event::BSuccess, Event::BFailed);
    let mut recv_ov = Overlapped::new(Event::CSuccess, Event::CFailed);
    send_cq.notify(NotifyType::Any, &mut send_ov).unwrap();
    recv_cq.notify(NotifyType::Any, &mut recv_ov).unwrap();
    let listener = adapter.create_listener(&adapter_file).unwrap();
    listener.bind(local_addr).unwrap();
    listener.listen(0).unwrap();
    let iocp = unsafe {
        CreateIoCompletionPort(HANDLE(adapter_file.as_raw_handle()), None, 0, 0).unwrap()
    };
    let mut connector = adapter.create_connector(&adapter_file).unwrap();
    let mut connect_ov = Overlapped::new(Event::ConSuccess, Event::ConFailed);

    listener
        .get_connection_request(&mut connector, &mut connect_ov)
        .unwrap();
    loop {
        let mut bytes_ret = 0u32;
        let mut key = 0usize;
        let mut rov_ptr = std::ptr::null_mut();
        println!("GetQueuedCompletionStatus");
        let result = unsafe {
            GetQueuedCompletionStatus(iocp, &mut bytes_ret, &mut key, &mut rov_ptr, INFINITE)
        };
        println!("GetQueuedCompletionStatus end result:{:?}", result);
        if rov_ptr.is_null() {
            println!("ov is null");
        }
        let overlapped = unsafe { &mut *(rov_ptr as *mut Overlapped<Event>) };
        match result {
            Ok(_) => {
                println!("success");
                if let Some(event_type) = &overlapped.success_event_type {
                    println!("event_type:{:?}", event_type);
                    connect_success_callback(
                        &adapter,
                        &connector,
                        &recv_cq,
                        &send_cq,
                        &memory_region,
                    );
                }
            }
            Err(e) => {
                eprintln!("failed: {:?}", e);
                if let Some(event_type) = &overlapped.failure_event_type {
                    println!("event_type:{:?}", event_type)
                }
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
        //run_client(&framework, &args.ip)
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
