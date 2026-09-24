use lapiz_image::image;

fn main() {
    println!("cargo:rerun-if-changed=shaders");

    let mut compiler = wesl::Wesl::new("shaders");
    compiler.add_package(&image::PACKAGE);
    compiler.build_artifact(&"package::render".parse().unwrap(), "render");
    compiler.build_artifact(&"package::composite".parse().unwrap(), "composite");
    compiler.build_artifact(&"package::preview".parse().unwrap(), "preview");
}
