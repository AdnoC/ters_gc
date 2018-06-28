// https://users.rust-lang.org/t/using-setjmp-longjmp-intrinsics-or-is-there-another-way/9350/11
use std::marker::PhantomData;
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
        unimplemented!()
    }

    pub fn run_with_gc<T: FnOnce(Proxy)>(&mut self, func: T) {
        self.stack_bottom = stack_ptr!();
        self.inner_run_with_gc(func);
    }

    #[inline(never)]
    fn inner_run_with_gc<T: FnOnce(Proxy)>(&mut self, func: T) {
        let proxy = self.proxy();
        func(proxy);
    }

    pub fn mark(&mut self) {
        unsafe { ::reg_flush::flush_registers_and_call(Collector::mark_landingpad, self as *mut Collector as *mut _) };
    }

    extern fn mark_landingpad(data: *mut Never) {
        let data = data as *mut Collector;
        let collector: &mut Collector = unsafe { &mut *data };
        collector.mark_impl();
    }

    #[inline(never)] // Is this even needed?
    fn mark_impl(&mut self) {
        println!("HELLOW WORLD");
    }

    fn mark_ptr(&mut self, ptr: *mut Never) {
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
                self.mark_ptr(val as *mut Never);
            }
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
    fn alloc<T>(&mut self) -> *mut T {
        unimplemented!()
    }
    pub fn store<T>(&mut self, payload: T) -> Gc<'a, T> {
        let ptr = self.alloc::<T>();
        unsafe { *ptr = payload };
        Gc {
            _marker: PhantomData,
            ptr,
        }
    }
}

pub struct Gc<'arena, T> {
    _marker: PhantomData<*mut &'arena ()>,
    ptr: *mut T,
}
