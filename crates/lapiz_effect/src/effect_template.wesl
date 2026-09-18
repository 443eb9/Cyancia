import image::image_tiling::{TileInfo, TILE_SIZE};
import render::math::{Rect, IRect};

//CODEGEN_FLAG_BINDINGS
//CODEGEN_FLAG_RESOURCE_HELPERS

@if(DISPATCH_ONCE)
alias DispatchIndex = u32;
@if(DISPATCH_EVERY_BUFFER_ELEMENT)
alias DispatchIndex = u32;
@if(DISPATCH_EVERY_OUTPUT_LAYER_PIXEL || DISPATCH_EVERY_INPUT_LAYER_PIXEL)
alias DispatchIndex = vec2i;

fn graph(dispatch_index: DispatchIndex) {
    //CODEGEN_FLAG_COMPILED_GRAPH
}

@if(EVAL && (DISPATCH_ONCE || DISPATCH_EVERY_BUFFER_ELEMENT))
@compute @workgroup_size(1, 1, 1)
fn eval() {
    graph(0u);
}

@if(EVAL && (DISPATCH_EVERY_OUTPUT_LAYER_PIXEL || DISPATCH_EVERY_INPUT_LAYER_PIXEL))
@compute @workgroup_size(1, 1, 1)
fn eval() {
    graph(vec2i(0));
}

@if(!EVAL && DISPATCH_ONCE)
@compute @workgroup_size(1, 1, 1)
fn main() {
    //CODEGEN_FLAG_DISPATCH_SETUP
    graph(dispatch_index);
}

@if(!EVAL && DISPATCH_EVERY_BUFFER_ELEMENT)
@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) id: vec3u) {
    //CODEGEN_FLAG_DISPATCH_SETUP
    graph(dispatch_index);
}

@if(!EVAL && (DISPATCH_EVERY_OUTPUT_LAYER_PIXEL || DISPATCH_EVERY_INPUT_LAYER_PIXEL))
@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) id: vec3u) {
    //CODEGEN_FLAG_DISPATCH_SETUP
    graph(dispatch_index);
}
