use UntypedGcBox;
use ::ptr::{Gc, Safe, GcBox};

// Impls: For every object `obj` that impls TraceTo, call `obj.trace_to(tracer)`
pub trait TraceTo {
    fn trace_to(&self, tracer: &mut Tracer);
}
struct TraceDest(*const UntypedGcBox);

pub struct Tracer {
    targets: Vec<TraceDest>,
}

impl Tracer {
    pub(crate) fn new() -> Tracer {
        Tracer {
            targets: vec![],
        }
    }
    fn add_box<T>(&mut self, gc_box: *const GcBox<T>) {
        self.targets.push(TraceDest(gc_box as *const _));
    }
}

impl<'a, T> TraceTo for Gc<'a, T> {
    fn trace_to(&self, tracer: &mut Tracer) {
        tracer.add_box(Gc::box_ptr(self));
    }
}

impl<'a, T> TraceTo for Safe<'a, T> {
    fn trace_to(&self, tracer: &mut Tracer) {
        if let Some(box_ptr) = self.box_ptr() {
            tracer.add_box(box_ptr);
        }
    }
}

mod trace_impls {
    use super::{TraceTo, Tracer};
    use std;
    use std::cmp::Eq;
    use std::hash::Hash;
    use std::cmp::Ord;


    impl<'a, T: TraceTo> TraceTo for &'a [T] {
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
            nm(), nm(), nm(), nm(), nm(),
            nm(), nm(), nm(), nm(), nm(),
            nm(), nm(), nm(), nm(), nm(),
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
}
