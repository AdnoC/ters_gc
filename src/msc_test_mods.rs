mod tracking_root_status {
    use std::marker::PhantomData;

    trait Traceable {
        fn set_root_to(&mut self, b: bool);
        fn is_root(&self) -> bool;
    }

    impl Traceable for () {
        fn set_root_to(&mut self, _b: bool) {}
        fn is_root(&self) -> bool { false }
    }
    #[derive(Default)]
    struct DeferredHeap<T: Traceable> {
        data: Vec<T>,
        roots: Vec<DpVoid>,
        nonroots: Vec<DpVoid>,
    }

    impl< T: Traceable> DeferredHeap<T> {
        fn new() -> DeferredHeap<T> {
                DeferredHeap {
                data: Vec::with_capacity(32),
                roots: vec![],
                nonroots: vec![],
            }
        }

        fn allocator(&mut self) -> Allocator<T> {
            Allocator {
                heap: self,
            }
        }

        fn dpvoid_inside_heap(&self, ptr: &DpVoid) -> bool {
            let data_start = self.data.as_ptr();
            let data_end = unsafe { data_start.offset(self.data.capacity() as isize) } as usize;
            let data_start = data_start as usize;
            let ptr_val = ptr as *const _ as usize;

            ptr_val >= data_start && ptr_val < data_end
        }
        fn enregister(&mut self, ptr: &DpVoid) {
            let copy = DpVoid {
                heap: ptr.heap,
                ptr: ptr.ptr,
            };
            if self.dpvoid_inside_heap(ptr) {
                self.nonroots.push(copy);
            } else {
                self.roots.push(copy);
            }
        }
        fn deregister(&mut self, ptr: &DpVoid) {
            if self.dpvoid_inside_heap(ptr) {
                let idx = self.nonroots.iter().position(|dpv| dpv == ptr);
                if let Some(idx) = idx {
                    self.nonroots.remove(idx);
                }
            } else {
                let idx = self.roots.iter().position(|dpv| dpv == ptr);
                if let Some(idx) = idx {
                    self.roots.remove(idx);
                }
            }
        }

    }

    struct Allocator<'a, T: Traceable + 'a> {
        heap: &'a mut DeferredHeap<T>,
    }

    impl<'a, T: Traceable + 'a> Allocator<'a, T> {
        fn insert(&mut self, mut data: T) -> Dp<'a, T> {
        // fn insert<'b>(&'b mut self, mut data: T) -> Dp<'b, T> where 'b: 'a {
            if data.is_root() {
                data.set_root_to(false);
            }
            self.heap.data.push(data);
            let len = self.heap.data.len();
            // Dp {
            //     _marker: Default::default(),
            //     impl_ptr: DpVoid {
            //         heap: unsafe {transmute(self as *const DeferredHeap<'a, T>)},
            //         ptr: unsafe {transmute(&self.data[len-1] as *const T)},
            //     }
            // }
            Dp::new(self.heap, &self.heap.data[len - 1])
        }
    }
    struct Dp<'a, T: Traceable> where T: 'a {
        _marker: PhantomData<*mut &'a T>,
        impl_ptr: DpVoid,
    }
    impl<'a, T: Traceable> Dp<'a, T> {
        fn new(heap: &DeferredHeap<T>, ptr: &T) -> Dp<'a, T> {
            Dp {
                _marker: Default::default(),
                impl_ptr: DpVoid::new(heap, ptr),
            }
        }
        fn borrow_mut(&mut self) -> &mut T {
            use std::mem::transmute;
            let ptr: *mut T = unsafe {transmute(self.impl_ptr.ptr)};
            unsafe {&mut *ptr}
        }
        fn borrow(&self) -> &T {
            use std::mem::transmute;
            let ptr: *const T = unsafe {transmute(self.impl_ptr.ptr)};
            unsafe {&*ptr}
        }
    }
    impl<'a, T: Traceable> ::std::ops::Deref for Dp<'a, T> where T: 'a {
        type Target = T;
        fn deref(&self) -> &T {
            self.borrow()
        }

    }

    #[derive(Eq, PartialEq)]
    struct DpVoid {
        heap: *mut DeferredHeap<()>,
        ptr: *const i8,
    }
    impl DpVoid {
        fn new<'a, T: Traceable>(heap: &DeferredHeap<T>, ptr: &T) -> DpVoid {
            use std::mem::transmute;
            let dpv = DpVoid {
                heap: unsafe {transmute(heap as *const DeferredHeap<T>)},
                ptr: unsafe {transmute(ptr as *const T)},
            };
            // dpv.heap_ref_mut().enregister(&dpv);
            dpv
        }
        fn heap_ref(&mut self) -> &DeferredHeap<()> {
            unsafe { &*self.heap }
        }
        fn heap_ref_mut(&mut self) -> &mut DeferredHeap<()> {
            unsafe { &mut *self.heap }
        }
    }
    impl ::std::ops::Drop for DpVoid {
        fn drop(&mut self) {
            // self.heap_ref_mut().deregister(self);
        }
    }


    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn linked_list1() {
            use ::std::cell::RefCell;
            #[derive(Default)]
            struct LLNode<'a> {
                data: i32,
                next: RefCell<Option<Dp<'a, LLNode<'a>>>>,
                prev: RefCell<Option<Dp<'a, LLNode<'a>>>>,
            }
            impl<'a> Traceable for LLNode<'a> {
                fn set_root_to(&mut self, _b: bool) {}
                fn is_root(&self) -> bool { false }
            }
            let mut heap: DeferredHeap<LLNode> = DeferredHeap::new();
            let mut alloc = heap.allocator();
            let mut node1: LLNode = Default::default();
            let a: Dp<LLNode> = alloc.insert(Default::default()); 
            // a is root
            let b: Dp<LLNode> = alloc.insert(Default::default());
            let c: Dp<LLNode> = alloc.insert(Default::default());
            let d: Dp<LLNode> = alloc.insert(Default::default());

            {
                *node1.next.borrow_mut() = Some(a);
            }
            {
                {
                    *b.next.borrow_mut() = Some(c);
                }
                *d.next.borrow_mut() = Some(b);
            }

        }

        #[test]
        fn make_new_dp() {
            #[derive(Default)]
            struct IntPayload {
                data: i32,
            }
            impl Traceable for IntPayload {
                fn set_root_to(&mut self, _b: bool) {}
                fn is_root(&self) -> bool { false }
            }
            impl IntPayload {
                fn new_default(data: i32) -> IntPayload {
                    let mut pay: IntPayload = Default::default();
                    pay.data = data;
                    pay
                }
            }
            // { // Shouldn't compile (due to borrowck)
            //     let ptr;
            //     {
            //         let mut heap: DeferredHeap<IntPayload> = DeferredHeap::new();
            //         let mut alloc = heap.allocator();
            //         ptr = alloc.insert(IntPayload { data: 3 });
            //     }
            //     assert_eq!(ptr.data, 3);
            // }

            let mut heap: DeferredHeap<IntPayload> = DeferredHeap::new();
            let mut alloc = heap.allocator();
            let ptr = alloc.insert(IntPayload::new_default(3));
            assert_eq!(ptr.borrow().data, 3);
            assert_eq!(ptr.data, 3);
        }
    }

}

mod reference_distributer_with_custom_ptr_type {
    use std::marker::PhantomData;

    trait Traceable {}
    #[derive(Default)]
    struct DeferredHeap<'longer_than_self, T: Traceable> {
        data: Vec<T>,
        _marker: PhantomData<*mut &'longer_than_self ()>,
    }

    impl<'a, T: Traceable> DeferredHeap<'a, T> {
        fn new() -> DeferredHeap<'a, T> {
                DeferredHeap {
                data: Vec::with_capacity(32),
                _marker: Default::default(),
            }
        }

        fn insert_ref(&mut self, data: T) -> &mut T {
            self.data.push(data);
            let len = self.data.len();
            &mut self.data[len - 1]
        }
        fn insert<'b>(&'b mut self, data: T) -> Dp<'b, T> where 'b: 'a {
            self.data.push(data);
            let len = self.data.len();
            Dp {
                _marker: Default::default(),
                heap: self as *const _,
                ptr: &self.data[len-1] as *const _,
            }
        }
    }

    struct Dp<'a, T: Traceable> where T: 'a {
        _marker: PhantomData<*mut &'a ()>,
        heap: *const DeferredHeap<'a, T>,
        ptr: *const T,
    }


    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn make_new_dp() {
            struct IntPayload {
                data: i32,
            }
            impl Traceable for IntPayload {}
            // { // Shouldn't compile (due to borrowck)
            //     let ptr;
            //     {
            //         let mut heap: DeferredHeap<IntPayload> = DeferredHeap::new();
            //         ptr = heap.insert(IntPayload { data: 3 });
            //     }
            //     assert_eq!(unsafe {&*ptr.ptr}.data, 3);
            // }

            let mut heap: DeferredHeap<IntPayload> = DeferredHeap::new();
            let ptr = heap.insert(IntPayload { data: 3 });
            assert_eq!(unsafe {&*ptr.ptr}.data, 3);
        }
    }
}

mod type_erased_dropping {
    struct Dropper {
        drop_glue: fn(*const i8),
    }

    impl Dropper {
        fn new<T>() -> Dropper {
            use std::intrinsics::drop_in_place;
            Dropper {
                drop_glue: |ptr: *const i8| unsafe { drop_in_place(ptr as *mut T) },
            }
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        #[test]
        fn can_drop() {
            use std::ops::Drop;
            use std::mem::forget;

            struct DoThing<'a> {
                val: &'a mut bool,
            }
            impl<'a> Drop for DoThing<'a> {
                fn drop(&mut self) {
                    *self.val = true;
                }
            }

            let mut a = false;
            {
                let _dt = DoThing {
                    val: &mut a,
                };
            }
            assert_eq!(a, true);
            a = false;
            {
                let dt = DoThing {
                    val: &mut a,
                };
                forget(dt);
            }
            assert_eq!(a, false);
            {
                let dt = DoThing {
                    val: &mut a,
                };
                let dropper = Dropper::new::<DoThing>();

                (dropper.drop_glue)(&dt as *const _ as *const i8);
                
                forget(dt);
            }
            assert_eq!(a, true);
        }
    }
}

mod reference_distributer {
    //! Just for testing the reference distributing lifetime stuff

    use std::marker::PhantomData;
    pub struct RefDist {
        vec: Vec<u8>,
    }
    pub struct Allocator<'a> {
        arena: &'a mut RefDist,
    }

    pub struct RefStruct<'a> {
        _marker: PhantomData<*mut &'a ()>,
        ptr: *mut u8,
    }
    impl<'a> ::std::ops::Deref for RefStruct<'a> {
        type Target = u8;
        fn deref(&self) -> &u8 {
            unsafe {&*self.ptr}
        }
    }


    impl RefDist {
        pub fn new() -> RefDist {
            RefDist {
                vec: vec![1, 2, 3, 4],
            }
        }
        pub fn allocator(&mut self) -> Allocator {
            Allocator {
                arena: self,
            }
        }
    }
    impl<'a> Allocator<'a> {
        // pub fn index<'b>(&'b mut self, idx: usize) -> &'b mut u8 {
        pub fn index(&mut self, idx: usize) -> &'a mut u8 {
            unsafe {::std::mem::transmute(self.arena.vec.get_mut(idx).unwrap())}
        }
        // pub fn index_struct<'b>(&'b mut self, idx: usize) -> RefStruct<'b> {
        //     RefStruct {
        //         _marker: Default::default(),
        //         ptr: (self.vec.get_mut(idx).unwrap()) as *mut _,
        //     }
        // }
        pub fn index_struct(&mut self, idx: usize) -> RefStruct<'a> {
            RefStruct {
                _marker: Default::default(),
                ptr: (&mut self.arena.vec[idx]) as *mut _,
            }
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        #[test]
        fn gives_ref() {
            // let mut rdist = RefDist::new();
            // {
            //     let val = rdist.index(0);
            //     *val = 3;
            // }
            // {
            //     let val = rdist.index(0);
            //     assert_eq!(*val, 3);
            // }
            let mut arena = RefDist::new();
            let mut rdist = arena.allocator();
            let ref1 = rdist.index(0);
            let ref2 = rdist.index(1);
            assert!(*ref1 != *ref2);
        }
        #[test]
        fn gives_ref_struct() {
            let mut arena = RefDist::new();
            let mut rdist = arena.allocator();
            let val1 = rdist.index_struct(0);
            let val2 = rdist.index_struct(1);
            assert!(*val1 != *val2);
        }

        // #[test]
        // fn does_not_let_outlive() { // Shouldn't compile
        //     let mut arena = RefDist::new();
        //     let mut rdist = arena.allocator();
        //     let _val_ref = rdist.index(0);
        //     ::std::mem::drop(rdist);
        //     ::std::mem::drop(arena);
        // }
        // #[test]
        // fn does_not_let_outlive_struct() { // Shouldn't compile
        //     let mut arena = RefDist::new();
        //     let mut rdist = arena.allocator();
        //     let _val_ref_struct = rdist.index_struct(0);
        //     ::std::mem::drop(rdist);
        //     ::std::mem::drop(arena);
        // }

    }
}
