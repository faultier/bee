use parser::*;
use std::collections::HashMap;
use std::io::{BufReader, InvalidInput, IoResult, standard_error};
use std::str::{SendStr, from_utf8};

#[test]
fn test_no_message() {
    let data = [];
    let mut parser = Parser::new(Request);
    let mut handler = TestHandler::new(true);
    assert_eq!(parser.parse(&mut BufReader::new(data), &mut handler), Ok(0));
    assert!(!handler.started);
    assert!(!handler.finished);
}

mod http_0_9 {
    use super::TestHandler;
    use parser::*;
    use std::io::BufReader;

    #[test]
    fn test_simple_request_get() {
        let msg = "GET /\r\n";
        let data = msg.as_bytes();
        let mut parser = Parser::new(Request);
        let mut handler = TestHandler::new(true);

        assert_eq!(parser.parse(&mut BufReader::new(data), &mut handler), Ok(6));
        assert!(handler.started);
        assert_eq!(handler.url, Some("/".to_string()));
        assert_eq!(parser.get_http_version(), Some(HTTP_0_9));
        assert!(handler.finished);

        // Parser is dead, no more read.
        assert_eq!(parser.parse(&mut BufReader::new(data), &mut handler), Ok(0));
    }
}

mod http_1_0 {
    use parser::*;
    use super::{TestHandler, assert_general_headers, create_request};
    use std::io::BufReader;

    #[test]
    fn test_request_without_header() {
        let msg = "GET / HTTP/1.0\r\n\r\n";
        let data = msg.as_bytes();
        let mut parser = Parser::new(Request);
        let mut handler = TestHandler::new(true);
        assert_eq!(parser.parse(&mut BufReader::new(data), &mut handler), Ok(data.len()));
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
        let mut handler = TestHandler::new(true);
        assert_eq!(parser.parse(&mut BufReader::new(data), &mut handler), Ok(data.len()));
        assert!(!parser.should_keep_alive());
        assert!(handler.started);
        assert_eq!(handler.url, Some("/get".to_string()));
        assert_eq!(parser.get_http_version(), Some(HTTP_1_0));
        assert!(handler.finished);
        assert_general_headers(&handler);
    }

    #[test]
    fn test_request_keep_alive() {
        let msg = create_request("GET", "/keep-alive", 0, Some(vec!("Connection", "keep-alive")), None);
        let data = msg.as_bytes();
        let mut parser = Parser::new(Request);
        let mut handler = TestHandler::new(true);
        assert_eq!(parser.parse(&mut BufReader::new(data), &mut handler), Ok(data.len()));
        assert!(parser.should_keep_alive());
    }

    #[test]
    fn test_response_without_header() {
        let msg = "HTTP/1.0 200 OK\r\n\r\n";
        let data = msg.as_bytes();
        let mut parser = Parser::new(Response);
        let mut handler = TestHandler::new(true);
        assert_eq!(parser.parse(&mut BufReader::new(data), &mut handler), Ok(data.len()));
        assert!(handler.started);
        assert!(handler.finished);
        assert_eq!(parser.get_http_version(), Some(HTTP_1_0));
        assert_eq!(parser.get_status_code(), 200);
    }
}

mod http_1_1 {
    use parser::*;
    use super::{TestHandler, assert_general_headers, create_request};
    use std::io::BufReader;

    #[test]
    fn test_request_get() {
        let msg = create_request("GET", "/tag/Rust", 1, None, None);
        let data = msg.as_bytes();
        let mut parser = Parser::new(Request);
        let mut handler = TestHandler::new(true);
        assert_eq!(parser.parse(&mut BufReader::new(data), &mut handler), Ok(data.len()));
        assert!(parser.should_keep_alive());
        assert!(handler.started);
        assert_eq!(handler.url, Some("/tag/Rust".to_string()));
        assert_eq!(parser.get_http_version(), Some(HTTP_1_1));
        assert!(handler.finished);
        assert_general_headers(&handler);
    }

    #[test]
    fn test_request_close() {
        let msg = create_request("GET", "/close", 1, Some(vec!("Connection", "close")), None);
        let data = msg.as_bytes();
        let mut parser = Parser::new(Request);
        let mut handler = TestHandler::new(true);
        assert_eq!(parser.parse(&mut BufReader::new(data), &mut handler), Ok(data.len()));
        assert!(!parser.should_keep_alive());
    }

    #[test]
    fn test_response_without_header() {
        let msg = "HTTP/1.1 200 OK\r\n\r\n";
        let data = msg.as_bytes();
        let mut parser = Parser::new(Response);
        let mut handler = TestHandler::new(true);
        assert_eq!(parser.parse(&mut BufReader::new(data), &mut handler), Ok(data.len()));
        assert!(handler.started);
        assert!(handler.finished);
        assert_eq!(parser.get_http_version(), Some(HTTP_1_1));
        assert_eq!(parser.get_status_code(), 200);
    }
}

pub struct TestHandler {
    skip_body: bool,
    started: bool,
    url: Option<String>,
    headers_finished: bool,
    headers: HashMap<SendStr, SendStr>,
    finished: bool,
    buffer: Vec<u8>,
}

impl TestHandler {
    fn new(skip_body: bool) -> TestHandler {
        TestHandler {
            skip_body: skip_body,
            started: false,
            url: None,
            headers_finished: false,
            headers: HashMap::new(),
            finished: false,
            buffer: Vec::new(),
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
            self.headers.insert(name.to_string().into_maybe_owned(), value.to_string().into_maybe_owned());
        }
        self.buffer.clear();
        Ok(())
    }

    fn on_headers_complete(&mut self, _: &Parser) -> bool {
        self.headers_finished = true;
        self.skip_body
    }

    fn on_message_complete(&mut self, _: &Parser) {
        self.finished = true;
    }

    fn push_data(&mut self, _: &Parser, byte: &[u8]) {
        self.buffer.push_all(byte);
    }
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
        println!("{}, {}", name, value);
        assert_eq!(handler.headers.find(&name.into_maybe_owned()), Some(&value.into_maybe_owned()));
    }
}

fn create_request(method: &'static str, url: &'static str, version: uint, header: Option<Vec<&'static str>>, body: Option<String>) -> String {
    let mut vec = Vec::new();
    let nl = "\r\n".to_string();
    vec.push(format!("{} {} HTTP/1.{}", method, url, version));
    for win in general_headers().as_slice().chunks(2) {
        vec.push(format!("{}: {}", win[0], win[1]));
    }
    if header.is_some() {
        for win in header.unwrap().as_slice().chunks(2) {
            vec.push(format!("{}: {}", win[0], win[1]));
        }
    }
    vec.push(nl.clone());
    if body.is_some() {
        vec.push(body.unwrap());
        vec.push(nl.clone());
    }
    vec.connect(nl.as_slice())
}
