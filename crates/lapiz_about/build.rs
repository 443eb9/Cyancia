use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set by Cargo"));
    let target_dir = out_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("OUT_DIR must be under target/<profile>/build/<package>/out");
    let source = target_dir.join("about").join("third_party_licenses.json");
    println!("cargo:rerun-if-changed={}", source.display());

    let destination = out_dir.join("third_party_licenses.json");
    if source.is_file() {
        fs::copy(&source, &destination).expect("failed to copy third party licenses");
    } else {
        // Plain `cargo build` without scripts/build.py must still compile; the
        // about view renders an empty listing for the placeholder.
        fs::write(&destination, b"{\"licenses\":[],\"crates\":[]}")
            .expect("failed to write placeholder third party licenses");
    }
}
