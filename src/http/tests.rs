use http::*;

use std::collections::HashMap;
use std::io::{InvalidInput, IoResult, standard_error};
use std::str::from_utf8;
use test::Bencher;

#[test]
fn test_no_message() {
    let mut parser = Parser::new(Request);
    let mut handler = TestHandler::new();
    assert_eq!(parser.parse([], &mut handler), Ok(0));
    assert!(!handler.started);
    assert!(!handler.finished);
}

#[bench]
fn bench_no_message(b: &mut Bencher) {
    b.iter(|| Parser::new(Request).parse([], &mut BenchHandler) );
}

mod http_0_9 {
    use http::*;
    use super::{BenchHandler, TestHandler};
    use test::Bencher;

    #[test]
    fn test_request_get() {
        let msg = "GET /\r\n";
        let data = msg.as_bytes();
        let mut parser = Parser::new(Request);
        let mut handler = TestHandler::new();

        assert_eq!(parser.parse(data, &mut handler), Ok(6));
        assert!(handler.started);
        assert_eq!(handler.url, Some("/".to_string()));
        assert_eq!(parser.get_http_version(), Some(HTTP_0_9));
        assert!(handler.finished);

        // Parser is dead, no more read.
        assert_eq!(parser.parse(data, &mut handler), Ok(0));
    }

    #[bench]
    fn bench_request_get(b: &mut Bencher) {
        let msg = "GET /\r\n";
        let data = msg.as_bytes();
        b.iter(|| Parser::new(Request).parse(data, &mut BenchHandler) );
    }
}

mod http_1_0 {
    use http::*;
    use super::{BenchHandler, TestHandler, assert_general_headers, create_request, create_response};
    use test::Bencher;

    #[test]
    fn test_request_without_header() {
        let msg = "GET / HTTP/1.0\r\n\r\n";
        let data = msg.as_bytes();
        let mut parser = Parser::new(Request);
        let mut handler = TestHandler::new();
        assert_eq!(parser.parse(data, &mut handler), Ok(data.len()));
        assert!(handler.started);
        assert_eq!(handler.url, Some("/".to_string()));
        assert_eq!(parser.get_http_version(), Some(HTTP_1_0));
        assert!(handler.headers_finished);
        assert!(handler.finished);
    }

    #[test]
    fn test_request_get() {
        let msg = create_request("GET", "/get", 0, None, None);
        let data = msg.as_bytes();
        let mut parser = Parser::new(Request);
        let mut handler = TestHandler::new();
        assert_eq!(parser.parse(data, &mut handler), Ok(data.len()));
        assert_eq!(parser.get_http_version(), Some(HTTP_1_0));
        assert!(!parser.should_keep_alive());
        assert!(handler.started);
        assert!(handler.finished);
        assert_eq!(handler.url, Some("/get".to_string()));
        assert_general_headers(&handler);
    }

    #[test]
    fn test_request_keep_alive() {
        let msg = create_request("GET", "/keep-alive", 0, Some(vec!("Connection", "keep-alive")), None);
        let data = msg.as_bytes();
        let mut parser = Parser::new(Request);
        let mut handler = TestHandler::new();
        assert_eq!(parser.parse(data, &mut handler), Ok(data.len()));
        assert!(parser.should_keep_alive());
    }

    #[test]
    fn test_response_without_header() {
        let msg = "HTTP/1.0 304 Not Modified\r\n\r\n";
        let data = msg.as_bytes();
        let mut parser = Parser::new(Response);
        let mut handler = TestHandler::new();
        assert_eq!(parser.parse(data, &mut handler), Ok(data.len()));
        assert!(handler.started);
        assert!(handler.finished);
        assert_eq!(parser.get_http_version(), Some(HTTP_1_0));
        assert_eq!(parser.get_status_code(), 304);
    }

    #[test]
    fn test_response_200() {
        let msg = create_response(0, "200 OK", Some(vec!("Content-Type", "text/plain")), Some("Hello, HTTP world!"));
        let data = msg.as_bytes();
        let mut parser = Parser::new(Response);
        let mut handler = TestHandler::new();
        assert_eq!(parser.parse(data, &mut handler), Ok(data.len()));
        assert!(handler.started);
        assert!(handler.finished);
        assert_eq!(parser.get_http_version(), Some(HTTP_1_0));
        assert_eq!(parser.get_status_code(), 200);
        assert_eq!(handler.body, Some("Hello, HTTP world!".to_string()));
    }

    #[bench]
    fn bench_request_get(b: &mut Bencher) {
        let msg = create_request("GET", "/path/to/some/contents", 0, None, None);
        let data = msg.as_bytes();
        b.iter(|| Parser::new(Request).parse(data, &mut BenchHandler) );
    }

    #[bench]
    fn bench_response_200(b: &mut Bencher) {
        let msg = create_response(0, "200 OK", Some(vec!("Content-Type", "text/plain")), Some("Hello, HTTP world!"));
        let data = msg.as_bytes();
        b.iter(|| Parser::new(Response).parse(data, &mut BenchHandler) );
    }
}

mod http_1_1 {
    use http::*;
    use super::{BenchHandler, TestHandler, assert_general_headers, create_request, create_response};
    use test::Bencher;

    #[test]
    fn test_request_get() {
        let msg = create_request("GET", "/get", 1, None, None);
        let data = msg.as_bytes();
        let mut parser = Parser::new(Request);
        let mut handler = TestHandler::new();
        assert_eq!(parser.parse(data, &mut handler), Ok(data.len()));
        assert!(parser.should_keep_alive());
        assert!(handler.started);
        assert_eq!(handler.url, Some("/get".to_string()));
        assert_eq!(parser.get_http_version(), Some(HTTP_1_1));
        assert!(handler.finished);
        assert_general_headers(&handler);
    }

    #[test]
    fn test_request_close() {
        let msg = create_request("GET", "/close", 1, Some(vec!("Connection", "close")), None);
        let data = msg.as_bytes();
        let mut parser = Parser::new(Request);
        let mut handler = TestHandler::new();
        assert_eq!(parser.parse(data, &mut handler), Ok(data.len()));
        assert!(!parser.should_keep_alive());
    }

    #[test]
    fn test_response_without_header() {
        let msg = "HTTP/1.1 304 Not Modified\r\n\r\n";
        let data = msg.as_bytes();
        let mut parser = Parser::new(Response);
        let mut handler = TestHandler::new();
        assert_eq!(parser.parse(data, &mut handler), Ok(data.len()));
        assert!(handler.started);
        assert!(handler.finished);
        assert_eq!(parser.get_http_version(), Some(HTTP_1_1));
        assert_eq!(parser.get_status_code(), 304);
    }

    #[test]
    fn test_response_200() {
        let msg = create_response(1, "200 OK", Some(vec!("Content-Type", "text/plain")), Some("Hello, HTTP world!"));
        let data = msg.as_bytes();
        let mut parser = Parser::new(Response);
        let mut handler = TestHandler::new();
        assert_eq!(parser.parse(data, &mut handler), Ok(data.len()));
        assert!(handler.started);
        assert!(handler.finished);
        assert_eq!(parser.get_http_version(), Some(HTTP_1_1));
        assert_eq!(parser.get_status_code(), 200);
        assert_eq!(handler.body, Some("Hello, HTTP world!".to_string()));
    }

    #[bench]
    fn bench_request_get(b: &mut Bencher) {
        let msg = create_request("GET", "/path/to/some/contents", 1, None, None);
        let data = msg.as_bytes();
        b.iter(|| Parser::new(Request).parse(data, &mut BenchHandler) );
    }

    #[bench]
    fn bench_response_200(b: &mut Bencher) {
        let msg = create_response(1, "200 OK", Some(vec!("Content-Type", "text/plain")), Some("Hello, HTTP world!"));
        let data = msg.as_bytes();
        b.iter(|| Parser::new(Response).parse(data, &mut BenchHandler) );
    }
}

pub struct TestHandler {
    started: bool,
    url: Option<String>,
    headers_finished: bool,
    headers: HashMap<String, String>,
    body: Option<String>,
    finished: bool,
    buffer: Vec<u8>,
}

impl TestHandler {
    fn new() -> TestHandler {
        TestHandler {
            started: false,
            url: None,
            headers_finished: false,
            headers: HashMap::new(),
            finished: false,
            buffer: Vec::new(),
            body: None,
        }
    }
}

impl Handler for TestHandler {
    fn on_message_begin(&mut self, _: &Parser) {
        self.started = true;
    }

    fn on_url(&mut self, _: &Parser, length: uint) -> IoResult<()> {
        {
            self.url = match from_utf8(self.buffer.slice_to(length)) {
                Some(url) => Some(url.to_string()),
                None => return Err(standard_error(InvalidInput)),
            };
        }
        self.buffer.clear();
        Ok(())
    }

    fn on_header_value(&mut self, _: &Parser, length: uint) -> IoResult<()> {
        {
            let len = self.buffer.len();
            let name = {
                let slice = self.buffer.slice_to(len-length);
                match from_utf8(slice) {
                    Some(s) => s.clone(),
                    None => return Err(standard_error(InvalidInput)),
                }
            };
            let value = {
                let slice = self.buffer.slice_from(len-length);
                match from_utf8(slice) {
                    Some(s) => s.clone(),
                    None => return Err(standard_error(InvalidInput)),
                }
            };
            self.headers.insert(name.to_string(), value.to_string());
        }
        self.buffer.clear();
        Ok(())
    }

    fn on_headers_complete(&mut self, _: &Parser) -> bool {
        self.headers_finished = true;
        return false;
    }

    fn on_body(&mut self, _: &Parser, length: uint) -> IoResult<()> {
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
        Ok(())
    }

    fn on_message_complete(&mut self, _: &Parser) {
        self.finished = true;
    }

    fn push_data(&mut self, _: &Parser, byte: u8) {
        self.buffer.push(byte);
    }

    fn push_data_all(&mut self, _: &Parser, byte: &[u8]) {
        self.buffer.push_all(byte);
    }
}

struct BenchHandler;

impl Handler for BenchHandler {
    fn push_data(&mut self, _: &Parser, _: u8) { /* ignore */ }
}

fn general_headers() -> Vec<&'static str> {
    vec!("Host", "faultier.jp",
         "User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.9; rv:30.0) Gecko/20100101 Firefox/30.0",
         "Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
         "Accept-Encoding", "gzip,deflate",
         "Accept-Language", "ja,en-US;q=0.8,en;q=0.6",
         "Cache-Control", "max-age=0",
         "Cookie", "key1=value1; key2=value2",
         "Referer", "http://faultier.blog.jp/")
}

fn assert_general_headers(handler: &TestHandler) {
    assert!(handler.headers_finished);
    for chunk in general_headers().as_slice().chunks(2) {
        let (name, value) = (chunk[0], chunk[1]);
        assert_eq!(handler.headers.find(&name.to_string()), Some(&value.to_string()));
    }
}

fn create_request(method: &'static str, url: &'static str, version: uint, header: Option<Vec<&'static str>>, body: Option<&'static str>) -> String {
    let mut vec = Vec::new();
    let mbody = if body.is_some() { body.unwrap() } else { "" };
    vec.push(format!("{} {} HTTP/1.{}", method, url, version));
    for win in general_headers().as_slice().chunks(2) {
        vec.push(format!("{}: {}", win[0], win[1]));
    }
    if header.is_some() {
        for win in header.unwrap().as_slice().chunks(2) {
            vec.push(format!("{}: {}", win[0], win[1]));
        }
    }
    if mbody.len() > 0 { vec.push(format!("Content-Length: {}", mbody.as_bytes().len())) }
    vec.push("".to_string());
    vec.push(mbody.to_string());
    vec.connect( "\r\n")
}

fn create_response(version: uint, status: &'static str, header: Option<Vec<&'static str>>, body: Option<&'static str>) -> String {
    let mut vec = Vec::new();
    let mbody = if body.is_some() { body.unwrap() } else { "" };
    vec.push(format!("HTTP/1.{} {}", version, status));
    for win in general_headers().as_slice().chunks(2) {
        vec.push(format!("{}: {}", win[0], win[1]));
    }
    if header.is_some() {
        for win in header.unwrap().as_slice().chunks(2) {
            vec.push(format!("{}: {}", win[0], win[1]));
        }
    }
    if mbody.len() > 0 { vec.push(format!("Content-Length: {}", mbody.as_bytes().len())) }
    vec.push("".to_string());
    vec.push(mbody.to_string());
    vec.connect("\r\n")
}
