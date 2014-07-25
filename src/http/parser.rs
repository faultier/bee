//! HTTP parser.

#![experimental]

use UINT_MAX = std::uint::MAX;

use http;

#[deriving(PartialEq, Eq, Clone, Show)]
/// A parser types.
pub enum ParseType {
    /// Parse request only.
    ParseRequest,
    /// Parse response only.
    ParseResponse,
}

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
                        self.method = Some(match byte {
                            UPPER_C => http::HttpConnect,     // or CHECKOUT, COPY
                            UPPER_D => http::HttpDelete,
                            UPPER_G => http::HttpGet,
                            UPPER_H => http::HttpHead,
                            UPPER_L => http::HttpLink,        // or LOCK
                            UPPER_M => http::HttpMkCol,       // or M-SEARCH, MERGE, MKACTIVITY, MKCALENDER
                            UPPER_N => http::HttpNotify,
                            UPPER_O => http::HttpOptions,
                            UPPER_P => http::HttpPut,         // or PATCH, POST, PROPPATCH, PROPFIND
                            UPPER_R => http::HttpReport,
                            UPPER_S => http::HttpSearch,      // or SUBSCRIBE
                            UPPER_T => http::HttpTrace,
                            UPPER_U => http::HttpUnlink,      // or UNLOCK, UNSUBSCRIBE
                            CR | LF => break,
                            _   => { self.state = Crashed; return Err(InvalidMethod) },
                        });
                        handler.on_message_begin(self);
                        self.state = ReqMethod;
                        self.index = 1;
                    }
                    StartRes => {
                        match byte {
                            UPPER_H => {
                                self.state = HttpStart;
                                self.index = 1;
                            },
                            CR | LF => break,
                            _ => { self.state = Crashed; return Err(InvalidMethod) },
                        }
                        handler.on_message_begin(self);
                    }
                    ReqMethod => {
                        let method = self.method.unwrap();
                        if byte == SPACE {
                            handler.on_method(self, method);
                            self.state = ReqUrl;
                            self.index = 0;
                        } else {
                            if !method.hit(self.index, byte as char) {
                                self.method = Some(match (method, self.index, byte) {
                                    (http::HttpConnect, 1, UPPER_H)   => http::HttpCheckout,
                                    (http::HttpConnect, 2, UPPER_P)   => http::HttpCopy,
                                    (http::HttpLink, 1, UPPER_O)      => http::HttpLock,
                                    (http::HttpMkCol, 1, HYPHEN)      => http::HttpMsearch,
                                    (http::HttpMkCol, 1, UPPER_E)     => http::HttpMerge,
                                    (http::HttpMkCol, 2, UPPER_A)     => http::HttpMkActivity,
                                    (http::HttpMkCol, 3, UPPER_A)     => http::HttpMkCalendar,
                                    (http::HttpPut, 1, UPPER_A)       => http::HttpPatch,
                                    (http::HttpPut, 1, UPPER_O)       => http::HttpPost,
                                    (http::HttpPut, 1, UPPER_R)       => http::HttpPropPatch,
                                    (http::HttpPut, 2, UPPER_R)       => http::HttpPurge,
                                    (http::HttpPropPatch, 4, UPPER_F) => http::HttpPropFind,
                                    (http::HttpSearch, 1, UPPER_U)    => http::HttpSubscribe,
                                    (http::HttpUnlink, 2, UPPER_S)    => http::HttpUnsubscribe,
                                    (http::HttpUnlink, 3, UPPER_O)    => http::HttpUnlock,
                                    _ => { self.state = Crashed; return Err(InvalidMethod) },
                                });
                            }
                            self.index += 1;
                        }
                    }
                    ReqUrl => {
                        match byte {
                            SPACE => {
                                if self.index == 0 { self.state = Crashed; return Err(InvalidUrl) }
                                let start = if read > self.index + 1 { read - self.index - 1 } else { 0 };
                                let end = read - 1;
                                handler.write(self, data.slice(start, end));
                                handler.on_url(self, self.index);
                                self.state = HttpStart;
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
                    HttpStart => {
                        match (byte, self.index) {
                            (UPPER_H, 0)    => self.index += 1,
                            (UPPER_T, 1..2) => self.index += 1,
                            (UPPER_P, 3)    => self.index += 1,
                            (SLASH, 4)      => self.index += 1,
                            (ZERO..NINE, 5) => {
                                self.state = HttpMajor;
                                self.major = (byte - ZERO) as uint;
                                self.index = 1;
                            }
                            _ => {
                                self.state = Crashed;
                                return Err(InvalidVersion);
                            }
                        }
                    }
                    HttpMajor => {
                        match byte {
                            DOT if self.index > 0 => {
                                self.state = HttpMinor;
                                self.index = 0;
                            }
                            ZERO..NINE => {
                                self.index += 1;
                                self.major *= 10;
                                self.major += (byte - ZERO) as uint;
                            }
                            _ => { self.state = Crashed; return Err(InvalidVersion) },
                        }
                    }
                    HttpMinor => {
                        match (byte, self.index, self.parser_type) {
                            (ZERO..NINE, _, _) => {
                                self.index += 1;
                                self.minor *= 10;
                                self.minor += (byte - ZERO) as uint;
                            }
                            (CR, 1..2, ParseRequest) | (LF, 1..2, ParseRequest) | (SPACE, 1..2, ParseResponse) => {
                                match http::HttpVersion::find(self.major, self.minor) {
                                    None => { self.state = Crashed; return Err(InvalidVersion) }
                                    v => {
                                        handler.on_version(self, v.unwrap());
                                        self.http_version = v;
                                        self.keep_alive = v == Some(http::HTTP_1_1);
                                        self.state = match (byte, self.parser_type) {
                                            (CR, ParseRequest) => ReqLineAlmostDone,
                                            (LF, ParseRequest) => HeaderFieldStart,
                                            (SPACE, ParseResponse) => ResStatusCode,
                                            _ => { self.state = Crashed; return Err(InvalidVersion) }
                                        };
                                        self.index = 0;
                                    }
                                }
                            }
                            _ => { self.state = Crashed; return Err(InvalidVersion) },
                        }
                    }
                    ReqLineAlmostDone => {
                        if byte != LF { self.state = Crashed; return Err(InvalidRequestLine) }
                        self.state = HeaderFieldStart;
                    }
                    ResStatusCode => {
                        if byte >= ZERO && byte <= NINE && self.index < 3 {
                            self.status_code *= 10;
                            self.status_code += (byte - ZERO) as uint;
                            self.index += 1;
                        } else {
                            handler.on_status(self, self.status_code);
                            self.state = match byte {
                                SPACE => ResStatus,
                                CR   => ResLineAlmostDone,
                                LF   => HeaderFieldStart,
                                _     => {
                                    self.state = Crashed;
                                    return Err(InvalidStatusLine);
                                }
                            };
                            self.index = 0;
                        }
                    }
                    ResStatus => {
                        self.state = match byte {
                            CR => ResLineAlmostDone,
                            LF => HeaderFieldStart,
                            _   => ResStatus, // ignore reason phrases
                        };
                    }
                    ResLineAlmostDone => {
                        if byte != LF { self.state = Crashed; return Err(InvalidStatusLine) }
                        self.state = HeaderFieldStart;
                    }
                    HeaderFieldStart => {
                        match byte {
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
                            0x21..0x7e => {
                                self.state = HeaderField;
                                self.hstate = match byte {
                                    UPPER_C | LOWER_C => HeaderConnection,
                                    UPPER_T | LOWER_T => HeaderTransferEncoding,
                                    UPPER_U | LOWER_U => HeaderUpgrade,
                                    _                 => HeaderGeneral,
                                };
                                self.index = 1;
                            }
                            _ => { self.state = Crashed; return Err(InvalidHeaderField) },
                        }
                    }
                    HeaderField => {
                        match byte {
                            COLON => {
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
                            0x21..0x7e => {
                                if self.hstate != HeaderGeneral {
                                    self.hstate = match self.hstate {
                                        HeaderConnection => match (byte, self.index) {
                                            (UPPER_T, 3) | (LOWER_T, 3) => HeaderContentLength,
                                            (UPPER_O, 1) | (LOWER_O, 1)
                                                | (UPPER_N, 2..3) | (LOWER_N, 2..3)
                                                | (UPPER_E, 4) | (LOWER_E, 4)
                                                | (UPPER_C, 5) | (LOWER_C, 5)
                                                | (UPPER_T, 6) | (LOWER_T, 6)
                                                | (UPPER_I, 7) | (LOWER_I, 7)
                                                | (UPPER_O, 8) | (LOWER_O, 8)
                                                | (UPPER_N, 9) | (LOWER_N, 9) => HeaderConnection,
                                            _ => HeaderGeneral,
                                        },
                                        HeaderContentLength => match (byte, self.index) {
                                            (UPPER_E, 4) | (LOWER_E, 4)
                                                | (UPPER_N, 5)  | (LOWER_N, 5)
                                                | (UPPER_T, 6)  | (LOWER_T, 6)
                                                | (HYPHEN, 7)
                                                | (UPPER_L, 8)  | (LOWER_L, 8)
                                                | (UPPER_E, 9)  | (LOWER_E, 9)
                                                | (UPPER_N, 10) | (LOWER_N, 10)
                                                | (UPPER_G, 11) | (LOWER_G, 11)
                                                | (UPPER_T, 12) | (LOWER_T, 12)
                                                | (UPPER_H, 13) | (LOWER_H, 13) => HeaderContentLength,
                                            _ => HeaderGeneral,
                                        },
                                        HeaderTransferEncoding => match (byte, self.index) {
                                            (UPPER_R, 1) | (LOWER_R, 1)
                                                | (UPPER_A, 2)  | (LOWER_A, 2)
                                                | (UPPER_N, 3)  | (LOWER_N, 3)
                                                | (UPPER_S, 4)  | (LOWER_S, 4)
                                                | (UPPER_F, 5)  | (LOWER_F, 5)
                                                | (UPPER_E, 6)  | (LOWER_E, 6)
                                                | (UPPER_R, 7)  | (LOWER_R, 7)
                                                | (HYPHEN, 8)
                                                | (UPPER_E, 9)  | (LOWER_E, 9)
                                                | (UPPER_N, 10) | (LOWER_N, 10)
                                                | (UPPER_C, 11) | (LOWER_C, 11)
                                                | (UPPER_O, 12) | (LOWER_O, 12)
                                                | (UPPER_D, 13) | (LOWER_D, 13)
                                                | (UPPER_I, 14) | (LOWER_I, 14)
                                                | (UPPER_N, 15) | (LOWER_N, 15)
                                                | (UPPER_G, 16) | (LOWER_G, 16) => HeaderTransferEncoding,
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
                        match byte {
                            SPACE | TAB => (), // skip
                            CR => self.state = HeaderValueDiscardWSAlmostDone,
                            LF => self.state = HeaderValueDiscardLWS,
                            _ => {
                                self.hstate = match (self.hstate, byte) {
                                    (HeaderConnection, UPPER_C)
                                        | (HeaderConnection, LOWER_C) => HeaderMatchingClose,
                                    (HeaderConnection, UPPER_K)
                                        | (HeaderConnection, LOWER_K) => HeaderMatchingKeepAlive,
                                    (HeaderConnection, UPPER_U)
                                        | (HeaderConnection, LOWER_U) => HeaderMatchingUpgrade,
                                    (HeaderTransferEncoding, UPPER_C)
                                        | (HeaderTransferEncoding, LOWER_C) => HeaderMatchingChunked,
                                    (HeaderContentLength, _) => {
                                        self.message_body_rest = (byte - ZERO) as uint;
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
                        if byte != LF { self.state = Crashed; return Err(InvalidHeaderField) }
                        self.state = HeaderValueDiscardLWS;
                    }
                    HeaderValueDiscardLWS => {
                        if byte == SPACE || byte == TAB {
                            self.state = HeaderValueDiscardWS;
                        } else {
                            // header value is empty.
                            handler.on_header_value(self, 0);
                            self.index = 0;
                            match byte {
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
                                0x21..0x7e => {
                                    self.state = HeaderFieldStart;
                                    self.index = 1;
                                }
                                _ => { self.state = Crashed; return Err(InvalidHeaderField) },
                            }
                        }
                    }
                    HeaderValue => {
                        match byte {
                            CR | LF => {
                                self.state = if byte == CR {
                                    HeaderAlmostDone
                                } else {
                                    HeaderFieldStart
                                };
                                match (self.hstate, self.index) {
                                    (HeaderMatchingChunked, 7)    => self.chunked = true,
                                    (HeaderMatchingClose, 5)      => self.keep_alive = false,
                                    (HeaderMatchingKeepAlive, 10) => self.keep_alive = true,
                                    (HeaderMatchingUpgrade, 6)    => self.upgrade = true,
                                    _ => (),
                                }
                                let start = if read > self.index + 1 { read - self.index - 1 } else { 0 };
                                let end = read - 1;
                                handler.write(self, data.slice(start, end));
                                handler.on_header_value(self, self.index);
                                self.index = 0;
                            }
                            _ => {
                                if self.hstate != HeaderGeneral {
                                    self.hstate = match (self.hstate, byte) {
                                        (HeaderMatchingKeepAlive, _) => match (byte, self.index) {
                                            (UPPER_E, 1) | (LOWER_E, 1)
                                                | (UPPER_E, 2) | (LOWER_E, 2)
                                                | (UPPER_P, 3) | (LOWER_P, 3)
                                                | (HYPHEN, 4)
                                                | (UPPER_A, 5) | (LOWER_A, 5)
                                                | (UPPER_L, 6) | (LOWER_L, 6)
                                                | (UPPER_I, 7) | (LOWER_I, 7)
                                                | (UPPER_V, 8) | (LOWER_V, 8)
                                                | (UPPER_E, 9) | (LOWER_E, 9) => HeaderMatchingKeepAlive,
                                            _ => HeaderGeneral,
                                        },
                                        (HeaderMatchingClose, _) => match (byte, self.index) {
                                            (UPPER_L, 1) | (LOWER_L, 1)
                                                | (UPPER_O, 2) | (LOWER_O, 2)
                                                | (UPPER_S, 3) | (LOWER_S, 3)
                                                | (UPPER_E, 4) | (LOWER_E, 4) => HeaderMatchingClose,
                                            _ => HeaderGeneral,
                                        },
                                        (HeaderMatchingChunked, _) => match (byte, self.index) {
                                            (UPPER_H, 1) | (LOWER_H, 1)
                                                | (UPPER_U, 2) | (LOWER_U, 2)
                                                | (UPPER_N, 3) | (LOWER_N, 3)
                                                | (UPPER_K, 4) | (LOWER_K, 4)
                                                | (UPPER_E, 5) | (LOWER_E, 5)
                                                | (UPPER_D, 6) | (LOWER_D, 6) => HeaderMatchingChunked,
                                            _ => HeaderGeneral,
                                        },
                                        (HeaderMatchingUpgrade, _) => match (byte, self.index) {
                                            (UPPER_P, 1) | (LOWER_P, 1)
                                                | (UPPER_G, 2) | (LOWER_G, 2)
                                                | (UPPER_R, 3) | (LOWER_R, 3)
                                                | (UPPER_A, 4) | (LOWER_A, 4)
                                                | (UPPER_D, 5) | (LOWER_D, 5)
                                                | (UPPER_E, 6) | (LOWER_E, 6) => HeaderMatchingUpgrade,
                                            _ => HeaderGeneral,
                                        },
                                        (HeaderContentLength, ZERO..NINE) => {
                                            self.message_body_rest *= 10;
                                            self.message_body_rest += (byte - ZERO) as uint;
                                            HeaderContentLength
                                        }
                                        (HeaderContentLength, _) => {
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
                        if byte != LF { self.state = Crashed; return Err(InvalidHeaderField) }
                        self.state = HeaderFieldStart;
                    }
                    HeadersAlmostDone => {
                        if byte != LF { self.state = Crashed; return Err(InvalidHeaders) }
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
                    BodyIdentity | BodyIdentityEOF
                        | ChunkSize | ChunkSizeAlmostDone | ChunkExtension | ChunkData
                        | Dead | Crashed => unreachable!(),
                }
            }
        }

        if self.chunked {
            'chunk: loop {
                if self.state == ChunkData {
                    let rest = data.len() - read;
                    if self.message_body_rest == 0 && rest > 2 {
                        if data[read] != CR || data[read+1] != LF {
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
                        match (self.state, byte) {
                            (ChunkExtension, CR) => {
                                self.state = ChunkSizeAlmostDone;
                            }
                            (ChunkExtension, _) => { /* ignore */ }
                            (ChunkSize, SEMICOLON) => {
                                self.state = ChunkExtension;
                            }
                            (ChunkSize, CR) => {
                                self.state = ChunkSizeAlmostDone;
                            }
                            (ChunkSize, _) => {
                                let val = unhex(byte);
                                if val > 15 { self.state = Crashed; return Err(InvalidChunk) }
                                self.message_body_rest *= 16;
                                self.message_body_rest += val;
                            }
                            (ChunkSizeAlmostDone, _) => {
                                if byte != LF { self.state = Crashed; return Err(InvalidChunk) }
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
            BodyIdentityEOF if data.len() != read => {
                handler.write(self, data.slice_from(read));
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

    /// Transfer-Encoding: chunked
    pub fn chunked(&self) -> bool {
        self.chunked
    }

    #[inline]
    fn reset(&mut self) {
        self.state = match self.parser_type {
            ParseRequest  => StartReq,
            ParseResponse => StartRes,
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

static TAB: u8       = 0x09;
static LF: u8        = 0x0a;
static CR: u8        = 0x0d;
static SPACE: u8     = 0x20;
static HYPHEN: u8    = 0x2d;
static DOT: u8       = 0x2e;
static SLASH: u8     = 0x2f;
static ZERO: u8      = 0x30;
static NINE: u8      = 0x39;
static COLON: u8     = 0x3a;
static SEMICOLON: u8 = 0x3b;
static UPPER_A: u8   = 0x41;
static UPPER_C: u8   = 0x43;
static UPPER_D: u8   = 0x44;
static UPPER_E: u8   = 0x45;
static UPPER_F: u8   = 0x46;
static UPPER_G: u8   = 0x47;
static UPPER_H: u8   = 0x48;
static UPPER_I: u8   = 0x49;
static UPPER_K: u8   = 0x4b;
static UPPER_L: u8   = 0x4c;
static UPPER_M: u8   = 0x4d;
static UPPER_N: u8   = 0x4e;
static UPPER_O: u8   = 0x4f;
static UPPER_P: u8   = 0x50;
static UPPER_R: u8   = 0x52;
static UPPER_S: u8   = 0x53;
static UPPER_T: u8   = 0x54;
static UPPER_U: u8   = 0x55;
static UPPER_V: u8   = 0x56;
static LOWER_A: u8   = 0x61;
static LOWER_C: u8   = 0x63;
static LOWER_D: u8   = 0x64;
static LOWER_E: u8   = 0x65;
static LOWER_F: u8   = 0x66;
static LOWER_G: u8   = 0x67;
static LOWER_H: u8   = 0x68;
static LOWER_I: u8   = 0x69;
static LOWER_K: u8   = 0x6b;
static LOWER_L: u8   = 0x6c;
static LOWER_N: u8   = 0x6e;
static LOWER_O: u8   = 0x6f;
static LOWER_P: u8   = 0x70;
static LOWER_R: u8   = 0x72;
static LOWER_S: u8   = 0x73;
static LOWER_T: u8   = 0x74;
static LOWER_U: u8   = 0x75;
static LOWER_V: u8   = 0x76;

#[inline]
fn unhex(b: u8) -> uint {
    if b < ZERO || b > NINE { UINT_MAX } else { (b - ZERO) as uint }
}

#[deriving(PartialEq, Eq, Clone, Show)]
enum ParserState {
    StartReq,
    StartRes,
    ReqMethod,
    ReqUrl,
    HttpStart,
    HttpMajor,
    HttpMinor,
    ReqLineAlmostDone,
    ResStatusCode,
    ResStatus,
    ResLineAlmostDone,
    HeaderFieldStart,
    HeaderField,
    HeaderValueDiscardWS,
    HeaderValueDiscardWSAlmostDone,
    HeaderValueDiscardLWS,
    HeaderValue,
    HeaderAlmostDone,
    HeadersAlmostDone,
    BodyIdentity,
    BodyIdentityEOF,
    ChunkSize,
    ChunkSizeAlmostDone,
    ChunkExtension,
    ChunkData,
    Dead,
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
