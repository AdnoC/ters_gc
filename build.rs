extern crate cc;

fn main() {
    cc::Build::new()
        .file("src/reg_flush_impl.c")
        .compile("reg_flush_impl");
}
