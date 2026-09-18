use anyhow::Result;
use encase::{
    DynamicUniformBuffer, ShaderSize, ShaderType, internal::WriteInto, private::ArrayMetadata,
};
use lapiz_render::{
    bind_group_entries::DynamicBindGroupEntries,
    bind_group_layout_entries::{DynamicBindGroupLayoutEntries, binding_types},
};
use wesl::syntax::*;
use wesl_quote::quote_declaration;
use wgpu::{Buffer, BufferDescriptor, BufferUsages, Device};

use crate::graph::slot::GraphShaderStage;

mod atomic;
mod compound;
mod handle;
mod primitive;
mod vector;

pub use atomic::*;
pub use compound::*;
pub use handle::*;
pub use primitive::*;
pub use vector::*;

pub fn layer_load_ident(name: &str) -> String {
    format!("{name}_load")
}

pub fn layer_store_ident(name: &str) -> String {
    format!("{name}_store")
}

pub fn layer_tile_info_ident(name: &str) -> String {
    format!("{name}_tile_info")
}

pub fn layer_bounds_ident(name: &str) -> String {
    format!("{name}_bounds")
}

fn write_storage_buffer(device: &Device, bytes: &[u8], label: &str) -> Result<Buffer> {
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: (bytes.len() as u64).max(4),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: true,
    });
    if !bytes.is_empty() {
        buffer
            .slice(..bytes.len() as u64)
            .get_mapped_range_mut()
            .copy_from_slice(bytes);
    }
    buffer.unmap();
    Ok(buffer)
}

// The stride encase assigns to the elements of `array<T>` in a storage buffer.
// WGSL rounds each element up to its alignment, so the stride can exceed the
// element size (vec3f: size 12, stride 16); read encase's own layout instead
// of restating the rule.
fn runtime_array_stride<T>() -> u64
where
    T: ShaderType + ShaderSize,
    Vec<T>: ShaderType<ExtraMetadata = ArrayMetadata>,
{
    <Vec<T> as ShaderType>::METADATA.stride().get()
}

// Inputs bind storage read-only; outputs bind read-write for both eval and main.
fn push_storage_layout(
    stage: GraphShaderStage,
    wgsl_ty: &str,
    name: &str,
    group: u32,
    binding: u32,
    bindings: DynamicBindGroupLayoutEntries,
    mut shader: String,
) -> Result<(u32, DynamicBindGroupLayoutEntries, String)> {
    let (declaration, entry) = match stage {
        GraphShaderStage::Input => (
            quote_declaration! {
                @group(#group) @binding(#binding) var<storage, read> #name: #wgsl_ty;
            },
            binding_types::storage_buffer_read_only_sized(false, None),
        ),
        GraphShaderStage::Eval | GraphShaderStage::Main => (
            quote_declaration! {
                @group(#group) @binding(#binding) var<storage, read_write> #name: #wgsl_ty;
            },
            binding_types::storage_buffer_sized(false, None),
        ),
    };
    shader.push_str(&declaration.to_string());
    shader.push('\n');
    let bindings = bindings.extend_with_indices(((binding, entry),));
    Ok((binding + 1, bindings, shader))
}

fn push_buffer_binding<'a>(
    value: &'a Buffer,
    binding: u32,
    bindings: DynamicBindGroupEntries<'a>,
) -> Result<(u32, DynamicBindGroupEntries<'a>)> {
    let bindings = bindings.extend_with_indices(((binding, value.as_entire_binding()),));
    Ok((binding + 1, bindings))
}

fn prepare_uniform_storage<T: ShaderType + WriteInto>(
    literal: &T,
    device: &Device,
    label: &str,
) -> Result<Buffer> {
    let mut bytes = DynamicUniformBuffer::new(Vec::new());
    bytes.write(literal)?;
    write_storage_buffer(device, bytes.as_ref(), label)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use lapiz_image::texel::TexelType;

    use super::*;
    use crate::graph::slot::ErasedGraphValueType;

    // Runtime-sized array strides follow WGSL alignment rules, which round an
    // element's size up to its alignment; vec3 keeps 12 bytes of data in a
    // 16-byte slot.
    #[test]
    fn array_strides_match_wgsl_layout_rules() {
        let cases: Vec<(&str, Arc<dyn ErasedGraphValueType>, u64)> = vec![
            ("f32", Arc::new(F32Type), 4),
            ("i32", Arc::new(I32Type), 4),
            ("u32", Arc::new(U32Type), 4),
            ("bool", Arc::new(BoolType), 4),
            ("vec2f", Arc::new(Vec2FType), 8),
            ("vec3f", Arc::new(Vec3FType), 16),
            ("vec4f", Arc::new(Vec4FType), 16),
            ("vec2i", Arc::new(Vec2IType), 8),
            ("vec3i", Arc::new(Vec3IType), 16),
            ("vec4i", Arc::new(Vec4IType), 16),
            ("vec2u", Arc::new(Vec2UType), 8),
            ("vec3u", Arc::new(Vec3UType), 16),
            ("vec4u", Arc::new(Vec4UType), 16),
            ("color", Arc::new(ColorType), 16),
            ("rect", Arc::new(RectType), 16),
        ];
        for (name, ty, expected) in cases {
            assert_eq!(
                ty.wgsl_array_element_stride(),
                Some(expected),
                "wrong stride for {name}"
            );
        }

        // Handle-like and composite types have no host-shareable stride.
        assert_eq!(
            LayerType {
                texel_type: TexelType::RGBA8,
            }
            .wgsl_array_element_stride(),
            None
        );
        assert_eq!(TextureType::default().wgsl_array_element_stride(), None);
        assert_eq!(
            ArrayType {
                element_type: Arc::new(F32Type),
                len: 4,
            }
            .wgsl_array_element_stride(),
            None
        );
    }
}
