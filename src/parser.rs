//! HTTP parser.

#![experimental]

use std::fmt::{Formatter, FormatError, Show};
use std::io::{EndOfFile, IoError, IoResult};
use UINT_MAX = std::uint::MAX;

#[deriving(PartialEq, Eq, Clone, Show)]
/// A parser types.
pub enum Type {
    /// Parse request.
    Request,
    /// Parse response.
    Response,
    /// Parse request or response.
    Both,
}

/// A list of supported HTTP versions.
#[allow(non_camel_case_types)]
#[deriving(PartialEq, Eq, Clone)]
pub enum HttpVersion {
    /// HTTP/0.9
    HTTP_0_9,
    /// HTTP/1.0
    HTTP_1_0,
    /// HTTP/1.1
    HTTP_1_1,
}

impl HttpVersion {
    /// Detect HTTP version with major and minor.
    pub fn find(major: uint, minor: uint) -> Option<HttpVersion> {
        match major {
            0 if minor == 9 => Some(HTTP_0_9),
            1 => match minor {
                0 => Some(HTTP_1_0),
                1 => Some(HTTP_1_1),
                _ => None,
            },
            _ => None,
        }
    }
}

impl Show for HttpVersion {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
        match *self {
            HTTP_0_9 => f.pad("HTTP/0.9"),
            HTTP_1_0 => f.pad("HTTP/1.0"),
            HTTP_1_1 => f.pad("HTTP/1.1"),
        }
    }
}

#[allow(missing_doc)]
#[deriving(PartialEq, Eq, Clone)]
pub enum HttpMethod {
    HttpCheckout,
    HttpConnect,
    HttpCopy,
    HttpDelete,
    HttpGet,
    HttpHead,
    HttpLink,
    HttpLock,
    HttpMerge,
    HttpMkActivity,
    HttpMkCalendar,
    HttpMkCol,
    HttpMove,
    HttpMsearch,
    HttpNotify,
    HttpOptions,
    HttpPatch,
    HttpPost,
    HttpPropFind,
    HttpPropPatch,
    HttpPurge,
    HttpPut,
    HttpReport,
    HttpSearch,
    HttpSubscribe,
    HttpTrace,
    HttpUnlink,
    HttpUnlock,
    HttpUnsubscribe,
}

impl HttpMethod {
    #[inline]
    fn name(&self) -> &'static str {
        match *self {
            HttpCheckout    => "CHECKOUT",
            HttpConnect     => "CONNECT",
            HttpCopy        => "COPY",
            HttpDelete      => "DELETE",
            HttpGet         => "GET",
            HttpHead        => "HEAD",
            HttpLink        => "LINK",
            HttpLock        => "LOCK",
            HttpMerge       => "MERGE",
            HttpMkActivity  => "MKACTIVITY",
            HttpMkCalendar  => "MKCALENDAR",
            HttpMkCol       => "MKCOL",
            HttpMove        => "MOVE",
            HttpMsearch     => "M-SEARCH",
            HttpNotify      => "NOTIFY",
            HttpOptions     => "OPTIONS",
            HttpPatch       => "PATCH",
            HttpPost        => "POST",
            HttpPropFind    => "PROPFIND",
            HttpPropPatch   => "PROPPATCH",
            HttpPut         => "PUT",
            HttpPurge       => "PURGE",
            HttpReport      => "REPORT",
            HttpSearch      => "SEARCH",
            HttpSubscribe   => "SUBSCRIBE",
            HttpTrace       => "TRACE",
            HttpUnlink      => "UNLINK",
            HttpUnlock      => "UNLOCK",
            HttpUnsubscribe => "UNSUBSCRIBE",
        }
    }

    #[inline]
    fn len(&self) -> uint {
        self.name().len()
    }

    #[inline]
    fn hit(&self, pos: uint, c: char) -> bool {
        self.name().char_at(pos) == c
    }
}

impl Show for HttpMethod {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
        f.pad(self.name())
    }
}

/// Parser callbacks.
pub trait Callbacks {
    /// Called when start to parsing of message.
    /// Default implementation is nothing to do.
    fn on_message_begin(&mut self, _: &Parser) {
    }

    /// Called when url parsed.
    /// Default implementation is nothing to do.
    fn on_url<'a>(&mut self, _: &Parser, _: &'a [u8]) -> IoResult<()> {
        Ok(())
    }

    /// Called when status parsed.
    /// Default implementation is nothing to do.
    fn on_status<'a>(&mut self, _: &Parser, _: &'a [u8]) -> IoResult<()> {
        Ok(())
    }

    /// Called when header field's name parsed.
    /// Default implementation is nothing to do.
    fn on_header_field<'a>(&mut self, _: &Parser, _: &'a [u8], _: &'a [u8]) -> IoResult<()> {
        Ok(())
    }

    /// Called when completed to parsing of headers.
    /// Default implementation is nothing to do.
    fn on_headers_complete(&mut self, _: &Parser) -> bool{
        return false;
    }

    /// Called when body parsed.
    /// Default implementation is nothing to do.
    fn on_body<'a>(&mut self, _: &Parser, _: &'a [u8]) -> IoResult<()> {
        Ok(())
    }

    /// Called when completed to parsing of whole message.
    /// Default implementation is nothing to do.
    fn on_message_complete(&mut self, _: &Parser) {
    }
}

/// A list specifying categories of parse errors.
#[deriving(PartialEq, Eq, Clone, Show)]
pub enum ParseError {
    /// Any parse error not part of this list.
    OtherParseError,
    /// Invalid HTTP method.
    InvalidMethod,
    /// Invalid URL.
    InvalidUrl,
    /// Invalid HTTP version.
    InvalidVersion,
    /// Invalid request line.
    InvalidRequestLine,
    /// `on_url` callback failed.
    UrlCallbackFail(IoError),
    /// Invalid header field.
    InvalidHeaderField,
    /// `on_header_field` callback failed.
    HeaderFieldCallbackFail(IoError),
    /// Invalid header section.
    InvalidHeaders,
    /// Expected data, but reached EOF.
    InvalidEOFState,
}

pub type ParseResult = Result<uint, ParseError>;

static CR: char = '\r';
static LF: char = '\n';

/// HTTP parser.
pub struct Parser {
    parser_type: Type,
    http_version: Option<HttpVersion>,
    state: ParserState,
    method: Option<HttpMethod>,
    status_code: Option<uint>,
    content_length: uint,
    skip_body: bool,
}

impl Parser {
    /// Create a new `Parser`.
    pub fn new(t: Type) -> Parser {
        Parser {
            parser_type: t,
            http_version: None,
            state: match t {
                Request  => StartReq,
                Response => StartRes,
                Both     => StartReqOrRes,
            },
            method: None,
            status_code: None,
            content_length: UINT_MAX,
            skip_body: false,
        }
    }

    /// Parse HTTP message.
    pub fn parse<R: Reader, C: Callbacks>(&mut self, reader: &mut R, callbacks: &mut C) -> ParseResult {
        if self.state == Dead { return Ok(0) }
        if self.state == Crashed { return Err(OtherParseError) }

        let mut read = 0u;
        let mut pos  = 0u;
        let mut rbuf = [0u8, ..6];
        let mut buf1: Vec<u8> = Vec::with_capacity(256);
        let mut buf2: Vec<u8> = Vec::with_capacity(256);

        let mut major = 0u;
        let mut minor = 0u;

        'read: loop {
            match self.state {
                StartReq => {
                    read += match reader.read(rbuf.mut_slice(0, 3)) {
                        Ok(len) if len == 3 => len,
                        Err(IoError { kind: EndOfFile, ..}) => return Ok(0), // no message
                        _ => return Err(InvalidMethod),
                    };
                    self.method = Some(match (rbuf[0] as char, rbuf[1] as char, rbuf[2] as char) {
                        ('C', 'H', 'E') => HttpCheckout, 
                        ('C', 'O', 'N') => HttpConnect,
                        ('C', 'O', 'P') => HttpCopy,
                        ('D', 'E', 'L') => HttpDelete,
                        ('G', 'E', 'T') => HttpGet,
                        ('H', 'E', 'A') => HttpHead,
                        ('L', 'I', 'N') => HttpLink,
                        ('L', 'O', 'K') => HttpLock,
                        ('M', '-', 'S') => HttpMsearch,
                        ('M', 'E', 'R') => HttpMerge,
                        ('M', 'K', 'A') => HttpMkActivity,
                        ('M', 'K', 'C') => HttpMkCalendar,  // or MKCOL
                        ('N', 'O', 'T') => HttpNotify,
                        ('O', 'P', 'T') => HttpOptions,
                        ('P', 'A', 'T') => HttpPatch,
                        ('P', 'O', 'S') => HttpPost,
                        ('P', 'R', 'O') => HttpPropFind,    // or PROPPATCH
                        ('P', 'U', 'R') => HttpPurge,
                        ('P', 'U', 'T') => HttpPut,
                        ('R', 'E', 'P') => HttpReport,
                        ('S', 'E', 'A') => HttpSearch,
                        ('S', 'U', 'B') => HttpSubscribe,
                        ('T', 'R', 'A') => HttpTrace,
                        ('U', 'N', 'L') => HttpUnlink,      // or UNLOCK
                        ('U', 'N', 'S') => HttpUnsubscribe,
                        _   => {
                            self.state = Crashed;
                            return Err(InvalidMethod);
                        }
                    });
                    pos += 3;
                    self.state = ReqMethod;
                    callbacks.on_message_begin(self);
                }
                ReqMethod => {
                    let method = self.method.unwrap();
                    read += match reader.read(rbuf.mut_slice(0, 1)) {
                        Ok(len) if len == 1 => len,
                        _ => return Err(InvalidMethod),
                    };
                    if rbuf[0] as char == ' ' && pos == method.len() {
                        self.state = ReqUrl;
                        pos = 0;
                        continue;
                    }
                    if !method.hit(pos, rbuf[0] as char) {
                        self.method = Some(match method {
                            HttpMkCalendar if pos == 3 && rbuf[0] as char == 'O' => HttpMkCol,
                            HttpPropFind if pos == 4 && rbuf[0] as char == 'P'  => HttpPropPatch,
                            HttpUnlink if pos == 3 && rbuf[0] as char == 'O' => HttpUnlock,
                            _ => {
                                self.state = Crashed;
                                return Err(InvalidMethod);
                            }
                        });
                    }
                    pos += 1;
                }
                ReqUrl => {
                    read += match reader.read(rbuf.mut_slice(0, 1)) {
                        Ok(len) if len == 1 => len,
                        _ => return Err(InvalidUrl),
                    };
                    match rbuf[0] as char {
                        ' ' => {
                            if pos == 0 { return Err(InvalidUrl); }
                            match callbacks.on_url(self, buf1.as_slice()) {
                                Ok(()) => {
                                    self.state = ReqHttpStart;
                                    pos = 0;
                                    buf1.clear();
                                }
                                Err(e) => {
                                    self.state = Crashed;
                                    return Err(UrlCallbackFail(e));
                                }
                            }
                        }
                        CR | LF => {
                            if pos == 0 { return Err(InvalidUrl); }
                            self.http_version = Some(HTTP_0_9);
                            match callbacks.on_url(self, buf1.as_slice()) {
                                Ok(()) => {
                                    self.state = Dead;
                                    callbacks.on_message_complete(self);
                                    break 'read;
                                }
                                Err(e) => {
                                    self.state = Crashed;
                                    return Err(UrlCallbackFail(e));
                                }
                            }
                        }
                        _ => {
                            buf1.push(rbuf[pos]);
                            pos += 1;
                        }
                    }
                }
                ReqHttpStart => {
                    read += match reader.read(rbuf.mut_slice(0, 6)) {
                        Ok(len) if len == 6 => len,
                        _ => return Err(InvalidVersion),
                    };
                    if rbuf[0] as char != 'H'
                        || rbuf[1] as char != 'T'
                            || rbuf[2] as char != 'T'
                            || rbuf[3] as char != 'P'
                            || rbuf[4] as char != '/'
                            || rbuf[5] < '0' as u8
                            || rbuf[5] > '9' as u8 {
                                return Err(InvalidVersion);
                            }
                    self.state = ReqHttpMajor;
                    pos += 1;
                    major = rbuf[5] as uint - '0' as uint;
                }
                ReqHttpMajor => {
                    read += match reader.read(rbuf.mut_slice(0, 1)) {
                        Ok(len) if len == 1 => len,
                        _ => return Err(InvalidVersion),
                    };
                    match rbuf[0] as char {
                        '.' if pos > 0 => {
                            self.state = ReqHttpMinor;
                            pos = 0;
                        }
                        n if n >= '0' && n <= '9' => {
                            pos += 1;
                            major *= 10;
                            major += n as uint - '0' as uint;
                        }
                        _ => return Err(InvalidVersion),
                    }
                }
                ReqHttpMinor => {
                    read += match reader.read(rbuf.mut_slice(0, 1)) {
                        Ok(len) if len == 1 => len,
                        _ => return Err(InvalidVersion),
                    };
                    match rbuf[0] as char {
                        n if n >= '0' && n <= '9' => {
                            pos += 1;
                            minor *= 10;
                            minor += n as uint - '0' as uint;
                        }
                        CR | LF if pos > 0 => match HttpVersion::find(major, minor) {
                            None => return Err(InvalidVersion),
                            v => {
                                self.http_version = v;
                                self.state = if rbuf[0] as char == CR {
                                    ReqLineAlmostDone
                                } else {
                                    HeaderFieldStart
                                };
                                pos = 0;
                            }
                        },
                        _ => return Err(InvalidVersion),
                    }
                }
                ReqLineAlmostDone => {
                    read += match reader.read(rbuf.mut_slice(0, 1)) {
                        Ok(len) if len == 1 => len,
                        _ => return Err(InvalidRequestLine),
                    };
                    if rbuf[0] as char != LF {
                        return Err(InvalidRequestLine);
                    }
                    self.state = HeaderFieldStart;
                }
                HeaderFieldStart => {
                    read += match reader.read(rbuf.mut_slice(0, 1)) {
                        Ok(len) if len == 1 => len,
                        _ => return Err(InvalidHeaderField),
                    };
                    match rbuf[0] as char {
                        CR => self.state = HeadersAlmostDone,
                        LF => {
                            self.state = HeadersDone;
                            self.skip_body = callbacks.on_headers_complete(self);
                        }
                        c if is_header_token(c) => {
                            self.state = HeaderField;
                            pos = 1;
                            buf1.push(rbuf[0]);
                        }
                        _ => return Err(InvalidHeaderField)
                    }
                }
                HeaderField => {
                    read += match reader.read(rbuf.mut_slice(0, 1)) {
                        Ok(len) if len == 1 => len,
                        _ => return Err(InvalidHeaderField),
                    };
                    match rbuf[0] as char {
                        ':' => {
                            self.state = HeaderValueDiscardWS;
                            pos = 0;
                        }
                        CR => {
                            self.state = HeaderAlmostDone;
                            pos = 0;
                        }
                        LF => {
                            self.state = HeaderFieldStart;
                            pos = 0;
                        }
                        c if is_header_token(c) => {
                            buf1.push(rbuf[0]);
                            pos += 1;
                        }
                        _ => return Err(InvalidHeaderField),
                    }
                }
                HeaderValueDiscardWS => {
                    read += match reader.read(rbuf.mut_slice(0, 1)) {
                        Ok(len) if len == 1 => len,
                        _ => return Err(InvalidHeaderField),
                    };
                    match rbuf[0] as char {
                        ' ' | '\t' => continue,
                        CR => {
                            self.state = HeaderValueDiscardWSAlmostDone;
                            continue
                        },
                        LF => {
                            self.state = HeaderValueDiscardLWS;
                            continue
                        },
                        _ => (),
                    }
                    self.state = HeaderValue;
                    buf2.push(rbuf[0]);
                    pos = 1;
                }
                HeaderValueDiscardWSAlmostDone => {
                    read += match reader.read(rbuf.mut_slice(0, 1)) {
                        Ok(len) if len == 1 => len,
                        _ => return Err(InvalidHeaderField),
                    };
                    if rbuf[0] as char != LF {
                        return Err(InvalidHeaderField);
                    }
                    self.state = HeaderValueDiscardLWS;
                }
                HeaderValueDiscardLWS => {
                    read += match reader.read(rbuf.mut_slice(0, 1)) {
                        Ok(len) if len == 1 => len,
                        _ => return Err(InvalidHeaderField),
                    };
                    if rbuf[0] as char == ' ' || rbuf[0] as char == '\t' {
                        self.state = HeaderValueDiscardWS;
                        continue;
                    }
                    let val = [];
                    match callbacks.on_header_field(self, buf1.as_slice(), val) {
                        Err(e) => return Err(HeaderFieldCallbackFail(e)),
                        _ => {
                            buf1.clear();
                            pos = 0;
                        },
                    }
                    match rbuf[0] as char {
                        CR => self.state = HeadersAlmostDone,
                        LF => {
                            self.state = HeadersDone;
                            self.skip_body = callbacks.on_headers_complete(self);
                        }
                        c if is_header_token(c) => {
                            self.state = HeaderFieldStart;
                            buf1.push(rbuf[0]);
                            pos = 1;
                        }
                        _ => return Err(InvalidHeaderField),
                    }
                }
                HeaderValue => {
                    read += match reader.read(rbuf.mut_slice(0, 1)) {
                        Ok(len) if len == 1 => len,
                        _ => return Err(InvalidHeaderField),
                    };
                    match rbuf[0] as char {
                        CR | LF => {
                            self.state = if rbuf[0] as char == CR {
                                HeaderAlmostDone
                            } else {
                                HeaderFieldStart
                            };
                            match callbacks.on_header_field(self, buf1.as_slice(), buf2.as_slice()) {
                                Err(e) => return Err(HeaderFieldCallbackFail(e)),
                                _ => {
                                    buf1.clear();
                                    buf2.clear();
                                    pos = 0;
                                },
                            }
                        }
                        _ => {
                            buf2.push(rbuf[0]);
                            pos += 1;
                        },
                    }
                }
                HeaderAlmostDone => {
                    read += match reader.read(rbuf.mut_slice(0, 1)) {
                        Ok(len) if len == 1 => len,
                        _ => return Err(InvalidHeaderField),
                    };
                    if rbuf[0] as char != LF {
                        return Err(InvalidHeaderField);
                    }
                    self.state = HeaderFieldStart;
                }
                HeadersAlmostDone => {
                    read += match reader.read(rbuf.mut_slice(0, 1)) {
                        Ok(len) if len == 1 => len,
                        _ => return Err(InvalidHeaders),
                    };
                    if rbuf[0] as char != LF {
                        return Err(InvalidHeaders);
                    }
                    self.state = if callbacks.on_headers_complete(self) || self.skip_body {
                        self.state = match self.parser_type {
                            Request  => StartReq,
                            Response => StartRes,
                            Both     => StartReqOrRes,
                        };
                        callbacks.on_message_complete(self);
                        break 'read;
                    } else {
                        HeadersDone
                    };
                }
                HeadersDone => {
                    if self.skip_body {
                        self.state = match self.parser_type {
                            Request  => StartReq,
                            Response => StartRes,
                            Both     => StartReqOrRes,
                        };
                        callbacks.on_message_complete(self);
                        break 'read;
                    }
                    match self.content_length {
                        0u => {
                            self.state = match self.parser_type {
                                Request  => StartReq,
                                Response => StartRes,
                                Both     => StartReqOrRes,
                            };
                            callbacks.on_message_complete(self);
                            break 'read;
                        }
                        UINT_MAX => {
                            if self.parser_type == Request || !self.needs_eof() {
                                self.state = match self.parser_type {
                                    Request  => StartReq,
                                    Response => StartRes,
                                    Both     => StartReqOrRes,
                                };
                                callbacks.on_message_complete(self);
                                break 'read;

                            }
                            self.state = BodyIdentityEOF;
                        }
                        _ => self.state = BodyIdentity,
                    }
                }
                Dead | Crashed => unreachable!(),
                ReqUrlStart | ReqHttp => unreachable!(), // deprecated
                _ => unimplemented!()
            }
        }

        return Ok(read);
    }

    fn needs_eof(&mut self) -> bool {
        if self.parser_type == Request {
            return false;
        }
        if self.status_code.is_some() {
            let status = self.status_code.unwrap();
            if status / 100 == 1 ||     // 1xx e.g. Continue
                status == 204 ||        // No Content
                status == 304 ||        // Not Modified
                self.skip_body {
                return false;
            }
        }
        // TODO: chanked
        return true;
    }
}

#[inline]
fn is_header_token(c: char) -> bool {
    (c >= '^' && c <= 'z')
        || (c >= 'A' && c <= 'Z')
        || (c >= '-' && c <= '.')
        || (c >= '#' && c <= '\\')
        || (c >= '*' && c <= '+')
        || (c >= '0' && c <= '9')
        || c == '!'
        || c == '|'
        || c == '~'
}

#[deriving(PartialEq, Eq, Clone, Show)]
enum ParserState {
    Dead,
    StartReq,
    StartRes,
    StartReqOrRes,
    ReqMethod,
    ReqUrlStart,
    ReqUrl,
    ReqHttpStart,
    ReqHttp,
    ReqHttpMajor,
    ReqHttpMinor,
    ReqLineAlmostDone,
    HeaderFieldStart,
    HeaderField,
    HeaderValueDiscardWS,
    HeaderValueDiscardWSAlmostDone,
    HeaderValueDiscardLWS,
    HeaderValueStart,
    HeaderValue,
    HeaderAlmostDone,
    HeadersAlmostDone,
    HeadersDone,
    BodyIdentity,
    BodyIdentityEOF,
    Crashed,

    // new
    RequestLine,
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;
    use std::io::{BufReader, InvalidInput, IoResult, standard_error};
    use std::str::{SendStr, from_utf8};

    pub struct TestCallbacks {
        skip_body: bool,
        started: bool,
        url: Option<String>,
        headers_finished: bool,
        headers: HashMap<SendStr, SendStr>,
        finished: bool,
    }

    impl TestCallbacks {
        fn new(skip_body: bool) -> TestCallbacks {
            TestCallbacks {
                skip_body: skip_body,
                started: false,
                url: None,
                headers_finished: false,
                headers: HashMap::new(),
                finished: false,
            }
        }
    }

    impl Callbacks for TestCallbacks {
        fn on_message_begin(&mut self, _: &Parser) {
            self.started = true;
        }

        fn on_url<'a>(&mut self, _: &Parser, data: &'a [u8]) -> IoResult<()> {
            match from_utf8(data) {
                Some(url) => {
                    self.url = Some(url.to_string());
                    Ok(())
                },
                None => Err(standard_error(InvalidInput)),
            }
        }

        fn on_headers_complete(&mut self, _: &Parser) -> bool {
            self.headers_finished = true;
            self.skip_body
        }

        fn on_header_field<'a>(&mut self, _: &Parser, name: &'a [u8], value: &'a [u8]) -> IoResult<()> {
            let mut header_name: SendStr;
            let mut header_value: SendStr;
            {
                let v = Vec::from_slice(name);
                header_name = match String::from_utf8(v) {
                    Ok(name) => name.into_maybe_owned(),
                    Err(_) => return Err(standard_error(InvalidInput)),
                }
            }
            {
                let v = Vec::from_slice(value);
                header_value = match String::from_utf8(v) {
                    Ok(value) => value.into_maybe_owned(),
                    Err(_) => return Err(standard_error(InvalidInput)),
                }
            }
            self.headers.insert(header_name, header_value);
            Ok(())
        }

        fn on_message_complete(&mut self, _: &Parser) {
            self.finished = true;
        }
    }

    #[test]
    fn test_no_message() {
        let buf: &[u8] = [];
        let mut reader = BufReader::new(buf);
        let mut parser = Parser::new(Request);
        let mut callbacks = TestCallbacks::new(true);
        assert_eq!(parser.parse(&mut reader, &mut callbacks), Ok(0));
        assert!(!callbacks.started);
        assert!(!callbacks.finished);
    }

    mod http_0_9 {
        use super::TestCallbacks;
        use super::super::*;
        use std::io::BufReader;

        #[test]
        fn test_simple_request() {
            let msg = "GET /\r\n";
            let data = msg.as_bytes();
            let mut reader = BufReader::new(data);
            let mut parser = Parser::new(Request);
            let mut callbacks = TestCallbacks::new(true);
            assert_eq!(parser.parse(&mut reader, &mut callbacks), Ok(6));
            assert!(callbacks.started);
            assert_eq!(callbacks.url, Some("/".to_string()));
            assert_eq!(parser.http_version, Some(HTTP_0_9));
            assert!(callbacks.finished);
            assert_eq!(reader.tell(), Ok(6));

            // Parser is dead, no more read.
            let mut reader = BufReader::new(msg.as_bytes());
            let mut callbacks = TestCallbacks::new(true);
            assert_eq!(parser.parse(&mut reader, &mut callbacks), Ok(0));
            assert_eq!(reader.tell(), Ok(0));
        }
    }

    mod http_1_0 {
        use super::TestCallbacks;
        use super::super::*;
        use std::io::BufReader;

        #[test]
        fn test_request_no_header() {
            let msg = "GET / HTTP/1.0\r\n\r\n";
            let data = msg.as_bytes();
            let mut reader = BufReader::new(data);
            let mut parser = Parser::new(Request);
            let mut callbacks = TestCallbacks::new(true);
            assert_eq!(parser.parse(&mut reader, &mut callbacks), Ok(data.len()));
            assert!(callbacks.started);
            assert_eq!(callbacks.url, Some("/".to_string()));
            assert_eq!(parser.http_version, Some(HTTP_1_0));
            assert!(callbacks.headers_finished);
            assert!(callbacks.finished);
            assert_eq!(reader.tell(), Ok(data.len() as u64));
        }

        #[test]
        fn test_request_no_body() {
            let mut parser = Parser::new(Request);
            let mut callbacks = TestCallbacks::new(true);
            let msg = create_request("GET", "/", None, None);
            let data = msg.as_bytes();
            let mut reader = BufReader::new(data);
            assert_eq!(parser.parse(&mut reader, &mut callbacks), Ok(data.len()));
            assert!(callbacks.started);
            assert_eq!(callbacks.url, Some("/".to_string()));
            assert_eq!(parser.http_version, Some(HTTP_1_0));
            assert_general_headers(&callbacks);
            assert!(callbacks.finished);
            assert_eq!(reader.tell(), Ok(data.len() as u64));
        }

//      #[test]
//      fn test_request_body() {
//          let mut parser = Parser::new(Request);
//          let mut callbacks = TestCallbacks::new(false);
//          let (header, body) = create_body("Hello, world!");
//          let msg = create_request("POST", "/", header, body);
//          let data = msg.as_bytes();
//          let mut reader = BufReader::new(data);
//          {
//              assert_eq!(parser.parse(&mut reader, &mut callbacks), Ok(data.len()));
//          }
//          assert!(callbacks.started);
//          assert_eq!(callbacks.url, Some("/".to_string()));
//          assert_eq!(parser.http_version, Some(HTTP_1_0));
//          assert_general_headers(&callbacks);
//          assert!(callbacks.finished);
//          assert_eq!(reader.tell(), Ok(data.len() as u64));
//      }

        fn create_request(method: &'static str, url: &'static str, header: Option<String>, body: Option<String>) -> String {
            let mut vec = Vec::new();
            let nl = "\r\n".to_string();
            vec.push(format!("{} {} HTTP/1.0", method, url));
            vec.push("Host: faultier.jp".to_string());
            vec.push("User-Agent: test".to_string());
            if header.is_some() {  vec.push(header.unwrap()) }
            vec.push(nl.clone());
            if body.is_some() { vec.push(body.unwrap()) }
            vec.connect(nl.as_slice())
        }

//      fn create_body(body: &'static str) -> (Option<String>, Option<String>) {
//          (Some(format!("Content-Length: {}", body.as_bytes().len())), Some(body.to_string()))
//      }

        fn assert_general_headers(cb: &TestCallbacks) {
            assert!(cb.headers_finished);
            assert_eq!(cb.headers.find(&"Host".into_maybe_owned()),
                Some(&"faultier.jp".into_maybe_owned()));
            assert_eq!(cb.headers.find(&"User-Agent".into_maybe_owned()),
                Some(&"test".into_maybe_owned()));
        }
    }

    mod http_1_1 {
        use super::TestCallbacks;
        use super::super::*;
        use std::io::BufReader;

        #[test]
        fn test_request_no_body() {
            let mut parser = Parser::new(Request);
            let mut callbacks = TestCallbacks::new(true);
            let msg = create_request("GET", "/", None, None);
            let data = msg.as_bytes();
            let mut reader = BufReader::new(data);
            assert_eq!(parser.parse(&mut reader, &mut callbacks), Ok(data.len()));
            assert!(callbacks.started);
            assert_eq!(callbacks.url, Some("/".to_string()));
            assert_eq!(parser.http_version, Some(HTTP_1_1));
            assert_general_headers(&callbacks);
            assert!(callbacks.finished);
            assert_eq!(reader.tell(), Ok(data.len() as u64));
        }

        fn create_request(method: &'static str, url: &'static str, header: Option<String>, body: Option<String>) -> String {
            let mut vec = Vec::new();
            let nl = "\r\n".to_string();
            vec.push(format!("{} {} HTTP/1.1", method, url));
            vec.push("Host: faultier.jp".to_string());
            vec.push("User-Agent: test".to_string());
            if header.is_some() {
                vec.push(header.unwrap());
            }
            vec.push(nl.clone());
            if body.is_some() {
                vec.push(body.unwrap());
                vec.push(nl.clone());
            }
            vec.connect(nl.as_slice())
        }

        fn assert_general_headers(cb: &TestCallbacks) {
            assert!(cb.headers_finished);
            assert_eq!(cb.headers.find(&"Host".into_maybe_owned()),
                Some(&"faultier.jp".into_maybe_owned()));
            assert_eq!(cb.headers.find(&"User-Agent".into_maybe_owned()),
                Some(&"test".into_maybe_owned()));
        }
    }
}

#[cfg(test)]
mod bench {
    use super::*;
    use test::Bencher;
    use std::io::BufReader;

    struct BenchCallbacks {
        skip_body: bool
    }

    impl Callbacks for BenchCallbacks {
        fn on_headers_complete(&mut self, _: &Parser) -> bool { self.skip_body }
    }

    #[bench]
    fn bench_no_message(b: &mut Bencher) {
        let buf: &[u8] = [];
        let mut cb = BenchCallbacks { skip_body: true };
        b.iter(|| Parser::new(Request).parse(&mut BufReader::new(buf), &mut cb) );
    }

    mod http_0_9 {
        use super::BenchCallbacks;
        use super::super::*;
        use test::Bencher;
        use std::io::BufReader;

        #[allow(unused_must_use)]
        #[bench]
        fn bench_no_parse(b: &mut Bencher) {
            let msg = "GET /\r\n";
            let data = msg.as_bytes();
            let len = data.len();
            let mut buf = [0u8];
            b.iter(|| {
                let mut pos = 0u;
                let mut reader = BufReader::new(data);
                while pos < len {
                    reader.read(buf);
                    pos += 1;
                }
            });
        }

        #[bench]
        fn bench_simple_request(b: &mut Bencher) {
            let msg = "GET /\r\n";
            let data = msg.as_bytes();
            let mut cb = BenchCallbacks { skip_body: true };
            b.iter(|| Parser::new(Request).parse(&mut BufReader::new(data), &mut cb) );
        }
    }

    mod http_1_0 {
        use super::BenchCallbacks;
        use super::super::*;
        use test::Bencher;
        use std::io::BufReader;

        #[allow(unused_must_use)]
        #[bench]
        fn bench_no_parse(b: &mut Bencher) {
            let msg = create_request("GET", "/", None, None);
            let data = msg.as_bytes();
            let len = data.len();
            b.iter(|| {
                let mut pos = 0u;
                let mut reader = BufReader::new(data);
                let mut buf = [0u8];
                while pos < len {
                    reader.read(buf);
                    pos += 1;
                }
            });
        }

        #[bench]
        fn bench_request_no_header(b: &mut Bencher) {
            let msg = "GET / HTTP/1.0\r\n\r\n";
            let data = msg.as_bytes();
            let mut cb = BenchCallbacks { skip_body: true };
            b.iter(|| Parser::new(Request).parse(&mut BufReader::new(data), &mut cb) );
        }

        #[bench]
        fn bench_request_no_body(b: &mut Bencher) {
            let msg = create_request("GET", "/", None, None);
            let data = msg.as_bytes();
            let mut cb = BenchCallbacks { skip_body: true };
            b.iter(|| Parser::new(Request).parse(&mut BufReader::new(data), &mut cb) );
        }

        fn create_request(method: &'static str, url: &'static str, header: Option<String>, body: Option<String>) -> String {
            let mut vec = Vec::new();
            let nl = "\r\n".to_string();
            vec.push(format!("{} {} HTTP/1.0", method, url));
            vec.push("Host: faultier.jp".to_string());
            vec.push("User-Agent: test".to_string());
            if header.is_some() {
                vec.push(header.unwrap());
            }
            vec.push(nl.clone());
            if body.is_some() {
                vec.push(body.unwrap());
                vec.push(nl.clone());
            }
            vec.connect(nl.as_slice())
        }
    }

    mod http_1_1 {
        use super::BenchCallbacks;
        use super::super::*;
        use test::Bencher;
        use std::io::BufReader;

        #[allow(unused_must_use)]
        #[bench]
        fn bench_no_parse(b: &mut Bencher) {
            let msg = create_request("GET", "/", None, None);
            let data = msg.as_bytes();
            let len = data.len();
            b.iter(|| {
                let mut pos = 0u;
                let mut reader = BufReader::new(data);
                let mut buf = [0u8];
                while pos < len {
                    reader.read(buf);
                    pos += 1;
                }
            });
        }

        #[bench]
        fn bench_request_no_body(b: &mut Bencher) {
            let msg = create_request("GET", "/", None, None);
            let data = msg.as_bytes();
            let mut cb = BenchCallbacks { skip_body: true };
            b.iter(|| Parser::new(Request).parse(&mut BufReader::new(data), &mut cb) );
        }

        fn create_request(method: &'static str, url: &'static str, header: Option<String>, body: Option<String>) -> String {
            let mut vec = Vec::new();
            let nl = "\r\n".to_string();
            vec.push(format!("{} {} HTTP/1.1", method, url));
            vec.push("Host: faultier.jp".to_string());
            vec.push("User-Agent: test".to_string());
            if header.is_some() {
                vec.push(header.unwrap());
            }
            vec.push(nl.clone());
            if body.is_some() {
                vec.push(body.unwrap());
                vec.push(nl.clone());
            }
            vec.connect(nl.as_slice())
        }
    }
}
