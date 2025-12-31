fn main() {
    wesl::Wesl::new("src/shaders").build_artifact(
        &"package::fullscreen_vertex".parse().unwrap(),
        "fullscreen_vertex",
    );
    wesl::Wesl::new("src/shaders")
        .build_artifact(&"package::canvas_render".parse().unwrap(), "canvas_render");

    wesl::Wesl::new("src/shaders").build_artifact(
        &"package::canvas_present".parse().unwrap(),
        "canvas_present",
    );
}
