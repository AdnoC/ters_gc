```
A tinee Rust garbage collector (ters gc)
  ^   ^ ^ ^  ^       ^
```

TODO: Copy info from module doc comment

TODO: Mention only `Sized`








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

