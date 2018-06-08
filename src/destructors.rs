struct Destructor {
    ptr: *const i8,
    drop_glue: fn(*const i8),
}

fn get_drop_glue<T>() -> fn(*const i8) {
    use std::intrinsics::drop_in_place;
    |ptr: *const i8| unsafe { drop_in_place(ptr as *mut T) }
}
struct Destructors {
    dtors: Vec<Destructor>,
}

// intrinsics::needs_drop::<T>()
impl Destructors {
    fn store<T>(&mut self, ptrs: &[T]) {
        use std::intrinsics::needs_drop;
        if unsafe { needs_drop::<T>() } {
            let drop_glue = get_drop_glue::<T>();
            for ptr in ptrs {
                let ptr = ptr as *const _ as *const i8;
                let dtor = Destructor {
                    ptr,
                    drop_glue,
                };
                self.dtors.push(dtor);
            }
        }
    }

    fn is_stored<T>(&self, ptr: &T) -> bool{
        use std::intrinsics::needs_drop;
        if ! unsafe { needs_drop::<T>() } {
            return false;
        }

        let ptr = ptr as *const _ as *const i8;
        self.dtors.iter().any(|dtor| dtor.ptr == ptr)
    }

    fn run_all(&mut self) {
        for dtor in &self.dtors {
            (dtor.drop_glue)(dtor.ptr);
        }
        self.dtors.clear();
    }

    fn run(range: &[i8]) { 
        // TODO: Deferred heap has a note about reentrancy safety. Do we need
        // to handle this? Currently are not.

        if range.len() == 0 {
            return;
        }

        let to_destroy = vec![];

        for 
    }
}
