use ::Never;

extern {
    pub(crate) fn flush_registers_and_call(callback: extern fn(*mut Never), data: *mut Never);
}
