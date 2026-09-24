use glam::{IVec3, Vec2};
use lapiz_shader_graph::{
    graph::slot::GraphValueType as _,
    wgsl_std::types::vector::{Vec2FType, Vec3IType},
};

#[test]
fn vector_literals_render_with_arguments() {
    assert_eq!(
        Vec2FType
            .literal_to_code(&Vec2::new(1.5, 2.5))
            .map(|expression| expression.to_string()),
        Some("vec2f(1.5f, 2.5f)".to_string())
    );
    assert_eq!(
        Vec3IType
            .literal_to_code(&IVec3::new(1, 2, 3))
            .map(|expression| expression.to_string()),
        Some("vec3i(1i, 2i, 3i)".to_string())
    );
}
