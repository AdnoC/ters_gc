extern crate ters_gc;
use ters_gc::*;

fn lifetimes_are_properly_constrained() {
    use ::std::mem::drop;
    use std::{
        rc::Rc,
        cell::RefCell,
    };
    let mut gc_ptr = Rc::new(RefCell::new(None));
    let mut collector = Collector::new();
    unsafe {
        collector.run_with_gc(move |proxy| {
            *gc_ptr.borrow_mut() = Some(proxy.store(42)); //~ ERROR cannot infer
        })
    }
}

fn main() { }
