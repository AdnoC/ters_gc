use ptr::GcBox;
use std::ptr::NonNull;
use std::cell::Cell;
use std::collections::HashMap;
use traceable::{TraceTo, Tracer};
use UntypedGcBox;
use {AsTyped, AsUntyped};

trait AsConstPtr {
    type Target;
    fn as_const_ptr(&self) -> *const Self::Target;
}
impl<T> AsConstPtr for NonNull<T> {
    type Target = T;
    fn as_const_ptr(&self) -> *const T {
        self.as_ptr() as *const _
    }
}

// TODO: Make mark stats into Cell so that the marking functs can take &self
/// Type-erased allocation info
pub(crate) struct AllocInfo {
    pub ptr: NonNull<UntypedGcBox>,
    rebox: fn(NonNull<UntypedGcBox>),
    reachable: Cell<bool>,    // # of marks from ptrs stored in stack (since we can't traverse heap)
    isolated: Cell<usize>, // # of marks from objects for which is_marked_reachable == false
    refs: fn(NonNull<UntypedGcBox>) -> usize,
    trace: fn(NonNull<UntypedGcBox>) -> Tracer,
}

impl AllocInfo {
    fn new<T: TraceTo>(value: T) -> AllocInfo {
        AllocInfo {
            ptr: store_single_value(value).as_untyped(),
            rebox: get_rebox::<T>(),
            reachable: Cell::new(false),
            isolated: Cell::new(0),
            refs: get_refs_accessor::<T>(),
            trace: get_tracer::<T>(),
        }
    }

    pub fn mark_reachable(&self) {
        self.reachable.set(true);
    }
    pub fn mark_isolated(&self) {
        self.isolated.set(self.isolated.get() + 1);
    }
    pub fn unmark_isolated(&self) {
        self.isolated.set(self.isolated.get() - 1);
    }

    pub fn unmark(&self) {
        self.reachable.set(false);
        self.isolated.set(0);
    }

    pub fn is_marked_reachable(&self) -> bool {
        self.reachable.get()
    }

    pub fn isolated_marks(&self) -> usize {
        self.isolated.get()
    }

    pub fn ref_count(&self) -> usize {
        (self.refs)(self.ptr)
    }

    pub(crate) fn children(&self) -> impl Iterator<Item = NonNull<UntypedGcBox>> {
        let tracer = (self.trace)(self.ptr);
        tracer.results().map(|dest| dest.0)
    }
}

impl Drop for AllocInfo {
    fn drop(&mut self) {
        (self.rebox)(self.ptr);
    }
}

pub(crate) struct Allocator {
    pub items: HashMap<*mut UntypedGcBox, AllocInfo>,
    // frees: Vec<AllocInfo>, // Only accessed in sweep func
    // max_ptr: usize,
    // min_ptr: usize,
}

impl Allocator {
    pub fn new() -> Allocator {
        Allocator {
            items: Default::default(),
            // max_ptr: 0,
            // min_ptr: ::std::usize::MAX,
        }
    }
    pub fn alloc<T: TraceTo>(&mut self, value: T) -> NonNull<GcBox<T>> {
        // use std::cmp::{min, max};
        let info = AllocInfo::new(value);
        // self.max_ptr = max(self.max_ptr, info.ptr as usize);
        // self.min_ptr = min(self.min_ptr, info.ptr as usize);
        let ptr = info.ptr;
        self.items.insert(ptr.as_ptr(), info);
        ptr.as_typed()
    }
    pub fn free(&mut self, ptr: NonNull<UntypedGcBox>) {
        self.items.remove(&ptr.as_ptr()); // Will be deallocated by Drop
    }
    pub fn _remove<T>(&mut self, ptr: NonNull<UntypedGcBox>) -> T {
        use std::mem::forget;
        let item = self.items.remove(&ptr.as_ptr());
        forget(item);
        let boxed: Box<GcBox<T>> = unsafe { Box::from_raw(ptr.as_typed().as_ptr()) };
        boxed.reclaim_value()
    }

    // pub fn is_ptr_in_range(&self, _ptr: *const UntypedGcBox) -> bool {
    //     true
    //     // let ptr_val = ptr as usize;
    //     // self.min_ptr >= ptr_val && self.max_ptr <= ptr_val
    // }

    // pub fn is_ptr_tracked<T>(&self, ptr: *const T) -> bool {
    //     let ptr: *const UntypedGcBox = ptr as *const _;
    //     self.items.contains_key(&ptr)
    // }

    pub(crate) fn info_for_ptr(&self, ptr: *const UntypedGcBox) -> Option<&AllocInfo> {
        self.items.get(&(ptr as *mut _))
    }

    pub fn should_shrink_items(&self) -> bool {
        false
    }

    pub fn shrink_items(&mut self) {}
}

fn store_single_value<T>(value: T) -> NonNull<GcBox<T>> {
    let storage = Box::new(GcBox::new(value));
    unsafe { NonNull::new_unchecked(Box::leak(storage)) }
}

fn get_rebox<T>() -> fn(NonNull<UntypedGcBox>) {
    |ptr: NonNull<UntypedGcBox>| unsafe {
        // Should be safe to cast to mut, as this is only used for destruction.
        // There shouldn't be any other active pointers to the object.
        Box::<GcBox<T>>::from_raw(ptr.cast::<GcBox<T>>().as_ptr());
    }
}

fn get_refs_accessor<T>() -> fn(NonNull<UntypedGcBox>) -> usize {
    |ptr: NonNull<UntypedGcBox>| unsafe {
        let ptr = ptr.as_typed();
        let gc_box: &GcBox<T> = ptr.as_ref();
        gc_box.ref_count()
    }
}

fn get_tracer<T: TraceTo>() -> fn(NonNull<UntypedGcBox>) -> Tracer {
    |ptr: NonNull<UntypedGcBox>| unsafe {
        let mut tracer = Tracer::new();
        let ptr = ptr.as_typed();
        let gc_box: &GcBox<T> = ptr.as_ref();
        gc_box.borrow().trace_to(&mut tracer);
        tracer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct DtorCounter {
        inner: Rc<RefCell<DtorCounterInner>>,
    }

    impl DtorCounter {
        fn new() -> DtorCounter {
            DtorCounter {
                inner: Default::default(),
            }
        }
        fn count(&self) -> usize {
            self.inner.borrow().num_run
        }

        fn incr(&self) -> CounterIncrementer {
            CounterIncrementer {
                counter: self.inner.clone(),
                ran_dtor: false,
            }
        }
        fn _incrs(&self, num: usize) -> Vec<CounterIncrementer> {
            let mut incrs = Vec::with_capacity(num);
            for _ in 0..num {
                incrs.push(CounterIncrementer {
                    counter: self.inner.clone(),
                    ran_dtor: false,
                })
            }
            incrs
        }
    }
    #[derive(Default, Clone)]
    struct DtorCounterInner {
        num_run: usize,
    }
    #[derive(Clone)]
    struct CounterIncrementer {
        counter: Rc<RefCell<DtorCounterInner>>,
        ran_dtor: bool,
    }
    impl Drop for CounterIncrementer {
        fn drop(&mut self) {
            if self.ran_dtor {
                return;
            }
            self.ran_dtor = true;

            let mut counter_ref = self.counter.borrow_mut();
            counter_ref.num_run += 1;
        }
    }
    impl TraceTo for CounterIncrementer {
        fn trace_to(&self, _: &mut ::traceable::Tracer) {
            // noop
        }
    }

    // #[test]
    // fn returns_valid_ptrs() {
    //     let mut alloc = Allocator::new();
    //     let num = alloc.alloc(22);
    //     unsafe {
    //         let num = &mut (*num).val;
    //         assert_eq!(*num, 22);
    //         *num = 42;
    //         assert_eq!(*num, 42);
    //     }
    // }
    // #[test]
    // fn doesnt_panic_when_freeing() {
    //     let mut alloc = Allocator::new();
    //     let num = alloc.alloc(22);
    //     alloc.free(num);
    //
    //     let num = alloc.alloc(42);
    //     let num_val = alloc._remove(num as *const _);
    //     assert_eq!(num_val, 42);
    // }
    #[test]
    fn runs_dtor_on_free() {
        let mut alloc = Allocator::new();
        let counter = DtorCounter::new();
        let ptr = alloc.alloc(counter.incr());
        alloc.free(ptr.as_untyped());
        assert_eq!(counter.count(), 1);
    }
}
