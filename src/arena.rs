pub struct Arena;
use std::marker::PhantomData;

impl Arena {
    pub fn new() -> Arena {
        unimplemented!()
    }

    // While allocator is active, all pointers to Arena are valid (since the arena
    // can't be moved while there is a reference to it)
    pub fn allocator(&mut self) -> Allocator {
        Allocator {
            arena: self,
        }
    }
}

pub struct Allocator<'arena> {
        arena: &'arena mut Arena,
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
