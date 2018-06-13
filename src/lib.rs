#![allow(dead_code)]
#![feature(core_intrinsics, ptr_offset_from)]

extern crate itertools;
extern crate page_size;
extern crate bit_vec;

mod destructors;
mod msc_test_mods; // For dev purposes only
mod chunk;
mod deferred_heap;
pub use deferred_heap::*;
