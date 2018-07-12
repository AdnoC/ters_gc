```
A tinee Rust garbage collector (ters gc)
  ^   ^ ^ ^  ^       ^
```

("tiny" is deliberately misspelled for the sake of the acronym)

A mark-and-sweep garbage collecting allocator.
Based loosely on orangeduck's
[`Tiny Garbage Collector`](https://github.com/orangeduck/tgc).

Provides the `Gc` type, essentially an [`Rc`](https://doc.rust-lang.org/std/rc/struct.Rc.html)
that can handle cycles.


# Example

Add this to your 'Cargo.toml':

```toml
[dependencies]
ters_gc = "0.1"
```

main.rs:

```rust
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

fn main() {
    // Make a new collector to keep the gc state
    let mut col = Collector::new();

    // Find out the meaning of life, and allow use of the gc while doing so
    let meaning_of_life = col.run_with_gc(|mut proxy| {
        // Do some computations that are best expressed with a cyclic data structure
        {
            let thing1 = proxy.store(CyclicStruct(RefCell::new(None)));
            let thing2 = proxy.store(CyclicStruct(RefCell::new(Some(thing1.clone()))));
            *thing1.0.borrow_mut() = Some(thing2.clone());
        } // They are out of scope and no longer reachable here

        // Collect garbage
        proxy.run();

        // And we've successfully cleaned up the unused cyclic data
        assert_eq!(proxy.num_tracked(), 0);

        // Return
        42
    });

    assert_eq!(meaning_of_life, 42);
}
```
