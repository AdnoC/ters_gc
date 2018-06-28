use std::collections::HashMap;

enum Never {}

/// Type-erased allocation info
struct AllocInfo {
    ptr: *mut Never,
    rebox: fn(*mut Never),
    marked: bool,
}

impl AllocInfo {
    fn new<T>(value: T) -> AllocInfo {
        AllocInfo {
            ptr: store_single_value(value) as *mut _,
            rebox: get_rebox::<T>(),
            marked: false,
        }
    }
}

impl Drop for AllocInfo {
    fn drop(&mut self) {
        (self.rebox)(self.ptr);
    }
}

struct Allocator {
    items: HashMap<*mut Never, AllocInfo>,
    // frees: Vec<AllocInfo>, // Only accessed in sweep func
    max_ptr: usize,
    min_ptr: usize,
}

impl Allocator {
    fn new() -> Allocator {
        Allocator {
            items: Default::default(),
            max_ptr: 0,
            min_ptr: ::std::usize::MAX,
        }
    }
    fn alloc<T>(&mut self, value: T) -> *mut T {
        use std::cmp::{min, max};
        let info = AllocInfo::new(value);
        self.max_ptr = max(self.max_ptr, info.ptr as usize);
        self.min_ptr = min(self.min_ptr, info.ptr as usize);
        let ptr = info.ptr;
        self.items.insert(ptr, info);
        ptr as *mut _
    }
    fn free<T>(&mut self, ptr: *mut T) {
        self.items.remove(&(ptr as *mut _)); // Will be deallocated by Drop
    }
    fn remove<T>(&mut self, ptr: *mut T) -> T {
        use ::std::mem::forget;
        let item = self.items.remove(&(ptr as *mut _));
        forget(item);
        let boxed = unsafe { Box::from_raw(ptr) };
        *boxed
    }
}

fn store_single_value<T>(value: T) -> *mut T {
    let storage = Box::new(value);
    Box::leak(storage)
}

// fn get_uninitialized_ptr<T>() -> *mut T {
//     use ::std::mem::forget;
//     let mut vec: Vec<T> = Vec::with_capacity(1);
//     let ptr = vec[0..].as_mut_ptr();
//     forget(vec);
//     ptr
// }

fn get_rebox<T>() -> fn(*mut Never) {
    |ptr: *mut Never| unsafe { Box::<T>::from_raw(ptr as *mut _); }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{
        rc::Rc,
        cell::RefCell,
    };

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
        fn incrs(&self, num: usize) -> Vec<CounterIncrementer> {
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
            if self.ran_dtor { return; }
            self.ran_dtor = true;

            let mut counter_ref = self.counter.borrow_mut();
            counter_ref.num_run += 1;

        }
    }

    #[test]
    fn returns_valid_ptrs() {
        let mut alloc = Allocator::new();
        let mut num = alloc.alloc(22);
        unsafe {
            assert_eq!(*num, 22);
            *num = 42;
            assert_eq!(*num, 42);
        }
    }
    #[test]
    fn doesnt_panic_when_freeing() {
        let mut alloc = Allocator::new();
        let num = alloc.alloc(22);
        alloc.free(num);

        let num = alloc.alloc(42);
        let num_val = alloc.remove(num);
        assert_eq!(num_val, 42);
    }
    #[test]
    fn runs_dtor_on_free() {
        let mut alloc = Allocator::new();
        let mut counter = DtorCounter::new();
        let ptr = alloc.alloc(counter.incr());
        alloc.free(ptr);
        assert_eq!(counter.count(), 1);
    }
}
