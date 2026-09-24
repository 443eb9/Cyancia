use lapiz_image::image;
use lapiz_render::render;

fn main() {
    println!("cargo:rerun-if-changed=src/shaders");

    let mut shaders = wesl::Wesl::new("src/shaders");
    shaders
        .add_package(&image::PACKAGE)
        .add_package(&render::PACKAGE);

    shaders.build_artifact(
        &"package::canvas_present".parse().unwrap(),
        "canvas_present",
    );
}
