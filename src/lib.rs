enum BoxedCollector {} // TODO Make NonNull<GcBox<T>>
pub(crate) enum UntypedGcBox {} // TODO Make NonNull<GcBox<T>>

mod allocator;
pub mod ptr;
pub mod traceable;
mod reg_flush {
    use BoxedCollector;
    extern "C" {
        pub(crate) fn flush_registers_and_call(
            callback: extern "C" fn(*mut BoxedCollector),
            data: *mut BoxedCollector,
        );
    }
}

pub use ptr::Gc;
use ptr::GcBox;
use allocator::Allocator;
use allocator::AllocInfo;
use traceable::TraceTo;
use std::marker::PhantomData;

trait AsUntyped {
    fn as_untyped(&self) -> *const UntypedGcBox;
}
impl<T> AsUntyped for *const GcBox<T> {
    fn as_untyped(&self) -> *const UntypedGcBox {
        (*self) as _
    }
}
trait AsTyped {
    fn as_typed<T>(&self) -> *const GcBox<T>;
}
impl AsTyped for *const UntypedGcBox {
    fn as_typed<T>(&self) -> *const GcBox<T> {
        (*self) as _
    }
}


macro_rules! stack_ptr {
    () => {{
        let a = 0usize; // usize so that it is aligned
        (&a) as *const _ as *const ()
    }};
}

const MAGIC: usize = 0x3d4a825;

pub struct Collector {
    allocator: Allocator,
    collection_threshold: usize,
    // load_factor: f64,
    sweep_factor: f64,
    paused: bool,
    stack_bottom: *const (),
    magic: usize,
}

impl Collector {
    pub fn new() -> Collector {
        Collector {
            allocator: Allocator::new(),
            collection_threshold: 25,
            // load_factor: 0.9,
            sweep_factor: 0.5,
            paused: false,
            stack_bottom: 0 as *const (),
            magic: MAGIC, // TODO: Make random
        }
    }

    /// Unsafe because there is an unsafe hole in garbage collection that cannot
    /// be fixed. Namely, you cannot store pointers to tracked objects on the heap.
    pub unsafe fn run_with_gc<R, T: FnOnce(Proxy) -> R>(&mut self, func: T) -> R {
        self.stack_bottom = stack_ptr!();
        self.inner_run_with_gc(func)
    }

    #[inline(never)]
    fn inner_run_with_gc<R, T: FnOnce(Proxy) -> R>(&mut self, func: T) -> R {
        let proxy = self.proxy();
        func(proxy)
    }

    fn alloc<T: TraceTo>(&mut self, val: T) -> *const GcBox<T> {
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

    fn mark(&mut self) {
        unsafe {
            ::reg_flush::flush_registers_and_call(
                Collector::mark_landingpad,
                self as *mut Collector as *mut _,
            )
        };
    }

    extern "C" fn mark_landingpad(data: *mut BoxedCollector) {
        let data = data as *mut Collector;
        let collector: &mut Collector = unsafe { &mut *data };
        let stack_top = stack_ptr!();
        collector.mark_impl(stack_top);
    }

    fn mark_impl(&self, stack_top: *const ()) {
        self.mark_stack(stack_top);
        self.mark_in_gc();
    }

    fn mark_in_gc(&self) {
        let unreachable_objects = self.allocator.items.values()
            .filter(|info| !info.is_marked_reachable());
        for info in unreachable_objects.clone() {
            self.mark_island_ptr(info.ptr);
        }

        let heap_objects = unreachable_objects
            .filter(|info| Self::is_object_reachable(info));
        for info in heap_objects {
            self.mark_newly_found_ptr(info.ptr);
        }
    }

    #[inline(never)]
    fn mark_stack(&self, stack_top: *const ()) {
        use std::mem::size_of;

        let top = stack_top as usize;
        let bottom = self.stack_bottom as usize;
        let (top, bottom) = if top < bottom {
            (bottom, top)
        } else {
            (top, bottom)
        };

        if top == bottom {
            return;
        }

        for addr in (bottom..top).step_by(size_of::<usize>()) {
            let stack_ptr = addr as *const *const UntypedGcBox;
            let stack_value = unsafe { *stack_ptr };
            self.mark_ptr(stack_value, true);
        }
    }

    fn mark_ptr(&self, ptr: *const UntypedGcBox, root: bool) {
        println!("Marking ptr {}", ptr as usize);
        if !self.allocator.is_ptr_in_range(ptr) {
            return;
        }

        let mut children = None;
        if let Some(info) = self.allocator.info_for_ptr(ptr) {
            if !info.is_marked_reachable() {
                children = Some(info.children());
            }
            if root {
                info.mark_root();
            } else {
                info.mark_branch();
            }
        }

        if let Some(children) = children {
            for val in children {
                self.mark_ptr(val, false);
            }
        }
    }

    // ptr MUST be a valid tracked object
    fn mark_island_ptr(&self, ptr: *const UntypedGcBox) {
        assert!(self.allocator.is_ptr_in_range(ptr));

        let mut children = None;
        if let Some(info) = self.allocator.info_for_ptr(ptr) {
            children = Some(info.children());
        }

        if let Some(children) = children {
            for val in children {
                if let Some(child) = self.allocator.info_for_ptr(val) {
                    child.mark_isolated();
                }
            }
        }
    }

    fn mark_newly_found_ptr(&self, ptr: *const UntypedGcBox) {
        assert!(self.allocator.is_ptr_in_range(ptr));

        let mut children = None;
        if let Some(info) = self.allocator.info_for_ptr(ptr) {
            children = Some(info.children());
        }

        if let Some(children) = children {
            for val in children {
                let mut is_valid = false;
                if let Some(child) = self.allocator.info_for_ptr(val) {
                    child.unmark_isolated();
                    child.mark_branch();
                    is_valid = true;
                }
                if is_valid {
                    self.mark_newly_found_ptr(val);
                }
            }
        }
    }

    fn is_object_reachable(info: &AllocInfo) -> bool {
        let stack_refs = info.root_marks();
        let refs_in_gc = info.branch_marks();
        let isolated_refs = info.isolated_marks();
        let known_refs = stack_refs + refs_in_gc + isolated_refs;
        let total_refs = info.ref_count();
        // assert!(stack_refs + refs_in_gc <= total_refs,
        //         "Found more references to object than were made.
        //          total: {}, stack: {}, in_gc_heap: {}, ptr: {}", total_refs, stack_refs, refs_in_gc, info.ptr as usize);

        // Don't actually do the subtraction, since it will underflow if
        // zombie values of an address are found on the stack.
        // let heap_refs = total_refs - stack_refs - refs_in_gc;
        // If we know it is reachable or the only refs are hidden in the heap
        if total_refs == isolated_refs {
            false
        } else {
            info.is_marked_reachable() || total_refs > known_refs
        }
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
        Gc::from_raw(ptr, self.collector.magic, PhantomData)
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
             eat_stack_and_exec(10, ||  {
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
            self_ptr: RefCell<Option<Gc<'a, SelfRef<'a>>>>
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
            let root = eat_stack_and_exec(6, || {
                let leaf = proxy.store(List {
                    ptr: None,
                });
                let root = proxy.store(List {
                    ptr: Some(leaf),
                });
                Box::new(root)
            });

            proxy.run();
            assert_eq!(num_tracked_objs(&proxy), 2);

        };

        unsafe { col.run_with_gc(body) };
    }
}
