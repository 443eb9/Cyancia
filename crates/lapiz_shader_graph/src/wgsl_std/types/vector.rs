use std::convert::identity;

use anyhow::Result;
use glam::{IVec2, IVec3, IVec4, UVec2, UVec3, UVec4, Vec2, Vec3, Vec4};
use iced_core::Element;
use iced_widget::Column;
use lapiz_render::{
    bind_group_entries::DynamicBindGroupEntries,
    bind_group_layout_entries::DynamicBindGroupLayoutEntries,
};
use lapiz_utils::random_oklch_hue_chroma;
use lapiz_widgets::spin_slider::SpinSlider;
use wgpu::{Buffer, Device, Queue};

use super::{prepare_uniform_storage, push_buffer_binding, push_storage_layout};
use crate::{
    GraphRenderer, GraphTheme,
    graph::slot::{GraphShaderStage, GraphValueType},
    save::GraphValueTypeId,
};

#[derive(Clone)]
pub struct VectorMessage<T> {
    index: usize,
    value: T,
}

macro_rules! vector_type {
    (
        $name:ident,
        $literal:ty,
        $scalar:ty,
        $len:expr,
        $id:literal,
        $wgsl:literal,
        $zero:expr,
        $range:expr,
        $component:ident
    ) => {
        #[derive(Default, Clone)]
        pub struct $name;

        impl GraphValueType for $name {
            type AssociatedLiteralType = $literal;
            type PreparedShaderType = Buffer;
            type Message = VectorMessage<$scalar>;

            fn hue_chroma(&self) -> (f32, f32) {
                random_oklch_hue_chroma!($name)
            }

            fn id(&self) -> GraphValueTypeId {
                GraphValueTypeId::new($id)
            }

            fn default_literal(&self) -> Self::AssociatedLiteralType {
                $zero
            }

            fn wgsl_type_name(&self) -> Option<&'static str> {
                Some($wgsl)
            }
            fn wgsl_array_element_stride(&self) -> Option<u64> {
                Some(super::runtime_array_stride::<$literal>())
            }

            fn push_shader_layout(
                &self,
                name: &str,
                stage: GraphShaderStage,
                group: u32,
                binding: u32,
                bindings: DynamicBindGroupLayoutEntries,
                shader: String,
            ) -> Result<(u32, DynamicBindGroupLayoutEntries, String)> {
                push_storage_layout(stage, $wgsl, name, group, binding, bindings, shader)
            }

            fn push_shader_binding<'a>(
                &self,
                _stage: GraphShaderStage,
                value: &'a Buffer,
                binding: u32,
                bindings: DynamicBindGroupEntries<'a>,
            ) -> Result<(u32, DynamicBindGroupEntries<'a>)> {
                push_buffer_binding(value, binding, bindings)
            }

            fn prepare_to_shader(
                &self,
                data: &Self::AssociatedLiteralType,
                device: &Device,
                _queue: &Queue,
            ) -> Result<Buffer> {
                prepare_uniform_storage(data, device, concat!("graph ", $wgsl, " literal"))
            }

            fn view_literal(
                &self,
                data: &Self::AssociatedLiteralType,
            ) -> Element<'static, Self::Message, GraphTheme, GraphRenderer> {
                let controls = (0..$len)
                    .map(|index| {
                        SpinSlider::new($range, data[index])
                            .on_change(move |value| VectorMessage { index, value })
                            .into()
                    })
                    .collect::<Vec<Element<'static, Self::Message, GraphTheme, GraphRenderer>>>();
                Column::with_children(controls).padding(2).into()
            }

            fn update_literal(
                &self,
                data: &mut Self::AssociatedLiteralType,
                message: Self::Message,
            ) {
                data[message.index] = message.value;
            }

            fn literal_to_code(&self, data: &Self::AssociatedLiteralType) -> Option<String> {
                let components = (0..$len)
                    .map(|index| scalar_code!($component, data[index]))
                    .collect::<Vec<_>>()
                    .join(", ");
                Some(format!("{}({components})", $wgsl))
            }
        }
    };
}

macro_rules! scalar_code {
    (f, $value:expr) => {
        format!("{:.5}", $value)
    };
    (i, $value:expr) => {
        format!("{}i", $value)
    };
    (u, $value:expr) => {
        format!("{}u", $value)
    };
}

vector_type!(
    Vec2FType,
    Vec2,
    f32,
    2,
    "vec2f",
    "vec2f",
    Vec2::ZERO,
    0.0..=1.0,
    f
);
vector_type!(
    Vec3FType,
    Vec3,
    f32,
    3,
    "vec3f",
    "vec3f",
    Vec3::ZERO,
    0.0..=1.0,
    f
);
vector_type!(
    Vec4FType,
    Vec4,
    f32,
    4,
    "vec4f",
    "vec4f",
    Vec4::ZERO,
    0.0..=1.0,
    f
);

vector_type!(
    Vec2IType,
    IVec2,
    i32,
    2,
    "vec2i",
    "vec2i",
    IVec2::ZERO,
    -10..=10,
    i
);
vector_type!(
    Vec3IType,
    IVec3,
    i32,
    3,
    "vec3i",
    "vec3i",
    IVec3::ZERO,
    -10..=10,
    i
);
vector_type!(
    Vec4IType,
    IVec4,
    i32,
    4,
    "vec4i",
    "vec4i",
    IVec4::ZERO,
    -10..=10,
    i
);

vector_type!(
    Vec2UType,
    UVec2,
    u32,
    2,
    "vec2u",
    "vec2u",
    UVec2::ZERO,
    0..=10,
    u
);
vector_type!(
    Vec3UType,
    UVec3,
    u32,
    3,
    "vec3u",
    "vec3u",
    UVec3::ZERO,
    0..=10,
    u
);
vector_type!(
    Vec4UType,
    UVec4,
    u32,
    4,
    "vec4u",
    "vec4u",
    UVec4::ZERO,
    0..=10,
    u
);
