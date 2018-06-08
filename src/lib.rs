#![allow(dead_code)]
#![feature(core_instrinsics)]

extern crate itertools;
extern crate page_size;
extern crate bit_vec;

mod msc_test_mods; // For dev purposes only
mod chunk;
mod deferred_heap;
pub use deferred_heap::*;
