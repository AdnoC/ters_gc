extern crate ters_gc;

use ters_gc::{Collector, Gc, trace};
use std::cell::RefCell;

// A struct that can hold references to itself
struct CyclicStruct<'a>(RefCell<Option<Gc<'a, CyclicStruct<'a>>>>);

// All things in the gc heap need to impl `Trace`
impl<'a> trace::Trace for CyclicStruct<'a> {
    fn trace(&self, tracer: &mut trace::Tracer) {
        // Tell the tracer where to find our gc pointer
        tracer.add_target(&self.0);
    }
}

impl<'a> Drop for CyclicStruct<'a> {
    fn drop(&mut self) {
        println!("dropping CyclicStruct");
    }
}

fn main() {
    // Make a new collector to keep the gc state
    let mut col = Collector::new();

    // Make a Proxy to access the API
    let mut proxy = col.proxy();

    // Do some computations that are best expressed with a cyclic data structure
    {
        let thing1 = proxy.store(CyclicStruct(RefCell::new(None)));
        let thing2 = proxy.store(CyclicStruct(RefCell::new(Some(thing1.clone()))));
        *thing1.0.borrow_mut() = Some(thing2.clone());
    } // They are out of scope and no longer reachable here

    // Collect garbage
    proxy.run(); // Prints "dropping CyclicStruct" twick

    // And we've successfully cleaned up the unused cyclic data
    assert_eq!(proxy.num_tracked(), 0);
}
