extern crate bee;

use std::collections::HashMap;
use std::io::{Acceptor, Listener, TcpListener, TcpStream};
use std::os;
use std::str::from_utf8;
use std::slice::bytes::copy_memory;

use bee::http;
use bee::http::parser::{Parser, ParseRequest, MessageHandler};

static HTML: &'static str = "<!DOCTYPE html>\n<html><body><h1>Hello, HTTP world!</h1><form action=\"/post\" method=\"post\"><input type=\"text\" name=\"name\" placeholder=\"Your Name\" /><input type=\"password\" name=\"password\" /><input type=\"submit\" /></body></html>\n";

pub struct RequestHandler {
    finished: bool,
    version: Option<http::HttpVersion>,
    method: Option<http::HttpMethod>,
    url: Option<String>,
    headers: HashMap<String, String>,
    body: Option<String>,
    buffer: Vec<u8>,
}

impl RequestHandler {
    fn new() -> RequestHandler {
        RequestHandler {
            finished: false,
            version: None,
            method: None,
            url: None,
            headers: HashMap::new(),
            buffer: Vec::new(),
            body: None,
        }
    }
}

impl MessageHandler for RequestHandler {
    fn on_version(&mut self, _: &Parser, version: http::HttpVersion) {
        self.version = Some(version);
    }

    fn on_method(&mut self, _: &Parser, method: http::HttpMethod) {
        self.method = Some(method);
    }

    fn on_url(&mut self, _: &Parser, length: uint) {
        {
            self.url = match from_utf8(self.buffer.slice_to(length)) {
                Some(url) => Some(url.to_string()),
                None => return,
            };
        }
        self.buffer.clear();
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

    fn on_message_complete(&mut self, _: &Parser) {
        self.finished = true;
    }

    fn write(&mut self, _: &Parser, byte: &[u8]) {
        self.buffer.push_all(byte);
    }
}

#[allow(unused_must_use)]
fn handle_req(mut stream: TcpStream) {
    let mut buf = [0u8, ..1024];
    let mut parser = Parser::new(ParseRequest);
    let mut request = RequestHandler::new();
    let mut offset = 0u;
    loop {
        let len = match stream.read(buf.mut_slice_from(offset)) {
            Ok(len) => len,
            Err(e) => fail!("{}", e),
        };
        if len == 0 { continue }
        let read = match parser.parse(buf.slice_to(offset+len), &mut request) {
            Ok(read) => read,
            Err(e) => {
                println!("{}", ::std::str::from_utf8(buf.slice_to(offset+len)));
                let data = format!("{}", e);
                write!(stream, "HTTP/1.1 400 Bad Request\r\n");
                write!(stream, "Content-Type: text/plain\r\n");
                write!(stream, "Server: bee\r\n");
                write!(stream, "Content-Length: {}\r\n", data.as_bytes().len());
                write!(stream, "Connection: close\r\n");
                write!(stream, "\r\n");
                write!(stream, "{}", data);
                break;
            },
        };
        if read < offset + len {
            let mut tmp = [0u8, ..1024];
            copy_memory(tmp, buf.slice(read, (offset + len)));
            offset = (offset + len) - read;
            copy_memory(buf.mut_slice(0, offset), tmp);
        } else {
            offset = 0;
        }
        if request.finished {
            if request.version.is_none() {
                write!(stream, "What's!? HTTP 0.9!?\n");
                break;
            }
            if parser.should_upgrade() {
                handle_error(&mut stream);
                break;
            }
            match request.url {
                Some(ref url) => match url.as_slice() {
                    "/"     => handle_index(&request, &mut stream, parser.should_keep_alive()),
                    "/post" => handle_post(&request, &mut stream, parser.should_keep_alive()),
                    "*"     => handle_options(&request, &mut stream, parser.should_keep_alive()),
                    _ => {
                        let data = "Not Found";
                        write!(stream, "{} 404 Not Found\r\n", request.version.unwrap());
                        write!(stream, "Content-Type: text/plain\r\n");
                        write!(stream, "Server: bee\r\n");
                        write!(stream, "Content-Length: {}\r\n", data.as_bytes().len());
                        write!(stream, "Connection: {}\r\n", if parser.should_keep_alive() { "keep-alive" } else { "close" });
                        write!(stream, "\r\n");
                        write!(stream, "{}", data);
                    }
                },
                None => { handle_error(&mut stream); break },
            }
            if !parser.should_keep_alive() {
                break;
            }
            request = RequestHandler::new();
        }
    }
    stream.close_read();
}

#[allow(unused_must_use)]
fn handle_index(request: &RequestHandler, stream: &mut TcpStream, keep_alive: bool) {
    match request.method {
        Some(http::HttpGet) => {
            write!(stream, "{} 200 OK\r\n", request.version.unwrap());
            write!(stream, "Server: bee\r\n");
            write!(stream, "Cache-Control: no-cache\r\n");
            write!(stream, "Content-Type: text/html\r\n");
            write!(stream, "Content-Length: {}\r\n", HTML.as_bytes().len());
            write!(stream, "Connection: {}\r\n", if keep_alive { "keep-alive" } else { "close" });
            write!(stream, "\r\n");
            write!(stream, "{}", HTML);
        }
        Some(http::HttpHead) => {
            write!(stream, "{} 200 OK\r\n", request.version.unwrap());
            write!(stream, "Server: bee\r\n");
            write!(stream, "Cache-Control: no-cache\r\n");
            write!(stream, "Content-Type: text/html\r\n");
            write!(stream, "Content-Length: {}\r\n", HTML.as_bytes().len());
            write!(stream, "Connection: {}\r\n", if keep_alive { "keep-alive" } else { "close" });
            write!(stream, "\r\n");
        }
        Some(http::HttpOptions) => {
            write!(stream, "{} 200 OK\r\n", request.version.unwrap());
            write!(stream, "Server: bee\r\n");
            write!(stream, "Allow: GET,HEAD,OPTIONS\r\n");
            write!(stream, "Content-Length: 0\r\n");
            write!(stream, "Connection: {}\r\n", if keep_alive { "keep-alive" } else { "close" });
            write!(stream, "\r\n");
        }
        _ => {
            let data = "Method Not Allowed";
            write!(stream, "{} 405 Method Not Allowed\r\n", request.version.unwrap());
            write!(stream, "Content-Type: text/plain\r\n");
            write!(stream, "Server: bee\r\n");
            write!(stream, "Content-Length: {}\r\n", data.as_bytes().len());
            write!(stream, "Connection: {}\r\n", if keep_alive { "keep-alive" } else { "close" });
            write!(stream, "\r\n");
            write!(stream, "{}", data);
        },
    }
}

#[allow(unused_must_use)]
fn handle_post(request: &RequestHandler, stream: &mut TcpStream, keep_alive: bool) {
    match request.method {
        Some(http::HttpPost) => {
            let data = format!("<!DOCTYPE html>\n<html><body><h1>Form data</h1><pre>{}</pre></body></html>\n", request.body);
            write!(stream, "{} 200 OK\r\n", request.version.unwrap());
            write!(stream, "Server: bee\r\n");
            write!(stream, "Cache-Control: no-cache\r\n");
            write!(stream, "Content-Type: text/html\r\n");
            write!(stream, "Content-Length: {}\r\n", data.as_bytes().len());
            write!(stream, "Connection: {}\r\n", if keep_alive { "keep-alive" } else { "close" });
            write!(stream, "\r\n");
            write!(stream, "{}", data);
        }
        Some(http::HttpOptions) => {
            write!(stream, "{} 200 OK\r\n", request.version.unwrap());
            write!(stream, "Server: bee\r\n");
            write!(stream, "Allow: POST,OPTIONS\r\n");
            write!(stream, "Content-Length: 0\r\n");
            write!(stream, "Connection: {}\r\n", if keep_alive { "keep-alive" } else { "close" });
            write!(stream, "\r\n");
        }
        _ => {
            let data = "Method Not Allowed";
            write!(stream, "{} 405 Method Not Allowed\r\n", request.version.unwrap());
            write!(stream, "Content-Type: text/plain\r\n");
            write!(stream, "Server: bee\r\n");
            write!(stream, "Content-Length: {}\r\n", data.as_bytes().len());
            write!(stream, "Connection: {}\r\n", if keep_alive { "keep-alive" } else { "close" });
            write!(stream, "\r\n");
            write!(stream, "{}", data);
        },
    }
}

#[allow(unused_must_use)]
fn handle_options(request: &RequestHandler, stream: &mut TcpStream, keep_alive: bool) {
    match request.method {
        Some(http::HttpOptions) => {
            write!(stream, "{} 200 OK\r\n", request.version.unwrap());
            write!(stream, "Server: bee\r\n");
            write!(stream, "Allow: GET,HEAD,POST,OPTIONS\r\n");
            write!(stream, "Content-Length: 0\r\n");
            write!(stream, "Connection: {}\r\n", if keep_alive { "keep-alive" } else { "close" });
            write!(stream, "\r\n");
        }
        _ => {
            let data = "Bad Request";
            write!(stream, "{} 400 Bad Request\r\n", request.version.unwrap());
            write!(stream, "Content-Type: text/plain\r\n");
            write!(stream, "Server: bee\r\n");
            write!(stream, "Content-Length: {}\r\n", data.as_bytes().len());
            write!(stream, "Connection: {}\r\n", if keep_alive { "keep-alive" } else { "close" });
            write!(stream, "\r\n");
            write!(stream, "{}", data);
        }
    }
}

#[allow(unused_must_use)]
fn handle_error(stream: &mut TcpStream) {
    let data = "Bad Request";
    write!(stream, "HTTP/1.0 400 Bad Request\r\n");
    write!(stream, "Content-Type: text/plain\r\n");
    write!(stream, "Server: bee\r\n");
    write!(stream, "Content-Length: {}\r\n", data.as_bytes().len());
    write!(stream, "Connection: close\r\n");
    write!(stream, "\r\n");
    write!(stream, "{}", data);
}

fn main() {
    let mut args: Vec<String> = os::args();

    let program = args.shift();
    if args.len() < 2 {
        println!("usage: {} <host> <port>", program);
        os::set_exit_status(1);
        return;
    }

    let host = args.shift().unwrap();
    let port = args.shift().unwrap();
    let listener = TcpListener::bind(host.as_slice(), from_str::<u16>(port.as_slice()).unwrap());
    let mut acceptor = listener.listen();

    for stream in acceptor.incoming() {
        match stream {
            Err(e) => fail!("{}", e),
            Ok(stream) => spawn(proc() {
                handle_req(stream)
            })
        }
    }

    drop(acceptor);
}
