extern crate bee;
extern crate url;

use std::collections::HashMap;
use std::io::TcpStream;
use std::io::net;
use std::os;
use std::slice::bytes::copy_memory;
use std::str::from_utf8;
use url::Url;

use bee::http;
use bee::http::parser::{Parser, ParseResponse, MessageHandler};

pub struct ResponseHandler {
    finished: bool,
    version: Option<http::HttpVersion>,
    status: uint,
    headers: HashMap<String, String>,
    body: Option<Vec<u8>>,
    buffer: Vec<u8>,
}

impl ResponseHandler {
    fn new() -> ResponseHandler {
        ResponseHandler {
            finished: false,
            version: None,
            status: 0,
            headers: HashMap::new(),
            buffer: Vec::new(),
            body: None,
        }
    }
}

impl MessageHandler for ResponseHandler {
    fn on_version(&mut self, _: &Parser, version: http::HttpVersion) {
        self.version = Some(version);
    }

    fn on_status(&mut self, _: &Parser, status: uint) {
        self.status = status;
    }

    fn on_header_value(&mut self, _: &Parser, length: uint) {
        {
            let len = self.buffer.len();
            let name = {
                let slice = self.buffer.slice_to(len-length);
                match from_utf8(slice) {
                    Some(s) => s.clone(),
                    None => return,
                }
            };
            let value = {
                let slice = self.buffer.slice_from(len-length);
                match from_utf8(slice) {
                    Some(s) => s.clone(),
                    None => return,
                }
            };
            self.headers.insert(name.to_string(), value.to_string());
        }
        self.buffer.clear();
    }

    fn on_headers_complete(&mut self, _: &Parser) -> bool {
        false
    }

    fn on_body(&mut self, _: &Parser, length: uint) {
        {
            let body = if length > 0 {
                let ref st = self.buffer;
                Some(st.clone())
            } else {
                None
            };
            self.body = body;
        }
        self.buffer.clear();
    }

    fn on_message_complete(&mut self, parser: &Parser) {
        if parser.chunked() {
            self.on_body(parser, ::std::uint::MAX);
        }
        self.finished = true;
    }

    fn write(&mut self, _: &Parser, byte: &[u8]) {
        self.buffer.push_all(byte);
    }
}

#[allow(unused_must_use)]
fn main() {
    let args = os::args();
    let url = Url::parse(args[1].as_slice()).unwrap();
    let ip = match net::get_host_addresses(url.host.as_slice()) {
        Ok(x) => format!("{}", x[0]),
        Err(e) => fail!("{}", e),
    };
    let mut stream = TcpStream::connect(ip.as_slice(), 80);
    write!(stream, "GET / HTTP/1.1\r\n");
    write!(stream, "Host: {}\r\n", url.host);
    write!(stream, "\r\n");
    let mut handler = ResponseHandler::new();
    let mut parser = Parser::new(ParseResponse);
    let mut buf = [0u8, ..1024];
    let mut offset = 0u;
    loop {
        let len = match stream.read(buf.mut_slice_from(offset)) {
            Ok(len) => len,
            Err(e) => fail!("{}", e),
        };
        if len == 0 { continue }
        let read = match parser.parse(buf.slice_to(offset+len), &mut handler) {
            Ok(read) => read,
            Err(e) => fail!("{}", e),
        };
        if read < offset + len {
            let mut tmp = [0u8, ..1024];
            copy_memory(tmp, buf.slice(read, (offset + len)));
            offset = (offset + len) - read;
            copy_memory(buf, tmp.mut_slice(0, offset));
        } else {
            offset = 0;
        }
        if handler.finished { break }
    }
    println!("{}", handler.status);
    println!("{}", handler.headers);
    println!("{}", match handler.body {
        Some(ref bytes) => match from_utf8(bytes.as_slice()) {
            Some(s) => s,
            None => "(charset != utf-8)",
        },
        None => "(no content body)",
    });
}
