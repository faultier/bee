//! HTTP parser.

#![experimental]

use std::char::to_lowercase;
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
    fn hit(&self, pos: uint, c: char) -> bool {
        self.name().char_at(pos) == c
    }
}

impl Show for HttpMethod {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FormatError> {
        f.pad(self.name())
    }
}

/// Parser event handler.
pub trait Handler {
    #[allow(unused_variable)]
    /// Called when start to parsing of message.
    /// Default implementation is nothing to do.
    fn on_message_begin(&mut self, parser: &Parser) {
    }

    #[allow(unused_variable)]
    /// Called when url parsed.
    /// Default implementation is nothing to do.
    fn on_url(&mut self, parser: &Parser, length: uint) -> IoResult<()> {
        Ok(())
    }

    #[allow(unused_variable)]
    /// Called when status parsed.
    /// Default implementation is nothing to do.
    fn on_status(&mut self, parser: &Parser, length: uint) -> IoResult<()> {
        Ok(())
    }

    #[allow(unused_variable)]
    /// Called when header field's name parsed.
    /// Default implementation is nothing to do.
    fn on_header_field(&mut self, parser: &Parser, length: uint) -> IoResult<()> {
        Ok(())
    }

    #[allow(unused_variable)]
    /// Called when header field's value parsed.
    /// Default implementation is nothing to do.
    fn on_header_value(&mut self, parser: &Parser, length: uint) -> IoResult<()> {
        Ok(())
    }

    #[allow(unused_variable)]
    /// Called when completed to parsing of headers.
    /// Default implementation is nothing to do.
    fn on_headers_complete(&mut self, parser: &Parser) -> bool{
        return false;
    }

    #[allow(unused_variable)]
    /// Called when body parsed.
    /// Default implementation is nothing to do.
    fn on_body(&mut self, parser: &Parser, length: uint) -> IoResult<()> {
        Ok(())
    }

    #[allow(unused_variable)]
    /// Called when completed to parsing of whole message.
    /// Default implementation is nothing to do.
    fn on_message_complete(&mut self, parser: &Parser) {
    }

    /// Push partial data, e.g. URL, header field, message body.
    fn push_data(&mut self, &Parser, &[u8]);
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
    /// Invalid header field.
    InvalidHeaderField,
    /// Invalid header section.
    InvalidHeaders,
    /// Expected data, but reached EOF.
    InvalidEOFState,
    /// An I/O error occurred.
    AnyIoError(IoError),
}

pub type ParseResult = Result<uint, ParseError>;

static CR: char = '\r';
static LF: char = '\n';

macro_rules! reset_state (
    ($t:expr) => (match $t {
        Request  => StartReq,
        Response => StartRes,
        Both     => StartReqOrRes,
    })
)

#[allow(dead_code)]
/// HTTP parser.
pub struct Parser {
    // parser internal state
    parser_type: Type,
    state: ParserState,
    hstate: HeaderParseState,
    index: uint,
    skip_body: bool,

    // http version
    http_version: Option<HttpVersion>,
    major: uint,
    minor: uint,

    // common header
    content_length: uint,
    upgrade: bool,

    // request
    method: Option<HttpMethod>,
    keep_alive: bool,

    // response
    status_code: Option<uint>,
}

impl Parser {
    /// Create a new `Parser`.
    pub fn new(t: Type) -> Parser {
        Parser {
            parser_type: t,
            http_version: None,
            state: reset_state!(t),
            hstate: HeaderGeneral,
            method: None,
            status_code: None,
            content_length: UINT_MAX,
            skip_body: false,
            index: 0,
            major: 0,
            minor: 0,
            keep_alive: false,
            upgrade: false,
        }
    }

    #[allow(unused_must_use)]
    /// Parse HTTP message.
    pub fn parse<R: Reader, C: Handler>(&mut self, reader: &mut R, handler: &mut C) -> ParseResult {
        if self.state == Dead { return Ok(0) }
        if self.state == Crashed { return Err(OtherParseError) }

        let mut buf = [0u8];
        let mut read = 0u;

        loop {
            match reader.read(buf) {
                Ok(len) => read += len,
                Err(IoError { kind: EndOfFile, ..}) => break,
                Err(e) => return Err(AnyIoError(e)),
            }
            match self.state {
                StartReq => {
                    self.major = 0;
                    self.minor = 0;
                    self.http_version = None;
                    self.content_length = UINT_MAX;
                    self.skip_body = false;
                    handler.on_message_begin(self);
                    self.method = Some(match buf[0] as char {
                        'C' => HttpConnect,     // or CHECKOUT, COPY
                        'D' => HttpDelete,
                        'G' => HttpGet,
                        'H' => HttpHead,
                        'L' => HttpLink,        // or LOCK
                        'M' => HttpMkCol,       // or M-SEARCH, MERGE, MKACTIVITY, MKCALENDER
                        'N' => HttpNotify,
                        'O' => HttpOptions,
                        'P' => HttpPut,         // or PATCH, POST, PROPPATCH, PROPFIND
                        'R' => HttpReport,
                        'S' => HttpSearch,      // or SUBSCRIBE
                        'T' => HttpTrace,
                        'U' => HttpUnlink,      // or UNLOCK, UNSUBSCRIBE
                        _   => { self.state = Crashed; return Err(InvalidMethod) },
                    });
                    self.state = ReqMethod;
                    self.index = 1;
                }
                ReqMethod => {
                    let method = self.method.unwrap();
                    if buf[0] as char == ' ' {
                        self.state = ReqUrl;
                        self.index = 0;
                    } else {
                        if !method.hit(self.index, buf[0] as char) {
                            self.method = Some(match method {
                                HttpConnect    if self.index == 2 && buf[0] as char == 'H' => HttpCheckout,
                                HttpConnect    if self.index == 3 && buf[0] as char == 'P' => HttpCheckout,
                                HttpLink       if self.index == 1 && buf[0] as char == 'O' => HttpLock,
                                HttpMkCol      if self.index == 1 && buf[0] as char == '-' => HttpMsearch,
                                HttpMkCol      if self.index == 1 && buf[0] as char == 'E' => HttpMerge,
                                HttpMkCol      if self.index == 2 && buf[0] as char == 'A' => HttpMkActivity,
                                HttpMkCol      if self.index == 3 && buf[0] as char == 'A' => HttpMkCalendar,
                                HttpPut        if self.index == 1 && buf[0] as char == 'A' => HttpPatch,
                                HttpPut        if self.index == 1 && buf[0] as char == 'O' => HttpPost,
                                HttpPut        if self.index == 1 && buf[0] as char == 'R' => HttpPropPatch,
                                HttpPut        if self.index == 2 && buf[0] as char == 'R' => HttpPurge,
                                HttpPropPatch  if self.index == 4 && buf[0] as char == 'F' => HttpPropFind,
                                HttpSearch     if self.index == 1 && buf[0] as char == 'U' => HttpSubscribe,
                                HttpUnlink     if self.index == 2 && buf[0] as char == 'S' => HttpUnsubscribe,
                                HttpUnlink     if self.index == 3 && buf[0] as char == 'O' => HttpUnlock,
                                _ => { self.state = Crashed; return Err(InvalidMethod) },
                            });
                        }
                        self.index += 1;
                    }
                }
                ReqUrl => {
                    match buf[0] as char {
                        ' ' => {
                            if self.index == 0 { self.state = Crashed; return Err(InvalidUrl) }
                            match handler.on_url(self, self.index) {
                                Ok(()) => {
                                    self.state = ReqHttpStart;
                                    self.index = 0;
                                }
                                Err(e) => { self.state = Crashed; return Err(AnyIoError(e)) },
                            }
                        }
                        CR | LF => {
                            if self.index == 0 { self.state = Crashed; return Err(InvalidUrl) }
                            self.http_version = Some(HTTP_0_9);
                            match handler.on_url(self, self.index) {
                                Ok(()) => {
                                    self.state = Dead;
                                    self.index = 0;
                                    handler.on_message_complete(self);
                                    break;
                                }
                                Err(e) => { self.state = Crashed; return Err(AnyIoError(e)) },
                            }
                        }
                        _ => {
                            handler.push_data(self, buf);
                            self.index += 1;
                        }
                    }
                }
                ReqHttpStart => {
                    let c = buf[0] as char;
                    if (c != 'H' && self.index == 0)
                        || (c != 'T' && (self.index == 1 || self.index == 2))
                        || (c != 'P' && self.index == 3)
                        || (c != '/' && self.index == 4)
                        || ((buf[0] < '0' as u8 || buf[0] > '9' as u8) && self.index == 5) {
                            self.state = Crashed;
                            return Err(InvalidVersion);
                        }
                    if self.index == 5 {
                        self.state = ReqHttpMajor;
                        self.major = buf[0] as uint - '0' as uint;
                        self.index = 1;
                    } else {
                        self.index += 1;
                    }
                }
                ReqHttpMajor => {
                    match buf[0] as char {
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
                    match buf[0] as char {
                        n if n >= '0' && n <= '9' => {
                            self.index += 1;
                            self.minor *= 10;
                            self.minor += n as uint - '0' as uint;
                        }
                        CR | LF if self.index > 0 => match HttpVersion::find(self.major, self.minor) {
                            None => { self.state = Crashed; return Err(InvalidVersion) },
                            v => {
                                self.http_version = v;
                                self.keep_alive = v == Some(HTTP_1_1);
                                self.state = if buf[0] as char == CR {
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
                    if buf[0] as char != LF {
                        return Err(InvalidRequestLine);
                    }
                    self.state = HeaderFieldStart;
                }
                HeaderFieldStart => {
                    match buf[0] as char {
                        CR => self.state = HeadersAlmostDone,
                        LF => {
                            self.state = HeadersDone;
                            self.skip_body = handler.on_headers_complete(self);
                        }
                        c if is_token(c) => {
                            self.state = HeaderField;
                            self.hstate = match to_lowercase(c) {
                                'c' => HeaderConnection,
                                't' => HeaderTransferEncoding,
                                'u' => HeaderUpgrade,
                                _   => HeaderGeneral,
                            };
                            handler.push_data(self, buf);
                            self.index = 1;
                        }
                        _ => { self.state = Crashed; return Err(InvalidHeaderField) },
                    }
                }
                HeaderField => {
                    match buf[0] as char {
                        ':' => {
                            match handler.on_header_field(self, self.index) {
                                Ok(()) => {
                                    self.state = HeaderValueDiscardWS;
                                    self.index = 0;
                                },
                                Err(e) => { self.state = Crashed; return Err(AnyIoError(e)) },
                            }
                        }
                        CR => {
                            self.state = HeaderAlmostDone;
                            self.index = 0;
                        }
                        LF => {
                            self.state = HeaderFieldStart;
                            self.index = 0;
                        }
                        c if is_token(c) => {
                            if self.hstate != HeaderGeneral {
                                self.hstate = match self.hstate {
                                    HeaderConnection => match to_lowercase(c) {
                                        'o' if self.index == 1 => HeaderConnection,
                                        'n' if self.index == 2 => HeaderConnection,
                                        'n' if self.index == 3 => HeaderConnection,
                                        'e' if self.index == 4 => HeaderConnection,
                                        'c' if self.index == 5 => HeaderConnection,
                                        't' if self.index == 6 => HeaderConnection,
                                        'i' if self.index == 7 => HeaderConnection,
                                        'o' if self.index == 8 => HeaderConnection,
                                        'n' if self.index == 9 => HeaderConnection,
                                        't' if self.index == 3 => HeaderContentLength,
                                        _ => HeaderGeneral,
                                    },
                                    HeaderContentLength => match to_lowercase(c) {
                                        'e' if self.index == 4  => HeaderContentLength,
                                        'n' if self.index == 5  => HeaderContentLength,
                                        't' if self.index == 6  => HeaderContentLength,
                                        '-' if self.index == 7  => HeaderContentLength,
                                        'l' if self.index == 8  => HeaderContentLength,
                                        'e' if self.index == 9  => HeaderContentLength,
                                        'n' if self.index == 10 => HeaderContentLength,
                                        'g' if self.index == 11 => HeaderContentLength,
                                        't' if self.index == 12 => HeaderContentLength,
                                        'h' if self.index == 13 => HeaderContentLength,
                                        _ => HeaderGeneral,
                                    },
                                    _ => HeaderGeneral,
                                };
                            }
                            handler.push_data(self, buf);
                            self.index += 1;
                        }
                        _ => { self.state = Crashed; return Err(InvalidHeaderField) },
                    }
                }
                HeaderValueDiscardWS => {
                    match buf[0] as char {
                        ' ' | '\t' => (), // skip
                        CR => self.state = HeaderValueDiscardWSAlmostDone,
                        LF => self.state = HeaderValueDiscardLWS,
                        _ => {
                            let c = to_lowercase(buf[0] as char);
                            self.hstate = match self.hstate {
                                HeaderConnection if c == 'k' => HeaderMatchingKeepAlive,
                                HeaderConnection if c == 'c' => HeaderMatchingClose,
                                HeaderConnection if c == 'u' => HeaderMatchingUpgrade,
                                _ => HeaderGeneral,
                            };
                            self.state = HeaderValue;
                            handler.push_data(self, buf);
                            self.index += 1;
                        },
                    }
                }
                HeaderValueDiscardWSAlmostDone => {
                    if buf[0] as char != LF { self.state = Crashed; return Err(InvalidHeaderField) }
                    self.state = HeaderValueDiscardLWS;
                }
                HeaderValueDiscardLWS => {
                    if buf[0] as char == ' ' || buf[0] as char == '\t' {
                        self.state = HeaderValueDiscardWS;
                    } else {
                        // header value is empty.
                        match handler.on_header_value(self, 0) {
                            Err(e) => { self.state = Crashed; return Err(AnyIoError(e)) },
                            _ => self.index = 0,
                        }
                        match buf[0] as char {
                            CR => self.state = HeadersAlmostDone,
                            LF => {
                                self.state = HeadersDone;
                                self.skip_body = handler.on_headers_complete(self);
                            }
                            c if is_token(c) => {
                                handler.push_data(self, buf);
                                self.state = HeaderFieldStart;
                                self.index = 1;
                            }
                            _ => { self.state = Crashed; return Err(InvalidHeaderField) },
                        }
                    }
                }
                HeaderValue => {
                    match buf[0] as char {
                        CR | LF => {
                            self.state = if buf[0] as char == CR {
                                HeaderAlmostDone
                            } else {
                                HeaderFieldStart
                            };
                            match self.hstate {
                                HeaderMatchingKeepAlive if self.index == 10 => self.keep_alive = true,
                                HeaderMatchingClose     if self.index == 5  => self.keep_alive = false,
                                HeaderMatchingUpgrade   if self.index == 6  => self.upgrade = true,
                                _ => (),
                            }
                            match handler.on_header_value(self, self.index) {
                                Err(e) => { self.state = Crashed; return Err(AnyIoError(e)) },
                                _ => self.index = 0,
                            }
                        }
                        _ => {
                            if self.hstate != HeaderGeneral && is_token(buf[0] as char) {
                                let c = to_lowercase(buf[0] as char);
                                self.hstate = match self.hstate {
                                    HeaderMatchingKeepAlive => match c {
                                        'e' if self.index == 1 => HeaderMatchingKeepAlive,
                                        'e' if self.index == 2 => HeaderMatchingKeepAlive,
                                        'p' if self.index == 3 => HeaderMatchingKeepAlive,
                                        '-' if self.index == 4 => HeaderMatchingKeepAlive,
                                        'a' if self.index == 5 => HeaderMatchingKeepAlive,
                                        'l' if self.index == 6 => HeaderMatchingKeepAlive,
                                        'i' if self.index == 7 => HeaderMatchingKeepAlive,
                                        'v' if self.index == 8 => HeaderMatchingKeepAlive,
                                        'e' if self.index == 9 => HeaderMatchingKeepAlive,
                                        _ => HeaderGeneral,
                                    },
                                    HeaderMatchingClose => match c {
                                        'l' if self.index == 1 => HeaderMatchingClose,
                                        'o' if self.index == 2 => HeaderMatchingClose,
                                        's' if self.index == 3 => HeaderMatchingClose,
                                        'e' if self.index == 4 => HeaderMatchingClose,
                                        _ => HeaderGeneral,
                                    },
                                    HeaderMatchingUpgrade => match c {
                                        'p' if self.index == 1 => HeaderMatchingUpgrade,
                                        'g' if self.index == 2 => HeaderMatchingUpgrade,
                                        'r' if self.index == 3 => HeaderMatchingUpgrade,
                                        'a' if self.index == 4 => HeaderMatchingUpgrade,
                                        'd' if self.index == 5 => HeaderMatchingUpgrade,
                                        'e' if self.index == 6 => HeaderMatchingUpgrade,
                                        _ => HeaderGeneral,
                                    },
                                    _ => HeaderGeneral,
                                };
                            }
                            handler.push_data(self, buf);
                            self.index += 1;
                        }
                    }
                }
                HeaderAlmostDone => {
                    if buf[0] as char != LF { self.state = Crashed; return Err(InvalidHeaderField) }
                    self.state = HeaderFieldStart;
                }
                HeadersAlmostDone => {
                    if buf[0] as char != LF { self.state = Crashed; return Err(InvalidHeaders) }
                    if handler.on_headers_complete(self) || self.skip_body {
                        self.state = reset_state!(self.parser_type);
                        handler.on_message_complete(self);
                        break;
                    }
                    self.state = HeadersDone;
                }
                HeadersDone => {
                    if self.skip_body {
                        self.state = reset_state!(self.parser_type);
                        handler.on_message_complete(self);
                        break;
                    }
                    match self.content_length {
                        0u => {
                            self.state = reset_state!(self.parser_type);
                            handler.on_message_complete(self);
                            break;
                        }
                        UINT_MAX => {
                            if self.parser_type == Request || !self.needs_eof() {
                                self.state = reset_state!(self.parser_type);
                                handler.on_message_complete(self);
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

        return Ok(read);
    }

    /// HTTP version
    pub fn get_http_version(&self) -> Option<HttpVersion> {
        self.http_version
    }

    /// Connection: keep-alive or Connection: close
    pub fn should_keep_alive(&self) -> bool {
        self.keep_alive
    }

    /// Connection: upgrade
    pub fn should_upgrade(&self) -> bool {
        self.upgrade
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
fn is_token(c: char) -> bool {
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
}

#[deriving(PartialEq, Eq, Clone, Show)]
enum HeaderParseState {
    HeaderGeneral,
    HeaderContentLength,
    HeaderConnection,
    HeaderMatchingKeepAlive,
    HeaderMatchingClose,
    HeaderMatchingUpgrade,
    HeaderTransferEncoding,
    HeaderUpgrade,
}
