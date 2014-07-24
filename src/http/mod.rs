//! HTTP parser.

#![experimental]

use std::fmt::{Formatter, FormatError, Show};

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

pub mod parser;
#[cfg(test)] pub mod tests;
