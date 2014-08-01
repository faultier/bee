Bee HTTP Parser
========
[![Build Status](https://travis-ci.org/faultier/bee.svg?branch=master)](https://travis-ci.org/faultier/bee)

This is a parser for HTTP messages written in Rust. It parses both requests and responses,
extracts the following information from HTTP message.

- Request method
- Request URL
- Response status code
- HTTP version
- Header fields and values
- Content-Length
- Transfer-Encoding
- Message body (Decodes chunked encoding)

This library is inspired by [http-parser](https://github.com/joyent/http-parser) written in C. Thanks to the original authors.

# Usage

```rust
extern crate bee;

use std::collections::HashMap;
use std::str::from_utf8;
use bee::http::{HttpMethod, HttpVersion};
use bee::http::parser::{Parser, ParseRequest, MessageHandler};

struct RequestHandler {
    version: Option<HttpVersion>,
    method: Option<HttpMethod>,
    url: Option<String>,
    headers: HashMap<String, String>,
    buffer: Vec<u8>,
}

impl MessageHandler for RequestHandler {
    fn on_method(&mut self, _: &Parser, method: HttpMethod) {
        self.method = Some(method);
    }

    fn on_url(&mut self, _: &Parser, length: uint) {
        self.url = match from_utf8(self.buffer.slice_to(length)) {
            Some(url) => Some(url.to_string()),
            None => None,
        };
        self.buffer.clear();
    }

    fn on_version(&mut self, _: &Parser, version: HttpVersion) {
        self.version = Some(version);
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

    fn write(&mut self, _: &Parser, byte: &[u8]) {
        self.buffer.push_all(byte);
    }
}

fn main() {
    let data = "GET / HTTP/1.1\r\nHost: example.com\r\n\r\n".as_bytes();
    let mut handler = RequestHandler {
        version: None,
        url: None,
        method: None,
        headers: HashMap::new(),
        buffer: Vec::new(),
    };
    let mut parser = Parser::new(ParseRequest);
    let parsed = parser.parse(data, &mut handler);
    println!("{} {} {}", handler.method, handler.url, handler.version);
}
```


