enum Never {}

/// Type-erased allocation info
struct AllocInfo {
    ptr: *mut Never,
    rebox: fn(*mut Never),
    // `flags` not implemented atm. TODO: Figure out if it is useful
}

impl AllocInfo {
    fn new<T>(value: T) -> AllocInfo {
        AllocInfo {
            ptr: store_single_value(value) as *mut _,
            rebox: get_rebox::<T>(),
        }
    }
}

impl Drop for AllocInfo {
    fn drop(&mut self) {
        (self.rebox)(self.ptr);
    }
}

struct Allocator {
    items: Vec<AllocInfo>,
    frees: Vec<AllocInfo>,
    max_ptr: usize,
    min_ptr: usize,
}

impl Allocator {
    fn new() -> Allocator {
        Allocator {
            items: vec![],
            frees: vec![],
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
        self.items.push(info);
        ptr as *mut _
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
