// Everything I know about variance is from the
// talk "Subtyping in Rust and Clarke's Third Law".
// http://pnkfx.org/presentations/rustfest-berlin-2016/slides.html

extern crate ters_gc;
use ters_gc::ptr::{Gc, Weak};

fn gc_covariant() {
    fn expect<'a>(_: &'a i32, _: Gc<&'a i32>) { unimplemented!() }
    fn provide(m: Gc<&'static i32>) { let val = 13; expect(&val, m); }
}

fn weak_covariant() {
    fn expect<'a>(_: &'a i32, _: Weak<&'a i32>) { unimplemented!() }
    fn provide(m: Weak<&'static i32>) { let val = 13; expect(&val, m); }
}

fn gc_covariant_cannot_extend_lifetime() {
    fn expect(_: Gc<&'static i32>) { unimplemented!() }
    fn provide<'a>(m: Gc<&'a i32>) { expect(m); } //~ ERROR mismatched types
                                                  //~| lifetime mismatch
}
fn weak_covariant_cannot_extend_lifetime() {
    fn expect(_: Weak<&'static i32>) { unimplemented!() }
    fn provide<'a>(m: Weak<&'a i32>) { expect(m); } //~ ERROR mismatched types
                                                    //~| lifetime mismatch
}

fn _covariant_with_gc_alloc_lifetime() {
    fn _expect<'a>(_: &'a i32, _: Gc<'a, ()>) {
        unimplemented!()
    }
    fn _provide(m: Gc<'static, ()>) {
        let val = 13;
        _expect(&val, m);
    }
}
fn _covariant_with_weak_alloc_lifetime() {
    fn _expect<'a>(_: &'a i32, _: Weak<'a, ()>) {
        unimplemented!()
    }
    fn _provide(m: Weak<'static, ()>) {
        let val = 13;
        _expect(&val, m);
    }
}

fn gc_covariant_cannot_extend_alloc_lifetime() {
    fn expect(_: Gc<'static, ()>) { unimplemented!() }
    fn provide<'a>(m: Gc<'a, ()>) { expect(m); } //~ ERROR mismatched types
                                                  //~| lifetime mismatch
}
fn weak_covariant_cannot_extend_alloc_lifetime() {
    fn expect(_: Weak<'static, ()>) { unimplemented!() }
    fn provide<'a>(m: Weak<'a, ()>) { expect(m); } //~ ERROR mismatched types
                                                  //~| lifetime mismatch
}

fn main() {}

fn gc_contravariant() {
    fn provide<'g>() -> Gc<'g, &'static i32> { unimplemented!() }
    fn expect<'a>(_: &'a i32) -> Gc<&'a i32> { provide() }
}
fn weak_contravariant() {
    fn provide<'g>() -> Weak<'g, &'static i32> { unimplemented!() }
    fn expect<'a>(_: &'a i32) -> Weak<&'a i32> { provide() }
}

fn gc_contravariant_cannot_extend_lifetime() {
    fn expect<'a, 'g>(gc: Gc<'g, &'a i32>) -> Gc<'g, &'static i32> {
        gc //~ ERROR mismatched types
           //~| lifetime mismatch
    } 
}
fn weak_contravariant_cannot_extend_lifetime() {
    fn expect<'a, 'w>(wk: Weak<'w, &'a i32>) -> Weak<'w, &'static i32> {
        wk //~ ERROR mismatched types
           //~| lifetime mismatch
    } 
}

fn gc_contravariant_alloc_lifetime() {
    fn provide() -> Gc<'static, ()> { unimplemented!() }
    fn expect<'a>(_: &'a i32) -> Gc<'a, ()> { provide() }
}
fn weak_contravariant_alloc_lifetime() {
    fn provide() -> Weak<'static, ()> { unimplemented!() }
    fn expect<'a>(_: &'a i32) -> Weak<'a, ()> { provide() }
}


fn gc_contravariant_cannot_extend_alloc_lifetime() {
    fn expect<'g>(m: Gc<'g, ()>) -> Gc<'static, ()> {
        m //~ ERROR mismatched types
          //~| lifetime mismatch
    }
}
fn weak_contravariant_cannot_extend_alloc_lifetime() {
    fn expect<'g>(m: Weak<'g, ()>) -> Weak<'static, ()> {
        m //~ ERROR mismatched types
          //~| lifetime mismatch
    }
}
