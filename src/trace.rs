//! Types needed to allow a type to be stored in the gc heap.
//!
//! A type must implement [`Trace`] to be stored in the gc heap.
//!
//! [`Trace`] lets the collector know what tracked objects an object has
//! references to. An incomplete [`Trace`] implementation will result in
//! memory leaks.
//!
//! A correct [`Trace`] implementation calls  TODO Finish
//!
//! [`Trace`]: trait.Trace.html

use ptr::{Gc, GcBox, Weak};
use std::ptr::NonNull;
use AsUntyped;
use UntypedGcBox;

// Impls: For every object `obj` that impls Trace, call `tracer.add_entry(&obj)`.
// Can act funny if you have Sp<Gc<T>> where Sp is a smart pointer that
// doesn't impl Trace.
/// Trait all types that are stored in the gc heap must implement.
pub trait Trace {
    /// Trace reachability information to the tracer.
    ///
    /// Should be called on all types that contain a [`Gc`] pointer.
    ///
    /// [`Gc`]: ../ptr/struct.Gc.html
    fn trace(&self, _tracer: &mut Tracer) {
        // noop
    }
}
pub(crate) struct TraceDest(pub NonNull<UntypedGcBox>);

/// Destination for trace information.
pub struct Tracer {
    targets: Vec<TraceDest>,
}

impl Tracer {
    pub(crate) fn new() -> Tracer {
        Tracer { targets: vec![] }
    }
    pub fn add_target<T: Trace + ?Sized>(&mut self, target: &T) {
        target.trace(self);
    }
    fn add_box<T>(&mut self, gc_box: NonNull<GcBox<T>>) {
        self.targets.push(TraceDest(gc_box.as_untyped()));
    }
    pub(crate) fn results(self) -> ::std::vec::IntoIter<TraceDest> {
        self.targets.into_iter()
    }
}


impl<'a, T> Trace for Gc<'a, T> {
    fn trace(&self, tracer: &mut Tracer) {
        if let Some(box_ptr) = self.box_ptr() {
            tracer.add_box(box_ptr);
        }
    }
}
impl<'a, T> Trace for Weak<'a, T> {
    fn trace(&self, _: &mut Tracer) {
        // noop
    }
}

mod trace_impls {
    use super::{Trace, Tracer};
    use std;
    use std::cmp::Eq;
    use std::cmp::Ord;
    use std::hash::Hash;

    macro_rules! noop_impls {
        ($($T:ty)+) => {
            $(
                impl Trace for $T {
                    fn trace(&self, _: &mut Tracer) {
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
    impl<'a> Trace for &'a str {
        fn trace(&self, _: &mut Tracer) {
            // noop
        }
    }
    macro_rules! noop_fn_impl {
        ($($T:tt)*) => {
            impl<$($T,)* R> Trace for fn($($T),*) -> R {
                fn trace(&self, _: &mut Tracer) {
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
    impl<T: ?Sized> Trace for *const T {
        fn trace(&self, _: &mut Tracer) {
            // noop
        }
    }
    impl<T: ?Sized> Trace for *mut T {
        fn trace(&self, _: &mut Tracer) {
            // noop
        }
    }

    impl<'a, T: Trace> Trace for [T] {
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self.iter() {
                tracer.add_target(tracee);
            }
        }
    }
    macro_rules! array_impls {
        ($($N:expr)+) => {
            $(
                impl<T: Trace> Trace for [T; $N] {
                    fn trace(&self, tracer: &mut Tracer) {
                        tracer.add_target(&self[..]);
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

    impl<T: Trace> Trace for Option<T> {
        fn trace(&self, tracer: &mut Tracer) {
            if let Some(ref contents) = self {
                tracer.add_target(contents);
            }
        }
    }
    // TODO: Is this one a good idea?
    impl<T: Trace, E> Trace for Result<T, E> {
        fn trace(&self, tracer: &mut Tracer) {
            if let Ok(ref contents) = self {
                tracer.add_target(contents);
            }
        }
    }
    impl<T: Trace + ?Sized> Trace for Box<T> {
        fn trace(&self, tracer: &mut Tracer) {
            let contents: &T = &*self;
            tracer.add_target(contents);
        }
    }
    impl<T: Trace> Trace for Vec<T> {
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracer.add_target(tracee);
            }
        }
    }
    impl<T: Trace + ?Sized> Trace for std::rc::Rc<T> {
        fn trace(&self, tracer: &mut Tracer) {
            let contents: &T = &*self;
            tracer.add_target(contents);
        }
    }
    impl<T: Trace + ?Sized> Trace for std::sync::Arc<T> {
        fn trace(&self, tracer: &mut Tracer) {
            let contents: &T = &*self;
            tracer.add_target(contents);
        }
    }
    impl<T: Trace + ?Sized> Trace for std::cell::RefCell<T> {
        fn trace(&self, tracer: &mut Tracer) {
            tracer.add_target(&*self.borrow());
        }
    }
    impl<T: Trace> Trace for std::collections::VecDeque<T> {
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracer.add_target(tracee);
            }
        }
    }
    impl<T: Trace> Trace for std::collections::LinkedList<T> {
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracer.add_target(tracee);
            }
        }
    }
    impl<T: Trace, K: Eq + Hash> Trace for std::collections::HashMap<K, T> {
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self.values() {
                tracer.add_target(tracee);
            }
        }
    }
    impl<T: Trace, K: Eq + Hash> Trace for std::collections::BTreeMap<K, T> {
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self.values() {
                tracer.add_target(tracee);
            }
        }
    }
    impl<T: Trace + Eq + Hash> Trace for std::collections::HashSet<T> {
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracer.add_target(tracee);
            }
        }
    }
    impl<T: Trace + Eq + Hash> Trace for std::collections::BTreeSet<T> {
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracer.add_target(tracee);
            }
        }
    }
    impl<T: Trace + Ord> Trace for std::collections::BinaryHeap<T> {
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracer.add_target(tracee);
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
    impl Trace for MustTrace {
        fn trace(&self, _: &mut Tracer) {
            self.traced.set(true);
        }
    }

    #[test]
    fn trace_box() {
        let mut tracer = Tracer::new();
        let tracee = Box::new(MustTrace::new());
        tracer.add_target(&tracee);
    }
    #[test]
    fn trace_vec() {
        let mut tracer = Tracer::new();
        let tracee = vec![MustTrace::new(); 25];
        tracer.add_target(&tracee);
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
        tracer.add_target(&tracee);
    }

    #[test]
    fn trace_slice() {
        let mut tracer = Tracer::new();
        let tracee = vec![MustTrace::new(); 25];
        let tra_slice: &[MustTrace] = &tracee[..];
        tracer.add_target(tra_slice);
    }
    #[test]
    fn trace_noops() {
        use std::mem::transmute;

        let mut tracer = Tracer::new();

        macro_rules! test_noop_run {
            ($($T:ty, $val:expr)+) => {
                $(
                    let t: $T = $val;
                    tracer.add_target(&t);
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
        tracer.add_target(&t);
    }
}
