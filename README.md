A "TinEe RuSt Garbage Collector"

("tiny" is deliberately misspelled for the sake of the acronym)

FIXME MEMORY SAFETY HOLE: Weak::get. `T` should only be accessible after conversion to `Gc` via `upgrade`

*TODO: CHECK THAT ALL LINKS ARE VALID*

A toy project implementing a mark-and-sweep garbage collecting allocator
in the rust programming language.
Based loosely on  <NAME>'s [`Tiny Garbage Collector`].

Use at your own risk: The author is neither experienced with writing
unsafe rust nor has he studied garbage collectors.

Having said that, it should be stable enough to play around with and
use for small projects.

It can be used to create cyclic data structures without leaking memory:

```
use ters_gc::{Collector, Proxy, Gc, trace};
use std::cell::RefCell;


// A struct that can hold references to itself
struct CyclicStruct<'a>(RefCell<Option<Gc<'a, CyclicStruct<'a>>>>);

// All things in the gc heap need to impl `TraceTo`
impl<'a> trace::TraceTo for CyclicStruct<'a> {
    fn trace_to(&self, tracer: &mut trace::Tracer) {
        // Tell the tracer where to find our gc pointer
        self.0.trace_to(tracer);
    }
}

// Do some computations that are best expressed with a cyclic data structure
fn compute_cyclic_data(proxy: &mut Proxy) {
    let thing1 = proxy.store(CyclicStruct(RefCell::new(None)));
    let thing2 = proxy.store(CyclicStruct(RefCell::new(Some(thing1.clone()))));
    *thing1.0.borrow_mut() = Some(thing2.clone());
}

// Make a new collector to keep the gc state
let mut col = Collector::new();

// Because of how unsafe scoping works, you shouldn't make a lambda
// within the arguments of `run_with_gc`, otherwise you might stray
// outside of safe rust.
fn find_meaning_of_life(mut proxy: Proxy) -> i32 {

    // Do some calculations. Do it later in the stack so that the pointers
    // to gc objects aren't in the used portion of the stack when collecting.
    proxy.exec_with_stack_barrier(compute_cyclic_data);

    // Collect garbage
    proxy.run();

    // And we've successfully cleaned up the unused cyclic data
    assert_eq!(proxy.num_tracked(), 0);

    // Return
    42
}

// Find out the meaning of life, and allow use of the gc while doing so
let meaning = unsafe { col.run_with_gc(find_meaning_of_life) };

assert_eq!(meaning, 42);
```
# Collection Overview

The collector determines reachability based on two sources of information: reference
counting and inter-object connections. It also works with an assumption:
all references stored outside the gc heap are reachable.

[`Gc`]s act like [`Rc`]s: they increment the reference count on `clone` and
decrement it on `drop`.

The [`TraceTo`] trait tells the collector what other objects in the gc heap
an object has references to. By iterating over all the objects in the gc heap,
you can determine the opposite, how many objects in the gc heap has references
to a particular object.

If at least one reference to an object is not stored within the gc heap, that
object is considered a root and reachable. Children of a reachable object
are themselves reachable. So, you recurs down the tree and mark all the children
who were marked unreachable as reachable.

Once all that is finished, the only things still marked unreachable will be
the things that _are_ actually unreachable, and can be safely freed.

# Type Overview

The [`Collector`] contains the garbage collector's internal state. In order
to communicate with it and get it to do things like store an object
or reclaim unused memory you have to go through a [`Proxy`].

Collection of unreachable memory only happens when either you call
[`Proxy::run`], or you store something in the gc heap and the heap is above
a size threshold.

The primary smart pointer type is [`Gc`]. It keeps the allocated memory alive
and dereferences to a shared reference.

The [`Weak`] pointer, on the other hand, isn't counted during reachability
analysis. You can have thousands of them, but if they are the only things
referencing an object, that object will be freed next time the collector
is run. It knows when the pointed-to object has been freed and will deny
access after that occurs.

The [`Safe`] pointer is  tracked during reachability analysis, and knows
if the underlying object has been freed. Just in case something goes wrong
and an object is accidentally freed.

# Storing Custom Structs

All types stored in the gc heap must implement the [`TaceTo`] trait, which
tells the collector where in your struct it can find pointers to other
things stored in the gc heap.

# Limitations

* You can't leak [`Gc`]s and [`Safe`]s outside of the gc heap

Calling [`mem::forget`] on a [`Gc`] will prevent the object it is pointing
to from being reclaimed, leaking that memory.

The collector knows how many pointers to an object exist. If it can't
find all of them it assumes the ones it can't find are somewhere
in the heap, but that the user still has a way of reaching it (like through
a [`Box`]).

# FAQ

* Doesn't rust's lifetimes prevent something like this? How does it placate the
    borrow checker?

From the borrow checker's point of view all references to objects in the gc heap
have the same lifetime, which is bounded by the life of the [`Proxy`] object that
created it.

* Won't there be use-after-free errors if // FIXME wording

A garbage collector's job is to clean up allocations once you don't need them.
But it doesn't touch something while you're still using it. As long as you have
a reference to an object it is guaranteed to be valid (modulo implementation bugs).

* If something goes wrong will I get segfaults?

Probably not. Any problems are much more likely to result in memory leaks
than dangling pointers.




[`Collector`]: struct.Collector.html
[`Proxy`]: struct.Proxy.html
[`Gc`]: ptr/struct.Gc.html
[`Weak`]: ptr/struct.Weak.html
[`Safe`]: ptr/struct.Safe.html
[`TraceTo`]: traceable/trait.TraceTo.html
[`Proxy::run`]: http://example.com // FIXME
[`mem::forget`]: http://example.com // FIXME
[`Box`]: http://example.com // FIXME
[`Rc`]: http://example.com // FIXME
[`Tiny Garbage Collector`]: http://example.com // FIXME
