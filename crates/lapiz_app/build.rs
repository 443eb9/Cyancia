use std::{
    env,
    path::{Path, PathBuf},
};

use fs_extra::dir::{self, CopyOptions};

fn main() {
    println!("cargo:rerun-if-changed=../../assets");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set by Cargo"));
    let profile_dir = out_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("OUT_DIR must be under target/<profile>/build/<package>/out");

    let assets_dir = profile_dir.join("assets");
    if assets_dir.exists() {
        dir::remove(&assets_dir).unwrap();
    }

    dir::copy(
        "../../assets",
        profile_dir,
        &CopyOptions {
            copy_inside: true,
            ..Default::default()
        },
    )
    .unwrap();
}
