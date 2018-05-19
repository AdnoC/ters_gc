#![allow(dead_code)]

extern crate page_size;
extern crate bit_vec;

mod chunk;
mod deferred_heap;
pub use deferred_heap::*;
