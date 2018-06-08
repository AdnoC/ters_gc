use page_size;
use std::marker::PhantomData;
use std::cell::RefCell;

use chunk::Chunk;

// Storing/getting drop impl: https://doc.rust-lang.org/1.1.0/src/arena/lib.rs.html#187

// longer_than_self lifetime from any_arena crate
pub struct DeferredHeap<'longer_than_self> {
    chunks: RefCell<ChunkList>,
    _marker: PhantomData<*mut &'longer_than_self ()>,
}

impl<'a> DeferredHeap<'a> {
    pub fn new() -> DeferredHeap<'a> {
        DeferredHeap::with_size(page_size::get())
    }

    pub fn with_size(chunk_size: usize) -> DeferredHeap<'a> {
        DeferredHeap {
            chunks: RefCell::new(ChunkList::with_size(chunk_size)),
            _marker: PhantomData,
        }
    }
}

struct ChunkList {
    pages: Vec<Chunk>,
    chunk_size: usize,
}

impl ChunkList {
    pub fn with_size(chunk_size: usize) -> ChunkList {
        ChunkList {
            pages: vec![],
            chunk_size,
        }
    }
}

pub struct Dp<T> {
    phantom: PhantomData<T>,
}
