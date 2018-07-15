//! Types needed to allow a type to be stored in the gc heap.
//!
//! A type must implement [`Trace`] to be stored in the gc heap.
//!
//! [`Trace`] is implemented for most standard library types.
//!
//! [`Trace`] lets the collector know what tracked objects an object has
//! references to. An incomplete [`Trace`] implementation will result in
//! memory leaks.
//!
//! A correct [`Trace`] implementation calls [`Tracer::add_target`] on all members
//! that can contain a [`Gc`].
//!
//! # Examples
//!
//! You can `#[derive(Trace)]` for a default implementation.
//!
//! ```
//! extern crate ters_gc;
//! #[macro_use] extern crate ters_gc_derive;
//!
//! use ters_gc::{Collector, Gc};
//!
//! #[derive(Trace)]
//! struct LinkedList<'a> {
//!     next: Option<Gc<'a, LinkedList<'a>>>,
//!     data: i32,
//! }
//!
//! let mut col = Collector::new();
//! let mut proxy = col.proxy();
//!
//! let tail = proxy.store(LinkedList {
//!     next: None,
//!     data: 0
//! });
//!
//! let head = proxy.store(LinkedList {
//!     next: Some(tail),
//!     data: 0
//! });
//! ```
//!
//! Or implement it manually.
//!
//! ```
//! use ters_gc::{Collector, Gc};
//! use ters_gc::trace::{Trace, Tracer};
//!
//! struct LinkedList<'a> {
//!     next: Option<Gc<'a, LinkedList<'a>>>,
//!     data: i32,
//! }
//!
//! impl<'a> Trace for LinkedList<'a> {
//!     fn trace(&self, tracer: &mut Tracer) {
//!         tracer.add_target(&self.next);
//!     }
//! }
//!
//! let mut col = Collector::new();
//! let mut proxy = col.proxy();
//!
//! let tail = proxy.store(LinkedList {
//!     next: None,
//!     data: 0
//! });
//!
//! let head = proxy.store(LinkedList {
//!     next: Some(tail),
//!     data: 0
//! });
//! ```
//!
//! # Excluding fields when deriving
//!
//! When deriving [`Trace`], all fields of the struct/enum must also implement
//! [`Trace`]. You can manually mark fields as non-targets of a trace with
//! the attribute `#[ignore_trace]`.
//!
//! ```
//! extern crate ters_gc;
//! #[macro_use] extern crate ters_gc_derive;
//!
//! use ters_gc::{Collector, Gc};
//!
//! // Notice that this doesn't implement `Trace`
//! struct NoTrace;
//!
//! #[derive(Trace)]
//! struct DoesTrace<'a> {
//!     #[ignore_trace]
//!     ignore1: NoTrace,
//!
//!     thing_to_trace: Option<Gc<'a, fn(i32, i32) -> i64>>,
//!
//!     #[ignore_trace]
//!     ignore2: NoTrace,
//!
//!     doesnt_matter: bool,
//!
//!     also_trace_this: Vec<Gc<'a, DoesTrace<'a>>>,
//! }
//! 
//!
//! let mut col = Collector::new();
//! let mut proxy = col.proxy();
//!
//! proxy.store(DoesTrace {
//!     ignore1: NoTrace,
//!     thing_to_trace: None,
//!     ignore2: NoTrace,
//!     doesnt_matter: false,
//!     also_trace_this: Vec::new(),
//! });
//! ```
//!
//!
//!
//!
//!
//! [`Trace`]: trait.Trace.html
//! [`Tracer::add_target`]: struct.Tracer.html#method.add_target
//! [`Gc`]: ../ptr/struct.Gc.html

use ptr::{Gc, GcBox, Weak};
use std::ptr::NonNull;
use AsUntyped;
use UntypedGcBox;

// Impls: For every object `obj` that impls Trace, call `tracer.add_entry(&obj)`.
// Can act funny if you have Sp<Gc<T>> where Sp is a smart pointer that
// doesn't impl Trace.
/// Trait all types that are stored in the gc heap must implement.
///
/// A correct implementation calls [`Tracer::add_target`] on all members that
/// can contain a Gc.
///
/// [`Tracer::add_target`]: struct.Tracer.html#method.add_target
pub trait Trace {
    /// Tell the tracer about [`Gc`] pointers
    ///
    /// [`Gc`]: ../ptr/struct.Gc.html
    fn trace(&self, _tracer: &mut Tracer);
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TraceDest(pub NonNull<UntypedGcBox>);

/// Destination for trace information.
#[derive(Debug, PartialEq, Eq)]
pub struct Tracer {
    targets: Vec<TraceDest>,
}

impl Tracer {
    pub(crate) fn new() -> Tracer {
        Tracer { targets: vec![] }
    }
    /// Add a trace target
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
        if let Some(box_ptr) = self.get_box_ptr() {
            tracer.add_box(box_ptr);
        }
    }
}
impl<'a, T> Trace for Weak<'a, T> {
    /// Noop
    #[inline]
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
                    /// Noop
                    #[inline]
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
        String
        std::cmp::Ordering
        std::ffi::CStr std::ffi::CString
        std::ffi::OsStr std::ffi::OsString
        std::fs::DirEntry std::fs::File
        std::fs::FileType std::fs::Metadata
        std::fs::OpenOptions std::fs::Permissions
        std::io::Repeat std::io::Sink
        std::io::Stderr std::io::Stdin
        std::io::Stdout std::io::ErrorKind
        std::net::Ipv4Addr std::net::Ipv6Addr
        std::net::SocketAddrV4 std::net::SocketAddrV6
        std::net::TcpStream std::net::UdpSocket
        std::net::IpAddr std::net::SocketAddr
        std::path::Path std::path::PathBuf
        std::sync::Condvar
        std::time::Duration std::time::Instant
        std::time::SystemTime
    }
    impl<'a> Trace for &'a str {
        /// Noop
        #[inline]
        fn trace(&self, _: &mut Tracer) {
            // noop
        }
    }
    macro_rules! noop_fn_impl {
        ($($T:tt)*) => {
            impl<$($T,)* R> Trace for fn($($T),*) -> R {
                /// Noop
                #[inline]
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
        /// Noop
        #[inline]
        fn trace(&self, _: &mut Tracer) {
            // noop
        }
    }
    impl<T: ?Sized> Trace for *mut T {
        /// Noop
        #[inline]
        fn trace(&self, _: &mut Tracer) {
            // noop
        }
    }

    impl<'a, T: Trace> Trace for [T] {
        /// Traces each element
        #[inline]
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
                    /// Traces each element
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
        /// Traces inner value if `Some`
        fn trace(&self, tracer: &mut Tracer) {
            if let Some(ref contents) = self {
                tracer.add_target(contents);
            }
        }
    }
    impl<T: Trace, E> Trace for Result<T, E> {
        /// Traces inner object if `Ok`
        fn trace(&self, tracer: &mut Tracer) {
            if let Ok(ref contents) = self {
                tracer.add_target(contents);
            }
        }
    }
    impl<T: Trace + ?Sized> Trace for Box<T> {
        /// Traces inner object (via deref)
        fn trace(&self, tracer: &mut Tracer) {
            let contents: &T = &*self;
            tracer.add_target(contents);
        }
    }
    impl<'a, T: Trace + 'a + ToOwned + ?Sized> Trace for std::borrow::Cow<'a, T> {
        /// Traces inner object (via deref)
        fn trace(&self, tracer: &mut Tracer) {
            tracer.add_target(&*self);
        }
    }
    impl<T: Trace> Trace for Vec<T> {
        /// Traces each element
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracer.add_target(tracee);
            }
        }
    }
    impl<T: Trace + ?Sized> Trace for std::rc::Rc<T> {
        /// Traces inner object (via deref)
        fn trace(&self, tracer: &mut Tracer) {
            let contents: &T = &*self;
            tracer.add_target(contents);
        }
    }
    impl<T: Trace + ?Sized> Trace for std::rc::Weak<T> {
        /// Noop
        #[inline]
        fn trace(&self, _: &mut Tracer) {
            // noop
        }
    }
    impl<T: Trace + ?Sized> Trace for std::sync::Arc<T> {
        /// Traces inner object (via deref)
        fn trace(&self, tracer: &mut Tracer) {
            let contents: &T = &*self;
            tracer.add_target(contents);
        }
    }
    impl<T: Trace + ?Sized> Trace for std::sync::Weak<T> {
        /// Noop
        #[inline]
        fn trace(&self, _: &mut Tracer) {
            // noop
        }
    }
    impl<T: Trace + ?Sized> Trace for std::cell::RefCell<T> {
        /// Borrows (Via `RefCell::borrow`) self and traces inner object
        fn trace(&self, tracer: &mut Tracer) {
            tracer.add_target(&*self.borrow());
        }
    }
    impl<T: Trace> Trace for std::collections::VecDeque<T> {
        /// Traces each element
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracer.add_target(tracee);
            }
        }
    }
    impl<T: Trace> Trace for std::collections::LinkedList<T> {
        /// Traces each element
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracer.add_target(tracee);
            }
        }
    }
    impl<T: Trace, K: Eq + Hash> Trace for std::collections::HashMap<K, T> {
        /// Traces each value
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self.values() {
                tracer.add_target(tracee);
            }
        }
    }
    impl<T: Trace, K: Eq + Hash> Trace for std::collections::BTreeMap<K, T> {
        /// Traces each value
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self.values() {
                tracer.add_target(tracee);
            }
        }
    }
    impl<T: Trace + Eq + Hash> Trace for std::collections::HashSet<T> {
        /// Traces each value
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracer.add_target(tracee);
            }
        }
    }
    impl<T: Trace + Eq + Hash> Trace for std::collections::BTreeSet<T> {
        /// Traces each value
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracer.add_target(tracee);
            }
        }
    }
    impl<T: Trace + Ord> Trace for std::collections::BinaryHeap<T> {
        /// Traces each value
        fn trace(&self, tracer: &mut Tracer) {
            for tracee in self {
                tracer.add_target(tracee);
            }
        }
    }
    impl<T, U> Trace for std::io::Chain<T, U> {
        /// Noop
        #[inline]
        fn trace(&self, _: &mut Tracer) {
            // noop
        }
    }
    impl<T> Trace for std::io::Cursor<T> {
        /// Noop
        #[inline]
        fn trace(&self, _: &mut Tracer) {
            // noop
        }
    }
    impl<T> Trace for std::io::Take<T> {
        /// Noop
        #[inline]
        fn trace(&self, _: &mut Tracer) {
            // noop
        }
    }
    impl<T> Trace for std::num::Wrapping<T> {
        /// Noop
        #[inline]
        fn trace(&self, _: &mut Tracer) {
            // noop
        }
    }


    // Things chosen not to implement
    // std::sync::Mutex - Not sure what behavior I want
    // std::sync::RwLock - Not sure what behavior I want
    // std::iter::* - Too many structs
    // Iterators in general - Too lazy to do all them
    // std::ops::Range* - Thats a lot of structs
    // std::any::Any - Not sure how to do this one
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

            // Unsafe used to create dummy pointers of the correct type
            // The pointers are never dereferenced
            fn(), unsafe { transmute(0usize) }
            fn() -> i8,  unsafe { transmute(0usize) }
            fn(i8) -> i8, unsafe { transmute(0usize) }
            fn(i8, u8, isize, usize) -> i8, unsafe { transmute(0usize) }
        );
        let t: &str = "Hello";
        tracer.add_target(&t);
    }
}
