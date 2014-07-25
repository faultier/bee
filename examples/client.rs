extern crate bee;
extern crate url;

use std::collections::HashMap;
use std::io::TcpStream;
use std::io::net;
use std::os;
use std::str::from_utf8;
use url::Url;

use bee::http;
use bee::http::parser::{Parser, ParseResponse, MessageHandler};

pub struct ResponseHandler {
    finished: bool,
    version: Option<http::HttpVersion>,
    status: uint,
    headers: HashMap<String, String>,
    body: Option<String>,
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

    fn on_body(&mut self, _: &Parser, length: uint) {
        {
            let body = if length > 0 {
                let ref st = self.buffer;
                Some(String::from_utf8(st.clone()).unwrap())
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
    write!(stream, "Connection: close\r\n");
    write!(stream, "\r\n");
    let data = match stream.read_to_end() {
        Ok(data) => data,
        Err(e) => fail!("{}", e),
    };
    let mut handler = ResponseHandler::new();
    let mut parser = Parser::new(ParseResponse);
    parser.parse(data.as_slice(), &mut handler);
    println!("{}", handler.headers);
    println!("{}", handler.body);
}
