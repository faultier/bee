//! HTTP parser.

#![experimental]

use std::char::to_lowercase;
use UINT_MAX = std::uint::MAX;

use http;

#[deriving(PartialEq, Eq, Clone, Show)]
/// A parser types.
pub enum ParseType {
    /// Parse request only.
    ParseRequest,
    /// Parse response only.
    ParseResponse,
    /// Parse request or response.
    ParseBoth,
}

static CR: char = '\r';
static LF: char = '\n';

/// Parser event handler.
pub trait MessageHandler {
    #[allow(unused_variable)]
    /// Called when start to parsing of message.
    /// Default implementation is nothing to do.
    fn on_message_begin(&mut self, parser: &Parser) {
    }

    #[allow(unused_variable)]
    /// Called when request method parsed.
    /// Default implementation is nothing to do.
    fn on_method(&mut self, parser: &Parser, method: http::HttpMethod) {
    }

    #[allow(unused_variable)]
    /// Called when url parsed.
    /// Default implementation is nothing to do.
    fn on_url(&mut self, parser: &Parser, length: uint) {
    }

    #[allow(unused_variable)]
    /// Called when HTTP version parsed.
    /// Default implementation is nothing to do.
    fn on_version(&mut self, parser: &Parser, version: http::HttpVersion) {
    }

    #[allow(unused_variable)]
    /// Called when request method parsed.
    /// Default implementation is nothing to do.
    fn on_status(&mut self, parser: &Parser, status: uint) {
    }

    #[allow(unused_variable)]
    /// Called when header field's name parsed.
    /// Default implementation is nothing to do.
    fn on_header_field(&mut self, parser: &Parser, length: uint) {
    }

    #[allow(unused_variable)]
    /// Called when header field's value parsed.
    /// Default implementation is nothing to do.
    fn on_header_value(&mut self, parser: &Parser, length: uint) {
    }

    #[allow(unused_variable)]
    /// Called when completed to parsing of headers.
    /// Default implementation is nothing to do.
    /// If returned true, skip parsing message body.
    fn on_headers_complete(&mut self, parser: &Parser) -> bool {
        return false;
    }

    #[allow(unused_variable)]
    /// Called when body parsed.
    /// Default implementation is nothing to do.
    fn on_body(&mut self, parser: &Parser, length: uint) {
    }

    #[allow(unused_variable)]
    /// Called when completed to parsing of whole message.
    /// Default implementation is nothing to do.
    fn on_message_complete(&mut self, parser: &Parser) {
    }

    /// Write partial data to buffer, e.g. URL, header field, message body.
    fn write(&mut self, &Parser, &[u8]);
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
    /// Invalid status code.
    InvalidStatusCode,
    /// Invalid status line.
    InvalidStatusLine,
    /// Invalid header field.
    InvalidHeaderField,
    /// Invalid header section.
    InvalidHeaders,
    /// Invalid chunk data.
    InvalidChunk,
    /// Expected data, but reached EOF.
    InvalidEOFState,
}

pub type ParseResult = Result<uint, ParseError>;

/// HTTP parser.
pub struct Parser {
    // parser internal state
    parser_type: ParseType,
    state: ParserState,
    hstate: HeaderState,
    index: uint,
    skip_body: bool,

    // http version
    http_version: Option<http::HttpVersion>,
    major: uint,
    minor: uint,

    // common header
    message_body_rest: uint,
    upgrade: bool,
    keep_alive: bool,
    chunked: bool,

    // request
    method: Option<http::HttpMethod>,

    // response
    status_code: uint,
}

impl Parser {
    /// Create a new `Parser`.
    pub fn new(t: ParseType) -> Parser {
        Parser {
            parser_type: t,
            http_version: None,
            state: match t {
                ParseRequest  => StartReq,
                ParseResponse => StartRes,
                ParseBoth     => StartReqOrRes,
            },
            hstate: HeaderGeneral,
            method: None,
            status_code: 0,
            message_body_rest: UINT_MAX,
            skip_body: false,
            index: 0,
            major: 0,
            minor: 0,
            keep_alive: false,
            upgrade: false,
            chunked: false,
        }
    }

    #[allow(unused_must_use)]
    /// Parse HTTP message.
    pub fn parse<C: MessageHandler>(&mut self, data: &[u8], handler: &mut C) -> ParseResult {
        if self.state == Dead { return Ok(0) }
        if self.state == Crashed { return Err(OtherParseError) }
        if data.len() == 0 { return Ok(0) }

        let mut read = 0u;

        if !(self.state == BodyIdentity
             || self.state == BodyIdentityEOF
             || self.state == ChunkSize
             || self.state == ChunkSizeAlmostDone
             || self.state == ChunkData) {
            for &byte in data.iter() {
                read += 1;
                match self.state {
                    StartReq => {
                        self.method = Some(match byte as char {
                            'C' => http::HttpConnect,     // or CHECKOUT, COPY
                            'D' => http::HttpDelete,
                            'G' => http::HttpGet,
                            'H' => http::HttpHead,
                            'L' => http::HttpLink,        // or LOCK
                            'M' => http::HttpMkCol,       // or M-SEARCH, MERGE, MKACTIVITY, MKCALENDER
                            'N' => http::HttpNotify,
                            'O' => http::HttpOptions,
                            'P' => http::HttpPut,         // or PATCH, POST, PROPPATCH, PROPFIND
                            'R' => http::HttpReport,
                            'S' => http::HttpSearch,      // or SUBSCRIBE
                            'T' => http::HttpTrace,
                            'U' => http::HttpUnlink,      // or UNLOCK, UNSUBSCRIBE
                            CR | LF => break,
                            _   => { self.state = Crashed; return Err(InvalidMethod) },
                        });
                        handler.on_message_begin(self);
                        self.state = ReqMethod;
                        self.index = 1;
                    }
                    StartRes => {
                        match byte as char {
                            'H' => {
                                self.state = ResHttpStart;
                                self.index = 1;
                            },
                            CR | LF => break,
                            _   => { self.state = Crashed; return Err(InvalidMethod) },
                        }
                        handler.on_message_begin(self);
                    }
                    ReqMethod => {
                        let method = self.method.unwrap();
                        if byte as char == ' ' {
                            handler.on_method(self, method);
                            self.state = ReqUrl;
                            self.index = 0;
                        } else {
                            if !method.hit(self.index, byte as char) {
                                self.method = Some(match method {
                                    http::HttpConnect    if self.index == 2 && byte as char == 'H' => http::HttpCheckout,
                                    http::HttpConnect    if self.index == 3 && byte as char == 'P' => http::HttpCheckout,
                                    http::HttpLink       if self.index == 1 && byte as char == 'O' => http::HttpLock,
                                    http::HttpMkCol      if self.index == 1 && byte as char == '-' => http::HttpMsearch,
                                    http::HttpMkCol      if self.index == 1 && byte as char == 'E' => http::HttpMerge,
                                    http::HttpMkCol      if self.index == 2 && byte as char == 'A' => http::HttpMkActivity,
                                    http::HttpMkCol      if self.index == 3 && byte as char == 'A' => http::HttpMkCalendar,
                                    http::HttpPut        if self.index == 1 && byte as char == 'A' => http::HttpPatch,
                                    http::HttpPut        if self.index == 1 && byte as char == 'O' => http::HttpPost,
                                    http::HttpPut        if self.index == 1 && byte as char == 'R' => http::HttpPropPatch,
                                    http::HttpPut        if self.index == 2 && byte as char == 'R' => http::HttpPurge,
                                    http::HttpPropPatch  if self.index == 4 && byte as char == 'F' => http::HttpPropFind,
                                    http::HttpSearch     if self.index == 1 && byte as char == 'U' => http::HttpSubscribe,
                                    http::HttpUnlink     if self.index == 2 && byte as char == 'S' => http::HttpUnsubscribe,
                                    http::HttpUnlink     if self.index == 3 && byte as char == 'O' => http::HttpUnlock,
                                    _ => { self.state = Crashed; return Err(InvalidMethod) },
                                });
                            }
                            self.index += 1;
                        }
                    }
                    ReqUrl => {
                        match byte as char {
                            ' ' => {
                                if self.index == 0 { self.state = Crashed; return Err(InvalidUrl) }
                                let start = if read > self.index + 1 { read - self.index - 1 } else { 0 };
                                let end = read - 1;
                                handler.write(self, data.slice(start, end));
                                handler.on_url(self, self.index);
                                self.state = ReqHttpStart;
                                self.index = 0;
                            }
                            CR | LF => {
                                if self.index == 0 { self.state = Crashed; return Err(InvalidUrl) }
                                self.http_version = Some(http::HTTP_0_9);
                                let start = if read > self.index + 1 { read - self.index - 1 } else { 0 };
                                let end = read - 1;
                                handler.write(self, data.slice(start, end));
                                handler.on_url(self, self.index);
                                self.state = Dead;
                                self.index = 0;
                                handler.on_message_complete(self);
                                break;
                            }
                            _ => {
                                self.index += 1;
                            }
                        }
                    }
                    ReqHttpStart => {
                        let c = byte as char;
                        if (c != 'H' && self.index == 0)
                            || (c != 'T' && (self.index == 1 || self.index == 2))
                            || (c != 'P' && self.index == 3)
                            || (c != '/' && self.index == 4)
                            || ((byte < '0' as u8 || byte > '9' as u8) && self.index == 5) {
                                self.state = Crashed;
                                return Err(InvalidVersion);
                            }
                        if self.index == 5 {
                            self.state = ReqHttpMajor;
                            self.major = byte as uint - '0' as uint;
                            self.index = 1;
                        } else {
                            self.index += 1;
                        }
                    }
                    ReqHttpMajor => {
                        match byte as char {
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
                        match byte as char {
                            n if n >= '0' && n <= '9' => {
                                self.index += 1;
                                self.minor *= 10;
                                self.minor += n as uint - '0' as uint;
                            }
                            CR | LF if self.index > 0 => match http::HttpVersion::find(self.major, self.minor) {
                                None => { self.state = Crashed; return Err(InvalidVersion) },
                                v => {
                                    handler.on_version(self, v.unwrap());
                                    self.http_version = v;
                                    self.keep_alive = v == Some(http::HTTP_1_1);
                                    self.state = if byte as char == CR {
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
                        if byte as char != LF {
                            return Err(InvalidRequestLine);
                        }
                        self.state = HeaderFieldStart;
                    }
                    ResHttpStart => {
                        let c = byte as char;
                        if (c != 'T' && (self.index == 1 || self.index == 2))
                            || (c != 'P' && self.index == 3)
                            || (c != '/' && self.index == 4)
                            || ((byte < '0' as u8 || byte > '9' as u8) && self.index == 5) {
                                self.state = Crashed;
                                return Err(InvalidVersion);
                            }
                        if self.index == 5 {
                            self.state = ResHttpMajor;
                            self.major = byte as uint - '0' as uint;
                            self.index = 1;
                        } else {
                            self.index += 1;
                        }
                    }
                    ResHttpMajor => {
                        match byte as char {
                            '.' if self.index > 0 => {
                                self.state = ResHttpMinor;
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
                    ResHttpMinor => {
                        match byte as char {
                            n if n >= '0' && n <= '9' => {
                                self.index += 1;
                                self.minor *= 10;
                                self.minor += n as uint - '0' as uint;
                            }
                            ' ' if self.index > 0 => match http::HttpVersion::find(self.major, self.minor) {
                                None => { self.state = Crashed; return Err(InvalidVersion) },
                                v => {
                                    handler.on_version(self, v.unwrap());
                                    self.http_version = v;
                                    self.keep_alive = v == Some(http::HTTP_1_1);
                                    self.state = ResStatusCode;
                                    self.index = 0;
                                }
                            },
                            _ => { self.state = Crashed; return Err(InvalidVersion) },
                        }
                    }
                    ResStatusCodeStart => {
                        if byte >= '0' as u8 && byte <= '9' as u8 {
                            self.state = ResStatusCode;
                            self.status_code = byte as uint - '0' as uint;
                            self.index = 1;
                        } else if byte as char != ' ' {
                            self.state = Crashed;
                            return Err(InvalidStatusCode);
                        }
                    }
                    ResStatusCode => {
                        if byte >= '0' as u8 && byte <= '9' as u8 && self.index < 3 {
                            self.status_code *= 10;
                            self.status_code += byte as uint - '0' as uint;
                            self.index += 1;
                        } else {
                            handler.on_status(self, self.status_code);
                            self.state = match byte as char {
                                ' ' => ResStatus,
                                CR  => ResLineAlmostDone,
                                LF  => HeaderFieldStart,
                                _   => {
                                    self.state = Crashed;
                                    return Err(InvalidStatusLine);
                                }
                            };
                            self.index = 0;
                        }
                    }
                    ResStatus => {
                        self.state = match byte as char {
                            CR => ResLineAlmostDone,
                            LF => HeaderFieldStart,
                            _  => ResStatus,
                        };
                    }
                    ResLineAlmostDone => {
                        if byte as char != LF {
                            return Err(InvalidStatusLine);
                        }
                        self.state = HeaderFieldStart;
                    }
                    HeaderFieldStart => {
                        match byte as char {
                            CR => self.state = HeadersAlmostDone,
                            LF => {
                                if handler.on_headers_complete(self) || self.skip_body {
                                    handler.on_message_complete(self);
                                    self.reset();
                                } else {
                                    match self.message_body_rest {
                                        0u => {
                                            handler.on_message_complete(self);
                                            self.reset();
                                        }
                                        UINT_MAX => if self.parser_type == ParseRequest || !self.needs_eof() {
                                            handler.on_message_complete(self);
                                            self.reset();
                                        } else {
                                            self.state = BodyIdentityEOF;
                                        },
                                        _ => self.state = BodyIdentity,
                                    }
                                };
                                break
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
                    }
                    HeaderField => {
                        match byte as char {
                            ':' => {
                                let start = if read > self.index + 1 { read - self.index - 1} else { 0 };
                                let end = read - 1;
                                handler.write(self, data.slice(start, end));
                                handler.on_header_field(self, self.index);
                                self.state = HeaderValueDiscardWS;
                                self.index = 0;
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
                                        HeaderTransferEncoding => match to_lowercase(c) {
                                            'r' if self.index == 1  => HeaderTransferEncoding,
                                            'a' if self.index == 2  => HeaderTransferEncoding,
                                            'n' if self.index == 3  => HeaderTransferEncoding,
                                            's' if self.index == 4  => HeaderTransferEncoding,
                                            'f' if self.index == 5  => HeaderTransferEncoding,
                                            'e' if self.index == 6  => HeaderTransferEncoding,
                                            'r' if self.index == 7  => HeaderTransferEncoding,
                                            '-' if self.index == 8  => HeaderTransferEncoding,
                                            'e' if self.index == 9  => HeaderTransferEncoding,
                                            'n' if self.index == 10 => HeaderTransferEncoding,
                                            'c' if self.index == 11 => HeaderTransferEncoding,
                                            'o' if self.index == 12 => HeaderTransferEncoding,
                                            'd' if self.index == 13 => HeaderTransferEncoding,
                                            'i' if self.index == 14 => HeaderTransferEncoding,
                                            'n' if self.index == 15 => HeaderTransferEncoding,
                                            'g' if self.index == 16 => HeaderTransferEncoding,
                                            _ => HeaderGeneral,
                                        },
                                        _ => HeaderGeneral,
                                    };
                                }
                                self.index += 1;
                            }
                            _ => { self.state = Crashed; return Err(InvalidHeaderField) },
                        }
                    }
                    HeaderValueDiscardWS => {
                        match byte as char {
                            ' ' | '\t' => (), // skip
                            CR => self.state = HeaderValueDiscardWSAlmostDone,
                            LF => self.state = HeaderValueDiscardLWS,
                            _ => {
                                let c = to_lowercase(byte as char);
                                self.hstate = match self.hstate {
                                    HeaderConnection if c == 'c' => HeaderMatchingClose,
                                    HeaderConnection if c == 'k' => HeaderMatchingKeepAlive,
                                    HeaderConnection if c == 'u' => HeaderMatchingUpgrade,
                                    HeaderTransferEncoding if c == 'c' => HeaderMatchingChunked,
                                    HeaderContentLength => {
                                        self.message_body_rest = byte as uint - '0' as uint;
                                        HeaderContentLength
                                    },
                                    _ => HeaderGeneral,
                                };
                                self.state = HeaderValue;
                                self.index += 1;
                            },
                        }
                    }
                    HeaderValueDiscardWSAlmostDone => {
                        if byte as char != LF { self.state = Crashed; return Err(InvalidHeaderField) }
                        self.state = HeaderValueDiscardLWS;
                    }
                    HeaderValueDiscardLWS => {
                        if byte as char == ' ' || byte as char == '\t' {
                            self.state = HeaderValueDiscardWS;
                        } else {
                            // header value is empty.
                            handler.on_header_value(self, 0);
                            self.index = 0;
                            match byte as char {
                                CR => self.state = HeadersAlmostDone,
                                LF => {
                                    if handler.on_headers_complete(self) || self.upgrade || self.skip_body {
                                        handler.on_message_complete(self);
                                        self.reset();
                                    } else if self.chunked {
                                        self.state = ChunkSize;
                                        self.message_body_rest = 0;
                                    } else {
                                        match self.message_body_rest {
                                            0u => {
                                                handler.on_message_complete(self);
                                                self.reset();
                                            }
                                            UINT_MAX => if self.parser_type == ParseRequest || !self.needs_eof() {
                                                handler.on_message_complete(self);
                                                self.reset();
                                            } else {
                                                self.state = BodyIdentityEOF;
                                            },
                                            _ => self.state = BodyIdentity,
                                        }
                                    };
                                    break
                                }
                                c if is_token(c) => {
                                    self.state = HeaderFieldStart;
                                    self.index = 1;
                                }
                                _ => { self.state = Crashed; return Err(InvalidHeaderField) },
                            }
                        }
                    }
                    HeaderValue => {
                        match byte as char {
                            CR | LF => {
                                self.state = if byte as char == CR {
                                    HeaderAlmostDone
                                } else {
                                    HeaderFieldStart
                                };
                                match self.hstate {
                                    HeaderMatchingChunked   if self.index == 7  => self.chunked = true,
                                    HeaderMatchingClose     if self.index == 5  => self.keep_alive = false,
                                    HeaderMatchingKeepAlive if self.index == 10 => self.keep_alive = true,
                                    HeaderMatchingUpgrade   if self.index == 6  => self.upgrade = true,
                                    _ => (),
                                }
                                let start = if read > self.index + 1 { read - self.index - 1 } else { 0 };
                                let end = read - 1;
                                handler.write(self, data.slice(start, end));
                                handler.on_header_value(self, self.index);
                                self.index = 0;
                            }
                            _ => {
                                if self.hstate != HeaderGeneral && is_token(byte as char) {
                                    let c = to_lowercase(byte as char);
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
                                        HeaderMatchingChunked => match c {
                                            'h' if self.index == 1 => HeaderMatchingChunked,
                                            'u' if self.index == 2 => HeaderMatchingChunked,
                                            'n' if self.index == 3 => HeaderMatchingChunked,
                                            'k' if self.index == 4 => HeaderMatchingChunked,
                                            'e' if self.index == 5 => HeaderMatchingChunked,
                                            'd' if self.index == 6 => HeaderMatchingChunked,
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
                                        HeaderContentLength if byte >= '0' as u8 && byte <= '9' as u8 => {
                                            self.message_body_rest *= 10;
                                            self.message_body_rest += byte as uint - '0' as uint;
                                            HeaderContentLength
                                        }
                                        HeaderContentLength if byte < '0' as u8 || byte > '9' as u8 => {
                                            self.message_body_rest = UINT_MAX;
                                            HeaderGeneral
                                        }
                                        _ => HeaderGeneral,
                                    };
                                }
                                self.index += 1;
                            }
                        }
                    }
                    HeaderAlmostDone => {
                        if byte as char != LF { self.state = Crashed; return Err(InvalidHeaderField) }
                        self.state = HeaderFieldStart;
                    }
                    HeadersAlmostDone => {
                        if byte as char != LF { self.state = Crashed; return Err(InvalidHeaders) }
                        if handler.on_headers_complete(self) || self.upgrade || self.skip_body {
                            handler.on_message_complete(self);
                            self.reset();
                        } else if self.chunked {
                            self.state = ChunkSize;
                            self.message_body_rest = 0;
                        } else {
                            match self.message_body_rest {
                                0u => {
                                    handler.on_message_complete(self);
                                    self.reset();
                                }
                                UINT_MAX => if self.parser_type == ParseRequest || !self.needs_eof() {
                                    handler.on_message_complete(self);
                                    self.reset();
                                } else {
                                    self.state = BodyIdentityEOF;
                                },
                                _ => self.state = BodyIdentity,
                            }
                        }

                        break
                    }
                    BodyIdentity | BodyIdentityEOF | Dead | Crashed => unreachable!(),
                    _ => unimplemented!()
                }
            }
        }

        if self.chunked {
            'chunk: loop {
                if self.state == ChunkData {
                    let rest = data.len() - read;
                    if self.message_body_rest == 0 && rest > 2 {
                        if data[read] as char != CR || data[read+1] as char != LF {
                            self.state = Crashed;
                            return Err(InvalidChunk);
                        }
                        read += 2;
                        self.state = ChunkSize;
                    } else if rest >= self.message_body_rest {
                        handler.write(self, data.slice(read, read + self.message_body_rest));
                        read += self.message_body_rest;
                        self.message_body_rest = 0;
                        if data.len() - read < 2 { break 'chunk }
                        read += 2;
                        self.state = ChunkSize;
                    } else {
                        handler.write(self, data.slice_from(read));
                        read += rest;
                        self.message_body_rest -= rest;
                        break 'chunk;
                    }
                } else {
                    'chunksize: for &byte in data.slice_from(read).iter() {
                        read += 1;
                        match self.state {
                            ChunkExtension if byte as char == CR => {
                                self.state = ChunkSizeAlmostDone;
                            }
                            ChunkExtension => { /* ignore */ }
                            ChunkSize if byte as char == ';' => {
                                self.state = ChunkExtension;
                            }
                            ChunkSize if byte as char == CR => {
                                self.state = ChunkSizeAlmostDone;
                            }
                            ChunkSize => {
                                let val = unhex(byte);
                                if val > 15 { self.state = Crashed; return Err(InvalidChunk) }
                                self.message_body_rest *= 16;
                                self.message_body_rest += val;
                            }
                            ChunkSizeAlmostDone => {
                                if byte as char != LF { self.state = Crashed; return Err(InvalidChunk) }
                                if self.message_body_rest == 0 {
                                    handler.on_message_complete(self);
                                    break 'chunk;
                                } else {
                                    self.state = ChunkData;
                                    break 'chunksize;
                                }
                            }
                            _ => unreachable!()
                        }
                    }
                }
            }
        }

        match self.state {
            BodyIdentity => {
                let rest = data.len() - read;
                if rest >= self.message_body_rest {
                    handler.write(self, data.slice(read, read + self.message_body_rest));
                    handler.on_body(self, self.message_body_rest);
                    handler.on_message_complete(self);
                    read += self.message_body_rest;
                    self.reset();
                } else {
                    handler.write(self, data.slice_from(read));
                    read += rest;
                    self.message_body_rest -= rest;
                }
            }
            ReqUrl | HeaderField | HeaderValue => {
                let start = if read > self.index { read - self.index } else { 0 };
                handler.write(self, data.slice(start, read));
            }
            _ => (), // unimplemented!(),
        }

        return Ok(read);
    }

    /// Connection: keep-alive or Connection: close
    pub fn should_keep_alive(&self) -> bool {
        self.keep_alive
    }

    /// Connection: upgrade
    pub fn should_upgrade(&self) -> bool {
        self.upgrade
    }

    /// Connection: upgrade
    pub fn chunked(&self) -> bool {
        self.chunked
    }

    #[inline]
    fn reset(&mut self) {
        self.state = match self.parser_type {
            ParseRequest  => StartReq,
            ParseResponse => StartRes,
            ParseBoth     => StartReqOrRes,
        };
        self.index = 0;
        self.major = 0;
        self.minor = 0;
        self.message_body_rest = UINT_MAX;
        self.skip_body = false;
        self.status_code = 0;
    }

    #[inline]
    fn needs_eof(&mut self) -> bool {
        if self.parser_type == ParseRequest {
            return false;
        }
        if self.status_code / 100 == 1 ||     // 1xx e.g. Continue
            self.status_code == 204 ||        // No Content
            self.status_code == 304 ||        // Not Modified
            self.skip_body {
            return false;
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

#[inline]
fn unhex(b: u8) -> uint {
    match to_lowercase(b as char) {
        '0' => 0,
        '1' => 1,
        '2' => 2,
        '3' => 3,
        '4' => 4,
        '5' => 5,
        '6' => 6,
        '7' => 7,
        '8' => 8,
        '9' => 9,
        'a' => 10,
        'b' => 11,
        'c' => 12,
        'd' => 13,
        'e' => 14,
        'f' => 15,
        _   => UINT_MAX,
    }
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
    ResHttpStart,
    ResHttpMajor,
    ResHttpMinor,
    ResStatusCodeStart,
    ResStatusCode,
    ResStatus,
    ResLineAlmostDone,
    HeaderFieldStart,
    HeaderField,
    HeaderValueDiscardWS,
    HeaderValueDiscardWSAlmostDone,
    HeaderValueDiscardLWS,
    HeaderValueStart,
    HeaderValue,
    HeaderAlmostDone,
    HeadersAlmostDone,
    BodyIdentity,
    BodyIdentityEOF,
    ChunkSize,
    ChunkSizeAlmostDone,
    ChunkExtension,
    ChunkData,
    Crashed,
}

#[deriving(PartialEq, Eq, Clone, Show)]
enum HeaderState {
    HeaderGeneral,
    HeaderConnection,
    HeaderContentLength,
    HeaderTransferEncoding,
    HeaderUpgrade,
    HeaderMatchingChunked,
    HeaderMatchingClose,
    HeaderMatchingKeepAlive,
    HeaderMatchingUpgrade,
}
