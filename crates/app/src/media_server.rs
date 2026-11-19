//! Local HTTP streaming server for WebView2 media playback.
//!
//! Streams local video and audio files to HTML5 `<video>` and `<audio>`
//! elements with support for HTTP 206 Partial Content (byte range requests),
//! enabling smooth scrubbing and hardware-accelerated playback.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicU16, Ordering};

pub static SERVER_PORT: AtomicU16 = AtomicU16::new(0);

/// Starts the background HTTP server on an ephemeral port.
pub fn start_media_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind local media server");
    let port = listener.local_addr().unwrap().port();
    SERVER_PORT.store(port, Ordering::SeqCst);

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            if let Ok(stream) = stream {
                std::thread::spawn(move || {
                    handle_connection(stream);
                });
            }
        }
    });

    port
}

/// Generates a local HTTP streaming URL for a given local file path.
pub fn make_stream_url(file_path: &str) -> String {
    let port = SERVER_PORT.load(Ordering::SeqCst);
    format!("http://127.0.0.1:{}/media?path={}", port, url_encode(file_path))
}

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0u8; 4096];
    let bytes_read = match stream.read(&mut buffer) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let mut lines = request.lines();
    let request_line = match lines.next() {
        Some(l) => l,
        None => return,
    };

    // Extract requested path from "GET /media?path=... HTTP/1.1"
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 || parts[0] != "GET" {
        send_response(&mut stream, 400, "Bad Request", "text/plain", b"Bad Request");
        return;
    }

    let uri = parts[1];
    if !uri.starts_with("/media?path=") {
        send_response(&mut stream, 404, "Not Found", "text/plain", b"Not Found");
        return;
    }

    let encoded_path = &uri["/media?path=".len()..];
    let file_path_str = url_decode(encoded_path);
    let path = Path::new(&file_path_str);

    if !path.exists() || !path.is_file() {
        send_response(&mut stream, 404, "Not Found", "text/plain", b"File Not Found");
        return;
    }

    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => {
            send_response(&mut stream, 500, "Internal Server Error", "text/plain", b"Cannot open file");
            return;
        }
    };

    let total_size = match file.metadata() {
        Ok(m) => m.len(),
        Err(_) => 0,
    };

    let mime = mime_from_path(path);

    // Look for Range header: "Range: bytes=start-end"
    let mut range_header: Option<(u64, Option<u64>)> = None;
    for line in lines {
        if line.to_ascii_lowercase().starts_with("range: bytes=") {
            let spec = &line["range: bytes=".len()..].trim();
            let rparts: Vec<&str> = spec.split('-').collect();
            if let Ok(start) = rparts[0].parse::<u64>() {
                let end = if rparts.len() > 1 && !rparts[1].is_empty() {
                    rparts[1].parse::<u64>().ok()
                } else {
                    None
                };
                range_header = Some((start, end));
                break;
            }
        }
    }

    if let Some((start, end_opt)) = range_header {
        let end = end_opt.unwrap_or(total_size.saturating_sub(1)).min(total_size.saturating_sub(1));
        if start > end || start >= total_size {
            let header = format!(
                "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{}\r\n\r\n",
                total_size
            );
            let _ = stream.write_all(header.as_bytes());
            return;
        }

        let chunk_len = end - start + 1;
        let header = format!(
            "HTTP/1.1 206 Partial Content\r\n\
             Content-Type: {}\r\n\
             Content-Length: {}\r\n\
             Content-Range: bytes {}-{}/{}\r\n\
             Accept-Ranges: bytes\r\n\
             Access-Control-Allow-Origin: *\r\n\
             Connection: keep-alive\r\n\r\n",
            mime, chunk_len, start, end, total_size
        );

        if stream.write_all(header.as_bytes()).is_err() {
            return;
        }

        if file.seek(SeekFrom::Start(start)).is_err() {
            return;
        }

        let mut remaining = chunk_len;
        let mut chunk_buf = [0u8; 64 * 1024];
        while remaining > 0 {
            let to_read = remaining.min(chunk_buf.len() as u64) as usize;
            match file.read(&mut chunk_buf[..to_read]) {
                Ok(0) => break,
                Ok(n) => {
                    if stream.write_all(&chunk_buf[..n]).is_err() {
                        break;
                    }
                    remaining -= n as u64;
                }
                Err(_) => break,
            }
        }
    } else {
        let header = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: {}\r\n\
             Content-Length: {}\r\n\
             Accept-Ranges: bytes\r\n\
             Access-Control-Allow-Origin: *\r\n\
             Connection: keep-alive\r\n\r\n",
            mime, total_size
        );

        if stream.write_all(header.as_bytes()).is_err() {
            return;
        }

        let mut chunk_buf = [0u8; 64 * 1024];
        while let Ok(n) = file.read(&mut chunk_buf) {
            if n == 0 {
                break;
            }
            if stream.write_all(&chunk_buf[..n]).is_err() {
                break;
            }
        }
    }
}

fn send_response(stream: &mut TcpStream, code: u16, status: &str, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
        code, status, content_type, body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
}

fn mime_from_path(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("mp4") | Some("m4v") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("wav") => "audio/wav",
        Some("mp3") => "audio/mpeg",
        Some("flac") => "audio/flac",
        Some("m4a") | Some("aac") => "audio/mp4",
        Some("ogg") => "audio/ogg",
        _ => "application/octet-stream",
    }
}

fn url_encode(input: &str) -> String {
    let mut out = String::new();
    for byte in input.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    out
}

fn url_decode(input: &str) -> String {
    let mut bytes = Vec::new();
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(c1), Some(c2)) = (h1, h2) {
                if let Ok(val) = u8::from_str_radix(&format!("{}{}", c1 as char, c2 as char), 16) {
                    bytes.push(val);
                    continue;
                }
            }
        } else if b == b'+' {
            bytes.push(b' ');
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8_lossy(&bytes).to_string()
}
