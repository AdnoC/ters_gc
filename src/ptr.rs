use std::ops::Deref;
use std::marker::PhantomData;
use std::rc::Rc;
use std::cell::RefCell;
use std::cell::Cell;
pub(crate) struct GcBox<T> {
    val: T,
    coroner: Coroner<T>,
}

impl<T> GcBox<T> {
    pub fn new(val: T) -> GcBox<T> {
        GcBox {
            val,
            coroner: Coroner::new(),
        }
    }
    pub fn borrow(&self) -> &T {
        &self.val
    }
    pub fn reclaim_value(self) -> T {
        self.val
    }

    fn tracking_ref(&self) -> TrackingRef<T> {
        if !self.coroner.is_tracking() {
        let self_ptr = self as *const _;
            self.coroner.track(self_ptr);
        }
        let tracker = self.coroner.tracker();
        TrackingRef(tracker)
    }
}

struct Coroner<T> (RefCell<Option<LifeTracker<T>>>);
impl<T> Drop for Coroner<T> {
    fn drop(&mut self) {
        if let Some(ref tracker) = *self.0.borrow() {
            tracker.dead();
        }
    }
}
impl<T> Coroner<T> {
    fn new() -> Coroner<T> {
        Coroner(RefCell::new(None))
    }
    fn track(&self, target: *const GcBox<T>) {
        *self.0.borrow_mut() = Some(LifeTracker::new(target));
    }

    fn is_tracking(&self) -> bool {
        self.0.borrow().is_some()
    }

    fn tracker(&self) -> LifeTracker<T> {
        self.0.borrow().as_ref().expect("was not tracking").clone()
    }
}

struct LifeTracker<T>(Rc<TrackingInfo<T>>);
impl<T> LifeTracker<T> {
    fn new(target: *const GcBox<T>) -> LifeTracker<T> {
        LifeTracker(Rc::new(TrackingInfo {
            alive: Cell::new(true),
            target,
        }))
    }
    fn is_alive(&self) -> bool {
        self.0.alive.get()
    }

    fn dead(&self) {
        self.0.alive.set(false);
    }
}
impl<T> Clone for LifeTracker<T> {
    fn clone(&self) -> Self {
        LifeTracker(self.0.clone())
    }
}
#[derive(Clone)]
struct TrackingInfo<T> {
    alive: Cell<bool>,
    target: *const GcBox<T>,
}

#[derive(Clone)]
struct TrackingRef<T>(LifeTracker<T>);
impl<T> TrackingRef<T> {
    fn is_alive(&self) -> bool {
        self.0.is_alive()
    }
    fn get(&self) -> Option<*const GcBox<T>> {
        if self.is_alive() {
            Some((self.0).0.target)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)] // Debug? Should `Clone` be done manually?
pub struct Gc<'arena, T> {
    _marker: PhantomData<*const &'arena ()>,
    ptr: *const GcBox<T>, // TODO Make NonNull<GcBox<T>>
}

impl<'a, T> Gc<'a, T> {

    pub(crate) fn from_raw<'b>(_marker: PhantomData<*const &'b ()>, ptr: *const GcBox<T>) -> Gc<'b, T> {
        Gc {
            _marker,
            ptr,
        }
    }
    pub fn downgrade(this: &Gc<'a, T>) -> Weak<'a, T> {
        Weak {
            _marker: PhantomData,
            weak_ptr: unsafe { (*this.ptr).tracking_ref() },
        }
    }
    pub fn safe(this: &Gc<'a, T>) -> Safe<'a, T> {
        unimplemented!()
    }
}
impl<'a, T> Deref for Gc<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { (*self.ptr).borrow() }
    }
}

#[derive(Clone)]
pub struct Weak<'arena, T> {
    _marker: PhantomData<*const &'arena ()>,
    weak_ptr: TrackingRef<T>,
}

impl<'a, T> Weak<'a, T> {
    pub fn get(&self) -> Option<&T> {
        self.weak_ptr.get()
            .map(|gc_box| unsafe {  (*gc_box).borrow() })
    }
}

#[derive(Clone)]
pub struct Safe<'arena, T> {
    _gc_marker: Gc<'arena, T>,
    ptr: Weak<'arena, T>,
}
impl<'a, T> Safe<'a, T> {
    pub fn get(&self) -> Option<&T> {
        self.ptr.get()
    }
}

