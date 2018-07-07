extern crate ters_gc;
use ters_gc::{Collector, Gc};
use std::mem::drop;
use std::thread;

fn proxy_not_send() {
    Collector::new().run_with_gc(|mut proxy| {
        thread::spawn(move || { //~ ERROR Send` is not satisfied
                                //~^ ERROR Send` is not satisfied
                                //~| cannot be sent between threads safely
            drop(proxy);
        });
    });
}

fn proxy_not_sync() {
    Collector::new().run_with_gc(|mut proxy| {
            let proxy_ref = &proxy;
            thread::spawn(move || { //~ ERROR cannot be shared between threads safely
                                    //~^ ERROR cannot be shared between threads safely
                                    //~^^ ERROR cannot be shared between threads safely
                                    //~^^^ ERROR cannot be shared between threads safely
                                    //~| Sync` is not implemented
                drop(proxy_ref);
            });
    });
}

fn main() {}

// proxy_not_send
// error[E0277]: `*mut UntypedGcBox` cannot be shared between threads safely
//    --> src\lib.rs:607:13
//     |
// 607 |             thread::spawn(move || {
//     |             ^^^^^^^^^^^^^ `*mut UntypedGcBox` cannot be shared between threads safely
//     |
//     = help: within `Proxy<'_>`, the trait `std::marker::Sync` is not implemented for `*mut UntypedGcBox`
//     = note: required because it appears within the type `(*mut UntypedGcBox, allocator::AllocInfo)`
//     = note: required because it appears within the type `std::marker::PhantomData<(*mut UntypedGcBox, allocator::AllocInfo)>`
//     = note: required because it appears within the type `std::collections::hash::table::RawTable<*mut UntypedGcBox, allocator::AllocInfo>`
//     = note: required because it appears within the type `std::collections::HashMap<*mut UntypedGcBox, allocator::AllocInfo>`
//     = note: required because it appears within the type `allocator::Allocator`
//     = note: required because it appears within the type `Collector`
//     = note: required because it appears within the type `&mut Collector`
//     = note: required because it appears within the type `Proxy<'_>`
//     = note: required because of the requirements on the impl of `std::marker::Send` for `&Proxy<'_>`
//     = note: required because it appears within the type `[closure@src\lib.rs:607:27: 609:14 proxy_ref:&Proxy<'_>]`
//     = note: required by `std::thread::spawn`
//
// error[E0277]: `std::ptr::NonNull<UntypedGcBox>` cannot be shared between threads safely
//    --> src\lib.rs:607:13
//     |
// 607 |             thread::spawn(move || {
//     |             ^^^^^^^^^^^^^ `std::ptr::NonNull<UntypedGcBox>` cannot be shared between threads safely
//     |
//     = help: within `Proxy<'_>`, the trait `std::marker::Sync` is not implemented for `std::ptr::NonNull<UntypedGcBox>`
//     = note: required because it appears within the type `allocator::AllocInfo`
//     = note: required because it appears within the type `(*mut UntypedGcBox, allocator::AllocInfo)`
//     = note: required because it appears within the type `std::marker::PhantomData<(*mut UntypedGcBox, allocator::AllocInfo)>`
//     = note: required because it appears within the type `std::collections::hash::table::RawTable<*mut UntypedGcBox, allocator::AllocInfo>`
//     = note: required because it appears within the type `std::collections::HashMap<*mut UntypedGcBox, allocator::AllocInfo>`
//     = note: required because it appears within the type `allocator::Allocator`
//     = note: required because it appears within the type `Collector`
//     = note: required because it appears within the type `&mut Collector`
//     = note: required because it appears within the type `Proxy<'_>`
//     = note: required because of the requirements on the impl of `std::marker::Send` for `&Proxy<'_>`
//     = note: required because it appears within the type `[closure@src\lib.rs:607:27: 609:14 proxy_ref:&Proxy<'_>]`
//     = note: required by `std::thread::spawn`
//
// error[E0277]: `std::cell::Cell<bool>` cannot be shared between threads safely
//    --> src\lib.rs:607:13
//     |
// 607 |             thread::spawn(move || {
//     |             ^^^^^^^^^^^^^ `std::cell::Cell<bool>` cannot be shared between threads safely
//     |
//     = help: within `Proxy<'_>`, the trait `std::marker::Sync` is not implemented for `std::cell::Cell<bool>`
//     = note: required because it appears within the type `allocator::AllocInfo`
//     = note: required because it appears within the type `(*mut UntypedGcBox, allocator::AllocInfo)`
//     = note: required because it appears within the type `std::marker::PhantomData<(*mut UntypedGcBox, allocator::AllocInfo)>`
//     = note: required because it appears within the type `std::collections::hash::table::RawTable<*mut UntypedGcBox, allocator::AllocInfo>`
//     = note: required because it appears within the type `std::collections::HashMap<*mut UntypedGcBox, allocator::AllocInfo>`
//     = note: required because it appears within the type `allocator::Allocator`
//     = note: required because it appears within the type `Collector`
//     = note: required because it appears within the type `&mut Collector`
//     = note: required because it appears within the type `Proxy<'_>`
//     = note: required because of the requirements on the impl of `std::marker::Send` for `&Proxy<'_>`
//     = note: required because it appears within the type `[closure@src\lib.rs:607:27: 609:14 proxy_ref:&Proxy<'_>]`
//     = note: required by `std::thread::spawn`
//
// error[E0277]: `std::cell::Cell<usize>` cannot be shared between threads safely
//    --> src\lib.rs:607:13
//     |
// 607 |             thread::spawn(move || {
//     |             ^^^^^^^^^^^^^ `std::cell::Cell<usize>` cannot be shared between threads safely
//     |
//     = help: within `Proxy<'_>`, the trait `std::marker::Sync` is not implemented for `std::cell::Cell<usize>`
//     = note: required because it appears within the type `allocator::AllocInfo`
//     = note: required because it appears within the type `(*mut UntypedGcBox, allocator::AllocInfo)`
//     = note: required because it appears within the type `std::marker::PhantomData<(*mut UntypedGcBox, allocator::AllocInfo)>`
//     = note: required because it appears within the type `std::collections::hash::table::RawTable<*mut UntypedGcBox, allocator::AllocInfo>`
//     = note: required because it appears within the type `std::collections::HashMap<*mut UntypedGcBox, allocator::AllocInfo>`
//     = note: required because it appears within the type `allocator::Allocator`
//     = note: required because it appears within the type `Collector`
//     = note: required because it appears within the type `&mut Collector`
//     = note: required because it appears within the type `Proxy<'_>`
//     = note: required because of the requirements on the impl of `std::marker::Send` for `&Proxy<'_>`
//     = note: required because it appears within the type `[closure@src\lib.rs:607:27: 609:14 proxy_ref:&Proxy<'_>]`
//     = note: required by `std::thread::spawn`
