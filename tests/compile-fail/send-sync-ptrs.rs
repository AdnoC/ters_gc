extern crate ters_gc;
use ters_gc::{Collector, Gc};
use std::mem::drop;
use std::thread;

fn gc_not_send() {
    Collector::new().run_with_gc(|mut proxy| {
        let num = proxy.store(5);
        thread::spawn(move || { //~ ERROR Send` is not satisfied
                                //~^ ERROR Send` is not satisfied
                                //~| cannot be sent between threads safely
            drop(num);
        });
    });
}

fn weak_not_send() {
    Collector::new().run_with_gc(|mut proxy| {
        let num = proxy.store(5);
        let num = Gc::downgrade(&num);
        thread::spawn(move || { //~ ERROR Send` is not satisfied
                                //~^ ERROR Send` is not satisfied
                                //~| cannot be sent between threads safely
            drop(num);
        });
    });
}


fn gc_not_sync() {
    Collector::new().run_with_gc(|mut proxy| {
        let num = proxy.store(5);
        let num_ref = &num;
        thread::spawn(move || { //~ ERROR cannot be shared between threads safely
                                //~^ ERROR cannot be shared between threads safely
                                //~| Sync` is not implemented for 
            drop(num_ref);
        });
    });
}

fn weak_not_sync() {
    Collector::new().run_with_gc(|mut proxy| {
        let num = proxy.store(5);
        let num = Gc::downgrade(&num);
        let num_ref = &num;
        thread::spawn(move || { //~ ERROR cannot be shared between threads safely
                                //~^ ERROR cannot be shared between threads safely
                                //~| Sync` is not implemented for 
            drop(num_ref);
        });
    });
}

fn main() {}

// gc_not_send
// error[E0277]: the trait bound `std::ptr::NonNull<ptr::GcBox<i32>>: std::marker::Send` is not satisfied in `[closure@src\ptr.rs:820:27: 822:14 num:ptr::Gc<'_, i32>]`
//    --> src\ptr.rs:820:13
//     |
// 820 |             thread::spawn(move || {
//     |             ^^^^^^^^^^^^^ `std::ptr::NonNull<ptr::GcBox<i32>>` cannot be sent between threads safely
//     |
//     = help: within `[closure@src\ptr.rs:820:27: 822:14 num:ptr::Gc<'_, i32>]`, the trait `std::marker::Send` is not implemented for `std::ptr::NonNull<ptr::GcBox<i32>>`
//     = note: required because it appears within the type `ptr::GcRef<'_, i32>`
//     = note: required because it appears within the type `ptr::Gc<'_, i32>`
//     = note: required because it appears within the type `[closure@src\ptr.rs:820:27: 822:14 num:ptr::Gc<'_, i32>]`
//     = note: required by `std::thread::spawn`




// safe_not_send
// error[E0277]: the trait bound `std::rc::Rc<std::cell::Cell<bool>>: std::marker::Send` is not satisfied in `[closure@src\ptr.rs:821:23: 824:10 num:ptr::Safe<'_, i32>]`
//    --> src\ptr.rs:821:9
//     |
// 821 |         thread::spawn(move || {
//     |         ^^^^^^^^^^^^^ `std::rc::Rc<std::cell::Cell<bool>>` cannot be sent between threads safely
//     |
//     = help: within `[closure@src\ptr.rs:821:23: 824:10 num:ptr::Safe<'_, i32>]`, the trait `std::marker::Send` is not implemented for `std::rc::Rc<std::cell::Cell<bool>>`
//     = note: required because it appears within the type `ptr::LifeTracker`
//     = note: required because it appears within the type `ptr::Safe<'_, i32>`
//     = note: required because it appears within the type `[closure@src\ptr.rs:821:23: 824:10 num:ptr::Safe<'_, i32>]`
//     = note: required by `std::thread::spawn`
//
// error[E0277]: the trait bound `std::ptr::NonNull<ptr::GcBox<i32>>: std::marker::Send` is not satisfied in `[closure@src\ptr.rs:821:23: 824:10 num:ptr::Safe<'_, i32>]`
//    --> src\ptr.rs:821:9
//     |
// 821 |         thread::spawn(move || {
//     |         ^^^^^^^^^^^^^ `std::ptr::NonNull<ptr::GcBox<i32>>` cannot be sent between threads safely
//     |
//     = help: within `[closure@src\ptr.rs:821:23: 824:10 num:ptr::Safe<'_, i32>]`, the trait `std::marker::Send` is not implemented for `std::ptr::NonNull<ptr::GcBox<i32>>`
//     = note: required because it appears within the type `ptr::GcRef<'_, i32>`
//     = note: required because it appears within the type `ptr::Gc<'_, i32>`
//     = note: required because it appears within the type `std::option::Option<ptr::Gc<'_, i32>>`
//     = note: required because it appears within the type `ptr::Safe<'_, i32>`
//     = note: required because it appears within the type `[closure@src\ptr.rs:821:23: 824:10 num:ptr::Safe<'_, i32>]`
//     = note: required by `std::thread::spawn`


// gc_not_sync
// error[E0277]: `std::ptr::NonNull<ptr::GcBox<i32>>` cannot be shared between threads safely
//    --> src\ptr.rs:822:9
//     |
// 822 |         thread::spawn(move || {
//     |         ^^^^^^^^^^^^^ `std::ptr::NonNull<ptr::GcBox<i32>>` cannot be shared between threads safely
//     |
//     = help: within `ptr::Gc<'_, i32>`, the trait `std::marker::Sync` is not implemented for `std::ptr::NonNull<ptr::GcBox<i32>>`
//     = note: required because it appears within the type `ptr::GcRef<'_, i32>`
//     = note: required because it appears within the type `ptr::Gc<'_, i32>`
//     = note: required because of the requirements on the impl of `std::marker::Send` for `&ptr::Gc<'_, i32>`
//     = note: required because it appears within the type `[closure@src\ptr.rs:822:23: 824:10 num_ref:&ptr::Gc<'_, i32>]`
//     = note: required by `std::thread::spawn`


// safe_not_sync
// error[E0277]: `std::rc::Rc<std::cell::Cell<bool>>` cannot be shared between threads safely
//    --> src\ptr.rs:821:9
//     |
// 821 |         thread::spawn(move || {
//     |         ^^^^^^^^^^^^^ `std::rc::Rc<std::cell::Cell<bool>>` cannot be shared between threads safely
//     |
//     = help: within `ptr::Safe<'_, i32>`, the trait `std::marker::Sync` is not implemented for `std::rc::Rc<std::cell::Cell<bool>>`
//     = note: required because it appears within the type `ptr::LifeTracker`
//     = note: required because it appears within the type `ptr::Safe<'_, i32>`
//     = note: required because of the requirements on the impl of `std::marker::Send` for `&ptr::Safe<'_, i32>`
//     = note: required because it appears within the type `[closure@src\ptr.rs:821:23: 823:10 num_ref:&ptr::Safe<'_, i32>]`
//     = note: required by `std::thread::spawn`
//
// error[E0277]: `std::ptr::NonNull<ptr::GcBox<i32>>` cannot be shared between threads safely
//    --> src\ptr.rs:821:9
//     |
// 821 |         thread::spawn(move || {
//     |         ^^^^^^^^^^^^^ `std::ptr::NonNull<ptr::GcBox<i32>>` cannot be shared between threads safely
//     |
//     = help: within `ptr::Safe<'_, i32>`, the trait `std::marker::Sync` is not implemented for `std::ptr::NonNull<ptr::GcBox<i32>>`
//     = note: required because it appears within the type `ptr::GcRef<'_, i32>`
//     = note: required because it appears within the type `ptr::Gc<'_, i32>`
//     = note: required because it appears within the type `std::option::Option<ptr::Gc<'_, i32>>`
//     = note: required because it appears within the type `ptr::Safe<'_, i32>`
//     = note: required because of the requirements on the impl of `std::marker::Send` for `&ptr::Safe<'_, i32>`
//     = note: required because it appears within the type `[closure@src\ptr.rs:821:23: 823:10 num_ref:&ptr::Safe<'_, i32>]`
//     = note: required by `std::thread::spawn`
