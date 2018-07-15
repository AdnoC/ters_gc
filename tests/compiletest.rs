extern crate compiletest_rs as compiletest; // https://github.com/laumann/compiletest-rs

use std::path::PathBuf;

fn run_mode(mode: &'static str) {
    let mut config = compiletest::Config::default();

    // config.verbose = true; // Uncomment when compiletest misbehaves

    config.mode = mode.parse().expect("Invalid mode");

    config.src_base = PathBuf::from(format!("tests/{}", mode));
    config.link_deps(); // Populate config.target_rustcflags with dependencies on the path
    // Correctly link deps. Above line errors with "multiple input filenames provided"
    // config.target_rustcflags = Some("-L target/debug/ -L target/debug/deps/".to_owned());
    config.target_rustcflags = Some("-L target/debug/deps/".to_owned());
    config.clean_rmeta(); // If your tests import the parent crate, this helps with E0464

    compiletest::run_tests(&config);
}

#[test]
fn compile_test() {
    run_mode("compile-fail");
    // run_mode("run-pass");
}
