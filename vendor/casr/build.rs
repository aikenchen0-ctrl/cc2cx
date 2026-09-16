fn main() {
    // Keep the CLI version string available when CASR is embedded as a path
    // dependency.  The previous vergen-gix build dependency pulled a large
    // Git implementation into the desktop build and required a newer Rust
    // toolchain than cc2cx supports.  Build metadata is diagnostic only, so a
    // deterministic fallback is sufficient for the library integration.
    println!("cargo:rustc-env=VERGEN_GIT_SHA=embedded");
    println!("cargo:rustc-env=VERGEN_BUILD_TIMESTAMP=unknown");
    println!(
        "cargo:rustc-env=VERGEN_CARGO_TARGET_TRIPLE={}",
        std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string())
    );
}
