```
A tinee Rust garbage collector (ters gc)
  ^   ^ ^ ^  ^       ^
```

("tiny" is deliberately misspelled for the sake of the acronym)

A toy mark-and-sweep garbage collecting allocator.
Based loosely on orangeduck's
[`Tiny Garbage Collector`](https://github.com/orangeduck/tgc).

Provides the `Gc` type, essentially an [`Rc`](https://doc.rust-lang.org/std/rc/struct.Rc.html)
that can handle cycles.

Won't be published on crates.io, so you have to `cargo doc --open` if you want to read the docs in your browser.


# Example

Add this to your 'Cargo.toml':

```toml
[dependencies]
ters_gc = "0.1"
ters_gc_derive = "0.1"
```

main.rs:

```rust
extern crate ters_gc;
#[macro_use]
extern crate ters_gc_derive;

use std::cell::RefCell;
use ters_gc::{Collector, Gc};

// Allow it to be stored in the gc heap
#[derive(Trace)]
// A struct that can hold references to itself
struct CyclicStruct<'a>(RefCell<Option<Gc<'a, CyclicStruct<'a>>>>);

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
```

# Limitations

## You cannot dereference a `Gc` inside of a [`Drop`](https://doc.rust-lang.org/std/ops/trait.Drop.html) implementation

Dereferencing a `Gc` inside of an object's destructor may result in a panic.

Many other methods on `Gc` also exhibit the same behavior. The documentation
for `Gc`'s methods specify if they can panic.

If you mean to store a struct inside the gc heap, that struct's destructor
cannot dereference any `Gc`s it contains. So if you *never* plan on storing
something in the gc heap it is safe to dereference a `Gc` in the destructor,
but **make sure** you aren't going to store it.

If you absolutely **must** dereference a `Gc` in a destructor, you either have to
first check `Gc::is_alive` or access using `Gc::get` (which checks that
it is alive).

## You can't leak `Gc`s outside of the gc heap

Calling [`mem::forget`](https://doc.rust-lang.org/std/mem/fn.forget.html)
on a `Gc` will prevent the object it is pointing to from being reclaimed,
leaking that memory.

The collector knows how many pointers to an object exist. If it can't
find all of them it assumes the ones it can't find are on the stack or somewhere
in the heap that the user has a way of reaching (like through a
[`Box`](https://doc.rust-lang.org/std/boxed/struct.Box.html)).

## Do not mix-and-match `Gc`s from different `Collector`s

Each `Collector` only knows about `Gc`s it gave out.

If you allocate two `Gc`s from two different `Collector`s and have them
reference each other, you will leak them.

## The garbage collector is for single threaded use only

Nothing should be
[`Sync`](https://doc.rust-lang.org/std/marker/trait.Sync.html) or
[`Send`](https://doc.rust-lang.org/std/marker/trait.Send.html).

## Supports [`Sized`](https://doc.rust-lang.org/std/marker/trait.Sized.html) type only

There are plans for unsized type support, but it is not yet implemented

# License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

# Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
