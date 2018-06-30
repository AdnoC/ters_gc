enum Never {} // TODO Make NonNull<GcBox<T>>

mod allocator;
mod ptr;
mod reg_flush {
    use Never;
    extern "C" {
        pub(crate) fn flush_registers_and_call(
            callback: extern "C" fn(*mut Never),
            data: *mut Never,
        );
    }
}

pub use ptr::Gc;
use ptr::GcBox;
use allocator::Allocator;
use std::marker::PhantomData;

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

    fn alloc<T>(&mut self, val: T) -> *const GcBox<T> {
        let ptr = self.allocator.alloc(val);
        if self.should_collect() {
            self.run();
        }
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

    extern "C" fn mark_landingpad(data: *mut Never) {
        let data = data as *mut Collector;
        let collector: &mut Collector = unsafe { &mut *data };
        let stack_top = stack_ptr!();
        collector.mark_impl(stack_top);
    }

    fn mark_impl(&mut self, stack_top: *const ()) {
        self.mark_stack(stack_top);
    }

    #[inline(never)]
    fn mark_stack(&mut self, stack_top: *const ()) {
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
            let stack_ptr = addr as *const *const Never;
            let stack_value = unsafe { *stack_ptr };
            self.mark_ptr(stack_value);
        }
    }

    fn mark_ptr(&mut self, ptr: *const Never) {
        if !self.allocator.is_ptr_in_range(ptr) {
            return;
        }

        let mut children = None;
        if let Some(info) = self.allocator.info_for_ptr_mut(ptr) {
            if !info.is_marked_reachable() {
                info.mark();
                children = Some(info.inner_ptrs());
            }
        }

        if let Some(children) = children {
            for val in children {
                let val = unsafe { *val };
                self.mark_ptr(val as *const Never);
            }
        }
    }

    fn sweep(&mut self) {
        let mut unreachable_objects = vec![];
        for info in self.allocator.items.values_mut() {
            if !info.is_marked_reachable() {
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
    // FIXME: Make private
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
        !self.paused && num_tracked > self.collection_threshold
    }
}

pub struct Proxy<'arena> {
    collector: &'arena mut Collector,
}

impl<'a> Proxy<'a> {
    pub fn store<T>(&mut self, payload: T) -> Gc<'a, T> {
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

    fn num_tracked_objs(proxy: &Proxy) -> usize {
        proxy.num_tracked()
    }
    #[inline(never)]
    fn eat_stack_and_exec<F: FnOnce()>(recurs: usize, callback: F) {
        let _nom = [22; 25];
        if recurs > 0 {
            eat_stack_and_exec(recurs - 1, callback);
            return;
        }

        callback();
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
}
