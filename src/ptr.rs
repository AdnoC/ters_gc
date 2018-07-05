use std::cell::Cell;
use std::cell::RefCell;
use std::ptr::NonNull;
use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Rc;
use UntypedGcBox;

/// TODO: Implement traits:
/// Clone | s
/// Debug | s
/// PartialEq | s
/// Eq | s
/// PartialOrd | s
/// Ord | s

/// TODO: Send & Sync safety


pub(crate) struct GcBox<T> {
    refs: Cell<usize>,
    coroner: Coroner,
    val: T, // TODO: Why does this fail if it is first in list?
}

impl<T> GcBox<T> {
    pub fn new(val: T) -> GcBox<T> {
        GcBox {
            val,
            refs: Cell::new(0),
            coroner: Coroner::new(),
        }
    }
    pub fn reclaim_value(self) -> T {
        self.val
    }
}
impl<T> GcBox<T> {
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

    fn tracking_ref(&self) -> TrackingRef<T> {
        if !self.coroner.is_tracking() {
            let self_ptr = self as *const _;
            self.coroner.track(self_ptr);
        }
        let tracker = self.coroner.tracker();
        TrackingRef(tracker, PhantomData)
    }
}

struct Coroner(RefCell<Option<LifeTracker>>);
impl Drop for Coroner {
    fn drop(&mut self) {
        if let Some(ref tracker) = *self.0.borrow() {
            tracker.dead();
        }
    }
}
impl Coroner {
    fn new() -> Coroner {
        Coroner(RefCell::new(None))
    }
    fn track<T>(&self, target: *const GcBox<T>) { // FIXME NonNull
        *self.0.borrow_mut() = Some(LifeTracker::new(target));
    }

    fn is_tracking(&self) -> bool {
        self.0.borrow().is_some()
    }

    fn tracker(&self) -> LifeTracker {
        self.0.borrow().as_ref().expect("was not tracking").clone()
    }
}

struct LifeTracker(Rc<TrackingInfo>);
impl LifeTracker {
    fn new<T>(target: *const GcBox<T>) -> LifeTracker { // FIXME NonNull
        LifeTracker(Rc::new(TrackingInfo {
            alive: Cell::new(true),
            target: target as *const _,
        }))
    }
    fn is_alive(&self) -> bool {
        self.0.alive.get()
    }

    fn dead(&self) {
        self.0.alive.set(false);
    }
}
impl Clone for LifeTracker {
    fn clone(&self) -> Self {
        LifeTracker(self.0.clone())
    }
}
#[derive(Clone)]
struct TrackingInfo {
    alive: Cell<bool>,
    // Type erased so that variance works?
    target: *const UntypedGcBox, // FIXME NonNull
}

struct TrackingRef<T>(LifeTracker, PhantomData<T>);
impl<T> TrackingRef<T> {
    fn is_alive(&self) -> bool {
        self.0.is_alive()
    }
    fn get(&self) -> Option<*const GcBox<T>> { // FIXME NonNull
        if self.is_alive() {
            let target_untyped: *const UntypedGcBox = (self.0).0.target;
            // let target_typed: *const GcBox<T> = target_untyped as *const _;
            Some(unsafe { ::std::mem::transmute(target_untyped) }) // FIXME NonNull
        } else {
            None
        }
    }
}
// Not sure why derive(Clone) didn't work
// Maybe a PhantomData thing?
impl<T> Clone for TrackingRef<T> {
    fn clone(&self) -> Self {
        TrackingRef(self.0.clone(), self.1.clone())
    }
}

// #[derive(PartialEq, Eq, Hash)] // Debug? Should `Clone` be done manually? // FIXME Delete
pub struct Gc<'arena, T: 'arena> {
    _marker: PhantomData<&'arena T>,
    ptr: NonNull<GcBox<T>>, // TODO Make NonNull<GcBox<T>>
}


impl<'a, T: 'a> Gc<'a, T> {
    pub(crate) fn from_raw_nonnull(
        ptr: NonNull<GcBox<T>>,
        _marker: PhantomData<&'a T>,
    ) -> Gc<'a, T> {
        let gc = Gc {
            _marker,
            ptr,
        };
        Gc::get_gc_box(&gc).incr_ref();
        gc
    }
    pub(crate) fn from_raw(
        ptr: *mut GcBox<T>,
        _marker: PhantomData<&'a T>,
    ) -> Gc<'a, T> {
        Self::from_raw_nonnull(
            NonNull::new(ptr).expect("created Gc from null ptr"),
            _marker
        )
    }

    fn get_gc_box<'t>(this: &'t Gc<'a, T>) -> &'t GcBox<T> {
        unsafe { this.ptr.as_ref() }
    }
    pub(crate) fn ref_count(this: &Gc<'a, T>) -> usize {
        Gc::get_gc_box(this).ref_count()
    }
    pub(crate) fn box_ptr(this: &Gc<'a, T>) -> NonNull<GcBox<T>> {
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
impl<'a, T: 'a> Deref for Gc<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        Gc::get_gc_box(self).borrow()
    }
}
impl<'a, T: 'a> Drop for Gc<'a, T> {
    fn drop(&mut self) {
        Gc::get_gc_box(self).decr_ref();
    }
}
/// Impls that aren't part of the core functionality of the struct, but
/// are implemented since it is a smart pointer
mod gc_impls {
    use super::Gc;
    use std::hash::{Hasher, Hash};
    use std::cmp::Ordering;
    use std::fmt;
    use std::borrow;
    // impl functions are marked inline when they are for `Rc`

    impl<'a, T: 'a> Clone for Gc<'a, T> {
        fn clone(&self) -> Self {
            let gc = Gc {
                _marker: self._marker.clone(),
                ptr: self.ptr,
            };
            Gc::get_gc_box(&gc).incr_ref();
            gc
        }
    }
    impl<'a, T: 'a> AsRef<T> for Gc<'a, T> {
        fn as_ref(&self) -> &T {
            &**self
        }
    }
    impl<'a, T: 'a + fmt::Debug> fmt::Debug for Gc<'a, T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            fmt::Debug::fmt(&**self, f)
        }
    }
    impl<'a, T: 'a + fmt::Display> fmt::Display for Gc<'a, T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            fmt::Display::fmt(&**self, f)
        }
    }
    impl<'a, T: 'a> fmt::Pointer for Gc<'a, T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            fmt::Pointer::fmt(&(&**self as *const T), f)
        }
    }
    impl<'a, T: 'a + Hash> Hash for Gc<'a, T> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            (**self).hash(state)
        }
    }
    impl<'a, T: 'a> borrow::Borrow<T> for Gc<'a, T> {
        fn borrow(&self) -> &T {
            &**self
        }
    }
    impl<'a, T: 'a + PartialEq> PartialEq for Gc<'a, T> {
        #[inline(always)]
        fn eq(&self, other: &Gc<'a, T>) -> bool {
            **self == **other
        }
        #[inline(always)]
        fn ne(&self, other: &Gc<'a, T>) -> bool {
            **self != **other
        }
    }
    impl<'a, T: 'a + Eq> Eq for Gc<'a, T> {}
    impl<'a, T: 'a + PartialOrd> PartialOrd for Gc<'a, T> {
        #[inline(always)]
        fn partial_cmp(&self, other: &Gc<'a, T>) -> Option<Ordering> {
            (**self).partial_cmp(&**other)
        }
        #[inline(always)]
        fn lt(&self, other: &Gc<'a, T>) -> bool {
            **self < **other
        }
        #[inline(always)]
        fn le(&self, other: &Gc<'a, T>) -> bool {
            **self <= **other
        }
        #[inline(always)]
        fn gt(&self, other: &Gc<'a, T>) -> bool {
            **self > **other
        }
        #[inline(always)]
        fn ge(&self, other: &Gc<'a, T>) -> bool {
            **self >= **other
        }
    }
    impl<'a, T: 'a + Ord> Ord for Gc<'a, T> {
        #[inline]
        fn cmp(&self, other: &Gc<'a, T>) -> Ordering {
            (**self).cmp(&**other)
        }
    }
}

pub struct Weak<'arena, T: 'arena> {
    _marker: PhantomData<*const &'arena ()>, // TODO: Is this the right PhantomData type?
    weak_ptr: TrackingRef<T>,
}

impl<'a, T: 'a> Weak<'a, T> {
    pub fn upgrade(&self) -> Option<Gc<'a, T>> {
        self.weak_ptr
            .get()
            .map(|gc_box| Gc::from_raw(gc_box as *mut _, PhantomData)) // FIXME NonNull conversion
    }

    pub fn is_alive(&self) -> bool {
        self.weak_ptr.is_alive()
    }
    pub fn get(&self) -> Option<&T> {
        self.weak_ptr
            .get()
            .map(|gc_box| unsafe { (*gc_box).borrow() })
    }
    fn get_borrow(&self) -> &T {
        self.get().expect("weak pointer was already dead")
    }
}

/// Impls that aren't part of the core functionality of the struct, but
/// are implemented since it is a smart pointer
mod weak_impls {
    use super::Weak;
    use std::cmp::Ordering;
    use std::fmt;

    impl<'a, T: 'a> Clone for Weak<'a, T> {
        fn clone(&self) -> Self {
            Weak {
                _marker: self._marker.clone(),
                weak_ptr: self.weak_ptr.clone(),
            }
        }
    }
    impl<'a, T: 'a + fmt::Debug> fmt::Debug for Weak<'a, T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self.get() {
                Some(value) => {
                    f.debug_struct("Weak")
                        .field("value", value)
                        .finish()
                },
                None => {
                    struct DeadPlaceholder;

                    impl fmt::Debug for DeadPlaceholder {
                        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                            f.write_str("<dead>")
                        }
                    }

                    f.debug_struct("Weak")
                        .field("value", &DeadPlaceholder)
                        .finish()
                }
            }
        }
    }
    impl<'a, T: 'a + PartialEq> PartialEq for Weak<'a, T> {
        #[inline(always)]
        fn eq(&self, other: &Weak<'a, T>) -> bool {
            *self.get_borrow() == *other.get_borrow()
        }
        #[inline(always)]
        fn ne(&self, other: &Weak<'a, T>) -> bool {
            *self.get_borrow() != *other.get_borrow()
        }
    }
    impl<'a, T: 'a + Eq> Eq for Weak<'a, T> {}
    impl<'a, T: 'a + PartialOrd> PartialOrd for Weak<'a, T> {
        #[inline(always)]
        fn partial_cmp(&self, other: &Weak<'a, T>) -> Option<Ordering> {
            (*self.get_borrow()).partial_cmp(other.get_borrow())
        }
        #[inline(always)]
        fn lt(&self, other: &Weak<'a, T>) -> bool {
            *self.get_borrow() < *other.get_borrow()
        }
        #[inline(always)]
        fn le(&self, other: &Weak<'a, T>) -> bool {
            *self.get_borrow() <= *other.get_borrow()
        }
        #[inline(always)]
        fn gt(&self, other: &Weak<'a, T>) -> bool {
            *self.get_borrow() > *other.get_borrow()
        }
        #[inline(always)]
        fn ge(&self, other: &Weak<'a, T>) -> bool {
            *self.get_borrow() >= *other.get_borrow()
        }
    }
    impl<'a, T: 'a + Ord> Ord for Weak<'a, T> {
        #[inline]
        fn cmp(&self, other: &Weak<'a, T>) -> Ordering {
            (*self.get_borrow()).cmp(other.get_borrow())
        }
    }
}

#[derive(Clone)]
pub struct Safe<'arena, T: 'arena> {
    _gc_marker: Option<Gc<'arena, T>>,
    ptr: Weak<'arena, T>,
}
impl<'a, T: 'a> Safe<'a, T> {
    pub fn to_unsafe(mut this: Safe<'a, T>) -> Gc<'a, T> {
        use std::mem::replace;
        let gc = replace(&mut this._gc_marker, None);
        gc.expect("convecsion from invalid Safe")
    }
    pub fn get(&self) -> Option<&T> {
        self.ptr.get()
    }
    fn get_borrow(&self) -> &T {
        self.get().expect("safe pointer was already dead")
    }
    pub fn is_alive(&self) -> bool {
        self.ptr.is_alive()
    }
    pub(crate) fn box_ptr(&self) -> Option<NonNull<GcBox<T>>> {
        if self.is_alive() {
            self._gc_marker.as_ref().map(|gc| Gc::box_ptr(gc)) // FIXME NonNull conversion
        } else {
            None
        }
    }
}
impl<'a, T: 'a> Drop for Safe<'a, T> {
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

/// Impls that aren't part of the core functionality of the struct, but
/// are implemented since it is a smart pointer
mod safe_impls {
    use super::Safe;
    use std::cmp::Ordering;
    use std::fmt;

    impl<'a, T: 'a + fmt::Debug> fmt::Debug for Safe<'a, T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self.get() {
                Some(value) => {
                    f.debug_struct("Safe")
                        .field("value", value)
                        .finish()
                },
                None => {
                    struct DeadPlaceholder;

                    impl fmt::Debug for DeadPlaceholder {
                        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                            f.write_str("<dead>")
                        }
                    }

                    f.debug_struct("Safe")
                        .field("value", &DeadPlaceholder)
                        .finish()
                }
            }
        }
    }
    impl<'a, T: 'a + PartialEq> PartialEq for Safe<'a, T> {
        #[inline(always)]
        fn eq(&self, other: &Safe<'a, T>) -> bool {
            *self.get_borrow() == *other.get_borrow()
        }
        #[inline(always)]
        fn ne(&self, other: &Safe<'a, T>) -> bool {
            *self.get_borrow() != *other.get_borrow()
        }
    }
    impl<'a, T: 'a + Eq> Eq for Safe<'a, T> {}
    impl<'a, T: 'a + PartialOrd> PartialOrd for Safe<'a, T> {
        #[inline(always)]
        fn partial_cmp(&self, other: &Safe<'a, T>) -> Option<Ordering> {
            (*self.get_borrow()).partial_cmp(other.get_borrow())
        }
        #[inline(always)]
        fn lt(&self, other: &Safe<'a, T>) -> bool {
            *self.get_borrow() < *other.get_borrow()
        }
        #[inline(always)]
        fn le(&self, other: &Safe<'a, T>) -> bool {
            *self.get_borrow() <= *other.get_borrow()
        }
        #[inline(always)]
        fn gt(&self, other: &Safe<'a, T>) -> bool {
            *self.get_borrow() > *other.get_borrow()
        }
        #[inline(always)]
        fn ge(&self, other: &Safe<'a, T>) -> bool {
            *self.get_borrow() >= *other.get_borrow()
        }
    }
    impl<'a, T: 'a + Ord> Ord for Safe<'a, T> {
        #[inline]
        fn cmp(&self, other: &Safe<'a, T>) -> Ordering {
            (*self.get_borrow()).cmp(other.get_borrow())
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

    #[test]
    fn store_unsized_types() {
        // TODO
    }
    #[test]
    fn variance_works() {
        // Check compile-test for cases that are illegal
        fn variant_with_gc() {
            fn expect<'a>(_: &'a i32, _: Gc<&'a i32>) { unimplemented!() }
            fn provide(m: Gc<&'static i32>) { let val = 13; expect(&val, m); }
        }

        fn variant_with_weak() {
            fn expect<'a>(_: &'a i32, _: Weak<&'a i32>) { unimplemented!() }
            fn provide(m: Weak<&'static i32>) { let val = 13; expect(&val, m); }
        }

        fn variant_with_safe() {
            fn expect<'a>(_: &'a i32, _: Safe<&'a i32>) { unimplemented!() }
            fn provide(m: Safe<&'static i32>) { let val = 13; expect(&val, m); }
        }
    }

    #[test]
    fn std_impls_gc() {
        let mut col = Collector::new();
        let body = |mut proxy: Proxy| {
            use std::cmp::Ordering;
            use std::hash::{Hash, Hasher};
            use std::borrow::Borrow;
            fn calculate_hash<H: Hash>(h: &H) -> u64 {
                use std::collections::hash_map::DefaultHasher;
                let mut s = DefaultHasher::new();
                h.hash(&mut s);
                s.finish()
            }
            fn requires_eq<E: Eq>(_e: &E) {}
            let one = proxy.store(1);
            let other_one = proxy.store(1);
            let two = proxy.store(2);
            let other_two = proxy.store(2);
            // Clone
            assert_eq!(1, *one.clone());
            // AsRef
            assert_eq!(1, *one.as_ref());
            // Debug
            assert_eq!(format!("{:?}", 1), format!("{:?}", one));
            // Display
            assert_eq!(format!("{}", 1), format!("{}", one));
            // Pointer
            assert_eq!(format!("{:p}", one), format!("{:p}", one.clone()));
            // Hash
            assert_eq!(calculate_hash(&1), calculate_hash(&one));
            // Borrow
            assert_eq!(1, *one.borrow());
            // PartialEq
            assert_eq!(one, other_one);
            assert!(one != two);
            // Eq
            requires_eq(&one);
            // PartialOrd
            assert_eq!(Some(Ordering::Less), one.partial_cmp(&two));
            assert_eq!(Some(Ordering::Equal), one.partial_cmp(&other_one));
            assert_eq!(Some(Ordering::Greater), two.partial_cmp(&one));
            assert!(one < two);
            assert!(one <= two);
            assert!(one <= other_one);
            assert!(two > one);
            assert!(two >= one);
            assert!(two >= other_two);
            // Ord
            assert_eq!(Ordering::Less, one.cmp(&two));
            assert_eq!(Ordering::Equal, one.cmp(&other_one));
            assert_eq!(Ordering::Greater, two.cmp(&one));
        };

        unsafe { col.run_with_gc(body) };
    }

    #[test]
    fn std_impls_weak() {
        let mut col = Collector::new();
        let body = |mut proxy: Proxy| {
            use std::cmp::Ordering;
            fn requires_eq<E: Eq>(_e: &E) {}
            let one = proxy.store(1);
            let other_one = proxy.store(1);
            let two = proxy.store(2);
            let other_two = proxy.store(2);

            let one = Gc::downgrade(&one);
            let other_one = Gc::downgrade(&other_one);
            let two = Gc::downgrade(&two);
            let other_two = Gc::downgrade(&other_two);
            // Clone
            assert_eq!(1, *one.clone().get().unwrap());
            // Debug
            let one_debug = format!("{:?}", one);
            assert!(one_debug.contains("Weak"));
            assert!(one_debug.contains(&format!("{:?}", 1)));
            // PartialEq
            assert_eq!(one, other_one);
            assert!(one != two);
            // Eq
            requires_eq(&one);
            // PartialOrd
            assert_eq!(Some(Ordering::Less), one.partial_cmp(&two));
            assert_eq!(Some(Ordering::Equal), one.partial_cmp(&other_one));
            assert_eq!(Some(Ordering::Greater), two.partial_cmp(&one));
            assert!(one < two);
            assert!(one <= two);
            assert!(one <= other_one);
            assert!(two > one);
            assert!(two >= one);
            assert!(two >= other_two);
            // Ord
            assert_eq!(Ordering::Less, one.cmp(&two));
            assert_eq!(Ordering::Equal, one.cmp(&other_one));
            assert_eq!(Ordering::Greater, two.cmp(&one));
        };

        unsafe { col.run_with_gc(body) };
    }

    #[test]
    fn std_impls_safe() {
        let mut col = Collector::new();
        let body = |mut proxy: Proxy| {
            use std::cmp::Ordering;
            fn requires_eq<E: Eq>(_e: &E) {}
            let one = proxy.store(1);
            let other_one = proxy.store(1);
            let two = proxy.store(2);
            let other_two = proxy.store(2);

            let one = Gc::to_safe(one);
            let other_one = Gc::to_safe(other_one);
            let two = Gc::to_safe(two);
            let other_two = Gc::to_safe(other_two);
            // Clone
            assert_eq!(1, *one.clone().get().unwrap());
            // Debug
            let one_debug = format!("{:?}", one);
            assert!(one_debug.contains("Safe"));
            assert!(one_debug.contains(&format!("{:?}", 1)));
            // PartialEq
            assert_eq!(one, other_one);
            assert!(one != two);
            // Eq
            requires_eq(&one);
            // PartialOrd
            assert_eq!(Some(Ordering::Less), one.partial_cmp(&two));
            assert_eq!(Some(Ordering::Equal), one.partial_cmp(&other_one));
            assert_eq!(Some(Ordering::Greater), two.partial_cmp(&one));
            assert!(one < two);
            assert!(one <= two);
            assert!(one <= other_one);
            assert!(two > one);
            assert!(two >= one);
            assert!(two >= other_two);
            // Ord
            assert_eq!(Ordering::Less, one.cmp(&two));
            assert_eq!(Ordering::Equal, one.cmp(&other_one));
            assert_eq!(Ordering::Greater, two.cmp(&one));
        };

        unsafe { col.run_with_gc(body) };
    }
}
