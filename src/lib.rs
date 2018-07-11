//! A **t**ine**e** **R**u**s**t **g**arbage **c**ollector
//!
//! ("tiny" is deliberately misspelled for the sake of the acronym)
//!
//!
//! *TODO: CHECK THAT ALL LINKS ARE VALID*
//! *TODO: Document UB when deref Gc in destructor*
//!
//! TODO: Mention that Gc is basically improved Rc
//!
//! A mark-and-sweep garbage collecting allocator 
//! Based loosely on the [`Tiny Garbage Collector`].
//!
//! Provides the [`Gc`] type, essentially an [`Rc`] that can handle cycles.
//!
//! An example of use with a cyclic data structure:
//!
//! ```
//! use ters_gc::{Collector, Gc, trace};
//! use std::cell::RefCell;
//!
//!
//! // A struct that can hold references to itself
//! struct CyclicStruct<'a>(RefCell<Option<Gc<'a, CyclicStruct<'a>>>>);
//!
//! // All things in the gc heap need to impl `TraceTo`
//! impl<'a> trace::TraceTo for CyclicStruct<'a> {
//!     fn trace_to(&self, tracer: &mut trace::Tracer) {
//!         // Tell the tracer where to find our gc pointer
//!         self.0.trace_to(tracer);
//!     }
//! }
//!
//!
//! // Make a new collector to keep the gc state
//! let mut col = Collector::new();
//!
//! // Find out the meaning of life, and allow use of the gc while doing so
//! let meaning = col.run_with_gc(|mut proxy| {
//!
//!     // Do some computations that are best expressed with a cyclic data structure
//!     {
//!         let thing1 = proxy.store(CyclicStruct(RefCell::new(None)));
//!         let thing2 = proxy.store(CyclicStruct(RefCell::new(Some(thing1.clone()))));
//!         *thing1.0.borrow_mut() = Some(thing2.clone());
//!     }
//!
//!     // Collect garbage
//!     proxy.run();
//!
//!     // And we've successfully cleaned up the unused cyclic data
//!     assert_eq!(proxy.num_tracked(), 0);
//!
//!     // Return
//!     42
//! });
//!
//! assert_eq!(meaning, 42);
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
//! The [`Weak`] pointer, on the other hand, isn't counted during reachability
//! analysis. You can have thousands of them, but if they are the only things
//! referencing an object, that object will be freed next time the collector
//! is run. It knows when the pointed-to object has been freed and will deny
//! access after that occurs.
//!
//! # Storing Custom Structs
//!
//! All types stored in the gc heap must implement the [`TaceTo`] trait, which
//! tells the collector where in your struct it can find pointers to other
//! things stored in the gc heap.
//!
//! *TODO: Mention that most standard types have it implemented*
//!
//! # Limitations
//!
//! * You cannot dereference a [`Gc`] inside of a [`Drop::drop`] implementation
//!
//! Dereferencing a [`Gc`] inside of an object's destructor may result in a panic.
//!
//! If you mean to store a struct inside the gc heap, that struct's destructor
//! cannot dereference any [`Gc`]s it contains. So if you never plan on storing
//! something in the gc heap it is safe to dereference a [`Gc`] in the destructor,
//! but **make sure** you aren't going to store it.
//!
//! As a general rule of thumb, if a type implements [`TraceTo`], it shouldn't
//! dereference any [`Gc`]s in its destructor.
//!
//! The order objects are destroyed during collection is unspecified, so you
//! should not rely on order to "safely" access data through [`Gc`]s.
//!
//! If you absolutely **must** dereference a [`Gc`] in a destructor, you have to
//! first chech [`Gc::is_alive`], or access using [`Gc::get`] (which checks that
//! it is alive).
//!
//! * You can't leak [`Gc`]s outside of the gc heap
//!
//! Calling [`mem::forget`] on a [`Gc`] will prevent the object it is pointing
//! to from being reclaimed, leaking that memory.
//!
//! The collector knows how many pointers to an object exist. If it can't
//! find all of them it assumes the ones it can't find are somewhere
//! in the heap, but that the user still has a way of reaching it (like through
//! a [`Box`]).
//!
//! * The garbage collector is for single threaded use only
//!
//!
//!
//!
//!
//! [`Collector`]: struct.Collector.html
//! [`Proxy`]: struct.Proxy.html
//! [`Gc`]: ptr/struct.Gc.html
//! [`Weak`]: ptr/struct.Weak.html
//! [`Safe`]: ptr/struct.Safe.html
//! [`TraceTo`]: traceable/trait.TraceTo.html
//! [`Proxy::run`]: struct.Proxy.html#method.run
//! [`Gc::is_alive`]: ptr/struct.Gc.html#method.is_alive
//! [`Gc::get`]: ptr/struct.Gc.html#method.get
//! [`Drop::drop`]: https://doc.rust-lang.org/std/ops/trait.Drop.html#tymethod.drop
//! [`mem::forget`]: https://doc.rust-lang.org/std/mem/fn.forget.html
//! [`Box`]: https://doc.rust-lang.org/std/boxed/struct.Box.html
//! [`Rc`]: https://doc.rust-lang.org/std/rc/struct.Rc.html
//! [`Tiny Garbage Collector`]: http://tinygc.sourceforge.net/

enum BoxedCollector {} // TODO Make NonNull<GcBox<T>>
pub(crate) enum UntypedGcBox {} // TODO Make NonNull<GcBox<T>>

pub mod ptr;
pub use ptr::Gc;
pub mod trace {
    pub use traceable::{TraceTo, Tracer};
}
mod traceable;
mod allocator;

use allocator::AllocInfo;
use allocator::Allocator;
use ptr::GcBox;
use std::ptr::NonNull;
use std::marker::PhantomData;
use traceable::TraceTo;


trait AsTyped {
    fn as_typed<T>(&self) -> NonNull<GcBox<T>>;
}
impl AsTyped for NonNull<UntypedGcBox> {
    fn as_typed<T>(&self) -> NonNull<GcBox<T>> {
        self.cast()
    }
}
trait AsUntyped {
    fn as_untyped(&self) -> NonNull<UntypedGcBox>;
}
impl<T> AsUntyped for NonNull<GcBox<T>> {
    fn as_untyped(&self) -> NonNull<UntypedGcBox> {
        self.cast()
    }
}


/// State container for grabage collection.
/// Access to the API goes through [`Proxy`].
///
/// See [`Proxy`] for gc use details.
///
/// [`Proxy`]: struct.Proxy.html
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

    /// Run the passed function, providing it access to gc operations via a
    /// [`Proxy`](struct.Proxy.html).
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::Collector;
    ///
    /// let mut col = Collector::new();
    ///
    /// let val = col.run_with_gc(|_proxy| 42);
    /// assert_eq!(val, 42);
    /// 
    /// ```
    pub fn run_with_gc<R, T: FnOnce(Proxy) -> R>(&mut self, func: T) -> R {
        let proxy = self.proxy();
        func(proxy)
    }

    fn alloc<T: TraceTo>(&mut self, val: T) -> NonNull<GcBox<T>> {
        if self.should_collect() {
            self.run();
        }
        let ptr = self.allocator.alloc(val);
        ptr
    }

    fn run(&mut self) {
        self.mark();
        self.sweep();
    }

    fn mark(&self) {
        for info in self.allocator.items.values() {
            self.mark_inter_connections(info.ptr);
        }

        let roots = self.allocator.items.values()
            .filter(|info| Self::is_object_reachable(info));

        for info in roots {
            self.mark_children_reachable(info.ptr);
            info.mark_reachable();
        }
    }

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

    fn is_object_reachable(info: &AllocInfo) -> bool {
        let inter_refs = info.inter_marks();
        let total_refs = info.ref_count();

        // If the object has more refs than we found, then there exists a
        // reference to that object outside of the gc heap. We assume that
        // all references stored outside of the gc heap are reachable.
        // Otherwise, the object is reachable if we marked it reachable.
        total_refs > inter_refs || info.is_marked_reachable()
    }

    fn sweep(&mut self) {
        let mut unreachable_objects = vec![];
        for info in self.allocator.items.values() {
            if !Self::is_object_reachable(info) {
                unreachable_objects.push(info.ptr);
            } else {
                info.unmark();
            }
        }

        for ptr in unreachable_objects.into_iter() {
            self.allocator.free(ptr);
        }

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
    // While allocator is active, all pointers to Collector are valid (since the arena
    // can't be moved while there is a reference to it)
    fn proxy(&mut self) -> Proxy {
        Proxy { collector: self }
    }
    fn try_remove<'a, T: 'a>(&mut self, gc: Gc<'a, T>) -> Result<T, Gc<'a, T>> {
        use std::ptr;
        if Gc::is_alive(&gc) && Gc::ref_count(&gc) == 1 {
            let ptr = gc.get_nonnull_gc_box().as_untyped();
            Ok(self.allocator.remove::<T>(ptr))
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
            0,       1,       5,       11,
            23,      53,      101,     197,
            389,     683,     1259,    2417,
            4733,    9371,    18617,   37097,
            74093,   148073,  296099,  592019,
            1100009, 2200013, 4400021, 8800019
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
/// Allows for allocation and collection.
pub struct Proxy<'arena> {
    collector: &'arena mut Collector,
}

impl<'a> Proxy<'a> {
    /// Stores something in the gc heap.
    ///
    /// If not paused, runs the gc if the heap got too big.
    ///
    /// # Examples
    ///
    ///
    pub fn store<T: TraceTo>(&mut self, payload: T) -> Gc<'a, T> {
        let ptr = self.collector.alloc(payload);
        Gc::from_raw_nonnull(ptr, PhantomData)
    }

    /// Runs the gc.
    pub fn run(&mut self) {
        self.collector.run();
    }
    /// Pauses automatic collection.
    ///
    /// Until [`Proxy::resume`][resume] is called, storing things in the gc
    /// heap will not trigger collection. The only way collection with run
    /// is if it is done manually with [`Proxy::run`][run].
    pub fn pause(&mut self) {
        self.collector.pause();
    }

    /// Resume automatic collection.
    ///
    /// When storing something, will run collection if the gc heap is too big.
    pub fn resume(&mut self) {
        self.collector.resume();
    }

    /// Gets the number of objects in the gc heap.
    pub fn num_tracked(&self) -> usize {
        self.collector.num_tracked()
    }

    /// Set how much the threshold to run the gc when storing things grows.
    pub fn set_threshold_growth(&mut self, factor: f64) {
        self.collector.sweep_factor = factor;
    }

    /// Get how much the threshold to run the gc when storing things grows.
    pub fn threshold(&self) -> usize {
        self.collector.collection_threshold
    }

    // Tested in ptr
    pub(crate) fn try_remove<T>(&mut self, gc: Gc<'a, T>) -> Result<T, Gc<'a, T>> {
        self.collector.try_remove(gc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct LinkedList<'a> {
        next: Option<Gc<'a, LinkedList<'a>>>,
    }
    impl<'a> TraceTo for LinkedList<'a> {
        fn trace_to(&self, tracer: &mut traceable::Tracer) {
            self.next.trace_to(tracer);
        }
    }

    fn num_tracked_objs(proxy: &Proxy) -> usize {
        proxy.num_tracked()
    }

    #[test]
    fn collect_while_in_stack_after_drop() {
        use std::mem::drop;
        let mut col = Collector::new();
        col.run_with_gc(|mut proxy| {

            for i in 0..60 {
                let num = proxy.store(i);
                assert_eq!(*num, i);
            }
            let num = proxy.store(-1);
            assert_eq!(*num, -1);
            assert!(proxy.num_tracked() > 0);
            proxy.run();
            assert!(proxy.num_tracked() > 0);
            drop(num);
            proxy.run();
            assert_eq!(0, proxy.num_tracked());
        });
    }

    #[test]
    fn msc_allocs_sanity_check() {
        let mut col = Collector::new();
        let body = |mut proxy: Proxy| {
            {
                let _num1 = proxy.store(42);
                assert_eq!(num_tracked_objs(&proxy), 1);
                proxy.run();
                assert_eq!(num_tracked_objs(&proxy), 1);
            }
            proxy.run();
            assert_eq!(num_tracked_objs(&proxy), 0);
        };
        col.run_with_gc(body);
    }

    #[test]
    fn collects_after_reaching_threshold() {
        let mut col = Collector::new();
        let threshold = col.collection_threshold;
        let num_useful = 13;
        let num_wasted = threshold - num_useful;
        assert!(threshold > num_useful);

        let body = |mut proxy: Proxy| {
            let mut head = LinkedList { next: None };
            macro_rules! prepend_ll {
                () => {{
                    let boxed = proxy.store(head);
                    LinkedList { next: Some(boxed) }
                }};
            }
            for _ in 0..num_useful {
                head = prepend_ll!(); //(&mut proxy, head);
            }
            {
                for _ in 0..num_wasted {
                    proxy.store(22);
                }
            }
            assert_eq!(num_tracked_objs(&proxy), threshold);
            head = prepend_ll!(); //(&mut proxy, head);
            assert_eq!(num_tracked_objs(&proxy), num_useful + 1);
            assert!(head.next.is_some());
        };
        col.run_with_gc(body);
    }

    #[test]
    fn pause_works() {
        let mut col = Collector::new();
        let threshold = col.collection_threshold;
        let num_useful = 13;
        let num_wasted = threshold - num_useful;
        assert!(threshold > num_useful);

        let body = |mut proxy: Proxy| {
            let mut head = LinkedList { next: None };
            macro_rules! prepend_ll {
                () => {{
                    let boxed = proxy.store(head);
                    LinkedList { next: Some(boxed) }
                }};
            }
            for _ in 0..num_useful {
                head = prepend_ll!(); //(&mut proxy, head);
            }
            {
                for _ in 0..num_wasted {
                    proxy.store(22);
                }
            }
            assert_eq!(num_tracked_objs(&proxy), threshold);
            proxy.pause();
            prepend_ll!(); //(&mut proxy, head);
            assert_eq!(num_tracked_objs(&proxy), threshold + 1);
        };
        col.run_with_gc(body);
    }

    #[test]
    fn resume_also_works() {
        let mut col = Collector::new();
        let threshold = col.collection_threshold;
        let num_useful = 13;
        let num_wasted = threshold - num_useful;
        assert!(threshold > num_useful);

        let body = |mut proxy: Proxy| {
            let mut head = LinkedList { next: None };
            macro_rules! prepend_ll {
                () => {{
                    let boxed = proxy.store(head);
                    LinkedList { next: Some(boxed) }
                }};
            }
            for _ in 0..num_useful {
                head = prepend_ll!(); //(&mut proxy, head);
            }
            for _ in 0..num_wasted {
                proxy.store(22);
            }
            assert_eq!(num_tracked_objs(&proxy), threshold);
            proxy.pause();
            proxy.resume();
            prepend_ll!(); //(&mut proxy, head);
            assert_eq!(num_tracked_objs(&proxy), num_useful + 1);
        };
        col.run_with_gc(body);
    }

    #[test]
    fn returning_a_value_works() {
        let mut col = Collector::new();
        let val = col.run_with_gc(|_proxy| 42);
        assert_eq!(val, 42);
    }

    #[test]
    fn self_ref_cycle() {
        use std::cell::RefCell;
        struct SelfRef<'a> {
            self_ptr: RefCell<Option<Gc<'a, SelfRef<'a>>>>,
        }
        impl<'a> TraceTo for SelfRef<'a> {
            fn trace_to(&self, tracer: &mut traceable::Tracer) {
                self.self_ptr.trace_to(tracer);
            }
        }
        let mut col = Collector::new();
        let body = |mut proxy: Proxy| {
            {
                let ptr = proxy.store(SelfRef {
                    self_ptr: RefCell::new(None),
                });
                *ptr.self_ptr.borrow_mut() = Some(ptr.clone());

                proxy.run();
            }

            proxy.run();
            assert_eq!(num_tracked_objs(&proxy), 0);
        };

        col.run_with_gc(body);
    }

    #[test]
    fn pointed_to_by_heap_root_arent_freed() {
        struct List<'a> {
            ptr: Option<Gc<'a, List<'a>>>,
        }
        impl<'a> TraceTo for List<'a> {
            fn trace_to(&self, tracer: &mut traceable::Tracer) {
                self.ptr.trace_to(tracer);
            }
        }
        let mut col = Collector::new();
        let body = |mut proxy: Proxy| {
            let _root = {
                let leaf = proxy.store(List { ptr: None });
                let root = proxy.store(List { ptr: Some(leaf) });
                Box::new(root)
            };

            proxy.run();
            assert_eq!(num_tracked_objs(&proxy), 2);
        };

        col.run_with_gc(body);
    }

    #[test]
    fn min_cycle() {
        use std::cell::RefCell;


        // A struct that can hold references to itself
        struct CyclicStruct<'a>(RefCell<Option<Gc<'a, CyclicStruct<'a>>>>);

        // All things in the gc heap need to impl `TraceTo`
        impl<'a> TraceTo for CyclicStruct<'a> {
            fn trace_to(&self, tracer: &mut trace::Tracer) {
                // Tell the tracer where to find our gc pointer
                self.0.trace_to(tracer);
            }
        }


        // Make a new collector to keep the gc state
        let mut col = Collector::new();

        // Find out the meaning of life, and allow use of the gc while doing so
        let meaning = col.run_with_gc(|mut proxy| {

            // Do some computations that are best expressed with a cyclic data structure
            {
                let thing1 = proxy.store(CyclicStruct(RefCell::new(None)));
                let thing2 = proxy.store(CyclicStruct(RefCell::new(Some(thing1.clone()))));
                *thing1.0.borrow_mut() = Some(thing2.clone());
            }

            // Collect garbage
            proxy.run();

            // And we've successfully cleaned up the unused cyclic data
            assert_eq!(proxy.num_tracked(), 0);

            // Return
            42
        });

        assert_eq!(meaning, 42);
    }

    #[test]
    fn get_current_threshold() {
        let mut col = Collector::new();
        let threshold = col.run_with_gc(|proxy| proxy.threshold());
        let init_thresh = threshold;
        assert_eq!(col.collection_threshold, threshold);

        let num_useful = 13;
        let num_wasted = threshold - num_useful;
        assert!(threshold > num_useful);

        col.run_with_gc(|mut proxy: Proxy| {
            let mut head = LinkedList { next: None };
            macro_rules! prepend_ll {
                () => {{
                    let boxed = proxy.store(head);
                    LinkedList { next: Some(boxed) }
                }};
            }
            for _ in 0..num_useful {
                head = prepend_ll!(); //(&mut proxy, head);
            }
            for _ in 0..num_wasted {
                proxy.store(22);
            }
            assert_eq!(proxy.num_tracked(), threshold);
            head = prepend_ll!(); //(&mut proxy, head);
            assert_eq!(proxy.num_tracked(), num_useful + 1);
            assert!(head.next.is_some());
        });

        let after_thresh = col.run_with_gc(|proxy| proxy.threshold());
        assert_eq!(20, after_thresh);
    }

    #[test]
    fn set_sweep_factor() {
        let mut col = Collector::new();
        col.run_with_gc(|mut proxy| proxy.set_threshold_growth(0.1));
        let factor1 = col.sweep_factor;
        assert_eq!(factor1, 0.1);
        col.run_with_gc(|mut proxy| proxy.set_threshold_growth(0.9));
        let factor2 = col.sweep_factor;
        assert_eq!(factor2, 0.9);
    }
}
