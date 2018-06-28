
enum Never {}

#[inline]
fn round_up(base: usize, align: usize) -> usize {
    base.checked_add(align - 1).unwrap() & !(align - 1)
}
pub mod collector;
mod allocator;
mod reg_flush {
    use ::Never;
    extern {
        pub(crate) fn flush_registers_and_call(callback: extern fn(*mut Never), data: *mut Never);
    }
}

