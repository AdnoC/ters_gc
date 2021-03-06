use ptr::GcBox;
use std::cell::Cell;
use std::collections::HashMap;
use std::ptr::NonNull;
use trace::{Trace, Tracer};
use UntypedGcBox;
use {AsTyped, AsUntyped};

/// Type-erased allocation info
#[derive(Debug, PartialEq)]
pub(crate) struct AllocInfo {
    pub ptr: NonNull<UntypedGcBox>,
    // unsafe is because it must be called with accompanying pointer
    free: unsafe fn(NonNull<UntypedGcBox>), // Frees allocation and calls destructor
    reachable: Cell<bool>,                  // Whether this has been found to be reachable
    inter_marks: Cell<usize>, // # of marks from objects for which is_marked_reachable == false
    // unsafe is because it must be called with accompanying pointer
    refs: unsafe fn(NonNull<UntypedGcBox>) -> usize,
    // unsafe is because it must be called with accompanying pointer
    trace: unsafe fn(NonNull<UntypedGcBox>) -> Tracer,
}

impl AllocInfo {
    fn new<T: Trace>(value: T) -> AllocInfo {
        AllocInfo {
            ptr: store_single_value(value).as_untyped(),
            free: get_free::<T>(),
            reachable: Cell::new(false),
            inter_marks: Cell::new(0),
            refs: get_refs_accessor::<T>(),
            trace: get_tracer::<T>(),
        }
    }

    pub fn mark_reachable(&self) {
        self.reachable.set(true);
    }
    pub fn mark_inter_ref(&self) {
        self.inter_marks.set(self.inter_marks.get() + 1);
    }

    pub fn unmark(&self) {
        self.reachable.set(false);
        self.inter_marks.set(0);
    }

    pub fn is_marked_reachable(&self) -> bool {
        self.reachable.get()
    }

    pub fn inter_marks(&self) -> usize {
        self.inter_marks.get()
    }

    pub fn ref_count(&self) -> usize {
        // Unsafe is fine since this is only called with the accompanying
        // valid pointer.
        unsafe { (self.refs)(self.ptr) }
    }

    pub(crate) fn children(&self) -> impl Iterator<Item = NonNull<UntypedGcBox>> {
        // Unsafe is fine since this is only called with the accompanying
        // valid pointer.
        let tracer = unsafe { (self.trace)(self.ptr) };
        tracer.results().map(|dest| dest.0)
    }
}

impl Drop for AllocInfo {
    fn drop(&mut self) {
        // This is used as the destructor for the pointer, so it should the only
        // reference to the object.
        unsafe { (self.free)(self.ptr) };
    }
}

/// Handles allocation and freeing of objects.
#[derive(Default, Debug, PartialEq)]
pub(crate) struct Allocator {
    pub items: HashMap<*mut UntypedGcBox, AllocInfo>,
    // frees: Vec<AllocInfo>, // Only accessed in sweep func
}

impl Allocator {
    pub fn new() -> Allocator {
        Allocator {
            items: Default::default(),
        }
    }
    pub fn alloc<T: Trace>(&mut self, value: T) -> NonNull<GcBox<T>> {
        let info = AllocInfo::new(value);
        let ptr = info.ptr;
        self.items.insert(ptr.as_ptr(), info);
        ptr.as_typed()
    }
    /// Just remove an object
    pub fn free(&mut self, ptr: NonNull<UntypedGcBox>) {
        self.items.remove(&ptr.as_ptr()); // Will be deallocated by Drop
    }
    /// Remove an object and return it's value
    ///
    /// Unsafe because `T` must be the type that was originally stored
    pub unsafe fn remove<T>(&mut self, ptr: NonNull<UntypedGcBox>) -> T {
        use std::mem::forget;
        let item = self.items.remove(&ptr.as_ptr());
        forget(item);
        // The unsafe part
        let boxed: Box<GcBox<T>> = Box::from_raw(ptr.as_typed().as_ptr());
        boxed.reclaim_value()
    }

    // pub fn is_ptr_tracked<T>(&self, ptr: *const T) -> bool {
    //     let ptr: *const UntypedGcBox = ptr as *const _;
    //     self.items.contains_key(&ptr)
    // }

    pub(crate) fn info_for_ptr(&self, ptr: *const UntypedGcBox) -> Option<&AllocInfo> {
        self.items.get(&(ptr as *mut _))
    }

    // Stub
    pub fn should_shrink_items(&self) -> bool {
        false
    }

    // Not a great way to shrink with HashMap
    pub fn shrink_items(&mut self) {}
}

fn store_single_value<T>(value: T) -> NonNull<GcBox<T>> {
    let storage = Box::new(GcBox::new(value));
    // Unsafe is for the call to `NonNull::new_unchecked`.
    // The call can't fail since `Box::leak` returns a reference, which must
    // be a valid, nonnull pointer.
    unsafe { NonNull::new_unchecked(Box::leak(storage)) }
}

fn get_free<T>() -> unsafe fn(NonNull<UntypedGcBox>) {
    /// Must be called with accompanying pointer
    unsafe fn free<T>(ptr: NonNull<UntypedGcBox>) {
        Box::<GcBox<T>>::from_raw(ptr.as_typed().as_ptr());
    }
    free::<T>
}

fn get_refs_accessor<T>() -> unsafe fn(NonNull<UntypedGcBox>) -> usize {
    /// Must be called with accompanying pointer
    unsafe fn refs<T>(ptr: NonNull<UntypedGcBox>) -> usize {
        let ptr = ptr.as_typed();
        let gc_box: &GcBox<T> = ptr.as_ref();
        gc_box.strong_count()
    }
    refs::<T>
}

fn get_tracer<T: Trace>() -> unsafe fn(NonNull<UntypedGcBox>) -> Tracer {
    /// Must be called with accompanying pointer
    unsafe fn tracer<T: Trace>(ptr: NonNull<UntypedGcBox>) -> Tracer {
        let mut tracer = Tracer::new();
        let ptr = ptr.as_typed();
        let gc_box: &GcBox<T> = ptr.as_ref();
        tracer.add_target(gc_box.borrow());
        tracer
    }
    tracer::<T>
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
    impl Trace for CounterIncrementer {
        fn trace(&self, _: &mut ::trace::Tracer) {
            // noop
        }
    }

    #[test]
    fn runs_dtor_on_free() {
        let mut alloc = Allocator::new();
        let counter = DtorCounter::new();
        let ptr = alloc.alloc(counter.incr());
        alloc.free(ptr.as_untyped());
        assert_eq!(counter.count(), 1);
    }
}
