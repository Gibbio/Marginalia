//! Thin wrapper around `uniffi::uniffi_bindgen_main` so the SwiftPM /
//! Xcode build script can invoke a stable `cargo run --bin uniffi-bindgen`
//! to generate language bindings from the crate's UDL.
fn main() {
    uniffi::uniffi_bindgen_main();
}
