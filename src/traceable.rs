use ptr::{Gc, GcBox, Safe, Weak};
use UntypedGcBox;

// Impls: For every object `obj` that impls TraceTo, call `obj.trace_to(tracer)`.
// Can act funny if you have Sp<Gc<T>> where Sp is a smart pointer that
// doesn't impl TraceTo.
pub trait TraceTo {
    fn trace_to(&self, tracer: &mut Tracer);
}
pub(crate) struct TraceDest(pub *const UntypedGcBox);

pub struct Tracer {
    targets: Vec<TraceDest>,
}

impl Tracer {
    pub(crate) fn new() -> Tracer {
        Tracer { targets: vec![] }
    }
    pub fn add_target<T: TraceTo>(&mut self, target: &T) {
        target.trace_to(self);
    }
    fn add_box<T>(&mut self, gc_box: *const GcBox<T>) {
        self.targets.push(TraceDest(gc_box as *const _));
    }
    pub(crate) fn results(self) -> ::std::vec::IntoIter<TraceDest> {
        self.targets.into_iter()
    }
}

pub struct NoTrace<T>(pub T);
impl<T> TraceTo for NoTrace<T> {
    fn trace_to(&self, _: &mut Tracer) {
        // noop
    }
}

impl<'a, T> TraceTo for Gc<'a, T> {
    fn trace_to(&self, tracer: &mut Tracer) {
        tracer.add_box(Gc::box_ptr(self).as_ptr() as *const _); // FIXME NonNull conv
    }
}

impl<'a, T> TraceTo for Safe<'a, T> {
    fn trace_to(&self, tracer: &mut Tracer) {
        if let Some(box_ptr) = self.box_ptr() {
            tracer.add_box(box_ptr.as_ptr() as *const _); // FIXME NonNull conv
        }
    }
}
impl<'a, T> TraceTo for Weak<'a, T> {
    fn trace_to(&self, _: &mut Tracer) {
        // noop
    }
}

mod trace_impls {
    use super::{TraceTo, Tracer};
    use std;
    use std::cmp::Eq;
    use std::cmp::Ord;
    use std::hash::Hash;

    macro_rules! noop_impls {
        ($($T:ty)+) => {
            $(
                impl TraceTo for $T {
                    fn trace_to(&self, _: &mut Tracer) {
                        // noop
                    }
                }
             )+
        }
    }
    noop_impls! {
        ()
        bool
        i8 i16 i32 i64 i128
        u8 u16 u32 u64 u128
        isize usize
        f32 f64
        char str
    }
    macro_rules! noop_fn_impl {
        ($($T:tt)*) => {
            impl<$($T,)* R> TraceTo for fn($($T),*) -> R {
                fn trace_to(&self, _: &mut Tracer) {
                    // noop
                }
            }
        }
    }
    noop_fn_impl!();
    noop_fn_impl!(Q);
    noop_fn_impl!(Q W);
    noop_fn_impl!(Q W E);
    noop_fn_impl!(Q W E T);
    impl<T> TraceTo for *const T {
        fn trace_to(&self, _: &mut Tracer) {
            // noop
        }
    }
    impl<T> TraceTo for *mut T {
        fn trace_to(&self, _: &mut Tracer) {
            // noop
        }
    }

    impl<'a, T: TraceTo> TraceTo for [T] {
        fn trace_to(&self, tracer: &mut Tracer) {
            for tracee in self.iter() {
                tracee.trace_to(tracer);
            }
        }
    }
    macro_rules! array_impls {
        ($($N:expr)+) => {
            $(
                impl<T: TraceTo> TraceTo for [T; $N] {
                    fn trace_to(&self, tracer: &mut Tracer) {
                        (&self[..]).trace_to(tracer);
                    }
                }
             )+
        }
    }
    array_impls! {
        0  1  2  3  4  5  6  7  8  9
        10 11 12 13 14 15 16 17 18 19
        20 21 22 23 24 25 26 27 28 29
        30 31 32
    }

    impl<T: TraceTo> TraceTo for Option<T> {
        fn trace_to(&self, tracer: &mut Tracer) {
            if let Some(ref contents) = self {
                contents.trace_to(tracer);
            }
        }
    }
    // TODO: Is this one a good idea?
    impl<T: TraceTo, E> TraceTo for Result<T, E> {
        fn trace_to(&self, tracer: &mut Tracer) {
            if let Ok(ref contents) = self {
                contents.trace_to(tracer);
            }
        }
    }
    impl<T: TraceTo> TraceTo for Box<T> {
        fn trace_to(&self, tracer: &mut Tracer) {
            let contents: &T = &*self;
            contents.trace_to(tracer);
        }
    }
    impl<T: TraceTo> TraceTo for Vec<T> {
        fn trace_to(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracee.trace_to(tracer);
            }
        }
    }
    impl<T: TraceTo> TraceTo for std::rc::Rc<T> {
        fn trace_to(&self, tracer: &mut Tracer) {
            let contents: &T = &*self;
            contents.trace_to(tracer);
        }
    }
    impl<T: TraceTo> TraceTo for std::sync::Arc<T> {
        fn trace_to(&self, tracer: &mut Tracer) {
            let contents: &T = &*self;
            contents.trace_to(tracer);
        }
    }
    impl<T: TraceTo> TraceTo for std::cell::RefCell<T> {
        fn trace_to(&self, tracer: &mut Tracer) {
            self.borrow().trace_to(tracer);
        }
    }
    impl<T: TraceTo> TraceTo for std::collections::VecDeque<T> {
        fn trace_to(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracee.trace_to(tracer);
            }
        }
    }
    impl<T: TraceTo> TraceTo for std::collections::LinkedList<T> {
        fn trace_to(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracee.trace_to(tracer);
            }
        }
    }
    impl<T: TraceTo, K: Eq + Hash> TraceTo for std::collections::HashMap<K, T> {
        fn trace_to(&self, tracer: &mut Tracer) {
            for tracee in self.values() {
                tracee.trace_to(tracer);
            }
        }
    }
    impl<T: TraceTo, K: Eq + Hash> TraceTo for std::collections::BTreeMap<K, T> {
        fn trace_to(&self, tracer: &mut Tracer) {
            for tracee in self.values() {
                tracee.trace_to(tracer);
            }
        }
    }
    impl<T: TraceTo + Eq + Hash> TraceTo for std::collections::HashSet<T> {
        fn trace_to(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracee.trace_to(tracer);
            }
        }
    }
    impl<T: TraceTo + Eq + Hash> TraceTo for std::collections::BTreeSet<T> {
        fn trace_to(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracee.trace_to(tracer);
            }
        }
    }
    impl<T: TraceTo + Ord> TraceTo for std::collections::BinaryHeap<T> {
        fn trace_to(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracee.trace_to(tracer);
            }
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[derive(Clone)]
    struct MustTrace {
        traced: Cell<bool>,
    }
    impl MustTrace {
        fn new() -> MustTrace {
            MustTrace {
                traced: Cell::new(false),
            }
        }
    }
    impl Drop for MustTrace {
        fn drop(&mut self) {
            assert!(self.traced.get());
        }
    }
    impl TraceTo for MustTrace {
        fn trace_to(&self, _: &mut Tracer) {
            self.traced.set(true);
        }
    }

    #[test]
    fn trace_box() {
        let mut tracer = Tracer::new();
        let tracee = Box::new(MustTrace::new());
        tracee.trace_to(&mut tracer);
    }
    #[test]
    fn trace_vec() {
        let mut tracer = Tracer::new();
        let tracee = vec![MustTrace::new(); 25];
        tracee.trace_to(&mut tracer);
    }
    #[test]
    fn trace_array() {
        fn nm() -> MustTrace {
            MustTrace::new()
        }
        let mut tracer = Tracer::new();
        let tracee = [
            nm(),
            nm(),
            nm(),
            nm(),
            nm(),
            nm(),
            nm(),
            nm(),
            nm(),
            nm(),
            nm(),
            nm(),
            nm(),
            nm(),
            nm(),
        ];
        tracee.trace_to(&mut tracer);
    }

    #[test]
    fn trace_slice() {
        let mut tracer = Tracer::new();
        let tracee = vec![MustTrace::new(); 25];
        let tra_slice: &[MustTrace] = &tracee[..];
        tra_slice.trace_to(&mut tracer);
    }
    #[test]
    fn trace_noops() {
        use std::mem::transmute;

        let mut tracer = Tracer::new();

        macro_rules! test_noop_run {
            ($($T:ty, $val:expr)+) => {
                $(
                    let t: $T = $val;
                    t.trace_to(&mut tracer);
                 )+
            }
        }
        test_noop_run!(
            (), ()
            bool, true
            i8, 0
            i16, 0
            i32, 0
            i64, 0
            i128, 0
            u8, 0
            u16, 0
            u32, 0
            u64, 0
            u128, 0
            isize, 0
            usize, 0
            f32, 0.0
            f64, 0.0
            char, 'a'
            Box<str>, "Hello".to_string().into_boxed_str()

            fn(), unsafe { transmute(0 as usize) }
            fn() -> i8,  unsafe { transmute(0 as usize) }
            fn(i8) -> i8, unsafe { transmute(0 as usize) }
            fn(i8, u8, isize, usize) -> i8, unsafe { transmute(0 as usize) }
        );
        let t: &str = "Hello";
        t.trace_to(&mut tracer);
    }
}
