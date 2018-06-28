// https://users.rust-lang.org/t/using-setjmp-longjmp-intrinsics-or-is-there-another-way/9350/11
use std::marker::PhantomData;
use std::ops::Deref;
use ::Never;
use ::allocator::Allocator;

macro_rules! stack_ptr {
    () => {
        {
            let a = ();
            (&a) as *const ()
        }
    }
}

pub struct Collector {
    allocator: Allocator,
    collection_threshold: usize,
    load_factor: f64,
    sweep_factor: f64,
    paused: bool,
    stack_bottom: *const (),
}

impl Collector {
    pub fn new() -> Collector {
        Collector {
            allocator: Allocator::new(),
            collection_threshold: 25,
            load_factor: 0.9,
            sweep_factor: 0.5,
            paused: false,
            stack_bottom: 0 as *const (),
        }
    }

    pub unsafe fn run_with_gc<T: FnOnce(Proxy)>(&mut self, func: T) {
        self.stack_bottom = stack_ptr!();
        self.inner_run_with_gc(func);
    }

    #[inline(never)]
    fn inner_run_with_gc<T: FnOnce(Proxy)>(&mut self, func: T) {
        let proxy = self.proxy();
        func(proxy);
    }

    pub fn alloc<T>(&mut self, val: T) -> *mut T {
        self.allocator.alloc(val)
    }

    pub fn run(&mut self) {
        self.mark();
        self.sweep();
    }

    pub fn mark(&mut self) {
        unsafe { ::reg_flush::flush_registers_and_call(Collector::mark_landingpad, self as *mut Collector as *mut _) };
    }

    extern fn mark_landingpad(data: *mut Never) {
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
        use ::std::mem::{ size_of, align_of };

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

        let bottom = ::round_up(bottom, align_of::<usize>());

        for addr in (bottom..top).step_by(size_of::<usize>()) {
            let stack_ptr = addr as *const *mut Never;
            let stack_value = unsafe { *stack_ptr };
            self.mark_ptr(stack_value);
        }
    }

    fn mark_ptr(&mut self, ptr: *mut Never) {
        if !self.allocator.is_ptr_in_range(ptr) {
            return;
        }

        let mut children = None;
        if let Some(info) = self.allocator.info_for_ptr_mut(ptr) {
            if !info.is_marked_reachable() {
                info.mark();
                assert!(info.is_marked_reachable());
                children = Some(info.inner_ptrs());
            }
        }

        if let Some(children) = children {
            for val in children {
                let val = unsafe { *val };
                self.mark_ptr(val as *mut Never);
            }
        }
    }

    pub fn sweep(&mut self) {
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

        if self.allocator.should_shrink_items() {
            self.allocator.shrink_items();
        }
    }

    // While allocator is active, all pointers to Collector are valid (since the arena
    // can't be moved while there is a reference to it)
    // TODO: Make private
    pub fn proxy(&mut self) -> Proxy {
        Proxy {
            collector: self,
        }
    }
}

pub struct Proxy<'arena> {
    collector: &'arena mut Collector,
}

impl<'a> Proxy<'a> {
    // fn alloc<T>(&mut self) -> *mut T {
    //     unimplemented!()
    // }
    pub fn store<T>(&mut self, payload: T) -> Gc<'a, T> {
        let ptr = self.collector.allocator.alloc(payload);
        Gc {
            _marker: PhantomData,
            ptr,
        }
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
}

#[derive(Clone)]
pub struct Gc<'arena, T> {
    _marker: PhantomData<*mut &'arena ()>,
    ptr: *const T,
}

impl<'a, T> Deref for Gc<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num_tracked_objs(proxy: &Proxy) -> usize {
        proxy.collector.allocator.items.len()
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
                let num1 = proxy.store(42);
                assert_eq!(num_tracked_objs(&proxy), 1);
                proxy.run();
                assert_eq!(num_tracked_objs(&proxy), 1);
            });
            proxy.run();
            assert_eq!(num_tracked_objs(&proxy), 0);
        };
        unsafe { col.run_with_gc(body) };
    }
}
