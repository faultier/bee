//! HTTP parser.

#![experimental]

use std::char::to_lowercase;
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

/// Parser event handler.
pub trait Handler {
    /// Called when start to parsing of message.
    /// Default implementation is nothing to do.
    fn on_message_begin(&mut self, _: &Parser) {
    }

    #[allow(unused_variable)]
    /// Called when url parsed.
    /// Default implementation is nothing to do.
    fn on_url<'a>(&mut self, _: &Parser, start: uint, end: uint) -> IoResult<()> {
        Ok(())
    }

    #[allow(unused_variable)]
    /// Called when status parsed.
    /// Default implementation is nothing to do.
    fn on_status<'a>(&mut self, _: &Parser, start: uint, end: uint) -> IoResult<()> {
        Ok(())
    }

    #[allow(unused_variable)]
    /// Called when header field's name parsed.
    /// Default implementation is nothing to do.
    fn on_header_field<'a>(&mut self, _: &Parser, keyst: uint, keyen: uint, valst: uint, valen: uint) -> IoResult<()> {
        Ok(())
    }

    /// Called when completed to parsing of headers.
    /// Default implementation is nothing to do.
    fn on_headers_complete(&mut self, _: &Parser) -> bool{
        return false;
    }

    #[allow(unused_variable)]
    /// Called when body parsed.
    /// Default implementation is nothing to do.
    fn on_body<'a>(&mut self, _: &Parser, start: uint, end: uint) -> IoResult<()> {
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
    pos: uint,
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
            pos: 0,
            index: 0,
            sep: 0,
            skip: 0,
            major: 0,
            minor: 0,
            keep_alive: false,
            upgrade: false,
        }
    }

   /// Parse HTTP message.
    pub fn parse<C: Handler>(&mut self, data: &[u8], handler: &mut C) -> ParseResult {
        if self.state == Dead { return Ok(0) }
        if self.state == Crashed { return Err(OtherParseError) }
        if data.len() == 0 { return Ok(0) }

        let mut read = 0u;
        let len = data.len();

        loop {
            match self.state {
                StartReq => {
                    self.major = 0;
                    self.minor = 0;
                    self.http_version = None;
                    self.content_length = UINT_MAX;
                    self.skip_body = false;
                    handler.on_message_begin(self);
                    self.method = Some(match data[read] as char {
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
                    read += 1;
                    self.pos += 1;
                    self.state = ReqMethod;
                    self.index = 1;
                }
                ReqMethod => {
                    let method = self.method.unwrap();
                    if data[read] as char == ' ' && self.pos == method.len() {
                        self.state = ReqUrl;
                        self.index = 0;
                    } else {
                        if !method.hit(self.index, data[read] as char) {
                            self.method = Some(match method {
                                HttpConnect    if self.index == 2 && data[read] as char == 'H' => HttpCheckout,
                                HttpConnect    if self.index == 3 && data[read] as char == 'P' => HttpCheckout,
                                HttpLink       if self.index == 1 && data[read] as char == 'O' => HttpLock,
                                HttpMkCol      if self.index == 1 && data[read] as char == '-' => HttpMsearch,
                                HttpMkCol      if self.index == 1 && data[read] as char == 'E' => HttpMerge,
                                HttpMkCol      if self.index == 2 && data[read] as char == 'A' => HttpMkActivity,
                                HttpMkCol      if self.index == 3 && data[read] as char == 'A' => HttpMkCalendar,
                                HttpPut        if self.index == 1 && data[read] as char == 'A' => HttpPatch,
                                HttpPut        if self.index == 1 && data[read] as char == 'O' => HttpPost,
                                HttpPut        if self.index == 1 && data[read] as char == 'R' => HttpPropPatch,
                                HttpPut        if self.index == 2 && data[read] as char == 'R' => HttpPurge,
                                HttpPropPatch  if self.index == 4 && data[read] as char == 'F' => HttpPropFind,
                                HttpSearch     if self.index == 1 && data[read] as char == 'U' => HttpSubscribe,
                                HttpUnlink     if self.index == 2 && data[read] as char == 'S' => HttpUnsubscribe,
                                HttpUnlink     if self.index == 3 && data[read] as char == 'O' => HttpUnlock,
                                _ => { self.state = Crashed; return Err(InvalidMethod) },
                            });
                        }
                        self.index += 1;
                    }
                    read += 1;
                    self.pos += 1;
                }
                ReqUrl => {
                    match data[read] as char {
                        ' ' => {
                            if self.index == 0 { self.state = Crashed; return Err(InvalidUrl) }
                            match handler.on_url(self, self.pos - self.index, self.pos) {
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
                            match handler.on_url(self, self.pos - self.index, self.pos) {
                                Ok(()) => {
                                    self.state = Dead;
                                    self.index = 0;
                                    self.pos = 0;
                                    handler.on_message_complete(self);
                                    read += 1;
                                    break;
                                }
                                Err(e) => { self.state = Crashed; return Err(UrlCallbackFail(e)) },
                            }
                        }
                        _ => self.index += 1,
                    }
                    read += 1;
                    self.pos += 1;
                }
                ReqHttpStart => {
                    let c = data[read] as char;
                    if (c != 'H' && self.index == 0)
                        || (c != 'T' && (self.index == 1 || self.index == 2))
                        || (c != 'P' && self.index == 3)
                        || (c != '/' && self.index == 4)
                        || ((data[read] < '0' as u8 || data[read] > '9' as u8) && self.index == 5) {
                            self.state = Crashed;
                            return Err(InvalidVersion);
                        }
                    if self.index == 5 {
                        self.state = ReqHttpMajor;
                        self.major = data[read] as uint - '0' as uint;
                        self.index = 1;
                    } else {
                        self.index += 1;
                    }
                    read += 1;
                    self.pos += 1;
                }
                ReqHttpMajor => {
                    match data[read] as char {
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
                    read += 1;
                    self.pos += 1;
                }
                ReqHttpMinor => {
                    match data[read] as char {
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
                                self.state = if data[read] as char == CR {
                                    ReqLineAlmostDone
                                } else {
                                    HeaderFieldStart
                                };
                                self.index = 0;
                            }
                        },
                        _ => { self.state = Crashed; return Err(InvalidVersion) },
                    }
                    read += 1;
                    self.pos += 1;
                }
                ReqLineAlmostDone => {
                    if data[read] as char != LF {
                        return Err(InvalidRequestLine);
                    }
                    self.state = HeaderFieldStart;
                    read += 1;
                    self.pos += 1;
                }
                HeaderFieldStart => {
                    match data[read] as char {
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
                            self.index = 1;
                        }
                        _ => { self.state = Crashed; return Err(InvalidHeaderField) },
                    }
                    read += 1;
                    self.pos += 1;
                }
                HeaderField => {
                    match data[read] as char {
                        ':' => {
                            self.state = HeaderValueDiscardWS;
                            self.sep = self.pos;
                            self.skip = self.pos;
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
                            self.index += 1;
                        }
                        _ => { self.state = Crashed; return Err(InvalidHeaderField) },
                    }
                    read += 1;
                    self.pos += 1;
                }
                HeaderValueDiscardWS => {
                    match data[read] as char {
                        ' ' | '\t' => {
                            self.skip += 1;
                        },
                        CR => self.state = HeaderValueDiscardWSAlmostDone,
                        LF => self.state = HeaderValueDiscardLWS,
                        _ => {
                            let c = to_lowercase(data[read] as char);
                            self.hstate = match self.hstate {
                                HeaderConnection if c == 'k' => HeaderMatchingKeepAlive,
                                HeaderConnection if c == 'c' => HeaderMatchingClose,
                                HeaderConnection if c == 'u' => HeaderMatchingUpgrade,
                                _ => HeaderGeneral,
                            };
                            self.state = HeaderValue;
                            self.index += 1;
                        },
                    }
                    read += 1;
                    self.pos += 1;
                }
                HeaderValueDiscardWSAlmostDone => {
                    if data[read] as char != LF { self.state = Crashed; return Err(InvalidHeaderField) }
                    self.state = HeaderValueDiscardLWS;
                    read += 1;
                    self.pos += 1;
                }
                HeaderValueDiscardLWS => {
                    if data[read] as char == ' ' || data[read] as char == '\t' {
                        self.state = HeaderValueDiscardWS;
                        self.skip += 1;
                    } else {
                        // header value is empty.
                        match handler.on_header_field(self, self.pos - self.index, self.sep, 0, 0) {
                            Err(e) => { self.state = Crashed; return Err(HeaderFieldCallbackFail(e)) },
                            _ => {
                                self.index = 0;
                                self.sep = 0;
                            },
                        }
                        match data[read] as char {
                            CR => self.state = HeadersAlmostDone,
                            LF => {
                                self.state = HeadersDone;
                                self.skip_body = handler.on_headers_complete(self);
                            }
                            c if is_token(c) => {
                                self.state = HeaderFieldStart;
                                self.index = 1;
                                self.sep = 0;
                            }
                            _ => { self.state = Crashed; return Err(InvalidHeaderField) },
                        }
                    }
                    read += 1;
                    self.pos += 1;
                }
                HeaderValue => {
                    match data[read] as char {
                        CR | LF => {
                            self.state = if data[read] as char == CR {
                                HeaderAlmostDone
                            } else {
                                HeaderFieldStart
                            };
                            let index = self.pos - self.skip - 1;
                            match self.hstate {
                                HeaderMatchingKeepAlive if index == 10 => self.keep_alive = true,
                                HeaderMatchingClose     if index == 5  => self.keep_alive = false,
                                HeaderMatchingUpgrade   if index == 6  => self.upgrade = true,
                                _ => (),
                            }
                            match handler.on_header_field(self, self.pos-(self.index+1), self.sep, self.skip+1, self.pos) {
                                Err(e) => { self.state = Crashed; return Err(HeaderFieldCallbackFail(e)) },
                                _ => {
                                    self.index = 0;
                                    self.sep = 0;
                                    self.skip = 0;
                                },
                            }
                        }
                        _ => {
                            if self.hstate != HeaderGeneral && is_token(data[read] as char) {
                                let c = to_lowercase(data[read] as char);
                                let index = self.pos - self.skip - 1;
                                self.hstate = match self.hstate {
                                    HeaderMatchingKeepAlive => match c {
                                        'e' if index == 1 => HeaderMatchingKeepAlive,
                                        'e' if index == 2 => HeaderMatchingKeepAlive,
                                        'p' if index == 3 => HeaderMatchingKeepAlive,
                                        '-' if index == 4 => HeaderMatchingKeepAlive,
                                        'a' if index == 5 => HeaderMatchingKeepAlive,
                                        'l' if index == 6 => HeaderMatchingKeepAlive,
                                        'i' if index == 7 => HeaderMatchingKeepAlive,
                                        'v' if index == 8 => HeaderMatchingKeepAlive,
                                        'e' if index == 9 => HeaderMatchingKeepAlive,
                                        _ => HeaderGeneral,
                                    },
                                    HeaderMatchingClose => match c {
                                        'l' if index == 1 => HeaderMatchingClose,
                                        'o' if index == 2 => HeaderMatchingClose,
                                        's' if index == 3 => HeaderMatchingClose,
                                        'e' if index == 4 => HeaderMatchingClose,
                                        _ => HeaderGeneral,
                                    },
                                    HeaderMatchingUpgrade => match c {
                                        'p' if index == 1 => HeaderMatchingUpgrade,
                                        'g' if index == 2 => HeaderMatchingUpgrade,
                                        'r' if index == 3 => HeaderMatchingUpgrade,
                                        'a' if index == 4 => HeaderMatchingUpgrade,
                                        'd' if index == 5 => HeaderMatchingUpgrade,
                                        'e' if index == 6 => HeaderMatchingUpgrade,
                                        _ => HeaderGeneral,
                                    },
                                    _ => HeaderGeneral,
                                };
                            }
                            self.index += 1;
                        }
                    }
                    read += 1;
                    self.pos += 1;
                }
                HeaderAlmostDone => {
                    if data[read] as char != LF { self.state = Crashed; return Err(InvalidHeaderField) }
                    self.state = HeaderFieldStart;
                    read += 1;
                    self.pos += 1;
                }
                HeadersAlmostDone => {
                    if data[read] as char != LF { self.state = Crashed; return Err(InvalidHeaders) }
                    self.pos += 1;
                    read += 1;
                    if handler.on_headers_complete(self) || self.skip_body {
                        self.state = reset_state!(self.parser_type);
                        handler.on_message_complete(self);
                        break;
                    }
                    self.state = HeadersDone;
                }
                HeadersDone => {
                    read += 1;
                    if self.skip_body {
                        self.state = reset_state!(self.parser_type);
                        handler.on_message_complete(self);
                        self.pos = 0;
                        break;
                    }
                    match self.content_length {
                        0u => {
                            self.state = reset_state!(self.parser_type);
                            handler.on_message_complete(self);
                            self.pos = 0;
                            break;
                        }
                        UINT_MAX => {
                            if self.parser_type == Request || !self.needs_eof() {
                                self.state = reset_state!(self.parser_type);
                                handler.on_message_complete(self);
                                self.pos = 0;
                                break;
                            }
                            self.state = BodyIdentityEOF;
                        }
                        _ => self.state = BodyIdentity,
                    }
                    self.pos += 1;
                }
                Dead | Crashed => unreachable!(),
                _ => unimplemented!()
            }
            if read == len { break }
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

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;
    use std::io::{InvalidInput, IoResult, standard_error};
    use std::str::{SendStr, from_utf8};

    pub struct TestHandler<'a> {
        skip_body: bool,
        started: bool,
        url: Option<String>,
        headers_finished: bool,
        headers: HashMap<SendStr, SendStr>,
        finished: bool,
        data: &'a [u8],
    }

    impl<'a> TestHandler<'a> {
        fn new(skip_body: bool, data: &'a [u8]) -> TestHandler<'a> {
            TestHandler {
                skip_body: skip_body,
                started: false,
                url: None,
                headers_finished: false,
                headers: HashMap::new(),
                finished: false,
                data: data,
            }
        }

        fn assert_general_headers(&self) {
            let mut h = HashMap::new();
            h.insert("Host".into_maybe_owned(), "faultier.jp".into_maybe_owned());
            h.insert("User-Agent".into_maybe_owned(), "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.9; rv:30.0) Gecko/20100101 Firefox/30.0".into_maybe_owned());
            h.insert("Accept".into_maybe_owned(), "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".into_maybe_owned());
            h.insert("Accept-Encoding".into_maybe_owned(), "gzip,deflate".into_maybe_owned());
            h.insert("Accept-Language".into_maybe_owned(), "ja,en-US;q=0.8,en;q=0.6".into_maybe_owned());
            h.insert("Cache-Control".into_maybe_owned(), "max-age=0".into_maybe_owned());
            h.insert("Cookie".into_maybe_owned(), "key1=value1; key2=value2".into_maybe_owned());
            h.insert("Referer".into_maybe_owned(), "http://faultier.blog.jp/".into_maybe_owned());
 
            assert!(self.headers_finished);
            for (name, value) in h.iter() {
                assert_eq!(self.headers.find(name), Some(value));
            }
        }
    }

    impl<'a> Handler for TestHandler<'a> {
        fn on_message_begin(&mut self, _: &Parser) {
            self.started = true;
        }

        fn on_url<'b>(&mut self, _: &Parser, start: uint, end: uint) -> IoResult<()> {
            match from_utf8(self.data.slice(start, end)) {
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

        fn on_header_field<'a>(&mut self, _: &Parser, keyst: uint, keyen: uint, valst: uint, valen: uint) -> IoResult<()> {
            let mut header_name: SendStr;
            let mut header_value: SendStr;
            {
                let v = Vec::from_slice(self.data.slice(keyst, keyen));
                header_name = match String::from_utf8(v) {
                    Ok(name) => name.into_maybe_owned(),
                    Err(_) => return Err(standard_error(InvalidInput)),
                }
            }
            {
                let v = Vec::from_slice(self.data.slice(valst, valen));
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
        let data = [];
        let mut parser = Parser::new(Request);
        let mut handler = TestHandler::new(true, data);
        assert_eq!(parser.parse(data, &mut handler), Ok(0));
        assert!(!handler.started);
        assert!(!handler.finished);
    }

    #[test]
    fn test_partial_data() {
        let data = "HEAD /hello.txt HTTP/1.0\r\n\r\n".as_bytes();
        let mut parser = Parser::new(Request);
        let mut handler = TestHandler::new(true, data);

        // parse "HE"
        let part = data.slice(0, 2);
        assert_eq!(parser.parse(part, &mut handler), Ok(2));
        assert_eq!(parser.state, super::ReqMethod);
        assert!(handler.started);
        assert!(!handler.finished);

        // parse "AD "
        let part = data.slice(2, 5);
        assert_eq!(parser.parse(part, &mut handler), Ok(3));
        assert_eq!(parser.state, super::ReqUrl);
        assert_eq!(parser.method, Some(HttpHead));
        assert!(!handler.finished);

        // parse "/hello"
        let part = data.slice(5, 11);
        assert_eq!(parser.parse(part, &mut handler), Ok(6));
        assert_eq!(parser.state, super::ReqUrl);
        assert!(!handler.finished);

        // parse ".txt HTTP/1.0"
        let part = data.slice_from(11);
        assert_eq!(parser.parse(part, &mut handler), Ok(part.len()));
        assert_eq!(handler.url, Some("/hello.txt".to_string()));
        assert_eq!(parser.http_version, Some(HTTP_1_0));
        assert!(handler.finished);
    }

    mod http_0_9 {
        use super::TestHandler;
        use super::super::*;

        #[test]
        fn test_simple_request_get() {
            let msg = "GET /\r\n";
            let data = msg.as_bytes();
            let mut parser = Parser::new(Request);
            let mut handler = TestHandler::new(true, data);

            assert_eq!(parser.parse(data, &mut handler), Ok(6));
            assert!(handler.started);
            assert_eq!(handler.url, Some("/".to_string()));
            assert_eq!(parser.http_version, Some(HTTP_0_9));
            assert!(handler.finished);

            // Parser is dead, no more read.
            assert_eq!(parser.parse(data, &mut handler), Ok(0));
        }
    }

    mod http_1_0 {
        use super::TestHandler;
        use super::super::*;
        use std::collections::HashMap;

        #[test]
        fn test_request_without_header() {
            let msg = "GET / HTTP/1.0\r\n\r\n";
            let data = msg.as_bytes();
            let mut parser = Parser::new(Request);
            let mut handler = TestHandler::new(true, data);
            assert_eq!(parser.parse(data, &mut handler), Ok(data.len()));
            assert!(handler.started);
            assert_eq!(handler.url, Some("/".to_string()));
            assert_eq!(parser.http_version, Some(HTTP_1_0));
            assert!(handler.headers_finished);
            assert!(handler.finished);
        }

        #[test]
        fn test_request_get() {
            let msg = create_request("GET", "/tag/Rust", None, None);
            let data = msg.as_bytes();
            let mut parser = Parser::new(Request);
            let mut handler = TestHandler::new(true, data);
            assert_eq!(parser.parse(data, &mut handler), Ok(data.len()));
            assert!(!parser.keep_alive);
            assert!(handler.started);
            assert_eq!(handler.url, Some("/tag/Rust".to_string()));
            assert_eq!(parser.http_version, Some(HTTP_1_0));
            assert!(handler.finished);
            handler.assert_general_headers();
        }

        #[test]
        fn test_request_keep_alive() {
            let mut header = HashMap::new();
            header.insert("Connection".to_string(), "keep-alive".to_string());
            let msg = create_request("GET", "/keep-alive", Some(header), None);
            let data = msg.as_bytes();
            let mut parser = Parser::new(Request);
            let mut handler = TestHandler::new(true, data);
            assert_eq!(parser.parse(data, &mut handler), Ok(data.len()));
            assert!(parser.keep_alive);
        }

        fn create_request(method: &'static str, url: &'static str, header: Option<HashMap<String, String>>, body: Option<String>) -> String {
            let mut h = HashMap::new();
            h.insert("Host".to_string(), "faultier.jp".to_string());
            h.insert("User-Agent".to_string(), "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.9; rv:30.0) Gecko/20100101 Firefox/30.0".to_string());
            h.insert("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".to_string());
            h.insert("Accept-Encoding".to_string(), "gzip,deflate".to_string());
            h.insert("Accept-Language".to_string(), "ja,en-US;q=0.8,en;q=0.6".to_string());
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
    }

    mod http_1_1 {
        use super::TestHandler;
        use super::super::*;
        use std::collections::HashMap;

        #[test]
        fn test_request_get() {
            let msg = create_request("GET", "/tag/Rust", None, None);
            let data = msg.as_bytes();
            let mut parser = Parser::new(Request);
            let mut handler = TestHandler::new(true, data);
            assert_eq!(parser.parse(data, &mut handler), Ok(data.len()));
            assert!(parser.keep_alive);
            assert!(handler.started);
            assert_eq!(handler.url, Some("/tag/Rust".to_string()));
            assert_eq!(parser.http_version, Some(HTTP_1_1));
            assert!(handler.finished);
            handler.assert_general_headers();
        }

        #[test]
        fn test_request_close() {
            let mut header = HashMap::new();
            header.insert("Connection".to_string(), "close".to_string());
            let msg = create_request("GET", "/close", Some(header), None);
            let data = msg.as_bytes();
            let mut parser = Parser::new(Request);
            let mut handler = TestHandler::new(true, data);
            assert_eq!(parser.parse(data, &mut handler), Ok(data.len()));
            assert!(!parser.keep_alive);
        }

        fn create_request(method: &'static str, url: &'static str, header: Option<HashMap<String, String>>, body: Option<String>) -> String {
            let mut h = HashMap::new();
            h.insert("Host".to_string(), "faultier.jp".to_string());
            h.insert("User-Agent".to_string(), "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.9; rv:30.0) Gecko/20100101 Firefox/30.0".to_string());
            h.insert("Accept".to_string(), "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8".to_string());
            h.insert("Accept-Encoding".to_string(), "gzip,deflate".to_string());
            h.insert("Accept-Language".to_string(), "ja,en-US;q=0.8,en;q=0.6".to_string());
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

    }
}

#[cfg(test)]
mod bench {
    use super::*;
    use test::Bencher;

    struct BenchHandler {
        skip_body: bool
    }

    impl Handler for BenchHandler {
        fn on_headers_complete(&mut self, _: &Parser) -> bool { self.skip_body }
    }

    #[bench]
    fn bench_no_message(b: &mut Bencher) {
        let buf: &[u8] = [];
        let mut handler = BenchHandler { skip_body: true };
        b.iter(|| Parser::new(Request).parse(buf, &mut handler) );
    }

    mod http_0_9 {
        use super::BenchHandler;
        use super::super::*;
        use test::Bencher;

        #[bench]
        fn bench_simple_request_get(b: &mut Bencher) {
            let msg = "GET /\r\n";
            let data = msg.as_bytes();
            let mut handler = BenchHandler { skip_body: true };
            b.iter(|| Parser::new(Request).parse(data, &mut handler) );
        }
    }

    mod http_1_0 {
        use super::BenchHandler;
        use super::super::*;
        use test::Bencher;
        use std::collections::HashMap;

        #[bench]
        fn bench_request_without_header(b: &mut Bencher) {
            let msg = "GET / HTTP/1.0\r\n\r\n";
            let data = msg.as_bytes();
            let mut handler = BenchHandler { skip_body: true };
            b.iter(|| Parser::new(Request).parse(data, &mut handler) );
        }

        #[bench]
        fn bench_request_get(b: &mut Bencher) {
            let msg = create_request("GET", "/tag/Rust", None, None);
            let data = msg.as_bytes();
            let mut handler = BenchHandler { skip_body: true };
            b.iter(|| Parser::new(Request).parse(data, &mut handler) );
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
        use super::BenchHandler;
        use super::super::*;
        use test::Bencher;
        use std::collections::HashMap;

        #[bench]
        fn bench_request_get(b: &mut Bencher) {
            let msg = create_request("GET", "/tag/Rust", None, None);
            let data = msg.as_bytes();
            let mut handler = BenchHandler { skip_body: true };
            b.iter(|| Parser::new(Request).parse(data, &mut handler) );
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
