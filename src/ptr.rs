use std::cell::Cell;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Rc;
pub(crate) struct GcBox<T> {
    val: T,
    refs: Cell<usize>,
    coroner: Coroner<T>,
}

impl<T> GcBox<T> {
    pub fn new(val: T) -> GcBox<T> {
        GcBox {
            val,
            refs: Cell::new(0),
            coroner: Coroner::new(),
        }
    }
    pub fn incr_ref(&self) {
        self.refs.set(self.refs.get() + 1);
    }
    pub fn decr_ref(&self) {
        self.refs.set(self.refs.get() - 1);
    }
    pub fn ref_count(&self) -> usize {
        self.refs.get()
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

struct Coroner<T>(RefCell<Option<LifeTracker<T>>>);
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

#[derive(PartialEq, Eq, Hash)] // Debug? Should `Clone` be done manually?
pub struct Gc<'arena, T> {
    _marker: PhantomData<*const &'arena ()>,
    ptr: *const GcBox<T>, // TODO Make NonNull<GcBox<T>>
}

impl<'a, T> Gc<'a, T> {
    pub(crate) fn from_raw<'b>(
        ptr: *const GcBox<T>,
        _marker: PhantomData<*const &'b ()>,
    ) -> Gc<'b, T> {
        let gc = Gc {
            _marker,
            ptr,
        };
        Gc::get_gc_box(&gc).incr_ref();
        gc
    }

    fn get_gc_box<'b>(this: &'b Gc<'a, T>) -> &'b GcBox<T> {
        unsafe { &* Self::box_ptr(this) }
    }
    pub(crate) fn ref_count(this: &Gc<'a, T>) -> usize {
        Gc::get_gc_box(this).ref_count()
    }
    pub(crate) fn box_ptr(this: &Gc<'a, T>) -> *const GcBox<T> {
        this.ptr
    }
    pub fn downgrade(this: &Gc<'a, T>) -> Weak<'a, T> {
        Weak {
            _marker: PhantomData,
            weak_ptr: Gc::get_gc_box(this).tracking_ref(),
        }
    }
    pub fn to_safe(this: Gc<'a, T>) -> Safe<'a, T> {
        Safe {
            _gc_marker: Some(this.clone()),
            ptr: Gc::downgrade(&this),
        }
    }

    pub fn from_safe(this: Safe<'a, T>) -> Gc<'a, T> {
        Safe::to_unsafe(this)
    }
}
impl<'a, T> Deref for Gc<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        Gc::get_gc_box(self).borrow()
    }
}
impl<'a, T> Drop for Gc<'a, T> {
    fn drop(&mut self) {
        Gc::get_gc_box(self).decr_ref();
    }
}
impl<'a, T> Clone for Gc<'a, T> {
    fn clone(&self) -> Self {
        let gc = Gc {
            _marker: PhantomData,
            ptr: self.ptr,
        };
        Gc::get_gc_box(&gc).incr_ref();
        gc
    }
}

#[derive(Clone)]
pub struct Weak<'arena, T> {
    _marker: PhantomData<*const &'arena ()>,
    weak_ptr: TrackingRef<T>,
}

impl<'a, T> Weak<'a, T> {
    pub fn upgrade(&self) -> Option<Gc<'a, T>> {
        self.weak_ptr
            .get()
            .map(|gc_box| Gc::from_raw(gc_box, PhantomData))
    }

    pub fn is_alive(&self) -> bool {
        self.weak_ptr.is_alive()
    }
    pub fn get(&self) -> Option<&T> {
        self.weak_ptr
            .get()
            .map(|gc_box| unsafe { (*gc_box).borrow() })
    }
}

#[derive(Clone)]
pub struct Safe<'arena, T> {
    _gc_marker: Option<Gc<'arena, T>>,
    ptr: Weak<'arena, T>,
}
impl<'a, T> Safe<'a, T> {
    pub fn to_unsafe(mut this: Safe<'a, T>) -> Gc<'a, T> {
        use std::mem::replace;
        let gc = replace(&mut this._gc_marker, None);
        gc.unwrap()
    }
    pub fn get(&self) -> Option<&T> {
        self.ptr.get()
    }
    pub fn is_alive(&self) -> bool {
        self.ptr.is_alive()
    }
    pub(crate) fn box_ptr(&self) -> Option<*const GcBox<T>> {
        if self.is_alive() {
            self._gc_marker.as_ref().map(|gc| Gc::box_ptr(gc))
        } else {
            None
        }
    }
}
impl<'a, T> Drop for Safe<'a, T> {
    fn drop(&mut self) {
        use std::mem::{forget, replace};
        println!("self living = {}", self.is_alive());
        if !self.is_alive() {
            println!("swapping");
            let gc = replace(&mut self._gc_marker, None);
            println!("swapped");
            forget(gc);
            println!("forget gc");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use *;

    #[inline(never)]
    fn eat_stack_and_exec<T, F: FnOnce() -> T>(recurs: usize, callback: F) -> T {
        let _nom = [22; 25];
        if recurs > 0 {
            eat_stack_and_exec(recurs - 1, callback)
        } else {
            callback()
        }
    }
    #[test]
    fn ref_count_works() {
        use std::mem::drop;
        let mut col = Collector::new();
        let body = |mut proxy: Proxy| {
            fn get_ref_num<'a, T>(gc: &Gc<'a, T>) -> usize {
                Gc::get_gc_box(gc).refs.clone().take()
            }
            let num = proxy.store(42);
            assert_eq!(get_ref_num(&num), 1);
            let num2 = num.clone();
            assert_eq!(get_ref_num(&num), 2);
            drop(num);
            assert_eq!(get_ref_num(&num2), 1);
        };
        unsafe { col.run_with_gc(body) };
    }

    #[test]
    fn casting_safe_and_weak() {
        use traceable::NoTrace;
        let mut col = Collector::new();
        let body = |mut proxy: Proxy| {
            let num = proxy.store(NoTrace(Cell::new(0)));
            let num_weak = Gc::downgrade(&num);
            {
                let num_ref = num_weak.get().unwrap();
                num_ref.0.set(num_ref.0.get() + 1);
            }
            let num = num_weak.upgrade().unwrap();
            num.0.set(num.0.get() + 1);
            let num_safe = Gc::to_safe(num);
            {
                let num_ref = num_weak.get().unwrap();
                num_ref.0.set(num_ref.0.get() + 1);
            }
            let num = Gc::from_safe(num_safe);
            assert_eq!(num.0.get(), 3);
        };

        unsafe { col.run_with_gc(body) };
    }

    #[test]
    fn weak_knows_when_dangling() {
        let mut col = Collector::new();
        let body = |mut proxy: Proxy| {
            let num_weak = eat_stack_and_exec(10, || {
                let num = proxy.store(0);
                let num_weak = Gc::downgrade(&num);
                num_weak
            });
            proxy.run();
            assert_eq!(proxy.num_tracked(), 0);
            assert!(num_weak.get().is_none());
        };

        unsafe { col.run_with_gc(body) };
    }

    #[test]
    fn safe_knows_when_dangling() {
        let mut col = Collector::new();
        let body = |mut proxy: Proxy| {
            let num_safe = eat_stack_and_exec(10, || {
                let num = proxy.store(0);
                Gc::get_gc_box(&num).decr_ref();
                let num_safe = Gc::to_safe(num);
                num_safe
            });

            proxy.run();
            assert_eq!(proxy.num_tracked(), 0);
            assert!(num_safe.get().is_none());
        };

        unsafe { col.run_with_gc(body) };
    }
}
