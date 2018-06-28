use page_size;
use std::marker::PhantomData;
// use std::cell::RefCell;

use chunk::Chunk;
use bit_vec::BitVec;

// Storing/getting drop impl: https://doc.rust-lang.org/1.1.0/src/arena/lib.rs.html#187

// longer_than_self lifetime from any_arena crate
pub struct DeferredHeap<'longer_than_self> {
    // pages: Vec<DhPage>,
    _marker: PhantomData<*mut &'longer_than_self ()>,
}

impl<'a> DeferredHeap<'a> {
    pub fn new() -> DeferredHeap<'a> {
        DeferredHeap::with_size(page_size::get())
    }

    pub fn with_size(_chunk_size: usize) -> DeferredHeap<'a> {
        DeferredHeap {
            // chunks: RefCell::new(ChunkList::with_size(chunk_size)),
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

// TODO
pub struct Dp<T> {
    phantom: PhantomData<T>,
}

// struct DpVoid<'a, 'b> {
//     heap: &'a DeferredHeap<'b>,
//     ptr: *const i8,
// }

struct NonRoot {
    // ptr: *const DpVoid,
    level: usize,
}

struct DhPage<'a> {
    page: Chunk,
    live_starts: BitVec,
    deferred_ptrs: Vec<NonRoot>,
    heap: *mut DeferredHeap<'a>,
}

#[cfg(target_os = "ios")]
mod use_tests {
    use super::*;

    fn graph_stuff() {
        // This function has implicit `Some`s
        struct LLNode<T> {
            data: T,
            prev: Option<Dp<RefCell<LLNode<T>>>>,
            next: Option<Dp<RefCell<LLNode<T>>>>,
        }

        let dheap = DeferredHeap::new();

        let a: Dp<LLnode<_>> = dheap.insert(LLNode::new(1));
        // rt: a        nr:
        let b = dheap.insert(LLNode::new(2));
        // rt: a, b     nr:
        {
            let c = dheap.insert(LLNode::new(3));
            // rt: a, b, c  nr:

            a.borrow_mut().prev = c.clone();
            a.borrow_mut().next = b.clone();

            b.borrow_mut().prev = a.clone();
            b.borrow_mut().next = c.clone();

            c.borrow_mut().prev = b.clone();
            c.borrow_mut().next = a.clone();

        }
        // rt: a, b     nr: c

        let mut ll = LLNode::new(4);
        ll.next = b.clone();
        // *b.next.borrow_mut() = ll; // c = ll, which has root `Dp`s

        {
            let d: Dp<RefCell<LLNode<_>>> = dheap.insert(RefCell::new(LLNode::new(1)));
            *d.borrow_mut().next.borrow_mut() = ll.clone();
            *d.borrow_mut().next.borrow_mut() = b.clone();
            *d.borrow_mut().next.borrow_mut() = a.prev.clone();
        }

        *b = a.clone(); // Only if DerefMut
        // b.assign(a.clone()); // If only Deref
        b = a.clone();
        // rt: a        nr: b, c

        a.borrow_mut().next.borrow_mut().next = None; // b.next = None
    }

    fn graph_stuff2() {
        // This function has implicit `Some`s
        struct LLNode<T> {
            data: T,
            prev: Option<Dp<RefCell<LLNode<T>>>>,
            next: Option<Dp<RefCell<LLNode<T>>>>,
        }

        let dheap = DeferredHeap::new();

        let a: Dp<LLnode<_>> = dheap.insert(LLNode::new(1));
        // rt: a        nr:
        let b = dheap.insert(LLNode::new(2));
        // rt: a, b     nr:
        {
            let c = dheap.insert(LLNode::new(3));
            // rt: a, b, c  nr:

            a.borrow_mut().prev.assign(&mut c);
            a.borrow_mut().next.assign(&mut b);

            b.borrow_mut().prev.assign(&mut a);
            b.borrow_mut().next.assign(&mut c);

            c.borrow_mut().prev.assign(&mut b);
            c.borrow_mut().next.assign(&mut a);

        }
        // rt: a, b     nr: c

        let mut ll = LLNode::new(4);
        ll.next = b.clone();
        // *b.next.borrow_mut() = ll; // c = ll, which has root `Dp`s

        {
            let d: Dp<RefCell<LLNode<_>>> = dheap.insert(RefCell::new(LLNode::new(1)));
            let e = dheap.insert(LLNode::new(0));
            let ll2 = LLNode::new(0);
            // *d.borrow_mut().next.borrow_mut() = e; // Doesn't compile due to only Deref
            // *d.borrow_mut().next.borrow_mut() = ll2; // Doesn't compile due to only Deref
            *d.borrow_mut().next.borrow_mut().assign(&mut a.prev);
        }

        *b = a.clone(); // Only if DerefMut
        b.assign(&mut a); // If only Deref
        // b = a.clone();
        // rt: a        nr: b, c

        a.borrow_mut().next.borrow_mut().next = None; // b.next = None
    }

    impl<T> Deref<Target=T> for Dp<T> {}
    #[derive(Clone)]
    struct Dp<T: Traceable> {}
    impl DeferredHeap {
        fn insert<T: Traceable>(&mut self, data: T) {
            let place = self.alloc::<T>();
            *place = data;
            place.set_root();
        }
    }
    impl<T: Traceable> Dp<T> {
        fn assign(&mut self, other: Dp<T>) {
            let was_in_heap = self.is_in_heap();
            *self = other;
            if self.is_in_heap() && !was_in_heap {
                self.set_non_root();
            } else if self.is_root() && was_in_heap {
                self.set_root();
            }
        }
        fn new(data: T) -> Dp<T> {
            let ptr = Dp { data: T };
            ptr.set_root();
            ptr
        }
    }
    impl<T: Traceable> Drop for Dp<T> {
        fn drop(self) {
            if self.is_root() {
                self.unregister();
            }
        }
    }

}
