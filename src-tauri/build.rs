//! Tauri build script: generates context and capability schemas.

fn main() {
    // generate_context!() embeds icons at compile time (the macOS dev Dock icon included), but
    // tauri-build doesn't track them; without this, `make icons` leaves the old icon in the binary.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build();
}
