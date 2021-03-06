//! A **t**ine**e** **R**u**s**t **g**arbage **c**ollector
//!
//! ("tiny" is deliberately misspelled for the sake of the acronym)
//!
//! A toy mark-and-sweep garbage collecting allocator.
//! Based loosely on orangeduck's [`Tiny Garbage Collector`].
//!
//! Provides the [`Gc`] type, essentially an [`Rc`] that can handle cycles.
//!
//! An example of use with a cyclic data structure:
//!
//! ```
//! extern crate ters_gc;
//! #[macro_use] extern crate ters_gc_derive;
//!
//! use ters_gc::{Collector, Gc, trace};
//! use std::cell::RefCell;
//!
//! // A struct that can hold references to itself
//! #[derive(Trace)]
//! struct CyclicStruct<'a>(RefCell<Option<Gc<'a, CyclicStruct<'a>>>>);
//!
//! // Make a new collector to keep the gc state
//! let mut col = Collector::new();
//!
//! // Make a Proxy to access the API
//! let mut proxy = col.proxy();
//!
//! // Do some computations that are best expressed with a cyclic data structure
//! {
//!     let thing1 = proxy.alloc(CyclicStruct(RefCell::new(None)));
//!     let thing2 = proxy.alloc(CyclicStruct(RefCell::new(Some(thing1.clone()))));
//!     *thing1.0.borrow_mut() = Some(thing2.clone());
//! }
//!
//! // Collect garbage
//! proxy.run();
//!
//! // And we've successfully cleaned up the unused cyclic data
//! assert_eq!(proxy.num_tracked(), 0);
//! ```
//!
//! # Type Overview
//!
//! The [`Collector`] contains the garbage collector's internal state. In order
//! to communicate with it and get it to do things like store an object
//! or reclaim unused memory you have to go through a [`Proxy`].
//!
//! Collection of unreachable memory only happens when either you call
//! [`Proxy::run`], or you store something in the gc heap and the heap is above
//! a size threshold.
//!
//! The primary smart pointer type is [`Gc`]. It keeps the allocated memory alive
//! and dereferences to a shared reference. Its API surface is meant to mimick
//! that of [`Rc`].
//!
//! The [`Weak`] pointer isn't counted during reachability analysis.
//! You can have thousands of them, but if they are the only things
//! referencing an object, that object will be freed next time the collector
//! is run. You have to [`upgrade`] to a [`Gc`] before you can access the
//! inner object.
//!
//! # Storing Custom Structs
//!
//! All types stored in the gc heap must implement the [`Trace`] trait, which
//! tells the collector where in your struct it can find pointers to other
//! things stored in the gc heap.
//!
//! To make it easy, you can `#[derive(Trace)]`.
//!
//! [`Trace`] is implemented for many of the types in `std`.
//!
//! Check the [`trace module`] documentation for more information.
//!
//! # Soundness (A.K.A. Is this safe?)
//!
//! This library should be safe to use and should not result in undefined
//! behavior.
//!
//! Assuming the absence of bugs, use of this library should not cause any
//! use-after-free errors (aside from the special case of destructors). [`Gc`]
//! essentially acts like an [`Rc`] that knows something about all other created
//! [`Rc`]s. Until you drop all the [`Gc`]s to an object, those [`Gc`]s won't be
//! invalidated by collection. Just like with [`Rc`], your pointers stay valid
//! until you no longer use them.
//!
//! The most likely result of a user error (e.g. not telling the [`Tracer`] about all
//! your [`Gc`] in your [`Trace`] impl or leaking a [`Gc`] pointer)
//! is that the memory is leaked.
//!
//! The library doesn't use any platform-specific tricks and doesn't touch any
//! memory outside of what it allocates. It uses typed, rust semantics when dealing
//! with the types you store.  If the rust compiler decides that the best way of
//! laying out a [`Gc`] in your struct is to split it in half and put in the middle
//! all your struct's other members, it will still function correctly. It doesn't
//! rely on pointer math nor the size, alignment, or layout of types when tracing.
//! The only raw pointers created or dereferenced are ones to allocations it made.
//! It doesn't touch the stack or crawl through the heap.
//!
//! # Garbage Collection Algorithm
//!
//! Collection is done in two phases. The mark phase determines which objects are
//! still reachable. If an object is reachable it can be considered in use by the
//! client program. The sweep phase frees all the objects that weren't marked
//! reachable during the mark phase.
//!
//! The mark phase is also split into two steps. First the collector visits every
//! tracked object and for each object it finds what other objects it has pointers
//! to. At the end of this part every tracked object has a count - the number
//! of pointers to that object that we found.
//!
//! Now the collector determines which objects the client has direct pointers to.
//! Every [`Gc`] has a reference count of the number of [`Gc`]s that
//! point to the object. From the previous step, we also know the number of
//! [`Gc`]s for an object that the client does not have direct access to.
//! So, if the total number of [`Gc`]s is greater than the number of found [`Gc`]s,
//! at least one [`Gc`] to the object must exist outside of the gc heap.
//! That [`Gc`] must be either on the stack or in the heap
//! (such as if it was stored in a [`Box`] or [`Vec`]). Regardless, we assume that
//! the client should be able to access it. If it is stored on the stack then
//! it will be [`drop`]ed when it become inaccessible (after it goes out of scope).
//! If it is stored in the heap, it should be [`drop`]ed after the owner of the
//! pointer goes out of scope. We assume that it hasn't been leaked, because
//! we have no way of determining that. So we can say that the client can reach
//! [`Gc`]s in the heap.
//!
//! We call objects with at least one [`Gc`] outside of the gc heap roots.
//! Roots are objects that the client can directly reach. Roots are marked reachable.
//!
//! You can go through a [`Gc`] stored in a root object to reach the object
//! that [`Gc`] points to. So, you can mark all the objects pointed to by
//! [`Gc`]s stored in a root object as reachable. Then you can mark all objects
//! pointed to by [`Gc`]s stored in the objects we just marked. Eventually
//! all objects that are transitively reachable will be marked so.
//!
//! Now that we know which objects are reachable and which are not we can free
//! objects the client is no longer using.
//!
//! # Limitations
//!
//! ## You cannot dereference a [`Gc`] inside of a [`Drop::drop`] implementation
//!
//! Dereferencing a [`Gc`] inside of an object's destructor may result in a panic.
//!
//! Many other methods on [`Gc`] also exhibit the same behavior. The documentation
//! for [`Gc`]'s methods specify if they can panic.
//!
//! If you mean to store a struct inside the gc heap, that struct's destructor
//! cannot dereference any [`Gc`]s it contains. So if you *never* plan on storing
//! something in the gc heap it is safe to dereference a [`Gc`] in the destructor,
//! but **make sure** you aren't going to store it.
//!
//! As a general rule of thumb, if a type implements [`Trace`], it shouldn't
//! dereference any [`Gc`]s in its destructor.
//!
//! The order objects are destroyed during collection might be changed in future
//! verstions, so you should not rely on order to "safely" access data through [`Gc`]s.
//!
//! If you absolutely **must** dereference a [`Gc`] in a destructor, you either have to
//! first check [`Gc::is_alive`] or access using [`Gc::get`] (which checks that
//! it is alive).
//!
//! ## You can't leak [`Gc`]s outside of the gc heap
//!
//! Calling [`mem::forget`] on a [`Gc`] will prevent the object it is pointing
//! to from being reclaimed, leaking that memory.
//!
//! The collector knows how many pointers to an object exist. If it can't
//! find all of them it assumes the ones it can't find are on the stack or somewhere
//! in the heap that the user has a way of reaching (like through a [`Box`]).
//!
//! ## Do not mix-and-match [`Gc`]s from different [`Collector`]s
//!
//! Each [`Collector`] only knows about [`Gc`]s it gave out.
//!
//! If you allocate two [`Gc`]s from two different [`Collector`]s and have them
//! reference each other, you will leak them.
//!
//! ## The garbage collector is for single threaded use only
//!
//! None of the pointer types, nor [`Proxy`] should be [`Sync`] or [`Send`].
//!
//! ## Supports [`Sized`] type only
//!
//! There are plans for unsized type support, but it is not yet implemented
//!
//!
//! [`Collector`]: struct.Collector.html
//! [`Proxy`]: struct.Proxy.html
//! [`Gc`]: ptr/struct.Gc.html
//! [`Weak`]: ptr/struct.Weak.html
//! [`Safe`]: ptr/struct.Safe.html
//! [`Trace`]: trace/trait.Trace.html
//! [`trace module`]: trace/index.html
//! [`Tracer`]: trace/struct.Tracer.html
//! [`Proxy::run`]: struct.Proxy.html#method.run
//! [`Gc::is_alive`]: ptr/struct.Gc.html#method.is_alive
//! [`Gc::get`]: ptr/struct.Gc.html#method.get
//! [`upgrade`]: ptr/struct.Weak.html#method.upgrade
//! [`Drop::drop`]: https://doc.rust-lang.org/std/ops/trait.Drop.html#tymethod.drop
//! [`drop`]: https://doc.rust-lang.org/std/ops/trait.Drop.html#tymethod.drop
//! [`mem::forget`]: https://doc.rust-lang.org/std/mem/fn.forget.html
//! [`Sync`]: https://doc.rust-lang.org/std/marker/trait.Sync.html
//! [`Send`]: https://doc.rust-lang.org/std/marker/trait.Send.html
//! [`Box`]: https://doc.rust-lang.org/std/boxed/struct.Box.html
//! [`Vec`]: https://doc.rust-lang.org/std/vec/struct.Vec.html
//! [`Rc`]: https://doc.rust-lang.org/std/rc/struct.Rc.html
//! [`Sized`]: https://doc.rust-lang.org/std/marker/trait.Sized.html
//! [`Tiny Garbage Collector`]: https://github.com/orangeduck/tgc

// #![feature(unsize, coerce_unsized)]

// Keep the version number in sync with crate version
#![doc(html_root_url = "https://docs.rs/ters_gc/0.1.0")]
#![deny(
    missing_docs,
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

pub mod ptr;
pub use ptr::Gc;
mod allocator;
pub mod trace;

use allocator::AllocInfo;
use allocator::Allocator;
use ptr::GcBox;
use std::marker::PhantomData;
use std::ptr::NonNull;
use trace::Trace;

/// Used for type-erasure
pub(crate) enum UntypedGcBox {}

/// Cast a type-erased NonNull pointer to its original typed type
/// (or at least to a type that is more likely to be correct and can be
/// dereferenced).
///
/// Made as a trait to add a little type safety and readability
trait AsTyped {
    fn as_typed<T>(&self) -> NonNull<GcBox<T>>;
}
impl AsTyped for NonNull<UntypedGcBox> {
    fn as_typed<T>(&self) -> NonNull<GcBox<T>> {
        self.cast()
    }
}
/// Cast a NonNull pointer to a Gc allocation into a type-erased version
/// for storage.
///
/// Made as a trait to add a little type safety and readability
trait AsUntyped {
    fn as_untyped(&self) -> NonNull<UntypedGcBox>;
}
impl<T> AsUntyped for NonNull<GcBox<T>> {
    fn as_untyped(&self) -> NonNull<UntypedGcBox> {
        self.cast()
    }
}

/// State container for grabage collection.
/// Access to gc API must go through a [`Proxy`].
///
/// See [`Proxy`] for gc usage details.
///
/// [`Proxy`]: struct.Proxy.html
#[derive(Default, Debug, PartialEq)]
pub struct Collector {
    allocator: Allocator,
    collection_threshold: usize,
    load_factor: f64,
    sweep_factor: f64,
    paused: bool,
}

impl Collector {
    /// Constructs a new `Collector`
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::Collector;
    ///
    /// let mut col = Collector::new();
    /// ```
    pub fn new() -> Collector {
        Collector {
            allocator: Allocator::new(),
            collection_threshold: 25,
            load_factor: 0.9,
            sweep_factor: 0.5,
            paused: false,
        }
    }

    /// Create a new [`Proxy`](struct.Proxy.html) for this collector.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::Collector;
    ///
    /// let mut col = Collector::new();
    ///
    /// let mut proxy = col.proxy();
    /// ```
    // While allocator is active, all pointers to Collector are valid (since the arena
    // can't be moved while there is a reference to it)
    pub fn proxy(&mut self) -> Proxy {
        Proxy { collector: self }
    }

    fn alloc<T: Trace>(&mut self, val: T) -> NonNull<GcBox<T>> {
        if self.should_collect() {
            self.run();
        }
        self.allocator.alloc(val)
    }

    fn run(&mut self) {
        // Find the tracked objects that the client can still use
        self.mark();
        // Remove the objects that the client can't
        self.sweep();
    }

    fn mark(&self) {
        // Count number of references to each other objects in the gc heap hold
        for info in self.allocator.items.values() {
            self.mark_inter_connections(info.ptr);
        }

        // Anything that is reachable must be a root
        let roots = self
            .allocator
            .items
            .values()
            .filter(|info| Collector::is_object_reachable(info));

        // Mark roots reachable and mark all their children reachable
        for info in roots {
            info.mark_reachable();
            self.mark_children_reachable(info.ptr);
        }
    }

    /// Increment an object's counter for each reference to it this object holds
    fn mark_inter_connections(&self, ptr: NonNull<UntypedGcBox>) {
        // assert!(self.allocator.is_ptr_in_range(ptr));

        if let Some(info) = self.allocator.info_for_ptr(ptr.as_ptr()) {
            for val in info.children() {
                if let Some(child) = self.allocator.info_for_ptr(val.as_ptr()) {
                    child.mark_inter_ref();
                }
            }
        }
    }

    /// Recusively mark all children as reachable
    fn mark_children_reachable(&self, ptr: NonNull<UntypedGcBox>) {
        // assert!(self.allocator.is_ptr_in_range(ptr));

        if let Some(info) = self.allocator.info_for_ptr(ptr.as_ptr()) {
            for val in info.children() {
                if let Some(child) = self.allocator.info_for_ptr(val.as_ptr()) {
                    if !child.is_marked_reachable() {
                        child.mark_reachable();
                        self.mark_children_reachable(val);
                    }
                }
            }
        }
    }

    /// Can the client access the object?
    fn is_object_reachable(info: &AllocInfo) -> bool {
        let inter_refs = info.inter_marks();
        let total_refs = info.ref_count();

        // If the object has more refs than we found, then there exists a
        // reference to that object outside of the gc heap. We assume that
        // all references stored outside of the gc heap are reachable.
        // Otherwise, the object is reachable if we marked it reachable.
        total_refs > inter_refs || info.is_marked_reachable()
    }

    /// Reclaim unreachable objects
    fn sweep(&mut self) {
        let mut unreachable_objects = vec![];
        for info in self.allocator.items.values() {
            if !Collector::is_object_reachable(info) {
                unreachable_objects.push(info.ptr);
            } else {
                info.unmark();
            }
        }

        for ptr in unreachable_objects {
            self.allocator.free(ptr);
        }

        // Update automatic collection threshold
        self.update_collection_threshold();

        if self.allocator.should_shrink_items() {
            self.allocator.shrink_items();
        }
    }

    fn pause(&mut self) {
        self.paused = true;
    }
    fn resume(&mut self) {
        self.paused = false;
    }

    fn num_tracked(&self) -> usize {
        self.allocator.items.len()
    }

    pub(crate) fn try_remove<'a, T: 'a>(&mut self, gc: Gc<'a, T>) -> Result<T, Gc<'a, T>> {
        // Gc must be valid and the only strong pointer to the object
        if Gc::is_alive(&gc) && Gc::strong_count(&gc) == 1 {
            let ptr = gc.nonnull_box_ptr().as_untyped();
            // This is safe because the we are taking both the `T` and the
            // pointer from the `Gc`.
            // We are guaranteed for the `T` to be the same type as was originally
            // stored when allocating the ptr.
            let val = unsafe { self.allocator.remove::<T>(ptr) };
            Ok(val)
        } else {
            Err(gc)
        }
    }

    // Get the ideal number of tracked objects that can hold at least the current
    // number of objects.
    // Algorithm for ideal size from tgc
    #[allow(dead_code)]
    fn ideal_size(&self) -> usize {
        // Primes taken from tgc
        const PRIMES: [usize; 24] = [
            0, 1, 5, 11, 23, 53, 101, 197, 389, 683, 1_259, 2_417, 4_733, 9_371, 18_617, 37_097,
            74_093, 148_073, 296_099, 592_019, 1_100_009, 2_200_013, 4_400_021, 8_800_019,
        ];

        let target = (self.num_tracked() + 1) as f64 / self.load_factor;
        let target = target as usize;

        let sat_prime = PRIMES.iter().find(|prime| **prime >= target);
        if let Some(prime) = sat_prime {
            *prime
        } else {
            let mut sat_size = PRIMES[PRIMES.len() - 1];
            while sat_size < target {
                sat_size += PRIMES[PRIMES.len() - 1];
            }
            sat_size
        }
    }

    /// Update point at which we do automatic collection
    fn update_collection_threshold(&mut self) {
        let num_tracked = self.num_tracked();
        let additional = (num_tracked as f64 * self.sweep_factor) as usize;
        self.collection_threshold = num_tracked + additional + 1;
    }

    fn should_collect(&self) -> bool {
        // !self.paused && self.ideal_size() > self.collection_threshold
        !self.paused && self.num_tracked() >= self.collection_threshold
    }
}

/// Provides access to the collector.
///
/// Allows for allocation and collection.
///
/// Can also be used to control collection.
#[derive(Debug, PartialEq)]
pub struct Proxy<'arena> {
    collector: &'arena mut Collector,
}

impl<'a> Proxy<'a> {
    /// Stores something in the gc heap.
    ///
    /// If not [`paused`], runs the gc if the heap got too big.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::Collector;
    ///
    /// let mut col = Collector::new();
    /// let mut proxy = col.proxy();
    ///
    /// let val = proxy.alloc(42);
    /// assert_eq!(*val, 42);
    /// ```
    ///
    /// [`paused`]: #method.paused
    pub fn alloc<T: Trace>(&mut self, payload: T) -> Gc<'a, T> {
        let ptr = self.collector.alloc(payload);
        Gc::from_raw_nonnull(ptr, PhantomData)
    }

    /// Runs the gc, freeing unreachable objects.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::Collector;
    ///
    /// let mut col = Collector::new();
    /// let mut proxy = col.proxy();
    ///
    /// {
    ///     proxy.alloc(42);
    /// }
    /// assert_eq!(proxy.num_tracked(), 1);
    /// proxy.run();
    /// assert_eq!(proxy.num_tracked(), 0);
    /// ```
    pub fn run(&mut self) {
        self.collector.run();
    }

    /// Returns whether or not automatic collection is paused.
    ///
    /// When paused, garbage collection will only occur if started manually
    /// via [`run`].
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::Collector;
    ///
    /// let mut col = Collector::new();
    /// let mut proxy = col.proxy();
    ///
    /// assert!(!proxy.paused());
    /// ```
    ///
    /// [`run`]: #method.run
    pub fn paused(&self) -> bool {
        self.collector.paused
    }

    /// Pauses automatic collection.
    ///
    /// Until [`resume`] is called, storing things in the gc
    /// heap will not trigger collection. The only time collection will occur
    /// is if it is done manually with [`run`].
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::Collector;
    ///
    /// let mut col = Collector::new();
    /// let mut proxy = col.proxy();
    ///
    /// proxy.pause();
    /// assert!(proxy.paused());
    /// ```
    ///
    /// [`resume`]: #method.resume
    /// [`run`]: #method.run
    pub fn pause(&mut self) {
        self.collector.pause();
    }

    /// Resume automatic collection.
    ///
    /// When storing something, it will run collection if the gc heap is too big.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::Collector;
    ///
    /// let mut col = Collector::new();
    /// let mut proxy = col.proxy();
    ///
    /// proxy.pause();
    /// assert!(proxy.paused());
    ///
    /// proxy.resume();
    /// assert!(!proxy.paused());
    /// ```
    pub fn resume(&mut self) {
        self.collector.resume();
    }

    /// Returns the number of objects in the gc heap.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::Collector;
    ///
    /// let mut col = Collector::new();
    /// let mut proxy = col.proxy();
    ///
    /// assert_eq!(proxy.num_tracked(), 0);
    ///
    /// let _ = proxy.alloc(());
    /// assert_eq!(proxy.num_tracked(), 1);
    ///
    /// let _ = proxy.alloc(());
    /// assert_eq!(proxy.num_tracked(), 2);
    /// ```
    pub fn num_tracked(&self) -> usize {
        self.collector.num_tracked()
    }

    /// Sets how much the threshold to run the gc when storing things grows.
    ///
    /// The higher the value the more objects you can store before storing triggers
    /// automatic collection.
    ///
    /// The automatic collection threshold will not be updated until collection
    /// is performed.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::Collector;
    ///
    /// let mut col = Collector::new();
    /// let mut proxy = col.proxy();
    ///
    /// proxy.set_threshold_growth(0.75);
    /// ```
    pub fn set_threshold_growth(&mut self, factor: f64) {
        self.collector.sweep_factor = factor;
    }

    /// Returns the number of objects that can be stored in the gc heap
    /// before collection is automatically run.
    ///
    /// Changes every time collection is performed.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::Collector;
    ///
    ///
    /// let mut col = Collector::new();
    /// let mut proxy = col.proxy();
    ///
    /// let init_thresh = proxy.threshold();
    ///
    /// for _ in 0..(init_thresh + 1) {
    ///     proxy.alloc(());
    /// }
    ///
    /// let new_thresh = proxy.threshold();
    ///
    /// assert!(init_thresh != new_thresh);
    /// ```
    ///
    pub fn threshold(&self) -> usize {
        self.collector.collection_threshold
    }
}

impl<'a> Drop for Proxy<'a> {
    fn drop(&mut self) {
        self.collector.allocator.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct LinkedList<'a> {
        next: Option<Gc<'a, LinkedList<'a>>>,
    }
    impl<'a> Trace for LinkedList<'a> {
        fn trace(&self, tracer: &mut trace::Tracer) {
            tracer.add_target(&self.next);
        }
    }

    fn num_tracked_objs(proxy: &Proxy) -> usize {
        proxy.num_tracked()
    }

    #[test]
    fn collect_while_in_stack_after_drop() {
        use std::mem::drop;
        let mut col = Collector::new();
        let mut proxy = col.proxy();

        for i in 0..60 {
            let num = proxy.alloc(i);
            assert_eq!(*num, i);
        }
        let num = proxy.alloc(-1);
        assert_eq!(*num, -1);
        assert!(proxy.num_tracked() > 0);
        proxy.run();
        assert!(proxy.num_tracked() > 0);
        drop(num);
        proxy.run();
        assert_eq!(0, proxy.num_tracked());
    }

    #[test]
    fn msc_allocs_sanity_check() {
        let mut col = Collector::new();
        let mut proxy = col.proxy();
        {
            let _num1 = proxy.alloc(42);
            assert_eq!(num_tracked_objs(&proxy), 1);
            proxy.run();
            assert_eq!(num_tracked_objs(&proxy), 1);
        }
        proxy.run();
        assert_eq!(num_tracked_objs(&proxy), 0);
    }

    #[test]
    fn collects_after_reaching_threshold() {
        let mut col = Collector::new();
        let threshold = col.collection_threshold;
        let num_useful = 13;
        let num_wasted = threshold - num_useful;
        assert!(threshold > num_useful);

        let mut proxy = col.proxy();

        let mut head = LinkedList { next: None };
        macro_rules! prepend_ll {
            () => {{
                let boxed = proxy.alloc(head);
                LinkedList { next: Some(boxed) }
            }};
        }
        for _ in 0..num_useful {
            head = prepend_ll!(); //(&mut proxy, head);
        }
        {
            for _ in 0..num_wasted {
                proxy.alloc(22);
            }
        }
        assert_eq!(num_tracked_objs(&proxy), threshold);
        head = prepend_ll!(); //(&mut proxy, head);
        assert_eq!(num_tracked_objs(&proxy), num_useful + 1);
        assert!(head.next.is_some());
    }

    #[test]
    fn pause_works() {
        let mut col = Collector::new();
        let threshold = col.collection_threshold;
        let num_useful = 13;
        let num_wasted = threshold - num_useful;
        assert!(threshold > num_useful);

        let mut proxy = col.proxy();

        let mut head = LinkedList { next: None };
        macro_rules! prepend_ll {
            () => {{
                let boxed = proxy.alloc(head);
                LinkedList { next: Some(boxed) }
            }};
        }
        for _ in 0..num_useful {
            head = prepend_ll!(); //(&mut proxy, head);
        }
        {
            for _ in 0..num_wasted {
                proxy.alloc(22);
            }
        }
        assert_eq!(num_tracked_objs(&proxy), threshold);
        proxy.pause();
        prepend_ll!(); //(&mut proxy, head);
        assert_eq!(num_tracked_objs(&proxy), threshold + 1);
    }

    #[test]
    fn resume_also_works() {
        let mut col = Collector::new();
        let threshold = col.collection_threshold;
        let num_useful = 13;
        let num_wasted = threshold - num_useful;
        assert!(threshold > num_useful);

        let mut proxy = col.proxy();
        let mut head = LinkedList { next: None };
        macro_rules! prepend_ll {
            () => {{
                let boxed = proxy.alloc(head);
                LinkedList { next: Some(boxed) }
            }};
        }
        for _ in 0..num_useful {
            head = prepend_ll!(); //(&mut proxy, head);
        }
        for _ in 0..num_wasted {
            proxy.alloc(22);
        }
        assert_eq!(num_tracked_objs(&proxy), threshold);
        proxy.pause();
        proxy.resume();
        prepend_ll!(); //(&mut proxy, head);
        assert_eq!(num_tracked_objs(&proxy), num_useful + 1);
    }

    #[test]
    fn self_ref_cycle() {
        use std::cell::RefCell;
        struct SelfRef<'a> {
            self_ptr: RefCell<Option<Gc<'a, SelfRef<'a>>>>,
        }
        impl<'a> Trace for SelfRef<'a> {
            fn trace(&self, tracer: &mut trace::Tracer) {
                tracer.add_target(&self.self_ptr);
            }
        }
        let mut col = Collector::new();
        let mut proxy = col.proxy();
        {
            let ptr = proxy.alloc(SelfRef {
                self_ptr: RefCell::new(None),
            });
            *ptr.self_ptr.borrow_mut() = Some(ptr.clone());

            proxy.run();
        }

        proxy.run();
        assert_eq!(num_tracked_objs(&proxy), 0);
    }

    #[test]
    fn pointed_to_by_heap_root_arent_freed() {
        struct List<'a> {
            ptr: Option<Gc<'a, List<'a>>>,
        }
        impl<'a> Trace for List<'a> {
            fn trace(&self, tracer: &mut trace::Tracer) {
                tracer.add_target(&self.ptr);
            }
        }
        let mut col = Collector::new();
        let mut proxy = col.proxy();
        let _root = {
            let leaf = proxy.alloc(List { ptr: None });
            let root = proxy.alloc(List { ptr: Some(leaf) });
            Box::new(root)
        };

        proxy.run();
        assert_eq!(num_tracked_objs(&proxy), 2);
    }

    #[test]
    // A.K.A. Crate doc test
    fn min_cycle() {
        use std::cell::RefCell;

        // A struct that can hold references to itself
        struct CyclicStruct<'a>(RefCell<Option<Gc<'a, CyclicStruct<'a>>>>);

        // All things in the gc heap need to impl `Trace`
        impl<'a> Trace for CyclicStruct<'a> {
            fn trace(&self, tracer: &mut trace::Tracer) {
                // Tell the tracer where to find our gc pointer
                tracer.add_target(&self.0);
            }
        }

        // Make a new collector to keep the gc state
        let mut col = Collector::new();

        // Make a Proxy to access the API
        let mut proxy = col.proxy();

        // Do some computations that are best expressed with a cyclic data structure
        {
            let thing1 = proxy.alloc(CyclicStruct(RefCell::new(None)));
            let thing2 = proxy.alloc(CyclicStruct(RefCell::new(Some(thing1.clone()))));
            *thing1.0.borrow_mut() = Some(thing2.clone());
        }

        // Collect garbage
        proxy.run();

        // And we've successfully cleaned up the unused cyclic data
        assert_eq!(proxy.num_tracked(), 0);
    }

    #[test]
    fn get_current_threshold() {
        let mut col = Collector::new();
        let mut proxy = col.proxy();
        let threshold = proxy.threshold();
        assert_eq!(proxy.collector.collection_threshold, threshold);

        let num_useful = 13;
        let num_wasted = threshold - num_useful;
        assert!(threshold > num_useful);

        let mut head = LinkedList { next: None };
        macro_rules! prepend_ll {
            () => {{
                let boxed = proxy.alloc(head);
                LinkedList { next: Some(boxed) }
            }};
        }
        for _ in 0..num_useful {
            head = prepend_ll!(); //(&mut proxy, head);
        }
        for _ in 0..num_wasted {
            proxy.alloc(22);
        }
        assert_eq!(proxy.num_tracked(), threshold);
        head = prepend_ll!(); //(&mut proxy, head);
        assert_eq!(proxy.num_tracked(), num_useful + 1);
        assert!(head.next.is_some());

        let after_thresh = proxy.threshold();
        assert_eq!(20, after_thresh);
    }

    #[test]
    fn set_sweep_factor() {
        let mut col = Collector::new();
        let mut proxy = col.proxy();
        proxy.set_threshold_growth(0.1);
        let factor1 = proxy.collector.sweep_factor;
        assert_eq!(factor1, 0.1);
        proxy.set_threshold_growth(0.9);
        let factor2 = proxy.collector.sweep_factor;
        assert_eq!(factor2, 0.9);
    }
    //    /// # use std::error::Error;
    //    /// #
    //    /// # fn try_main() -> Result<(), Box<Error>> {
    //    /// <PUT CODE HERE>
    //    /// #
    //    /// #     Ok(())
    //    /// # }
    //    /// #
    //    /// # fn main() {
    //    /// #     try_main().unwrap();
    //    /// # }

}
