//! HTTP parser.

#![experimental]

use std::fmt::{Formatter, FormatError, Show};
use std::io::{IoError, IoResult};
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

pub type ParseResult = Result<(uint, int), ParseError>;

static CR: char = '\r';
static LF: char = '\n';

#[allow(dead_code)]
/// HTTP parser.
pub struct Parser {
    // parser internal state
    parser_type: Type,
    state: ParserState,
    index: uint,
    sep: uint,
    skip: uint,
    skip_body: bool,

    // http version
    http_version: Option<HttpVersion>,
    major: uint,
    minor: uint,

    // common header
    content_length: uint,

    // request
    method: Option<HttpMethod>,

    // response
    status_code: Option<uint>,
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
            index: 0,
            sep: 0,
            skip: 0,
            major: 0,
            minor: 0,
        }
    }

   /// Parse HTTP message.
    pub fn parse<C: Callbacks>(&mut self, data: &[u8], callbacks: &mut C) -> ParseResult {
        if self.state == Dead { return Ok((0, 0)) }
        if self.state == Crashed { return Err(OtherParseError) }
        if data.len() == 0 { return Ok((0, 0)) }

        let mut read = 0u;
        let mut pos  = 0u;
        let mut needs = -1i;
        let len = data.len();

        loop {
            pos = read;
            match self.state {
                StartReq => {
                    if len - pos < 3 {
                        needs = 3 - (len - pos) as int;
                        break;
                    }
                    self.major = 0;
                    self.minor = 0;
                    self.http_version = None;
                    self.content_length = UINT_MAX;
                    self.skip_body = false;
                    callbacks.on_message_begin(self);
                    let maybe = match (data[pos] as char, data[pos+1] as char, data[pos+2] as char) {
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
                        ('P', 'R', 'O') => HttpPropPatch,    // or PROPFIND
                        ('P', 'U', 'R') => HttpPurge,
                        ('P', 'U', 'T') => HttpPut,
                        ('R', 'E', 'P') => HttpReport,
                        ('S', 'E', 'A') => HttpSearch,
                        ('S', 'U', 'B') => HttpSubscribe,
                        ('T', 'R', 'A') => HttpTrace,
                        ('U', 'N', 'L') => HttpUnlink,      // or UNLOCK
                        ('U', 'N', 'S') => HttpUnsubscribe,
                        _               => { self.state = Crashed; return Err(InvalidMethod) },
                    };
                    if len - pos < maybe.len() + 1 {
                        needs = (maybe.len() + 1) as int - (len - pos) as int;
                        break;
                    }
                    read += 3;
                    self.method = Some(maybe);
                    self.state = ReqMethod;
                    self.index = 3;
                }
                ReqMethod => {
                    if pos == len { break }
                    read += 1;
                    let method = self.method.unwrap();
                    if data[pos] as char == ' ' && pos == method.len() {
                        self.state = ReqUrl;
                        self.index = 0;
                    } else {
                        if !method.hit(self.index, data[pos] as char) {
                            self.method = Some(match method {
                                HttpMkCalendar if self.index == 3 && data[pos] as char == 'O' => HttpMkCol,
                                HttpPropPatch  if self.index == 4 && data[pos] as char == 'F' => HttpPropFind,
                                HttpUnlink     if self.index == 3 && data[pos] as char == 'O' => HttpUnlock,
                                _ => { self.state = Crashed; return Err(InvalidMethod) },
                            });
                        }
                        self.index += 1;
                    }
                }
                ReqUrl => {
                    if pos == len {
                        read -= self.index;
                        self.index = 0;
                        break
                    }
                    read += 1;
                    match data[pos] as char {
                        ' ' => {
                            if self.index == 0 { self.state = Crashed; return Err(InvalidUrl) }
                            let url = data.slice(pos-self.index, pos);
                            match callbacks.on_url(self, url) {
                                Ok(()) => {
                                    self.state = ReqHttpStart;
                                    self.index = 0;
                                }
                                Err(e) => { self.state = Crashed; return Err(UrlCallbackFail(e)) },
                            }
                        }
                        CR | LF => {
                            if self.index == 0 { self.state = Crashed; return Err(InvalidUrl) }
                            self.http_version = Some(HTTP_0_9);
                            // TODO: merge buffer
                            let url = data.slice(pos-self.index, pos);
                            match callbacks.on_url(self, url) {
                                Ok(()) => {
                                    self.state = Dead;
                                    self.index = 0;
                                    needs = 0;
                                    callbacks.on_message_complete(self);
                                    break;
                                }
                                Err(e) => { self.state = Crashed; return Err(UrlCallbackFail(e)) },
                            }
                        }
                        _ => {
                            self.index += 1;
                        },
                    }
                }
                ReqHttpStart => {
                    read += 6;
                    if data[pos] as char != 'H'
                        || data[pos+1] as char != 'T'
                            || data[pos+2] as char != 'T'
                            || data[pos+3] as char != 'P'
                            || data[pos+4] as char != '/'
                            || data[pos+5] < '0' as u8
                            || data[pos+5] > '9' as u8 {
                                self.state = Crashed;
                                return Err(InvalidVersion);
                            }
                    self.state = ReqHttpMajor;
                    self.major = data[pos+5] as uint - '0' as uint;
                    self.index += 1;
                }
                ReqHttpMajor => {
                    read += 1;
                    match data[pos] as char {
                        '.' if self.index > 0 => {
                            self.state = ReqHttpMinor;
                            self.index = 0;
                        }
                        n if n >= '0' && n <= '9' => {
                            self.index += 1;
                            self.major *= 10;
                            self.major += n as uint - '0' as uint;
                        }
                        _ => { self.state = Crashed; return Err(InvalidVersion) },
                    }
                }
                ReqHttpMinor => {
                    read += 1;
                    match data[pos] as char {
                        n if n >= '0' && n <= '9' => {
                            self.index += 1;
                            self.minor *= 10;
                            self.minor += n as uint - '0' as uint;
                        }
                        CR | LF if self.index > 0 => match HttpVersion::find(self.major, self.minor) {
                            None => { self.state = Crashed; return Err(InvalidVersion) },
                            v => {
                                self.http_version = v;
                                self.state = if data[pos] as char == CR {
                                    ReqLineAlmostDone
                                } else {
                                    HeaderFieldStart
                                };
                                self.index = 0;
                            }
                        },
                        _ => { self.state = Crashed; return Err(InvalidVersion) },
                    }
                }
                ReqLineAlmostDone => {
                    read += 1;
                    if data[pos] as char != LF {
                        return Err(InvalidRequestLine);
                    }
                    self.state = HeaderFieldStart;
                }
                HeaderFieldStart => {
                    read += 1;
                    match data[pos] as char {
                        CR => self.state = HeadersAlmostDone,
                        LF => {
                            self.state = HeadersDone;
                            self.skip_body = callbacks.on_headers_complete(self);
                        }
                        c if is_header_token(c) => {
                            self.state = HeaderField;
                            self.index = 1;
                        }
                        _ => { self.state = Crashed; return Err(InvalidHeaderField) },
                    }
                }
                HeaderField => {
                    read += 1;
                    match data[pos] as char {
                        ':' => {
                            self.state = HeaderValueDiscardWS;
                            self.sep = pos;
                            self.skip = pos;
                            self.index += 1;
                        }
                        CR => {
                            self.state = HeaderAlmostDone;
                            self.index = 0;
                        }
                        LF => {
                            self.state = HeaderFieldStart;
                            self.index = 0;
                        }
                        c if is_header_token(c) => {
                            self.index += 1;
                        }
                        _ => { self.state = Crashed; return Err(InvalidHeaderField) },
                    }
                }
                HeaderValueDiscardWS => {
                    read += 1;
                    match data[pos] as char {
                        ' ' | '\t' => {
                            self.skip += 1;
                        },
                        CR => self.state = HeaderValueDiscardWSAlmostDone,
                        LF => self.state = HeaderValueDiscardLWS,
                        _ => {
                            self.state = HeaderValue;
                            self.index += 1;
                        },
                    }
                }
                HeaderValueDiscardWSAlmostDone => {
                    read += 1;
                    if data[pos] as char != LF { self.state = Crashed; return Err(InvalidHeaderField) }
                    self.state = HeaderValueDiscardLWS;
                }
                HeaderValueDiscardLWS => {
                    read += 1;
                    if data[pos] as char == ' ' || data[pos] as char == '\t' {
                        self.state = HeaderValueDiscardWS;
                        self.skip += 1;
                    } else {
                        // header value is empty.
                        let name = data.slice(pos-self.index, self.sep);
                        match callbacks.on_header_field(self, name, []) {
                            Err(e) => { self.state = Crashed; return Err(HeaderFieldCallbackFail(e)) },
                            _ => {
                                self.index = 0;
                            },
                        }
                        match data[pos] as char {
                            CR => self.state = HeadersAlmostDone,
                            LF => {
                                self.state = HeadersDone;
                                self.skip_body = callbacks.on_headers_complete(self);
                            }
                            c if is_header_token(c) => {
                                self.state = HeaderFieldStart;
                                self.index = 1;
                            }
                            _ => { self.state = Crashed; return Err(InvalidHeaderField) },
                        }
                    }
                }
                HeaderValue => {
                    read += 1;
                    match data[pos] as char {
                        CR | LF => {
                            self.state = if data[pos] as char == CR {
                                HeaderAlmostDone
                            } else {
                                HeaderFieldStart
                            };
                            let name = data.slice(pos-(self.index+1), self.sep);
                            let value = data.slice(self.skip+1, pos);
                            match callbacks.on_header_field(self, name, value) {
                                Err(e) => { self.state = Crashed; return Err(HeaderFieldCallbackFail(e)) },
                                _ => self.index = 0,
                            }
                        }
                        _ => {
                            self.index += 1;
                        },
                    }
                }
                HeaderAlmostDone => {
                    read += 1;
                    if data[pos] as char != LF { self.state = Crashed; return Err(InvalidHeaderField) }
                    self.state = HeaderFieldStart;
                }
                HeadersAlmostDone => {
                    read += 1;
                    if data[pos] as char != LF { self.state = Crashed; return Err(InvalidHeaders) }
                    if callbacks.on_headers_complete(self) || self.skip_body {
                        self.state = match self.parser_type {
                            Request  => StartReq,
                            Response => StartRes,
                            Both     => StartReqOrRes,
                        };
                        needs = 0;
                        callbacks.on_message_complete(self);
                        break;
                    }
                    self.state = HeadersDone;
                }
                HeadersDone => {
                    read += 1;
                    if self.skip_body {
                        self.state = match self.parser_type {
                            Request  => StartReq,
                            Response => StartRes,
                            Both     => StartReqOrRes,
                        };
                        needs = 0;
                        callbacks.on_message_complete(self);
                        break;
                    }
                    match self.content_length {
                        0u => {
                            self.state = match self.parser_type {
                                Request  => StartReq,
                                Response => StartRes,
                                Both     => StartReqOrRes,
                            };
                            needs = 0;
                            callbacks.on_message_complete(self);
                            break;
                        }
                        UINT_MAX => {
                            if self.parser_type == Request || !self.needs_eof() {
                                self.state = match self.parser_type {
                                    Request  => StartReq,
                                    Response => StartRes,
                                    Both     => StartReqOrRes,
                                };
                                needs = 0;
                                callbacks.on_message_complete(self);
                                break;
                            }
                            self.state = BodyIdentityEOF;
                        }
                        _ => self.state = BodyIdentity,
                    }
                }
                Dead | Crashed => unreachable!(),
                _ => unimplemented!()
            }
        }

        return Ok((read, needs));
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
    ReqUrl,
    ReqHttpStart,
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
    use std::io::{InvalidInput, IoResult, standard_error};
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
        let mut parser = Parser::new(Request);
        let mut callbacks = TestCallbacks::new(true);
        assert_eq!(parser.parse([], &mut callbacks), Ok((0, 0)));
        assert!(!callbacks.started);
        assert!(!callbacks.finished);
    }

    #[test]
    fn test_partial_data() {
        let data = "HEAD /hello.txt HTTP/1.0\r\n\r\n".as_bytes();
        let mut parser = Parser::new(Request);
        let mut cb = TestCallbacks::new(true);

        // parse "HE", len < 3
        let part = data.slice(0, 2);
        let (read, needs) = parser.parse(part, &mut cb).unwrap();
        assert_eq!(read, 0);
        assert_eq!(needs, 1);
        assert_eq!(parser.state, super::StartReq);
        assert!(!cb.started);
        assert!(!cb.finished);

        // parse "HEA", len < HttpHead.len()
        let len = ((part.len() as int - read as int) + needs) as uint;
        let part = data.slice(read, len);
        let (read, needs) = parser.parse(part, &mut cb).unwrap();
        assert_eq!(read, 0);
        assert_eq!(needs, 2);
        assert_eq!(parser.state, super::StartReq);
        assert!(cb.started);
        assert!(!cb.finished);

        // parse "HEAD "
        let len = ((part.len() as int - read as int) + needs) as uint;
        let part = data.slice(read, len);
        let (read, needs) = parser.parse(part, &mut cb).unwrap();
        assert_eq!(read, 5);
        assert_eq!(needs, -1);
        assert_eq!(parser.state, super::ReqUrl);
        assert!(cb.started);
        assert!(!cb.finished);

        // parse "/he"
        let part = data.slice(5, 8);
        let (read, needs) = parser.parse(part, &mut cb).unwrap();
        assert_eq!(read, 0);
        assert_eq!(needs, -1);
        assert_eq!(parser.state, super::ReqUrl);
        assert!(!cb.finished);

        // parse "/hello.txt HTTP/1.0"
        let part = data.slice_from(5);
        let (read, needs) = parser.parse(part, &mut cb).unwrap();
        assert_eq!(read, part.len());
        assert_eq!(needs, 0);
        assert!(cb.finished);
    }

    mod http_0_9 {
        use super::TestCallbacks;
        use super::super::*;

        #[test]
        fn test_simple_request_get() {
            let msg = "GET /\r\n";
            let data = msg.as_bytes();
            let mut parser = Parser::new(Request);
            let mut callbacks = TestCallbacks::new(true);

            assert_eq!(parser.parse(data, &mut callbacks), Ok((6, 0)));
            assert!(callbacks.started);
            assert_eq!(callbacks.url, Some("/".to_string()));
            assert_eq!(parser.http_version, Some(HTTP_0_9));
            assert!(callbacks.finished);

            // Parser is dead, no more read.
            assert_eq!(parser.parse(data, &mut callbacks), Ok((0, 0)));
        }
    }

    mod http_1_0 {
        use super::TestCallbacks;
        use super::super::*;
        use std::collections::HashMap;

        #[test]
        fn test_request_without_header() {
            let msg = "GET / HTTP/1.0\r\n\r\n";
            let data = msg.as_bytes();
            let mut parser = Parser::new(Request);
            let mut callbacks = TestCallbacks::new(true);
            assert_eq!(parser.parse(data, &mut callbacks), Ok((data.len(), 0)));
            assert!(callbacks.started);
            assert_eq!(callbacks.url, Some("/".to_string()));
            assert_eq!(parser.http_version, Some(HTTP_1_0));
            assert!(callbacks.headers_finished);
            assert!(callbacks.finished);
        }

        #[test]
        fn test_request_get() {
            let mut parser = Parser::new(Request);
            let mut callbacks = TestCallbacks::new(true);
            let msg = create_request("GET", "/tag/Rust", None, None);
            let data = msg.as_bytes();
            assert_eq!(parser.parse(data, &mut callbacks), Ok((data.len(), 0)));
            assert!(callbacks.started);
            assert_eq!(callbacks.url, Some("/tag/Rust".to_string()));
            assert_eq!(parser.http_version, Some(HTTP_1_0));
            assert_general_headers(&callbacks);
            assert!(callbacks.finished);
        }

        fn create_request(method: &'static str, url: &'static str, header: Option<HashMap<String, String>>, body: Option<String>) -> String {
            let mut h = HashMap::new();
            h.insert("Host".to_string(), "faultier.jp".to_string());
            h.insert("User-Agent".to_string(), "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.9; rv:30.0) Gecko/20100101 Firefox/30.0".to_string());
            h.insert("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".to_string());
            h.insert("Accept-Encoding".to_string(), "gzip,deflate".to_string());
            h.insert("Accept-Language".to_string(), "ja,en-US;q=0.8,en;q=0.6".to_string());
            h.insert("Connection".to_string(), "close".to_string());
            h.insert("Cache-Control".to_string(), "max-age=0".to_string());
            h.insert("Cookie".to_string(), "key1=value1; key2=value2".to_string());
            h.insert("Referer".to_string(), "http://faultier.blog.jp/".to_string());
            if header.is_some() {
                h.extend(header.unwrap().move_iter());
            }
            let mut vec = Vec::new();
            let nl = "\r\n".to_string();
            vec.push(format!("{} {} HTTP/1.0", method, url));
            for (name, value) in h.iter() {
                vec.push(format!("{}: {}", *name, *value));
            }
            vec.push(nl.clone());
            if body.is_some() {
                vec.push(body.unwrap());
                vec.push(nl.clone());
            }
            vec.connect(nl.as_slice())
        }

        fn assert_general_headers(cb: &TestCallbacks) {
            let mut h = HashMap::new();
            h.insert("Host".into_maybe_owned(), "faultier.jp".into_maybe_owned());
            h.insert("User-Agent".into_maybe_owned(), "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.9; rv:30.0) Gecko/20100101 Firefox/30.0".into_maybe_owned());
            h.insert("Accept".into_maybe_owned(), "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".into_maybe_owned());
            h.insert("Accept-Encoding".into_maybe_owned(), "gzip,deflate".into_maybe_owned());
            h.insert("Accept-Language".into_maybe_owned(), "ja,en-US;q=0.8,en;q=0.6".into_maybe_owned());
            h.insert("Connection".into_maybe_owned(), "close".into_maybe_owned());
            h.insert("Cache-Control".into_maybe_owned(), "max-age=0".into_maybe_owned());
            h.insert("Cookie".into_maybe_owned(), "key1=value1; key2=value2".into_maybe_owned());
            h.insert("Referer".into_maybe_owned(), "http://faultier.blog.jp/".into_maybe_owned());
 
            assert!(cb.headers_finished);
            for (name, value) in h.iter() {
                assert_eq!(cb.headers.find(name), Some(value));
            }
        }
    }

    mod http_1_1 {
        use super::TestCallbacks;
        use super::super::*;
        use std::collections::HashMap;

        #[test]
        fn test_request_get() {
            let mut parser = Parser::new(Request);
            let mut callbacks = TestCallbacks::new(true);
            let msg = create_request("GET", "/tag/Rust", None, None);
            let data = msg.as_bytes();
            assert_eq!(parser.parse(data, &mut callbacks), Ok((data.len(), 0)));
            assert!(callbacks.started);
            assert_eq!(callbacks.url, Some("/tag/Rust".to_string()));
            assert_eq!(parser.http_version, Some(HTTP_1_1));
            assert_general_headers(&callbacks);
            assert!(callbacks.finished);
        }

        fn create_request(method: &'static str, url: &'static str, header: Option<HashMap<String, String>>, body: Option<String>) -> String {
            let mut h = HashMap::new();
            h.insert("Host".to_string(), "faultier.jp".to_string());
            h.insert("User-Agent".to_string(), "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.9; rv:30.0) Gecko/20100101 Firefox/30.0".to_string());
            h.insert("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".to_string());
            h.insert("Accept-Encoding".to_string(), "gzip,deflate".to_string());
            h.insert("Accept-Language".to_string(), "ja,en-US;q=0.8,en;q=0.6".to_string());
            h.insert("Connection".to_string(), "close".to_string());
            h.insert("Cache-Control".to_string(), "max-age=0".to_string());
            h.insert("Cookie".to_string(), "key1=value1; key2=value2".to_string());
            h.insert("Referer".to_string(), "http://faultier.blog.jp/".to_string());
            if header.is_some() {
                h.extend(header.unwrap().move_iter());
            }
            let mut vec = Vec::new();
            let nl = "\r\n".to_string();
            vec.push(format!("{} {} HTTP/1.1", method, url));
            for (name, value) in h.iter() {
                vec.push(format!("{}: {}", *name, *value));
            }
            vec.push(nl.clone());
            if body.is_some() {
                vec.push(body.unwrap());
                vec.push(nl.clone());
            }
            vec.connect(nl.as_slice())
        }

        fn assert_general_headers(cb: &TestCallbacks) {
            let mut h = HashMap::new();
            h.insert("Host".into_maybe_owned(), "faultier.jp".into_maybe_owned());
            h.insert("User-Agent".into_maybe_owned(), "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.9; rv:30.0) Gecko/20100101 Firefox/30.0".into_maybe_owned());
            h.insert("Accept".into_maybe_owned(), "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".into_maybe_owned());
            h.insert("Accept-Encoding".into_maybe_owned(), "gzip,deflate".into_maybe_owned());
            h.insert("Accept-Language".into_maybe_owned(), "ja,en-US;q=0.8,en;q=0.6".into_maybe_owned());
            h.insert("Connection".into_maybe_owned(), "close".into_maybe_owned());
            h.insert("Cache-Control".into_maybe_owned(), "max-age=0".into_maybe_owned());
            h.insert("Cookie".into_maybe_owned(), "key1=value1; key2=value2".into_maybe_owned());
            h.insert("Referer".into_maybe_owned(), "http://faultier.blog.jp/".into_maybe_owned());
 
            assert!(cb.headers_finished);
            for (name, value) in h.iter() {
                assert_eq!(cb.headers.find(name), Some(value));
            }
        }
    }
}

#[cfg(test)]
mod bench {
    use super::*;
    use test::Bencher;

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
        b.iter(|| Parser::new(Request).parse(buf, &mut cb) );
    }

    mod http_0_9 {
        use super::BenchCallbacks;
        use super::super::*;
        use test::Bencher;

        #[bench]
        fn bench_simple_request_get(b: &mut Bencher) {
            let msg = "GET /\r\n";
            let data = msg.as_bytes();
            let mut cb = BenchCallbacks { skip_body: true };
            b.iter(|| Parser::new(Request).parse(data, &mut cb) );
        }
    }

    mod http_1_0 {
        use super::BenchCallbacks;
        use super::super::*;
        use test::Bencher;
        use std::collections::HashMap;

        #[bench]
        fn bench_request_without_header(b: &mut Bencher) {
            let msg = "GET / HTTP/1.0\r\n\r\n";
            let data = msg.as_bytes();
            let mut cb = BenchCallbacks { skip_body: true };
            b.iter(|| Parser::new(Request).parse(data, &mut cb) );
        }

        #[bench]
        fn bench_request_get(b: &mut Bencher) {
            let msg = create_request("GET", "/tag/Rust", None, None);
            let data = msg.as_bytes();
            let mut cb = BenchCallbacks { skip_body: true };
            b.iter(|| Parser::new(Request).parse(data, &mut cb) );
        }

        fn create_request(method: &'static str, url: &'static str, header: Option<HashMap<String, String>>, body: Option<String>) -> String {
            let mut h = HashMap::new();
            h.insert("Host".to_string(), "faultier.jp".to_string());
            h.insert("User-Agent".to_string(), "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.9; rv:30.0) Gecko/20100101 Firefox/30.0".to_string());
            h.insert("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".to_string());
            h.insert("Accept-Encoding".to_string(), "gzip,deflate".to_string());
            h.insert("Accept-Language".to_string(), "ja,en-US;q=0.8,en;q=0.6".to_string());
            h.insert("Connection".to_string(), "close".to_string());
            h.insert("Cache-Control".to_string(), "max-age=0".to_string());
            h.insert("Cookie".to_string(), "key1=value1; key2=value2; key3=value3; key4=value4; key5=value5".to_string());
            h.insert("Referer".to_string(), "http://faultier.blog.jp/".to_string());
            if header.is_some() {
                h.extend(header.unwrap().move_iter());
            }
            let mut vec = Vec::new();
            let nl = "\r\n".to_string();
            vec.push(format!("{} {} HTTP/1.0", method, url));
            for (name, value) in h.iter() {
                vec.push(format!("{}: {}", *name, *value));
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
        use std::collections::HashMap;

        #[bench]
        fn bench_request_get(b: &mut Bencher) {
            let msg = create_request("GET", "/tag/Rust", None, None);
            let data = msg.as_bytes();
            let mut cb = BenchCallbacks { skip_body: true };
            b.iter(|| Parser::new(Request).parse(data, &mut cb) );
        }

        fn create_request(method: &'static str, url: &'static str, header: Option<HashMap<String, String>>, body: Option<String>) -> String {
            let mut h = HashMap::new();
            h.insert("Host".to_string(), "faultier.jp".to_string());
            h.insert("User-Agent".to_string(), "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.9; rv:30.0) Gecko/20100101 Firefox/30.0".to_string());
            h.insert("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".to_string());
            h.insert("Accept-Encoding".to_string(), "gzip,deflate".to_string());
            h.insert("Accept-Language".to_string(), "ja,en-US;q=0.8,en;q=0.6".to_string());
            h.insert("Connection".to_string(), "close".to_string());
            h.insert("Cache-Control".to_string(), "max-age=0".to_string());
            h.insert("Cookie".to_string(), "key1=value1; key2=value2; key3=value3; key4=value4; key5=value5".to_string());
            h.insert("Referer".to_string(), "http://faultier.blog.jp/".to_string());
            if header.is_some() {
                h.extend(header.unwrap().move_iter());
            }
            let mut vec = Vec::new();
            let nl = "\r\n".to_string();
            vec.push(format!("{} {} HTTP/1.1", method, url));
            for (name, value) in h.iter() {
                vec.push(format!("{}: {}", *name, *value));
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
