use page_size;
use std::marker::PhantomData;
use std::cell::RefCell;

use chunk::Chunk;

// longer_than_self lifetime from any_arena crate
pub struct DeferredHeap<'longer_than_self> {
    chunks: RefCell<ChunkList>,
    _marker: PhantomData<*mut &'longer_than_self ()>,
}

impl<'longer_than_self> DeferredHeap<'longer_than_self> {
    pub fn new() -> DeferredHeap<'longer_than_self> {
        DeferredHeap::with_size(page_size::get())
    }

    pub fn with_size(chunk_size: usize) -> DeferredHeap<'longer_than_self> {
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
