extern crate terse;
use terse::collector::*;

fn lifetimes_are_properly_constrained() {
    use ::std::mem::drop;
    let mut collector = Collector::new();
    let mut alloc = collector.allocator();

    let gc_ptr = alloc.store(42);

    drop(alloc);

    // Should borrowck error. Can't move collector since it is borrowed by gc_ptr
    let collector2 = collector; //~ ERROR cannot move out of `collector` because it is borrowed

}

fn main() { }
