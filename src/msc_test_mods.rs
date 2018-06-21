mod tracking_root_status {
    use std::marker::PhantomData;

    trait Traceable {}
    #[derive(Default)]
    struct DeferredHeap<'longer_than_self, T: Traceable> {
        data: Vec<T>,
        nonroots: Vec<DpVoid>,
        _marker: PhantomData<*mut &'longer_than_self ()>,
    }

    impl<'a, T: Traceable> DeferredHeap<'a, T> {
        fn new() -> DeferredHeap<'a, T> {
                DeferredHeap {
                data: Vec::with_capacity(32),
                nonroots: vec![],
                _marker: Default::default(),
            }
        }

        fn insert_ref(&mut self, data: T) -> &mut T {
            self.data.push(data);
            let len = self.data.len();
            &mut self.data[len - 1]
        }
        fn insert<'b>(&'b mut self, data: T) -> Dp<'b, T> where 'b: 'a {
            use std::mem::transmute;

            self.data.push(data);
            let len = self.data.len();
            Dp {
                _marker: Default::default(),
                impl_ptr: DpVoid {
                    heap: unsafe {transmute(self as *const DeferredHeap<'a, T>)},
                    ptr: unsafe {transmute(&self.data[len-1] as *const T)},
                }
            }
        }
    }

    struct Dp<'a, T: Traceable> where T: 'a {
        _marker: PhantomData<*mut &'a T>,
        impl_ptr: DpVoid,
    }
    impl<'a, T: Traceable> Dp<'a, T> {
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

    struct DpVoid {
        heap: *const DeferredHeap<'static, Payload>,
        ptr: *const i8,
    }

    struct Payload {
        data: i32,
    }
    impl Traceable for Payload {}
    impl Payload {
        fn new_default(data: i32) -> Payload {
            Payload {
                data,
            }
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn make_new_dp() {
            // { // Shouldn't compile (due to borrowck)
            //     let ptr;
            //     {
            //         let mut heap: DeferredHeap<Payload> = DeferredHeap::new();
            //         ptr = heap.insert(Payload { data: 3 });
            //     }
            //     assert_eq!(unsafe {&*ptr.ptr}.data, 3);
            // }

            let mut heap: DeferredHeap<Payload> = DeferredHeap::new();
            let ptr = heap.insert(Payload::new_default(3));
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

    struct Payload {
        data: i32,
    }
    impl Traceable for Payload {}

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn make_new_dp() {
            // { // Shouldn't compile (due to borrowck)
            //     let ptr;
            //     {
            //         let mut heap: DeferredHeap<Payload> = DeferredHeap::new();
            //         ptr = heap.insert(Payload { data: 3 });
            //     }
            //     assert_eq!(unsafe {&*ptr.ptr}.data, 3);
            // }

            let mut heap: DeferredHeap<Payload> = DeferredHeap::new();
            let ptr = heap.insert(Payload { data: 3 });
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
    pub struct RefDist<'longer_than_self> {
        _marker: PhantomData<*mut &'longer_than_self ()>,
        vec: Vec<u8>,
    }

    impl<'a> RefDist<'a> {
        pub fn new() -> RefDist<'a> {
            RefDist {
                _marker: PhantomData,
                vec: vec![1, 2, 3, 4],
            }
        }
        pub fn index(&mut self, idx: usize) -> &mut u8 {
            &mut self.vec[idx]
        }
    }

    #[cfg(test)]
    mod test {
        use super::*;
        #[test]
        fn gives_ref() {
            let mut rdist = RefDist::new();
            {
                let val = rdist.index(0);
                *val = 3;
            }
            {
                let val = rdist.index(0);
                assert_eq!(*val, 3);
            }
        }

        // #[test]
        // fn does_not_let_outlive() {
        //     let val_ref;
        //     {
        //         let mut rdist = RefDist::new();
        //         val_ref = rdist.index(0);
        //     }
        // }

    }
}
