/*! HTTP request/response parser for Rust.

This is a parser for HTTP messages written in Rust.
It's inspired by [http-parser](https://github.com/joyent/http-parser) written in C.
*/

#![crate_name="bee"]
#![crate_type="rlib"]
#![warn(missing_doc)]
#![feature(globs, macro_rules)]
#![experimental]

#[cfg(test)] extern crate test;

pub use self::version::version;

#[allow(missing_doc)]
pub mod version {
    pub static MAJOR: uint = 0;
    pub static MINOR: uint = 1;
    pub static PATCH: uint = 0;
    pub static PRE: &'static str = "alpha";

    /// Show version string.
    pub fn version() -> String {
        format!("{}.{}.{}{}",
                MAJOR, MINOR, PATCH,
                if PRE.len() > 0 { ["-", PRE].concat() } else { "".to_string() })
    }
}

pub mod parser;
#[cfg(test)] pub mod tests;
#[cfg(test)] pub mod benchmarks;
