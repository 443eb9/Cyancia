struct BrushMask {
    bounds: render::math::Rect,
    value: f32,
}

fn elliptical_mask(pixel: vec2f, center: vec2f, radii: vec2f, rotation: f32) -> BrushMask {
    let relative = pixel - center;
    let rotated = render::math::rotate_mat2x2(rotation) * relative;
    let extent = vec2f(max(radii.x, radii.y));
    let distance = render::math::sdf_ellipse(rotated, vec2f(0.0), radii);
    return BrushMask(
        render::math::Rect(center - extent, center + extent),
        smoothstep(1.0, 0.0, distance),
    );
}
