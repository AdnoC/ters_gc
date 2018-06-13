#[derive(Eq, PartialEq, Debug)]
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
    pub fn new() -> Destructors {
        Destructors {
            dtors: vec![],
        }
    }
    pub fn store<T>(&mut self, ptrs: &[T]) {
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

    pub fn is_stored<T>(&self, ptr: &T) -> bool{
        use std::intrinsics::needs_drop;
        if ! unsafe { needs_drop::<T>() } {
            return false;
        }

        let ptr = ptr as *const _ as *const i8;
        self.dtors.iter().any(|dtor| dtor.ptr == ptr)
    }

    pub fn run_all(&mut self) {
        for dtor in &self.dtors {
            (dtor.drop_glue)(dtor.ptr);
        }
        self.dtors.clear();
    }

    pub fn run(&mut self, range: &[i8]) { 
        // TODO: Deferred heap has a note about reentrancy safety. Do we need
        // to handle this? Currently are not.

        if range.len() == 0 {
            return;
        }

        let mut to_destroy = vec![];

        let ptr = range.as_ptr();
        let max_offset = range.len() as isize;

        for (idx, dtor) in self.dtors.iter().enumerate() {
            let offset = unsafe { dtor.ptr.offset_from(ptr) };
            println!("0..{}: {} ({})", max_offset, offset, idx);
            if offset >= 0 && offset < max_offset {
                println!("\tDestroying");
                to_destroy.push(idx);
                (dtor.drop_glue)(dtor.ptr);
            }
        }

        for idx in to_destroy.into_iter().rev() {
            self.dtors.swap_remove(idx);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{
        rc::Rc,
        cell::RefCell,
        mem::forget,
        ops::RangeBounds,
    };

    fn slice_t_as_slice_i8<'a, T>(slice: &'a [T]) -> &'a [i8] {
        use std::mem::{
            size_of,
            transmute,
        };
        let vec = unsafe {
            Vec::from_raw_parts(
                slice.as_ptr() as *mut i8,
                slice.len() * size_of::<T>(),
                slice.len() * size_of::<T>(),
                               )
        };
        let converted = unsafe { transmute(&vec[..]) };
        forget(vec);
        converted
    }

    fn drain_forget<T, R>(vec: &mut Vec<T>, range: R) 
        where R: RangeBounds<usize> {
        for val in vec.drain(range) {
            forget(val);
        }
    }
    fn drain_forget_at<T>(vec: &mut Vec<T>, idx: usize) {
        drain_forget(vec, idx..(idx + 1))
    }

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

    #[derive(Default, Clone)]
    struct HasDrop {}
    impl Drop for HasDrop {
        fn drop(&mut self) { }
    }

    #[test]
    fn stores_addrs_with_dtors() {
        let mut dtors = Destructors::new();

        let ints = vec![5; 25];
        dtors.store(&ints);

        let droppers = vec![HasDrop {}; 25];
        dtors.store(&droppers);

        let strings = vec!["Hello World".to_owned(); 25];
        dtors.store(&strings);

        assert_eq!(dtors.dtors.len(), droppers.len() + strings.len());
    }
    
    #[test]
    fn knows_what_addrs_it_contins() {
        let mut dtors = Destructors::new();

        let in_dtors1 = vec![HasDrop {}; 25];
        dtors.store(&in_dtors1);

        let in_dtors2 = vec![HasDrop {}; 25];
        dtors.store(&in_dtors2);

        let not_in1 = vec![0; 25];
        assert!(!dtors.is_stored(not_in1.first().unwrap()));
        assert!(!dtors.is_stored(not_in1.last().unwrap()));
        assert!(!dtors.is_stored(&not_in1[not_in1.len()/2]));

        assert!(dtors.is_stored(in_dtors1.first().unwrap()));
        assert!(dtors.is_stored(in_dtors1.last().unwrap()));
        assert!(dtors.is_stored(&in_dtors1[in_dtors1.len()/2]));

        assert!(dtors.is_stored(in_dtors2.first().unwrap()));
        assert!(dtors.is_stored(in_dtors2.last().unwrap()));
        assert!(dtors.is_stored(&in_dtors2[in_dtors2.len()/2]));
    }

    #[test]
    fn stores_glue_with_ptr() {
        let mut dtors = Destructors::new();
        let counter = DtorCounter::new();

        let mut incr = vec![counter.incr(); 25];
        dtors.store(&incr);

        let ptr = incr.first().unwrap() as *const _ as *const i8;
        let drop_glue = get_drop_glue::<CounterIncrementer>();

        let first_dtor = dtors.dtors.first().unwrap();
        assert_eq!(*first_dtor, Destructor { ptr, drop_glue });

        (first_dtor.drop_glue)(first_dtor.ptr);
        drain_forget_at(&mut incr, 0);
        assert_eq!(counter.count(), 1);

    }

    #[test]
    fn runs_a_dtor() {
        let mut dtors = Destructors::new();
        let counter = DtorCounter::new();

        let mut incr = vec![counter.incr()];
        dtors.store(&incr);

        dtors.run(slice_t_as_slice_i8(&incr[..]));
        drain_forget(&mut incr, ..);
        assert_eq!(counter.count(), 1);
    }
    #[test]
    fn runs_a_dtor_range() {
        let mut dtors = Destructors::new();
        let counter = DtorCounter::new();

        const LEN_INCR: usize = 25;
        let mut incr = vec![counter.incr(); LEN_INCR];
        dtors.store(&incr);

        dtors.run(slice_t_as_slice_i8(&incr[..]));
        assert_eq!(counter.count(), LEN_INCR);
        drain_forget(&mut incr, ..);
        assert_eq!(counter.count(), LEN_INCR);
    }

    #[test]
    fn runs_only_dtor_range() {
        use std::ops::Range;
        let mut dtors = Destructors::new();
        let counter = DtorCounter::new();

        const LEN_INCR: usize = 25;
        const RANGE_TO_RUN: Range<usize> = 0..(LEN_INCR / 2);
        const LEN_RANGE: usize = RANGE_TO_RUN.end - RANGE_TO_RUN.start;

        let mut incr1 = vec![counter.incr(); LEN_INCR];
        dtors.store(&incr1);

        let mut incr2 = vec![counter.incr(); LEN_INCR];
        dtors.store(&incr2);

        dtors.run(slice_t_as_slice_i8(&incr1[RANGE_TO_RUN]));
        drain_forget(&mut incr1, RANGE_TO_RUN);
        assert_eq!(counter.count(), LEN_RANGE);
    }

    // #[test]
    // fn runs_all_dtors() {
    //     let mut dtors = Destructors::new();
    //     let counter = DtorCounter::new();
    //
    //     let incrs1 = vec![counter.incr(); 25];
    //     dtors.store(&incrs1);
    //
    //     let ints1 = vec![0; 25];
    //     dtors.store(&incrs1);
    //
    //     let incrs2 = vec![counter.incr(); 25];
    //     dtors.store(&incrs2);
    //
    //     let strings = vec!["Hello World".to_owned(); 25];
    //     dtors.store(&strings);
    //
    //     
    //     dtors.run_all();
    //     assert_eq!(counter.count(), incrs1.len() + incrs2.len());
    //
    //     drain_forget(incrs1, ..);
    //     drain_forget(incrs2, ..);
    //     drain_forget(strings, ..);
    //
    // }

    #[test]
    fn dtor_counter_increments() {
        use std::mem::drop;
        let counter = DtorCounter::new();
        assert_eq!(0, counter.count());
        const LEN_ANNON: usize = 25;
        {
            let _ = vec![counter.incr(); LEN_ANNON];
        }
        assert_eq!(LEN_ANNON, counter.count());

        const LEN_VEC: usize = 25;
        let vec = vec![counter.incr(); LEN_VEC];
        assert_eq!(LEN_VEC, counter.count());
        drop(vec);
        assert_eq!(LEN_ANNON + LEN_VEC, counter.count());

    }
    #[test]
    fn sane_drain_forget() {
        let counter = DtorCounter::new();

        const LEN_INCR: usize = 25;
        let mut incr = vec![counter.incr(); LEN_INCR];

        let ptr = incr.first().unwrap() as *const _ as *const i8;
        let drop_glue = get_drop_glue::<CounterIncrementer>();

        drop_glue(ptr);
        assert_eq!(counter.count(), 1);

        drain_forget_at(&mut incr, 0);
        assert_eq!(incr.len(), LEN_INCR - 1);
        assert_eq!(counter.count(), 1);
        drop(incr);
        assert_eq!(counter.count(), LEN_INCR);
    }
}
