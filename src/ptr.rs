use std::cell::Cell;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;
use std::rc::Rc;
use trace::TraceTo;
use Proxy;

pub(crate) struct GcBox<T> {
    refs: Cell<usize>,
    weak: Cell<usize>,
    coroner: Coroner,
    val: T, // TODO: Why does this fail if it is first in list when `T: ?Sized`?
}

impl<T> GcBox<T> {
    pub fn new(val: T) -> GcBox<T> {
        GcBox {
            val,
            refs: Cell::new(0),
            weak: Cell::new(0),
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
    pub fn incr_weak(&self) {
        self.weak.set(self.weak.get() + 1);
    }
    pub fn decr_weak(&self) {
        self.weak.set(self.weak.get() - 1);
    }
    pub fn strong_count(&self) -> usize {
        self.refs.get()
    }
    pub fn weak_count(&self) -> usize {
        self.weak.get()
    }
    pub fn borrow(&self) -> &T {
        &self.val
    }
    // Unsfe due to stronger requirements than `borrow`, that it should be
    // the only active reference.
    pub unsafe fn borrow_mut(&mut self) -> &mut T {
        &mut self.val
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
        GcRef { _marker, ptr }
    }

    unsafe fn get_gc_box<'t>(&'t self) -> &'t GcBox<T> {
        // This is fine because as long as there is a Gc the pointer to the data
        // should be valid
        self.ptr.as_ref()
    }
    unsafe fn get_gc_box_mut<'t>(&'t mut self) -> &'t mut GcBox<T> {
        // This is fine because as long as there is a Gc the pointer to the data
        // should be valid
        self.ptr.as_mut()
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

/// A single-threaded garbage collected pointer.
/// 'Gc' stands for 'Garbage Collected'.
///
/// The API is meant to mirror that of [`Rc`].
///
/// The inherent methods of `Gc` are all associated functions, which means you
/// have to call them as e.g. [`Gc::downgrade(&value)`][downgrade] instead of
/// `value.downgrade()`. This avoids conflicts with the inner type `T`.
///
/// Dereferencing a `Gc` inside a destructor (impl of [`Drop::drop`]) can lead
/// to undefined behavior. If you must do so, either check that the `Gc` is still
/// alive with [`Gc::is_alive`][is_alive] first, or use dereference it using
/// [`Gc::get`][get]. Unless otherwise mentioned, using most methods inside a destructor
/// can result in undefined behavior.
///
/// The typical way of obtaining a `Gc` pointer is to call [`Proxy::store`].
///
/// [`Proxy::store`]: ../struct.Proxy.html#method.store
/// [get]: #method.get
/// [is_alive]: #method.is_alive
/// [downgrade]: #method.downgrade
/// [`Rc`]: https://doc.rust-lang.org/std/rc/struct.Rc.html
/// [`Drop::drop`]: https://doc.rust-lang.org/std/ops/trait.Drop.html#tymethod.drop
// TODO Mention reference counts?
pub struct Gc<'arena, T: 'arena> {
    ptr: GcRef<'arena, T>,
    life_tracker: LifeTracker,
}
impl<'a, T: 'a> Gc<'a, T> {
    pub(crate) fn from_raw_gcref(gc_ref: GcRef<'a, T>) -> Gc<'a, T> {
        let gc = Gc {
            life_tracker: unsafe { gc_ref.get_gc_box().tracker() },
            ptr: gc_ref,
        };
        gc.incr_ref();
        assert!(Gc::is_alive(&gc));
        gc
    }

    pub(crate) fn from_raw_nonnull(
        ptr: NonNull<GcBox<T>>,
        _marker: PhantomData<&'a T>,
    ) -> Gc<'a, T> {
        Gc::from_raw_gcref(GcRef::from_raw_nonnull(ptr, _marker))
    }

    fn incr_ref(&self) {
        assert!(Gc::is_alive(self));
        Gc::get_gc_box(self).incr_ref();
    }

    fn decr_ref(&self) {
        assert!(Gc::is_alive(self));
        Gc::get_gc_box(self).decr_ref();
    }

    /// Whether or not the object pointed to by this `Gc` is still valid and has
    /// not been freed.
    ///
    /// Allows you to check in destructors that the data a `Gc` point to has
    /// not already been reclaimed.
    ///
    /// Safe to use in destructors.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::{Collector, Gc};
    ///
    /// Collector::new().run_with_gc(|mut proxy| {
    ///     let meaning_of_life = proxy.store(42);
    ///
    ///     assert!(Gc::is_alive(&meaning_of_life));
    /// });
    /// ```
    pub fn is_alive(this: &Self) -> bool {
        this.life_tracker.is_alive()
    }

    // TODO in doc, mention safe to use during Drop::drop
    /// Safely obtain a reference to the inner value.
    ///
    /// Returns [`None`] if the pointed-to object has already been freed
    /// ([`is_alive`] is `false`)
    ///
    /// Can be used in destructors to obtain a reference to the pointed-to object
    /// if it is still valid.
    ///
    /// Safe to use in destructors.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::{Collector, Gc};
    ///
    /// Collector::new().run_with_gc(|mut proxy| {
    ///     let dk_high_score = proxy.store(1_247_700);
    ///
    ///     let score_ref = Gc::get(&dk_high_score).unwrap();
    ///
    ///     assert_eq!(*score_ref, 1_247_700);
    /// });
    /// ```
    ///
    /// [`is_alive`]: #method.is_alive
    /// [`None`]: https://doc.rust-lang.org/std/option/enum.Option.html#variant.None
    pub fn get(this: &Self) -> Option<&T> {
        if Self::is_alive(this) {
            Some(this.get_gc_box().borrow())
        } else {
            None
        }
    }

    /// Returns a mutable reference to the inner value,
    /// if the inner object is still alive and there are no other
    /// `Gc` or [`Weak`] pointers to the same object.
    ///
    /// Returns [`None`] otherwise, because it is either not safe to access the
    /// object or it is not safe to mutate a shared value.
    ///
    /// See also [`make_mut`], which will [`clone`] the inner value when it's shared.
    ///
    /// Safe to use in destructors.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::{Collector, Gc};
    ///
    /// Collector::new().run_with_gc(|mut proxy| {
    ///     let mut gpa = proxy.store(2.4);
    ///
    ///     *Gc::get_mut(&mut gpa).unwrap() = 4.0;
    ///
    ///     assert_eq!(*gpa, 4.0);
    ///
    ///     {
    ///         let other_gpa_ptr = gpa.clone();
    ///
    ///         assert!(Gc::get_mut(&mut gpa).is_none());
    ///     }
    ///
    ///     assert!(Gc::get_mut(&mut gpa).is_some());
    ///
    ///     {
    ///         let weak_other_gpa_ptr = Gc::downgrade(&gpa);
    ///
    ///         assert!(Gc::get_mut(&mut gpa).is_none());
    ///     }
    ///
    ///     assert!(Gc::get_mut(&mut gpa).is_some());
    /// });
    /// ```
    ///
    /// [`make_mut`]: #method.make_mut
    /// [`Weak`]: struct.Weak.html
    /// [`None`]: https://doc.rust-lang.org/std/option/enum.Option.html#variant.None
    /// [`clone`]: https://doc.rust-lang.org/std/clone/trait.Clone.html#tymethod.clone
    pub fn get_mut(this: &mut Self) -> Option<&mut T> {
        if Self::is_alive(this) && Self::strong_count(this) == 1 && Self::weak_count(this) == 0 {
            // This `Gc` is garunteed to be the sole strong reference to the data.
            // So, we can safely get a mut reference to the `GcBox` since there
            // is nobody else who can who can access the data.
            unsafe { Some(this.get_gc_box_mut().borrow_mut()) }
        } else {
            None
        }
    }

    pub(crate) fn get_nonnull_gc_box(&self) -> NonNull<GcBox<T>> {
        self.ptr.ptr
    }

    fn get_gc_box(&self) -> &GcBox<T> {
        assert!(Self::is_alive(self));
        // This is fine because as long as there is a Gc the pointer to the data
        // should be valid (unless we are in the `sweep` phase, in which case
        // this isn't called when dead).
        unsafe { self.ptr.get_gc_box() }
    }

    unsafe fn get_gc_box_mut(&mut self) -> &mut GcBox<T> {
        assert!(Self::is_alive(self));
        // This is fine because as long as there is a Gc the pointer to the data
        // should be valid (unless we are in the `sweep` phase, in which case
        // this isn't called when dead).
        self.ptr.get_gc_box_mut()
    }

    /// Returns `true` if the two `Gc`s point to the same value
    /// (not just values that compare as equal).
    ///
    /// Safe to use in destructors.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::{Collector, Gc};
    ///
    /// Collector::new().run_with_gc(|mut proxy| {
    ///     let nes_sales = proxy.store(61_910_000);
    ///     let same_nes_sales = nes_sales.clone();
    ///     let famicom_sales = proxy.store(61_910_000);
    ///
    ///     assert!(Gc::ptr_eq(&nes_sales, &same_nes_sales));
    ///     assert!(!Gc::ptr_eq(&nes_sales, &famicom_sales));
    /// });
    /// ```
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        this.ptr.ptr == other.ptr.ptr
    }

    /// Get the number of strong (`Gc`) pointers to this value.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::{Collector, Gc};
    ///
    /// Collector::new().run_with_gc(|mut proxy| {
    ///     let months = proxy.store(12);
    ///     let also_months = months.clone();
    ///
    ///     assert_eq!(Gc::strong_count(&months), 2);
    /// });
    /// ```
    pub fn strong_count(this: &Gc<'a, T>) -> usize {
        Gc::get_gc_box(this).strong_count()
    }

    /// Gets the number of [`Weak`] pointers to this value.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::{Collector, Gc};
    ///
    /// Collector::new().run_with_gc(|mut proxy| {
    ///     let days_in_year = proxy.store(365);
    ///     let _weak_days = Gc::downgrade(&days_in_year);
    ///
    ///     assert_eq!(Gc::weak_count(&days_in_year), 1);
    /// });
    /// ```
    ///
    /// [`Weak`]: struct.Weak.html
    pub fn weak_count(this: &Gc<'a, T>) -> usize {
        Gc::get_gc_box(this).weak_count()
    }

    /// Creates a new [`Weak`] pointer to this value.
    ///
    /// Safe to use in destructors.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::{Collector, Gc};
    ///
    /// Collector::new().run_with_gc(|mut proxy| {
    ///     let mariana_depth = proxy.store(10_994); // meters
    ///
    ///     let weak_depth = Gc::downgrade(&mariana_depth);
    /// });
    /// ```
    ///
    /// [`Weak`]: struct.Weak.html
    pub fn downgrade(this: &Gc<'a, T>) -> Weak<'a, T> {
        let weak = Weak {
            life_tracker: this.life_tracker.clone(),
            ptr: this.ptr.clone(),
        };
        weak.incr_weak();
        weak
    }

    fn get_borrow(&self) -> &T {
        Self::get(self).expect("gc pointer was already dead")
    }

    pub(crate) fn box_ptr(&self) -> Option<NonNull<GcBox<T>>> {
        if Self::is_alive(self) {
            Some(self.ptr.ptr)
        } else {
            None
        }
    }

    /// Returns the contained value, if the `Gc` is alive and has exactly one
    /// strong reference.
    ///
    /// Otherwise, an [`Err`] is returned with the same `Gc` that was passed in.
    ///
    /// This will succeed even if there are outstanding weak references.
    ///
    /// Requires access to the [`Proxy`] in order to stop tracking the object.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::{Collector, Gc};
    ///
    /// let mut col = Collector::new();
    /// let zambia_gdp = col.run_with_gc(|mut proxy| {
    ///
    ///     let zambia_2016_gdp = proxy.store(19_550_000_000); // USD
    ///
    ///     let gdp_clone = zambia_2016_gdp.clone();
    ///
    ///     let gdp_gc_again = Gc::try_unwrap(zambia_2016_gdp, &mut proxy)
    ///         .unwrap_err();
    ///
    ///     drop(gdp_clone);
    ///
    ///     Gc::try_unwrap(gdp_gc_again, &mut proxy).unwrap()
    /// });
    ///
    /// assert_eq!(zambia_gdp, 19_550_000_000);
    /// ```
    ///
    /// [`Proxy`]: ../struct.Proxy.html
    /// [`Err`]: https://doc.rust-lang.org/std/result/enum.Result.html
    // Not safe in destructor: Allocator::remove dereferences the passed ptr
    pub fn try_unwrap(this: Self, proxy: &mut Proxy<'a>) -> Result<T, Self> {
        proxy.try_remove(this)
    }
}
impl<'a, T: 'a + Clone + TraceTo> Gc<'a, T> {
    /// Makes a mutable reference into the given `Gc`.
    ///
    /// If there are other `Gc` or [`Weak`] pointers to the same value,
    /// then `make_mut` will invoke [`clone`] on the inner value to
    /// ensure unique ownership. This is also referred to as clone-on-write.
    ///
    /// Requires access to the [`Proxy`] in case a new `Gc` has to be created.
    ///
    /// See also [`get_mut`], which will fail rather than cloning.
    ///
    /// # Panics
    ///
    /// Panics if the inner object has already been freed ([`is_alive`] is
    /// `false`).
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::{Collector, Gc};
    ///
    /// Collector::new().run_with_gc(|mut proxy| {
    ///     let mut num_us_states = proxy.store(50);
    ///
    ///     let mut num_without_hawaii = num_us_states.clone();
    ///     *Gc::make_mut(&mut num_without_hawaii, &mut proxy) = 49;
    ///
    ///     assert_eq!(*num_us_states, 50);
    ///     assert_eq!(*num_without_hawaii, 49);
    ///
    ///     *Gc::make_mut(&mut num_us_states, &mut proxy) = 500;
    ///
    ///     assert_eq!(*num_us_states, 500);
    /// });
    /// ```
    ///
    /// [`Proxy`]: ../struct.Proxy.html
    /// [`is_alive`]: #method.is_alive
    /// [`get_mut`]: #method.get_mut
    /// [`Weak`]: struct.Weak.html
    /// [`clone`]: https://doc.rust-lang.org/std/clone/trait.Clone.html#tymethod.clone
    pub fn make_mut<'g>(this: &'g mut Self, proxy: &mut Proxy<'a>) -> &'g mut T {
        if !Gc::is_alive(this) {
            panic!("gc pointer was already dead");
        } else {
            // TODO Split case in 2 if I split data's destructure with GcBox's
            if Gc::strong_count(this) != 1 || Gc::weak_count(this) != 0 {
                // Clone the data into a new Gc
                *this = proxy.store((**this).clone());
            }

            // At this point this `Gc` is garunteed to be the sole strong
            // reference to the data.
            // So, we can safely get a mut reference to the `GcBox` since there
            // is nobody else who can who can access the data.
            unsafe { this.get_gc_box_mut().borrow_mut() }
        }
    }
}
impl<'a, T: 'a> Drop for Gc<'a, T> {
    /// Drops the `Gc`.
    ///
    /// This will decrement the strong reference count if the inner object
    /// is still alive.
    ///
    /// The inner value is only `drop`ped when the object is reclaimed when
    /// the gc is run.
    ///
    /// Safe to use in destructors.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::{Collector, Gc};
    /// use ters_gc::trace::{TraceTo};
    ///
    /// struct Foo;
    ///
    /// impl Drop for Foo {
    ///     fn drop(&mut self) {
    ///         println!("dropped!");
    ///     }
    /// }
    ///
    /// impl TraceTo for Foo { }
    ///
    /// Collector::new().run_with_gc(|mut proxy| {
    ///     let foo = proxy.store(Foo);
    ///     let foo2 = foo.clone();
    ///
    ///     drop(foo);
    ///
    ///     proxy.run(); // Doesn't print anything
    ///
    ///     drop(foo2);
    ///
    ///     proxy.run(); // Prints "dropped!"
    /// });
    /// ```
    fn drop(&mut self) {
        if Self::is_alive(self) {
            self.decr_ref();
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
    /// Makes a clone of the `Gc` pointer.
    ///
    /// This creates another pointer to the same inner value, increasing the
    /// strong reference count.
    ///
    /// Safe to use in destructors.
    ///
    /// # Panics
    ///
    /// Panics if the inner object has already been freed ([`is_alive`] is
    /// `false`).
    ///
    /// [`is_alive`]: #method.is_alive
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::{Collector, Gc};
    ///
    /// Collector::new().run_with_gc(|mut proxy| {
    ///     let five = proxy.store(5);
    ///     five.clone();
    /// });
    /// ```
    fn clone(&self) -> Self {
        if !Self::is_alive(self) {
            panic!("gc pointer was already dead");
        }
        self.incr_ref();
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
    use std::borrow;
    use std::cmp::Ordering;
    use std::fmt;
    use std::hash::{Hash, Hasher};

    impl<'a, T: 'a + fmt::Debug> fmt::Debug for Gc<'a, T> {
        /// Formats the value using the given formatter.
        ///
        /// Safe to use in destructors.
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match Self::get(self) {
                Some(value) => f.debug_struct("Gc").field("value", value).finish(),
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

/// `Weak` is a version of [`Gc`] that holds a non-owning reference to the managed
/// value. The value is accessed by calling [`upgrade`] on the `Weak` pointer,
/// which returns an [`Option`]`<`[`Gc`]`<'a, T>>`.
///
/// Since a `Weak` reference does not count towards ownership, it will not prevent
/// the inner value from being reclaimed during collection. `Weak` itself makes
/// no guarantees about the value still being present and may return [`None`]
/// when [`upgrade`]d.
///
/// Unlike an [`rc::Weak`] pointer, `Weak` pointers can be successfully
/// [`upgrade`]d even when there are no longer and strong [`Gc`] pointers.
/// A `Weak` pointer will remain alive even without any [`Gc`] pointers 
/// until garbage collection is run and the inner object is reclaimed.
///
/// The typical way to obtain a `Weak` pointer is to call [`Gc::downgrade`].
///
/// [`Gc`]: struct.Gc.html
/// [`Gc::downgrade`]: struct.Gc.html#method.downgrade
/// [`upgrade`]: #method.upgrade
/// [`rc::Weak`]: https://doc.rust-lang.org/std/rc/struct.Weak.html
/// [`Option`]: https://doc.rust-lang.org/std/option/enum.Option.html
/// [`None`]: https://doc.rust-lang.org/std/option/enum.Option.html#variant.None
// TODO: Note difference from Rc's Weak - Can upgrade even if strong_count == 0
// so long as inner object wasn't reclaimed
pub struct Weak<'arena, T: 'arena> {
    life_tracker: LifeTracker,
    ptr: GcRef<'arena, T>,
}

impl<'a, T: 'a> Weak<'a, T> {
    /// Attempts to upgrade the `Weak` pointer to a [`Gc`], preventing the inner
    /// value from being reclaimed if successful.
    ///
    /// Returns [`None`] if the inner value has been reclaimed.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::{Collector, Gc};
    ///
    /// Collector::new().run_with_gc(|mut proxy| {
    ///     let everest_height = proxy.store(8_848); // meters
    ///
    ///     let weak_height = Gc::downgrade(&everest_height);
    ///
    ///     let strong_height: Option<Gc<_>> = weak_height.upgrade();
    ///     assert!(strong_height.is_some());
    ///
    ///     // Destroy all strong pointers
    ///     drop(everest_height);
    ///     drop(strong_height);
    ///     // Reclaim memory
    ///     proxy.run();
    ///
    ///     assert!(weak_height.upgrade().is_none());
    /// });
    /// ```
    ///
    /// [`Gc`]: struct.Gc.html
    /// [`None`]: https://doc.rust-lang.org/std/option/enum.Option.html#variant.None
    pub fn upgrade(&self) -> Option<Gc<'a, T>> {
        if self.is_alive() {
            Some(Gc::from_raw_gcref(self.ptr.clone()))
        } else {
            None
        }
    }

    /// Returns whether the inner object has been reclaimed and freed.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::{Collector, Gc};
    ///
    /// Collector::new().run_with_gc(|mut proxy| {
    ///     let hd = proxy.store(1080);
    ///
    ///     let weak_hd = Gc::downgrade(&hd);
    ///
    ///     assert!(weak_hd.is_alive());
    ///
    ///     // Reclaim memory
    ///     drop(hd);
    ///     proxy.run();
    ///
    ///     assert!(!weak_hd.is_alive());
    /// });
    /// ```
    pub fn is_alive(&self) -> bool {
        self.life_tracker.is_alive()
    }

    fn get(&self) -> Option<&T> {
        if self.is_alive() {
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

    fn get_gc_box(&self) -> Option<&GcBox<T>> {
        if self.is_alive() {
            // Unsfe is ok since we checked that we won't be accessing freed memory
            Some(unsafe { self.ptr.get_gc_box() })
        } else {
            None
        }
    }
    fn incr_weak(&self) {
        if let Some(gc_box) = self.get_gc_box() {
            gc_box.incr_weak();
        }
    }
    fn decr_weak(&self) {
        if let Some(gc_box) = self.get_gc_box() {
            gc_box.decr_weak();
        }
    }
}
impl<'a, T: 'a> Clone for Weak<'a, T> {
    /// Makes a clone of the `Weak` pointer that points to the same value.
    ///
    /// # Examples
    ///
    /// ```
    /// use ters_gc::{Collector, Gc};
    ///
    /// Collector::new().run_with_gc(|mut proxy| {
    ///     let weak_five = Gc::downgrade(&proxy.store(5));
    ///
    ///     weak_five.clone();
    /// });
    /// ```
    fn clone(&self) -> Self {
        self.incr_weak();
        Weak {
            life_tracker: self.life_tracker.clone(),
            ptr: self.ptr.clone(),
        }
    }
}
impl<'a, T: 'a> Drop for Weak<'a, T> {
    /// Drops the `Weak` pointer.
    fn drop(&mut self) {
        self.decr_weak();
    }
}

/// Impls that aren't part of the core functionality of the struct, but
/// are implemented since it is a smart pointer
mod weak_impls {
    use super::Weak;
    use std::cmp::Ordering;
    use std::fmt;

    impl<'a, T: 'a + fmt::Debug> fmt::Debug for Weak<'a, T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self.get() {
                Some(value) => f.debug_struct("Weak").field("value", value).finish(),
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

    #[test]
    fn strong_count_works() {
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
        use trace::NoTrace;
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
            let num_weak = {
                let num = proxy.store(0);
                let num_weak = Gc::downgrade(&num);
                num_weak
            };
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
            let num_safe = {
                let num = proxy.store(0);
                Gc::get_gc_box(&num).decr_ref();
                num
            };

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
            fn _expect<'a>(_: &'a i32, _: Gc<&'a i32>) {
                unimplemented!()
            }
            fn _provide(m: Gc<&'static i32>) {
                let val = 13;
                _expect(&val, m);
            }
        }

        fn _variant_with_weak() {
            fn _expect<'a>(_: &'a i32, _: Weak<&'a i32>) {
                unimplemented!()
            }
            fn _provide(m: Weak<&'static i32>) {
                let val = 13;
                _expect(&val, m);
            }
        }
    }

    #[test]
    fn std_impls_gc() {
        let mut col = Collector::new();
        let body = |mut proxy: Proxy| {
            use std::borrow::Borrow;
            use std::cmp::Ordering;
            use std::hash::{Hash, Hasher};
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

    #[test]
    fn gc_ptr_eq() {
        Collector::new().run_with_gc(|mut proxy| {
            let num = proxy.store(0);
            let num_cl = num.clone();
            let other_num = proxy.store(0);

            assert!(Gc::ptr_eq(&num, &num_cl));
            assert!(!Gc::ptr_eq(&num, &other_num));
            assert!(!Gc::ptr_eq(&num_cl, &other_num));
        });
    }

    #[test]
    fn get_mut_only_when_lone_ref() {
        use std::mem::drop;
        Collector::new().run_with_gc(|mut proxy| {
            let mut num = proxy.store(0);
            assert!(Gc::get_mut(&mut num).is_some());

            let num_cl = num.clone();
            assert!(Gc::get_mut(&mut num).is_none());
            drop(num_cl);
            assert!(Gc::get_mut(&mut num).is_some());

            let num_w = Gc::downgrade(&num);
            assert!(Gc::get_mut(&mut num).is_none());
            drop(num_w);
            assert!(Gc::get_mut(&mut num).is_some());
        });
    }

    #[test]
    fn make_mut_when_lone() {
        Collector::new().run_with_gc(|mut proxy| {
            let mut num = proxy.store(0);
            assert_eq!(0, *num);
            {
                let num_ref = Gc::make_mut(&mut num, &mut proxy);
                {
                    // Checking that the mut ref doesn't take proxy's lifetime
                    let _ = proxy.store(0);
                }
                *num_ref = 42;
            }
            assert_eq!(42, *num);
        });
    }

    #[test]
    fn make_mut_clones_when_others() {
        use std::mem::drop;
        Collector::new().run_with_gc(|mut proxy| {
            let mut num = proxy.store(0);
            let num_cl = num.clone();
            {
                let num_ref = Gc::make_mut(&mut num, &mut proxy);
                *num_ref = 42;
            }
            assert_eq!(42, *num);
            assert_eq!(0, *num_cl);
            drop(num_cl);

            let num_w = Gc::downgrade(&num);
            {
                let num_ref = Gc::make_mut(&mut num, &mut proxy);
                *num_ref = 99;
            }
            let num_from_w = num_w.upgrade().unwrap();
            assert_eq!(99, *num);
            assert_eq!(42, *num_from_w);
        });
    }

    #[test]
    fn unwrap_ok_when_lone_or_has_weak() {
        Collector::new().run_with_gc(|mut proxy| {
            let num = proxy.store(42);
            let removed_num = Gc::try_unwrap(num, &mut proxy);
            let ok_num = removed_num.unwrap();
            assert_eq!(42, ok_num);
        });

        Collector::new().run_with_gc(|mut proxy| {
            let num = proxy.store(42);
            let weak_1 = Gc::downgrade(&num);
            let weak_2 = Gc::downgrade(&num);
            let weak_3 = Gc::downgrade(&num);
            let weak_4 = Gc::downgrade(&num);
            let weak_5 = Gc::downgrade(&num);
            let removed_num = Gc::try_unwrap(num, &mut proxy);
            let ok_num = removed_num.unwrap();
            assert_eq!(42, ok_num);

            assert!(!weak_1.is_alive());
            assert!(!weak_2.is_alive());
            assert!(!weak_3.is_alive());
            assert!(!weak_4.is_alive());
            assert!(!weak_5.is_alive());
        });
    }

    #[test]
    fn unwrap_err_when_multiple_refs() {
        Collector::new().run_with_gc(|mut proxy| {
            let num = proxy.store(42);
            let num_cl = num.clone();
            let err_num = Gc::try_unwrap(num, &mut proxy);
            assert!(err_num.is_err());
            if let Err(err_num_inner) = err_num {
                assert_eq!(42, *err_num_inner);
                assert!(Gc::ptr_eq(&err_num_inner, &num_cl));
            }
        });
    }
}
