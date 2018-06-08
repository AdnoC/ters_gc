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

    }
}
