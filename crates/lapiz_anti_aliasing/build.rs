use lapiz_image::image;
use wesl::Wesl;

fn main() {
    println!("cargo:rerun-if-changed=shaders");

    let mut compiler = Wesl::new("shaders");
    compiler.add_package(&image::PACKAGE);

    compiler.build_artifact(&"package::fxaa.wesl".parse().unwrap(), "fxaa");
}
