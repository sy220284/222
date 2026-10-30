//! Consume the entire HTTP request from local test servers before replying.
//! Windows resets sockets closed with unread request-body bytes; reading only
//! a single byte made otherwise-correct SSE/WebFetch tests fail on WinSock.
use std::io;
use tokio::{io::AsyncReadExt, net::TcpStream};

pub async fn consume_request(socket: &mut TcpStream) -> io::Result<()> {
    let mut received = Vec::new();
    let mut block = [0u8; 4096];
    loop {
        if let Some(marker) = received.windows(4).position(|part| part == b"\r\n\r\n") {
            let body_start = marker + 4;
            let headers = String::from_utf8_lossy(&received[..marker]);
            let mut length = 0usize;
            for line in headers.lines() {
                let Some((name, value)) = line.split_once(':') else { continue };
                if name.eq_ignore_ascii_case("Content-Length") {
                    length = value.trim().parse::<usize>().map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "Invalid Content-Length")
                    })?;
                }
            }
            if length > 10 * 1024 * 1024 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Oversized test request"));
            }
            while received.len().saturating_sub(body_start) < length {
                let n = socket.read(&mut block).await?;
                if n == 0 {
                    return Err(io::Error::new(io::ErrorKind::UnexpectedEof,
                        "HTTP test request ended before its body"));
                }
                received.extend_from_slice(&block[..n]);
            }
            return Ok(());
        }
        if received.len() > 128 * 1024 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Oversized test HTTP header"));
        }
        let n = socket.read(&mut block).await?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof,
                "HTTP test request ended before its headers"));
        }
        received.extend_from_slice(&block[..n]);
    }
}
