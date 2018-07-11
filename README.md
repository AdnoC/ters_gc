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

# Garbage Collection Algorithm

Collection is done in two phases. The mark phase determines which objects are
still reachable. The sweep phase frees all the objects that weren't marked
reachable during the mark phase.

The mark phase is also split into two steps. First the collector visits every
tracked object and for each object it finds what other objects it has pointers
to. At the end of this part every tracked object has a count - the number
of pointers to that object that we found.

Now the collector determines which objects the client has direct pointers to.
Every [`Gc`] has a reference count of the number of [`Gc`]s in existence that
point to the object. From the previous step, we also know the number of
[`Gc`]s for an object that the client does not have direct access to.
So, if the total number of [`Gc`]s is greater than the number of found [`Gc`]s,
there has to be some [`Gc`]s outside of the gc heap. Either on the stack
of in the heap through a [`Box`] or something. TODO: Finish  paragraph






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

