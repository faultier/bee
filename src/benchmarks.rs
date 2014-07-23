use parser::*;
use test::Bencher;
use std::io::BufReader;

#[bench]
fn bench_no_message(b: &mut Bencher) {
    let buf: &[u8] = [];
    let mut handler = BenchHandler { skip_body: true };
    b.iter(|| Parser::new(Request).parse(&mut BufReader::new(buf), &mut handler) );
}

mod http_0_9 {
    use parser::*;
    use super::BenchHandler;
    use test::Bencher;
    use std::io::BufReader;

    #[bench]
    fn bench_simple_request_get(b: &mut Bencher) {
        let msg = "GET /\r\n";
        let data = msg.as_bytes();
        let mut handler = BenchHandler { skip_body: true };
        b.iter(|| Parser::new(Request).parse(&mut BufReader::new(data), &mut handler) );
    }
}

mod http_1_0 {
    use parser::*;
    use super::{BenchHandler, create_request};
    use test::Bencher;
    use std::io::BufReader;

    #[bench]
    fn bench_request_without_header(b: &mut Bencher) {
        let msg = "GET / HTTP/1.0\r\n\r\n";
        let data = msg.as_bytes();
        let mut handler = BenchHandler { skip_body: true };
        b.iter(|| Parser::new(Request).parse(&mut BufReader::new(data), &mut handler) );
    }

    #[bench]
    fn bench_request_get(b: &mut Bencher) {
        let msg = create_request("GET", "/path/to/some/contents", 0, None, None);
        let data = msg.as_bytes();
        let mut handler = BenchHandler { skip_body: true };
        b.iter(|| Parser::new(Request).parse(&mut BufReader::new(data), &mut handler) );
    }
}

mod http_1_1 {
    use parser::*;
    use super::{BenchHandler, create_request};
    use test::Bencher;
    use std::io::BufReader;

    #[bench]
    fn bench_request_get(b: &mut Bencher) {
        let msg = create_request("GET", "/path/to/some/contents", 1, None, None);
        let data = msg.as_bytes();
        let mut handler = BenchHandler { skip_body: true };
        b.iter(|| Parser::new(Request).parse(&mut BufReader::new(data), &mut handler) );
    }
}

struct BenchHandler {
    skip_body: bool
}

impl Handler for BenchHandler {
    fn on_headers_complete(&mut self, _: &Parser) -> bool { self.skip_body }
    fn push_data(&mut self, _: &Parser, _: &[u8]) { /* ignore */ }
}

fn general_headers() -> Vec<&'static str> {
    vec!("Host", "faultier.jp",
         "User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.9; rv:30.0) Gecko/20100101 Firefox/30.0",
         "Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
         "Accept-Encoding", "gzip,deflate",
         "Accept-Language", "ja,en-US;q=0.8,en;q=0.6",
         "Cache-Control", "max-age=0",
         "Cookie", "key1=value1; key2=value2; key3=value3; key4=value4; key5=value5",
         "Referer", "http://faultier.blog.jp/")
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
