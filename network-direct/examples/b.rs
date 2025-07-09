use std::io::{self, Error, ErrorKind};
use std::net::{TcpListener, TcpStream};
use std::ptr;
use std::mem;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use winapi::um::ioapiset::{CreateIoCompletionPort, GetQueuedCompletionStatus, PostQueuedCompletionStatus};
use winapi::um::winnt::HANDLE;
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::minwinbase::OVERLAPPED;
use winapi::um::winsock2::{WSARecv, WSASend};
use winapi::shared::ws2def::WSABUF;
use winapi::shared::ws2def::SOCKADDR;
use winapi::um::handleapi::CloseHandle;

const BUFFER_SIZE: usize = 1024;
const COMPLETION_KEY_SHUTDOWN: usize = 0;
const COMPLETION_KEY_SOCKET: usize = 1;

#[repr(C)]
struct IoOperation {
    overlapped: OVERLAPPED,
    buffer: [u8; BUFFER_SIZE],
    operation_type: OperationType,
}

#[derive(Clone, Copy)]
enum OperationType {
    Read,
    Write,
}

impl IoOperation {
    fn new(op_type: OperationType) -> Self {
        Self {
            overlapped: unsafe { mem::zeroed() },
            buffer: [0; BUFFER_SIZE],
            operation_type: op_type,
        }
    }
}

struct IocpServer {
    completion_port: HANDLE,
}

// SAFETY: Windows HANDLE can be safely shared between threads
// IOCP handles are designed to be thread-safe
unsafe impl Send for IocpServer {}
unsafe impl Sync for IocpServer {}

impl IocpServer {
    fn new() -> io::Result<Self> {
        let completion_port = unsafe {
            CreateIoCompletionPort(
                INVALID_HANDLE_VALUE,
                ptr::null_mut(),
                0,
                0, // 使用系统默认线程数
            )
        };

        if completion_port.is_null() {
            return Err(Error::last_os_error());
        }

        Ok(Self { completion_port })
    }

    fn associate_socket(&self, socket: &TcpStream) -> io::Result<()> {
        use std::os::windows::io::AsRawSocket;
        
        let socket_handle = socket.as_raw_socket() as HANDLE;
        let result = unsafe {
            CreateIoCompletionPort(
                socket_handle,
                self.completion_port,
                COMPLETION_KEY_SOCKET,
                0,
            )
        };

        if result.is_null() {
            return Err(Error::last_os_error());
        }

        Ok(())
    }

    fn post_read(&self, socket: &TcpStream) -> io::Result<Box<IoOperation>> {
        use std::os::windows::io::AsRawSocket;
        
        let mut io_op = Box::new(IoOperation::new(OperationType::Read));
        let mut wsabuf = WSABUF {
            len: BUFFER_SIZE as u32,
            buf: io_op.buffer.as_mut_ptr() as *mut i8,
        };
        
        let mut bytes_received = 0u32;
        let mut flags = 0u32;
        
        let result = unsafe {
            WSARecv(
                socket.as_raw_socket() as usize,
                &mut wsabuf,
                1,
                &mut bytes_received,
                &mut flags,
                &mut io_op.overlapped as *mut OVERLAPPED,
                None,
            )
        };

        if result != 0 {
            let error = Error::last_os_error();
            if error.raw_os_error() != Some(997) { // ERROR_IO_PENDING
                return Err(error);
            }
        }

        Ok(io_op)
    }

    fn run(&self) -> io::Result<()> {
        println!("IOCP服务器开始运行...");
        
        loop {
            let mut bytes_transferred = 0u32;
            let mut completion_key = 0usize;
            let mut overlapped_ptr: *mut OVERLAPPED = ptr::null_mut();

            let result = unsafe {
                GetQueuedCompletionStatus(
                    self.completion_port,
                    &mut bytes_transferred,
                    &mut completion_key,
                    &mut overlapped_ptr,
                    5000, // 5秒超时
                )
            };

            if result == 0 {
                let error = Error::last_os_error();
                if error.raw_os_error() == Some(258) { // WAIT_TIMEOUT
                    println!("等待超时，继续监听...");
                    continue;
                }
                println!("GetQueuedCompletionStatus 错误: {:?}", error);
                continue;
            }

            match completion_key {
                COMPLETION_KEY_SHUTDOWN => {
                    println!("收到关闭信号");
                    break;
                }
                COMPLETION_KEY_SOCKET => {
                    if !overlapped_ptr.is_null() {
                        // 计算IoOperation的地址
                        let io_op_ptr = (overlapped_ptr as *mut u8)
                            .wrapping_sub(mem::offset_of!(IoOperation, overlapped))
                            as *mut IoOperation;
                        
                        let io_op = unsafe { Box::from_raw(io_op_ptr) };
                        
                        match io_op.operation_type {
                            OperationType::Read => {
                                if bytes_transferred > 0 {
                                    let data = &io_op.buffer[..bytes_transferred as usize];
                                    println!("接收到 {} 字节数据: {:?}", 
                                        bytes_transferred, 
                                        String::from_utf8_lossy(data));
                                } else {
                                    println!("客户端断开连接");
                                }
                            }
                            OperationType::Write => {
                                println!("写入操作完成，传输了 {} 字节", bytes_transferred);
                            }
                        }
                    }
                }
                _ => {
                    println!("未知的完成键: {}", completion_key);
                }
            }
        }

        Ok(())
    }

    fn shutdown(&self) -> io::Result<()> {
        let result = unsafe {
            PostQueuedCompletionStatus(
                self.completion_port,
                0,
                COMPLETION_KEY_SHUTDOWN,
                ptr::null_mut(),
            )
        };

        if result == 0 {
            return Err(Error::last_os_error());
        }

        Ok(())
    }
}

impl Drop for IocpServer {
    fn drop(&mut self) {
        if !self.completion_port.is_null() {
            unsafe {
                CloseHandle(self.completion_port);
            }
        }
    }
}

fn main() -> io::Result<()> {
    println!("启动IOCP示例服务器...");

    // 创建IOCP服务器
    let server = Arc::new(IocpServer::new()?);
    
    // 创建TCP监听器
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    println!("服务器监听在 127.0.0.1:8080");

    // 在单独的线程中处理连接
    let server_clone = server.clone();
    let connection_thread = thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(socket) => {
                    println!("新连接来自: {:?}", socket.peer_addr());
                    
                    // 将socket关联到完成端口
                    if let Err(e) = server_clone.associate_socket(&socket) {
                        eprintln!("关联socket失败: {:?}", e);
                        continue;
                    }

                    // 投递读取操作
                    match server_clone.post_read(&socket) {
                        Ok(io_op) => {
                            // 防止io_op被释放
                            Box::leak(io_op);
                        }
                        Err(e) => {
                            eprintln!("投递读取操作失败: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("接受连接失败: {:?}", e);
                }
            }
        }
    });

    // 运行IOCP事件循环
    let server_for_run = server.clone();
    let iocp_thread = thread::spawn(move || {
        if let Err(e) = server_for_run.run() {
            eprintln!("IOCP运行错误: {:?}", e);
        }
    });

    // 等待一段时间后关闭服务器
    thread::sleep(Duration::from_secs(30));
    println!("关闭服务器...");
    server.shutdown()?;

    // 等待线程结束
    let _ = connection_thread.join();
    let _ = iocp_thread.join();

    println!("服务器已关闭");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_iocp_server_creation() {
        let server = IocpServer::new();
        assert!(server.is_ok());
    }

    #[test]
    fn test_client_connection() -> io::Result<()> {
        // 这个测试需要在实际环境中运行
        let mut stream = TcpStream::connect("127.0.0.1:8080")?;
        stream.write_all(b"Hello IOCP Server!")?;
        thread::sleep(Duration::from_millis(100));
        Ok(())
    }
}