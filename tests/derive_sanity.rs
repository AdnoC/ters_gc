extern crate ters_gc;
#[macro_use]
extern crate ters_gc_derive;

use ters_gc::trace::*;
use ters_gc::*;

#[derive(Trace)]
struct GcI32<'a>((), (), Gc<'a, i32>, (), ());

#[derive(Trace)]
struct GcNewType<'a, T: 'a + Trace>(Gc<'a, T>);

struct NoTrace;

#[derive(Trace)]
struct GcWithNoTrace<'a>(Gc<'a, i32>, #[ignore_trace] NoTrace);

#[derive(Trace)]
struct GcEmpty;

#[test]
fn derive_trace_compiles() {
    let mut col = Collector::new();
    let mut proxy = col.proxy();

    {
        let num = proxy.store(5);
        let _cust_i32 = proxy.store(GcI32((), (), num.clone(), (), ()));
        let _cust_newtype = proxy.store(GcNewType(num.clone()));

        let _cust_with_no_trace = proxy.store(GcWithNoTrace(num.clone(), NoTrace));

        let _cust_empty = proxy.store(GcEmpty);
    }

    proxy.run();
}
