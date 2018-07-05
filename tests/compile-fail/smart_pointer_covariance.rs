extern crate ters_gc;
use ters_gc::ptr::{Gc, Weak, Safe};

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

fn should_not_work_gc() {
    fn expect(_: Gc<&'static i32>) { unimplemented!() }
    fn provide<'a>(m: Gc<&'a i32>) { expect(m); } //~ ERROR mismatched types
                                                  //~| lifetime mismatch
}
fn should_not_work_weak() {
    fn expect(_: Weak<&'static i32>) { unimplemented!() }
    fn provide<'a>(m: Weak<&'a i32>) { expect(m); } //~ ERROR mismatched types
                                                    //~| lifetime mismatch
}
fn should_not_work_safe() {
    fn expect(_: Safe<&'static i32>) { unimplemented!() }
    fn provide<'a>(m: Safe<&'a i32>) { expect(m); } //~ ERROR mismatched types
                                                    //~| lifetime mismatch
}
fn main() {}
