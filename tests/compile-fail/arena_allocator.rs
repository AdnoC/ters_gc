extern crate terse;
use terse::arena::*;

fn lifetimes_are_properly_constrained() {
    use ::std::mem::drop;
    let mut arena = Arena::new();
    let mut alloc = arena.allocator();

    let gc_ptr = alloc.store(42);

    drop(alloc);

    // Should borrowck error. Can't move arena since it is borrowed by gc_ptr
    let arena2 = arena; //~ ERROR cannot move out of `arena` because it is borrowed

}

fn main() { }
