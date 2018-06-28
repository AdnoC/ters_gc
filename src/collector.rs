use std::marker::PhantomData;

pub struct Collector {

}

impl Collector {
    pub fn new() -> Collector {
        unimplemented!()
    }

    // While allocator is active, all pointers to Collector are valid (since the arena
    // can't be moved while there is a reference to it)
    pub fn allocator(&mut self) -> Allocator {
        Allocator {
            arena: self,
        }
    }
}

pub struct Allocator<'arena> {
        arena: &'arena mut Collector,
}

impl<'a> Allocator<'a> {
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
