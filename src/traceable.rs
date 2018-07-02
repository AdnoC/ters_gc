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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

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
}
