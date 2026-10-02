fn filter_within_bounds(pixel: vec2i, color: vec4f, bounds: render::math::Rect) -> vec4f {
    let pixelf = vec2f(pixel);
    if any(pixelf < bounds.min) || any(pixelf >= bounds.max) {
        return vec4f(0.0);
    }
    return color;
}
