use ptr::GcBox;
use std::ptr::NonNull;
use std::cell::Cell;
use std::collections::HashMap;
use traceable::{TraceTo, Tracer};
use UntypedGcBox;
use ptr_convs::*;

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
    rebox: fn(*const UntypedGcBox),
    branches: Cell<usize>, // # of marks from ptrs stored in tracked objects
    roots: Cell<usize>,    // # of marks from ptrs stored in stack (since we can't traverse heap)
    isolated: Cell<usize>, // # of marks from objects for which is_marked_reachable == false
    refs: fn(*const UntypedGcBox) -> usize,
    trace: fn(*const UntypedGcBox) -> Tracer,
}

impl AllocInfo {
    fn new<T: TraceTo>(value: T) -> AllocInfo {
        AllocInfo {
            ptr: store_single_value(value).cast::<UntypedGcBox>(), // FIXME as_untyped
            rebox: get_rebox::<T>(),
            branches: Cell::new(0),
            roots: Cell::new(0),
            isolated: Cell::new(0),
            refs: get_refs_accessor::<T>(),
            trace: get_tracer::<T>(),
        }
    }
    pub fn const_ptr(&self) -> *const UntypedGcBox{
        self.ptr.as_const_ptr()
    }

    pub fn mark_branch(&self) {
        self.branches.set(self.branches.get() + 1);
    }
    pub fn mark_root(&self) {
        self.roots.set(self.roots.get() + 1);
    }
    pub fn mark_isolated(&self) {
        self.isolated.set(self.isolated.get() + 1);
    }
    pub fn unmark_isolated(&self) {
        self.isolated.set(self.isolated.get() - 1);
    }

    pub fn unmark(&self) {
        self.branches.set(0);
        self.roots.set(0);
        self.isolated.set(0);
    }

    pub fn is_marked_reachable(&self) -> bool {
        self.branches.get() > 0 || self.roots.get() > 0
    }

    pub fn root_marks(&self) -> usize {
        self.roots.get()
    }

    pub fn branch_marks(&self) -> usize {
        self.branches.get()
    }

    pub fn isolated_marks(&self) -> usize {
        self.isolated.get()
    }

    pub fn ref_count(&self) -> usize {
        (self.refs)(self.ptr.as_const_ptr())
    }

    pub(crate) fn children(&self) -> impl Iterator<Item = *const UntypedGcBox> {
        let tracer = (self.trace)(self.ptr.as_const_ptr());
        tracer.results().map(|dest| dest.0)
    }
}

impl Drop for AllocInfo {
    fn drop(&mut self) {
        (self.rebox)(self.ptr.as_const_ptr());
    }
}

pub(crate) struct Allocator {
    pub items: HashMap<*const UntypedGcBox, AllocInfo>,
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
    pub fn alloc<T: TraceTo>(&mut self, value: T) -> *const GcBox<T> {
        // use std::cmp::{min, max};
        let info = AllocInfo::new(value);
        // self.max_ptr = max(self.max_ptr, info.ptr as usize);
        // self.min_ptr = min(self.min_ptr, info.ptr as usize);
        let ptr = info.ptr.as_const_ptr();
        self.items.insert(ptr, info);
        ptr as *const _ // FIXME as_typed
    }
    pub fn free(&mut self, ptr: *const UntypedGcBox) {
        self.items.remove(&(ptr)); // Will be deallocated by Drop
    }
    pub fn _remove<T>(&mut self, ptr: *const UntypedGcBox) -> T {
        use std::mem::forget;
        let item = self.items.remove(&ptr);
        forget(item);
        let boxed: Box<GcBox<T>> = unsafe { Box::from_raw(ptr as *mut _) };
        boxed.reclaim_value()
    }

    pub fn is_ptr_in_range(&self, _ptr: *const UntypedGcBox) -> bool {
        true
        // let ptr_val = ptr as usize;
        // self.min_ptr >= ptr_val && self.max_ptr <= ptr_val
    }

    // pub fn is_ptr_tracked<T>(&self, ptr: *const T) -> bool {
    //     let ptr: *const UntypedGcBox = ptr as *const _;
    //     self.items.contains_key(&ptr)
    // }

    pub(crate) fn info_for_ptr(&self, ptr: *const UntypedGcBox) -> Option<&AllocInfo> {
        self.items.get(&ptr)
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

fn get_rebox<T>() -> fn(*const UntypedGcBox) {
    |ptr: *const UntypedGcBox| unsafe {
        // Should be safe to cast to mut, as this is only used for destruction.
        // There shouldn't be any other active pointers to the object.
        Box::<GcBox<T>>::from_raw(ptr as *const _ as *mut _);
    }
}

fn get_refs_accessor<T>() -> fn(*const UntypedGcBox) -> usize {
    |ptr: *const UntypedGcBox| unsafe {
        let gc_box: &GcBox<T> = &*(ptr as *const _); // FIXME as_typed
        gc_box.ref_count()
    }
}

fn get_tracer<T: TraceTo>() -> fn(*const UntypedGcBox) -> Tracer {
    |ptr: *const UntypedGcBox| unsafe {
        let mut tracer = Tracer::new();
        let gc_box: &GcBox<T> = &*(ptr as *const _); // FIXME as_typed
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
