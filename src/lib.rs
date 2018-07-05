//! A "TinEe RuSt Garbage Collector"
//!
//! ("tiny" is deliberately misspelled for the sake of the acronym)
//!
//! FIXME MEMORY SAFETY HOLE: Weak::get. `T` should only be accessible after conversion to `Gc` via `upgrade`
//!
//! *TODO: Remove all stack-related mechanisms*
//! *TODO: CHECK THAT ALL LINKS ARE VALID*
//! *TODO: Ensure Proxy is !Send*
//!
//! A toy project implementing a mark-and-sweep garbage collecting allocator
//! in the rust programming language.
//! Based loosely on  <NAME>'s [`Tiny Garbage Collector`].
//!
//! Use at your own risk: The author is neither experienced with writing
//! unsafe rust nor has he studied garbage collectors.
//!
//! Having said that, it should be stable enough to play around with and
//! use for small projects.
//!
//! A short usage example:
//!
//! ```
//! use ters_gc::{Collector, Proxy, Gc, trace};
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
//! // Do some computations that are best expressed with a cyclic data structure
//! fn compute_cyclic_data(proxy: &mut Proxy) {
//!     let thing1 = proxy.store(CyclicStruct(RefCell::new(None)));
//!     let thing2 = proxy.store(CyclicStruct(RefCell::new(Some(thing1.clone()))));
//!     *thing1.0.borrow_mut() = Some(thing2.clone());
//! }
//!
//! // Make a new collector to keep the gc state
//! let mut col = Collector::new();
//!
//! // Because of how unsafe scoping works, you shouldn't make a lambda
//! // within the arguments of `run_with_gc`, otherwise you might stray
//! // outside of safe rust.
//! fn find_meaning_of_life(mut proxy: Proxy) -> i32 {
//!
//!     // Do some calculations. Do it later in the stack so that the pointers
//!     // to gc objects aren't in the used portion of the stack when collecting.
//!     compute_cyclic_data(&mut proxy); // FIXME: inline the function
//!
//!     // Collect garbage
//!     proxy.run();
//!
//!     // And we've successfully cleaned up the unused cyclic data
//!     assert_eq!(proxy.num_tracked(), 0);
//!
//!     // Return
//!     42
//! }
//!
//! // Find out the meaning of life, and allow use of the gc while doing so
//! let meaning = unsafe { col.run_with_gc(find_meaning_of_life) };
//!
//! assert_eq!(meaning, 42);
//! ```
//!
//! # Overview
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
//! and dereferences to a shared reference.
//!
//! The [`Weak`] pointer, on the other hand, isn't counted during reachability
//! analysis. You can have thousands of them, but if they are the only things
//! referencing an object, that object will be freed next time the collector
//! is run. It knows when the pointed-to object has been freed and will deny
//! access after that occurs.
//!
//! The [`Safe`] pointer is  tracked during reachability analysis, and knows
//! if the underlying object has been freed. Just in case something goes wrong
//! and an object is accidentally freed.
//!
//! # Storing Custom Structs
//!
//! All types stored in the gc heap must implement the [`TaceTo`] trait, which
//! tells the collector where in your struct it can find pointers to other
//! things stored in the gc heap.
//!
//! # Limitations
//!
//! * You can't leak [`Gc`]s and [`Safe`]s outside of the gc heap
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
//! [`Proxy::run`]: http://example.com // FIXME
//! [`mem::forget`]: http://example.com // FIXME
//! [`Box`]: http://example.com // FIXME
//! [`Tiny Garbage Collector`]: http://example.com // FIXME

enum BoxedCollector {} // TODO Make NonNull<GcBox<T>>
pub(crate) enum UntypedGcBox {} // TODO Make NonNull<GcBox<T>>

mod allocator;
pub mod ptr;
pub mod traceable;

use allocator::AllocInfo;
use allocator::Allocator;
use ptr::GcBox;
use std::ptr::NonNull;
use std::marker::PhantomData;
use traceable::TraceTo;

pub use ptr::Gc;
pub mod trace {
    pub use traceable::{TraceTo, Tracer};
}

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


pub struct Collector {
    allocator: Allocator,
    collection_threshold: usize,
    // load_factor: f64,
    sweep_factor: f64,
    paused: bool,
}

impl Collector {
    pub fn new() -> Collector {
        Collector {
            allocator: Allocator::new(),
            collection_threshold: 25,
            // load_factor: 0.9,
            sweep_factor: 0.5,
            paused: false,
        }
    }

    /// Unsafe because there is an unsafe hole in garbage collection that cannot
    /// be fixed. Namely, you cannot store pointers to tracked objects on the heap.
    pub unsafe fn run_with_gc<R, T: FnOnce(Proxy) -> R>(&mut self, func: T) -> R {
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
            self.mark_island_ptr(info.ptr);
        }

        let roots = self.allocator.items.values()
            .filter(|info| Self::is_object_reachable(info));

        for info in roots {
            self.mark_newly_found_ptr(info.ptr);
        }
    }

    fn mark_island_ptr(&self, ptr: NonNull<UntypedGcBox>) {
        // assert!(self.allocator.is_ptr_in_range(ptr));

        if let Some(info) = self.allocator.info_for_ptr(ptr.as_ptr()) {
            for val in info.children() {
                if let Some(child) = self.allocator.info_for_ptr(val.as_ptr()) {
                    child.mark_isolated();
                }
            }
        }
    }

    fn mark_newly_found_ptr(&self, ptr: NonNull<UntypedGcBox>) {
        // assert!(self.allocator.is_ptr_in_range(ptr));

        if let Some(info) = self.allocator.info_for_ptr(ptr.as_ptr()) {
            for val in info.children() {
                if let Some(child) = self.allocator.info_for_ptr(val.as_ptr()) {
                    if !child.is_marked_reachable() {
                        child.unmark_isolated();
                        child.mark_reachable();
                        self.mark_newly_found_ptr(val);
                    }
                }
            }
        }
    }

    fn is_object_reachable(info: &AllocInfo) -> bool {
        let isolated_refs = info.isolated_marks();
        let total_refs = info.ref_count();
        // assert!(stack_refs + refs_in_gc <= total_refs,
        //         "Found more references to object than were made.
        //          total: {}, stack: {}, in_gc_heap: {}, ptr: {}", total_refs, stack_refs, refs_in_gc, info.ptr as usize);

        // Don't actually do the subtraction, since it will underflow if
        // zombie values of an address are found on the stack.
        // let heap_refs = total_refs - stack_refs - refs_in_gc;
        // If we know it is reachable or the only refs are hidden in the heap
        total_refs > isolated_refs || info.is_marked_reachable()
    }

    fn sweep(&mut self) {
        let mut unreachable_objects = vec![];
        for info in self.allocator.items.values_mut() {
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

    fn update_collection_threshold(&mut self) {
        let num_tracked = self.num_tracked();
        let additional = (num_tracked as f64 * self.sweep_factor) as usize;
        self.collection_threshold = num_tracked + additional + 1;
    }

    fn should_collect(&self) -> bool {
        let num_tracked = self.num_tracked();
        !self.paused && num_tracked >= self.collection_threshold
    }
}

pub struct Proxy<'arena> {
    collector: &'arena mut Collector,
}

impl<'a> Proxy<'a> {
    pub fn store<T: TraceTo>(&mut self, payload: T) -> Gc<'a, T> {
        let ptr = self.collector.alloc(payload);
        Gc::from_raw_nonnull(ptr, PhantomData)
    }

    pub fn run(&mut self) {
        self.collector.run();
    }
    pub fn mark(&mut self) {
        self.collector.mark();
    }
    pub fn sweep(&mut self) {
        self.collector.sweep();
    }
    pub fn pause(&mut self) {
        self.collector.pause();
    }
    pub fn resume(&mut self) {
        self.collector.resume();
    }
    pub fn num_tracked(&self) -> usize {
        self.collector.num_tracked()
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
    #[inline(never)]
    fn eat_stack_and_exec<T, F: FnOnce() -> T>(recurs: usize, callback: F) -> T {
        let _nom = [22; 25];
        if recurs > 0 {
            eat_stack_and_exec(recurs - 1, callback)
        } else {
            callback()
        }
    }
    #[test]
    fn msc_allocs_sanity_check() {
        let mut col = Collector::new();
        let body = |mut proxy: Proxy| {
            eat_stack_and_exec(6, || {
                let _num1 = proxy.store(42);
                assert_eq!(num_tracked_objs(&proxy), 1);
                proxy.run();
                assert_eq!(num_tracked_objs(&proxy), 1);
            });
            proxy.run();
            assert_eq!(num_tracked_objs(&proxy), 0);
        };
        unsafe { col.run_with_gc(body) };
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
            eat_stack_and_exec(10, || {
                for _ in 0..num_wasted {
                    proxy.store(22);
                }
            });
            assert_eq!(num_tracked_objs(&proxy), threshold);
            head = prepend_ll!(); //(&mut proxy, head);
            assert_eq!(num_tracked_objs(&proxy), num_useful + 1);
            assert!(head.next.is_some());
        };
        unsafe { col.run_with_gc(body) };
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
            eat_stack_and_exec(10, || {
                for _ in 0..num_wasted {
                    proxy.store(22);
                }
            });
            assert_eq!(num_tracked_objs(&proxy), threshold);
            proxy.pause();
            prepend_ll!(); //(&mut proxy, head);
            assert_eq!(num_tracked_objs(&proxy), threshold + 1);
        };
        unsafe { col.run_with_gc(body) };
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
            eat_stack_and_exec(10, || {
                for _ in 0..num_wasted {
                    proxy.store(22);
                }
            });
            assert_eq!(num_tracked_objs(&proxy), threshold);
            proxy.pause();
            proxy.resume();
            prepend_ll!(); //(&mut proxy, head);
            assert_eq!(num_tracked_objs(&proxy), num_useful + 1);
        };
        unsafe { col.run_with_gc(body) };
    }

    #[test]
    fn returning_a_value_works() {
        let mut col = Collector::new();
        let val = unsafe { col.run_with_gc(|_proxy| 42) };
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
            eat_stack_and_exec(6, || {
                let ptr = proxy.store(SelfRef {
                    self_ptr: RefCell::new(None),
                });
                *ptr.self_ptr.borrow_mut() = Some(ptr.clone());

                proxy.run();
            });

            proxy.run();
            assert_eq!(num_tracked_objs(&proxy), 0);
        };

        unsafe { col.run_with_gc(body) };
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
            let _root = eat_stack_and_exec(6, || {
                let leaf = proxy.store(List { ptr: None });
                let root = proxy.store(List { ptr: Some(leaf) });
                Box::new(root)
            });

            proxy.run();
            assert_eq!(num_tracked_objs(&proxy), 2);
        };

        unsafe { col.run_with_gc(body) };
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
            compute_cyclic_data(&mut proxy); // FIXME: inline the function

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
    }
}
