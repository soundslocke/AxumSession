#![doc = include_str!("../README.md")]
#![allow(dead_code)]
#![warn(clippy::all, nonstandard_style, future_incompatible)]
#![forbid(unsafe_code)]

mod redis_bb8_pool;
pub use self::redis_bb8_pool::*;
pub(crate) mod redis_bb8_tools;

fn key(id: &str, prefix: &str) -> String {
    match prefix.is_empty() {
        true => id.to_string(),
        false => format!("{}:{}", prefix, id),
    }
}
