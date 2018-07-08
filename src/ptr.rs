use std::cell::Cell;
use std::cell::RefCell;
use std::ptr::NonNull;
use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Rc;
use UntypedGcBox;

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

    fn tracker(&self) -> LifeTracker {
        if !self.coroner.is_tracking() {
            self.coroner.track();
        }
        self.coroner.tracker()
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
    fn track(&self) {
        *self.0.borrow_mut() = Some(LifeTracker::new());
    }

    fn is_tracking(&self) -> bool {
        self.0.borrow().is_some()
    }

    fn tracker(&self) -> LifeTracker {
        self.0.borrow().as_ref().expect("was not tracking").clone()
    }
}

struct LifeTracker(Rc<Cell<bool>>);
impl LifeTracker {
    fn new() -> LifeTracker {
        LifeTracker(Rc::new(Cell::new(true)))
    }
    fn is_alive(&self) -> bool {
        self.0.get()
    }

    fn dead(&self) {
        self.0.set(false);
    }
}
impl Clone for LifeTracker {
    fn clone(&self) -> Self {
        LifeTracker(self.0.clone())
    }
}

pub struct GcRef<'arena, T: 'arena> {
    _marker: PhantomData<&'arena T>,
    ptr: NonNull<GcBox<T>>,
}
impl<'a, T: 'a> GcRef<'a, T> {
    pub(crate) fn from_raw_nonnull(
        ptr: NonNull<GcBox<T>>,
        _marker: PhantomData<&'a T>,
    ) -> GcRef<'a, T> {
        GcRef {
            _marker,
            ptr,
        }
    }

    unsafe fn get_gc_box<'t>(&'t self) -> &'t GcBox<T> {
        // This is fine because as long as there is a Gc the pointer to the data
        // should be valid
        self.ptr.as_ref() 
    }
}

impl<'a, T: 'a> Clone for GcRef<'a, T> {
    fn clone(&self) -> Self {
        GcRef {
            _marker: self._marker,
            ptr: self.ptr.clone(),
        }
    }
}

pub struct Gc<'arena, T: 'arena> {
    ptr: GcRef<'arena, T>,
    life_tracker: LifeTracker,
}
impl<'a, T: 'a> Gc<'a, T> {
    pub(crate) fn from_raw_gcref(
        gc_ref: GcRef<'a, T>,
        ) -> Gc<'a, T> {
        let gc = Gc {
            life_tracker: unsafe{ gc_ref.get_gc_box().tracker() },
            ptr: gc_ref,
        };
        Gc::get_gc_box(&gc).incr_ref();
        gc
    }
    pub(crate) fn from_raw_nonnull(
        ptr: NonNull<GcBox<T>>,
        _marker: PhantomData<&'a T>,
        ) -> Gc<'a, T> {
        Gc::from_raw_gcref(GcRef::from_raw_nonnull(ptr, _marker))
    }
    pub fn is_alive(this: &Self) -> bool {
        this.life_tracker.is_alive()
    }
    pub fn get(this: &Self) -> Option<&T> {
        if Self::is_alive(this) {
            Some(this.get_gc_box().borrow())
        } else {
            None
        }
    }
    fn get_gc_box(&self) -> &GcBox<T> {
        // This is fine because as long as there is a Gc the pointer to the data
        // should be valid (unless we are in the `sweep` phase, in which case
        // this isn't called when dead)
        unsafe { self.ptr.get_gc_box() }
    }
    pub(crate) fn ref_count(this: &Gc<'a, T>) -> usize {
        Gc::get_gc_box(this).ref_count()
    }
    pub fn downgrade(this: &Gc<'a, T>) -> Weak<'a, T> {
        Weak {
            life_tracker: this.life_tracker.clone(),
            ptr: this.ptr.clone(),
        }
    }
    fn get_borrow(&self) -> &T {
        Self::get(self).expect("safe pointer was already dead")
    }
    pub(crate) fn box_ptr(&self) -> Option<NonNull<GcBox<T>>> {
        if Self::is_alive(self) {
            Some(self.ptr.ptr)
        } else {
            None
        }
    }
}
impl<'a, T: 'a> Drop for Gc<'a, T> {
    fn drop(&mut self) {
        if Self::is_alive(self) {
            Gc::get_gc_box(self).decr_ref();
        }
    }
}
impl<'a, T: 'a> Deref for Gc<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get_borrow()
    }
}
impl<'a, T: 'a> Clone for Gc<'a, T> {
    fn clone(&self) -> Self {
        Gc::get_gc_box(self).incr_ref();
        Gc {
            ptr: self.ptr.clone(),
            life_tracker: self.life_tracker.clone(),
        }
    }
}

/// Impls that aren't part of the core functionality of the struct, but
/// are implemented since it is a smart pointer
mod gc_impls {
    use super::Gc;
    use std::cmp::Ordering;
    use std::fmt;
    use std::hash::{Hasher, Hash};
    use std::borrow;

    impl<'a, T: 'a + fmt::Debug> fmt::Debug for Gc<'a, T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match Self::get(self) {
                Some(value) => {
                    f.debug_struct("Gc")
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

                    f.debug_struct("Gc")
                        .field("value", &DeadPlaceholder)
                        .finish()
                }
            }
        }
    }

    impl<'a, T: 'a> AsRef<T> for Gc<'a, T> {
        fn as_ref(&self) -> &T {
            &**self
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
            (*self.get_borrow()).partial_cmp(other.get_borrow())
        }
        #[inline(always)]
        fn lt(&self, other: &Gc<'a, T>) -> bool {
            *self.get_borrow() < *other.get_borrow()
        }
        #[inline(always)]
        fn le(&self, other: &Gc<'a, T>) -> bool {
            *self.get_borrow() <= *other.get_borrow()
        }
        #[inline(always)]
        fn gt(&self, other: &Gc<'a, T>) -> bool {
            *self.get_borrow() > *other.get_borrow()
        }
        #[inline(always)]
        fn ge(&self, other: &Gc<'a, T>) -> bool {
            *self.get_borrow() >= *other.get_borrow()
        }
    }
    impl<'a, T: 'a + Ord> Ord for Gc<'a, T> {
        #[inline]
        fn cmp(&self, other: &Gc<'a, T>) -> Ordering {
            (*self.get_borrow()).cmp(other.get_borrow())
        }
    }

}


pub struct Weak<'arena, T: 'arena> {
    life_tracker: LifeTracker,
    ptr: GcRef<'arena, T>,
}

impl<'a, T: 'a> Weak<'a, T> {
    pub fn upgrade(&self) -> Option<Gc<'a, T>> {
        if self.life_tracker.is_alive() {
            Some(Gc::from_raw_gcref(self.ptr.clone()))
        } else {
            None
        }
    }

    pub fn is_alive(&self) -> bool {
        self.life_tracker.is_alive()
    }

    fn get(&self) -> Option<&T> {
        if self.life_tracker.is_alive() {
            // Unsafe is fine because if we are alive the pointer is valid
            let gc_ref = unsafe { self.ptr.get_gc_box() };
            Some(gc_ref.borrow())
        } else {
            None
        }
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
                life_tracker: self.life_tracker.clone(),
                ptr: self.ptr.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use Collector;
    use Proxy;

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
        col.run_with_gc(body);
    }

    #[test]
    fn casting_weak() {
        use traceable::NoTrace;
        let mut col = Collector::new();
        let body = |mut proxy: Proxy| {
            let num = proxy.store(NoTrace(Cell::new(0)));
            let num_weak = Gc::downgrade(&num);
            {
                let num_ref = num_weak.upgrade().unwrap();
                num_ref.0.set(num_ref.0.get() + 1);
            }
            let num = num_weak.upgrade().unwrap();
            assert_eq!(num.0.get(), 1);
        };

        col.run_with_gc(body);
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
            assert!(!num_weak.is_alive());
        };

        col.run_with_gc(body);
    }

    #[test]
    fn gc_knows_when_dangling() {
        let mut col = Collector::new();
        let body = |mut proxy: Proxy| {
            let num_safe = eat_stack_and_exec(10, || {
                let num = proxy.store(0);
                Gc::get_gc_box(&num).decr_ref();
                num
            });

            proxy.run();
            assert_eq!(proxy.num_tracked(), 0);
            assert!(Gc::get(&num_safe).is_none());
        };

        col.run_with_gc(body);
    }

    #[test]
    #[should_panic]
    fn panic_when_deref_dangling_safe() {
        let mut col = Collector::new();
        col.run_with_gc(|mut proxy| {
            let num = proxy.store(0);
            Gc::get_gc_box(&num).decr_ref();

            proxy.run();
            assert_eq!(proxy.num_tracked(), 0);
            *num
        });
    }

    // #[test]
    // fn store_unsized_types() {
    //     // TODO
    // }
    #[test]
    fn variance_works() {
        // Check compile-test for cases that are illegal
        fn _variant_with_gc() {
            fn _expect<'a>(_: &'a i32, _: Gc<&'a i32>) { unimplemented!() }
            fn _provide(m: Gc<&'static i32>) { let val = 13; _expect(&val, m); }
        }

        fn _variant_with_weak() {
            fn _expect<'a>(_: &'a i32, _: Weak<&'a i32>) { unimplemented!() }
            fn _provide(m: Weak<&'static i32>) { let val = 13; _expect(&val, m); }
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

            // Deref
            assert_eq!(1, *one);
            // Clone
            assert_eq!(1, *Gc::get(&one).unwrap());
            // Debug
            let one_debug = format!("{:?}", one);
            assert!(one_debug.contains("Gc"));
            assert!(one_debug.contains(&format!("{:?}", 1)));
            // AsRef
            assert_eq!(1, *one.as_ref());
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

        col.run_with_gc(body);
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
            assert_eq!(1, *one.clone().upgrade().unwrap());
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

        col.run_with_gc(body);
    }

}
