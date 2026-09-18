use anyhow::Result;
use encase::{DynamicUniformBuffer, ShaderType, internal::WriteInto};
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

fn write_storage_buffer(device: &Device, bytes: &[u8], label: &str) -> Result<Buffer> {
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: bytes.len() as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
        mapped_at_creation: true,
    });
    buffer
        .slice(..)
        .get_mapped_range_mut()
        .copy_from_slice(bytes);
    buffer.unmap();
    Ok(buffer)
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
