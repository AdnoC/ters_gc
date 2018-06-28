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

// #[cfg(test)]
// mod test {
//     use super::*;
//     #[test]
//     fn lifetimes_are_properly_constrained() {
//         use ::std::mem::drop;
//         let mut arena = Arena::new();
//         let mut alloc = arena.allocator();
//
//         let gc_ptr = alloc.store(42);
//
//         drop(alloc);
//
//         // Should borrowck error. Can't move arena since it is borrowed by gc_ptr
//         let arena2 = arena; //~ ERROR error[E0505]: cannot move out of `arena` because it is borrowed
//
//         //```compile_fail
//         //let arena2 = arena;
//         //```
//
//         //```compile_fail
//         //let arena2 = //arena;
//         //```
//     }
// }
