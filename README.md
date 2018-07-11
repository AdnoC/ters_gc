```
A tinee Rust garbage collector (ters gc)
  ^   ^ ^ ^  ^       ^
```

TODO: Copy info from module doc comment

TODO: Mention only `Sized`


# Soundness

Assuming the absence of bugs, use of this library should not cause any
use-after-free errors (aside from the special case of destructors). [`Gc`]
essentially acts like an [`Rc`] that knows something about all other created
[`Rc`]s. Until you drop all the [`Gc`]s to an object, those [`Gc`]s won't be
invalidated by collection. Just like with [`Rc`], your pointers stay valid
until you no longer use them.

The most likely result of a user error (e.g. not telling the [`Tracer`] about all
your [`Gc`] in your [`Trace`] impl or leaking a [`Gc`] pointer)
is that the memory is leaked.






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

