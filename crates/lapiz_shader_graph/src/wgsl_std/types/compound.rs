use std::{convert::identity, sync::Arc};

use anyhow::{Context, Result, bail};
use bevy_math::{IRect, IVec2, IVec4, Rect};
use encase::{DynamicUniformBuffer, ShaderType, internal::WriteInto};
use glam::{Vec2, Vec4};
use iced_core::Element;
use iced_widget::{column, space};
use lapiz_image::{
    texel::TexelType,
    tile::{DynamicLayerStorage, GpuLayerInfo},
};
use lapiz_render::{
    bind_group_entries::DynamicBindGroupEntries,
    bind_group_layout_entries::{DynamicBindGroupLayoutEntries, binding_types},
    readback::{create_readback_buffer_and_schedule_copy_buffer, readback_buffer_on_submit_async},
    util::DevicePollExt,
};
use lapiz_utils::random_oklch_hue_chroma;
use lapiz_widgets::{checkbox::Checkbox, spin_slider::SpinSlider};
use serde::{Deserialize, Serialize};
use wesl::syntax::*;
use wesl_quote::quote_expression;
use wgpu::{
    Buffer, BufferDescriptor, BufferUsages, Device, Extent3d, Queue, TextureDescriptor,
    TextureDimension, TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
};

use super::{prepare_uniform_storage, push_buffer_binding, push_storage_layout};
use crate::{
    GraphRenderer, GraphTheme,
    graph::{
        node::GraphNodeCodeGenContext,
        slot::{
            ErasedGraphValueType, GraphDefaultInputSlot, GraphDefaultOutputSlot, GraphShaderStage,
            GraphValueType,
        },
    },
    save::GraphValueTypeId,
};

#[derive(Default, Clone)]
pub struct ColorType;

#[derive(Debug, Clone)]
pub enum ColorMessage {
    R(f32),
    G(f32),
    B(f32),
    A(f32),
}

impl GraphValueType for ColorType {
    type AssociatedLiteralType = Vec4;
    type PreparedShaderType = Buffer;

    type Message = ColorMessage;

    fn hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(ColorType)
    }

    fn id(&self) -> GraphValueTypeId {
        GraphValueTypeId::new("color")
    }

    fn wgsl_array_element_stride(&self) -> Option<u64> {
        Some(super::runtime_array_stride::<Vec4>())
    }

    fn default_literal(&self) -> Self::AssociatedLiteralType {
        Vec4::ZERO
    }

    fn wgsl_type_name(&self) -> Option<&'static str> {
        Some("vec4f")
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
        push_storage_layout(stage, "vec4f", name, group, binding, bindings, shader)
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

    fn prepare_to_shader(&self, data: &Vec4, device: &Device, _queue: &Queue) -> Result<Buffer> {
        prepare_uniform_storage(data, device, "graph vec4f literal")
    }

    fn view_literal(
        &self,
        data: &Self::AssociatedLiteralType,
        _assets: &lapiz_assets::store::AssetRegistry,
    ) -> Element<'static, Self::Message, GraphTheme, GraphRenderer> {
        column![
            SpinSlider::new(0.0..=1.0, data.x)
                .on_change(ColorMessage::R)
                .allow_beyond_range(false),
            SpinSlider::new(0.0..=1.0, data.y)
                .on_change(ColorMessage::G)
                .allow_beyond_range(false),
            SpinSlider::new(0.0..=1.0, data.z)
                .on_change(ColorMessage::B)
                .allow_beyond_range(false),
            SpinSlider::new(0.0..=1.0, data.w)
                .on_change(ColorMessage::A)
                .allow_beyond_range(false),
        ]
        .padding(2)
        .into()
    }

    fn update_literal(&self, data: &mut Self::AssociatedLiteralType, message: Self::Message) {
        match message {
            ColorMessage::R(r) => data.x = r,
            ColorMessage::G(g) => data.y = g,
            ColorMessage::B(b) => data.z = b,
            ColorMessage::A(a) => data.w = a,
        }
    }

    fn literal_to_code(&self, data: &Self::AssociatedLiteralType) -> Option<Expression> {
        let (x, y, z, w): (Expression, Expression, Expression, Expression) =
            (data.x.into(), data.y.into(), data.z.into(), data.w.into());
        Some(quote_expression! { vec4f(#x, #y, #z, #w) })
    }
}

#[derive(Default, Clone)]
pub struct RectType;

#[derive(Debug, Clone)]
pub enum RectMessage {
    MinX(f32),
    MinY(f32),
    MaxX(f32),
    MaxY(f32),
}

impl GraphValueType for RectType {
    type AssociatedLiteralType = Rect;
    type PreparedShaderType = Buffer;

    type Message = RectMessage;

    fn hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(RectType)
    }

    fn id(&self) -> GraphValueTypeId {
        GraphValueTypeId::new("rect")
    }

    fn wgsl_array_element_stride(&self) -> Option<u64> {
        Some(super::runtime_array_stride::<Rect>())
    }

    fn default_literal(&self) -> Self::AssociatedLiteralType {
        Rect::default()
    }

    fn wgsl_type_name(&self) -> Option<&'static str> {
        Some("render::math::Rect")
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
        push_storage_layout(
            stage,
            "render::math::Rect",
            name,
            group,
            binding,
            bindings,
            shader,
        )
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

    fn prepare_to_shader(&self, data: &Rect, device: &Device, _queue: &Queue) -> Result<Buffer> {
        prepare_uniform_storage(data, device, "graph rect literal")
    }

    fn view_literal(
        &self,
        data: &Self::AssociatedLiteralType,
        _assets: &lapiz_assets::store::AssetRegistry,
    ) -> Element<'static, Self::Message, GraphTheme, GraphRenderer> {
        column![
            SpinSlider::new(0.0..=1.0, data.min.x).on_change(RectMessage::MinX),
            SpinSlider::new(0.0..=1.0, data.min.y).on_change(RectMessage::MinY),
            SpinSlider::new(0.0..=1.0, data.max.x).on_change(RectMessage::MaxX),
            SpinSlider::new(0.0..=1.0, data.max.y).on_change(RectMessage::MaxY),
        ]
        .padding(2)
        .into()
    }

    fn update_literal(&self, data: &mut Self::AssociatedLiteralType, message: Self::Message) {
        match message {
            RectMessage::MinX(x) => data.min.x = x,
            RectMessage::MinY(y) => data.min.y = y,
            RectMessage::MaxX(x) => data.max.x = x,
            RectMessage::MaxY(y) => data.max.y = y,
        }
    }

    fn literal_to_code(&self, data: &Self::AssociatedLiteralType) -> Option<Expression> {
        let (min_x, min_y, max_x, max_y): (Expression, Expression, Expression, Expression) = (
            data.min.x.into(),
            data.min.y.into(),
            data.max.x.into(),
            data.max.y.into(),
        );
        Some(quote_expression! {
            render::math::Rect(vec2f(#min_x, #min_y), vec2f(#max_x, #max_y))
        })
    }
}
