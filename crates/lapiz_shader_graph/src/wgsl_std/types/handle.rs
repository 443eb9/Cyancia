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
use lapiz_i18n::t;
use lapiz_utils::random_oklch_hue_chroma;
use lapiz_widgets::{checkbox::Checkbox, combo_box::ComboBox, spin_slider::SpinSlider};
use serde::{Deserialize, Serialize};
use wesl::syntax::*;
use wesl_quote::{quote_declaration, quote_expression, quote_statement};
use wgpu::{
    Buffer, BufferDescriptor, BufferUsages, Device, Extent3d, Queue, TextureDescriptor,
    TextureDimension, TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
};

use super::{
    ColorType, F32Type, I32Type, RectType, layer_bounds_ident, layer_load_ident, layer_store_ident,
    layer_tile_info_ident, push_buffer_binding, push_storage_layout,
};
use crate::{
    GraphRenderer, GraphTheme,
    graph::{
        node::{GraphNodeCodeGenContext, ident_expression},
        slot::{
            ErasedGraphValueType, GraphDefaultInputSlot, GraphDefaultOutputSlot, GraphShaderStage,
            GraphValueType,
        },
    },
    save::GraphValueTypeId,
};

/// ComboBox payload: an image asset handle plus its display name.
#[derive(Clone)]
pub struct TextureOption {
    pub name: String,
    pub handle: lapiz_assets::asset::AssetHandle<lapiz_render::texture::Image>,
}

impl std::fmt::Display for TextureOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

impl PartialEq for TextureOption {
    fn eq(&self, other: &Self) -> bool {
        self.handle.id() == other.handle.id()
    }
}

#[derive(Clone)]
pub struct TextureChanged(
    pub lapiz_assets::asset::AssetHandle<lapiz_render::texture::Image>,
);

#[derive(Clone)]
pub struct TextureType {
    pub texel_type: TexelType,
}

impl Default for TextureType {
    fn default() -> Self {
        Self {
            texel_type: TexelType::RGBA8,
        }
    }
}

/// A texture literal: the image asset the shader samples. The NULL reference
/// has no asset and prepares as an empty placeholder texture.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct TextureReference {
    pub texture: Option<lapiz_assets::asset::AssetHandle<lapiz_render::texture::Image>>,
}

impl TextureReference {
    pub const NULL: Self = Self { texture: None };

    pub fn from_asset(
        asset: lapiz_assets::asset::AssetHandle<lapiz_render::texture::Image>,
    ) -> Self {
        Self {
            texture: Some(asset),
        }
    }
}

// Plain serde only sees the asset id; resolving it back to a handle needs the
// asset registry, which `deserialize_literal` does.
impl Serialize for TextureReference {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.texture {
            Some(handle) => serializer.serialize_some(&handle.id()),
            None => serializer.serialize_none(),
        }
    }
}

impl<'de> Deserialize<'de> for TextureReference {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<lapiz_assets::asset::AssetId<lapiz_render::texture::Image>>::deserialize(
            deserializer,
        )?;
        Ok(Self::NULL)
    }
}

impl GraphValueType for TextureType {
    type AssociatedLiteralType = TextureReference;
    type PreparedShaderType = wgpu::TextureView;
    type Message = TextureChanged;

    fn hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(TextureType)
    }

    fn id(&self) -> GraphValueTypeId {
        GraphValueTypeId::new(match self.texel_type {
            TexelType::RGBA8 => "texture_rgba8",
            TexelType::A8 => "texture_a8",
        })
    }

    fn push_shader_layout(
        &self,
        name: &str,
        stage: GraphShaderStage,
        group: u32,
        binding: u32,
        bindings: DynamicBindGroupLayoutEntries,
        mut shader: String,
    ) -> Result<(u32, DynamicBindGroupLayoutEntries, String)> {
        if stage != GraphShaderStage::Input {
            bail!("Texture values cannot be shader outputs");
        }
        let scalar = match self.texel_type.sample_type() {
            wgpu::TextureSampleType::Uint => "u32",
            wgpu::TextureSampleType::Sint => "i32",
            _ => "f32",
        };
        let scalar = Ident::new(scalar.to_string());
        shader.push_str(
            &quote_declaration! {
                @group(#group) @binding(#binding) var #name: texture_2d<#scalar>;
            }
            .to_string(),
        );
        shader.push('\n');
        let bindings = bindings.extend_with_indices(((
            binding,
            binding_types::texture_2d(self.texel_type.sample_type()),
        ),));
        Ok((binding + 1, bindings, shader))
    }

    fn push_shader_binding<'a>(
        &self,
        _stage: GraphShaderStage,
        value: &'a wgpu::TextureView,
        binding: u32,
        bindings: DynamicBindGroupEntries<'a>,
    ) -> Result<(u32, DynamicBindGroupEntries<'a>)> {
        let bindings = bindings.extend_with_indices(((binding, value),));
        Ok((binding + 1, bindings))
    }

    fn prepare_to_shader(
        &self,
        data: &Self::AssociatedLiteralType,
        device: &Device,
        queue: &Queue,
    ) -> Result<Self::PreparedShaderType> {
        // TODO Validate texture format matches current type declaration.
        //      We should prevent dismatch at UI level?
        if let Some(handle) = &data.texture {
            let image = handle.get().context("texture asset is not loaded")?;
            let gpu = lapiz_render::texture::GpuImage::from_asset(
                device,
                queue,
                &image,
                TextureUsages::TEXTURE_BINDING,
            );
            return Ok(gpu.texture.create_view(&TextureViewDescriptor::default()));
        }
        // The NULL reference still binds an empty placeholder.
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("graph dummy texture"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: self.texel_type.wgpu_format(),
            usage: TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        Ok(texture.create_view(&Default::default()))
    }

    fn serialize_literal(
        &self,
        data: &Self::AssociatedLiteralType,
        _assets: &lapiz_assets::store::AssetRegistry,
    ) -> Result<toml::Value, toml::ser::Error> {
        let mut table = toml::map::Map::new();
        if let Some(handle) = &data.texture {
            table.insert("asset".into(), toml::Value::try_from(handle.id())?);
        }
        Ok(toml::Value::Table(table))
    }

    fn deserialize_literal<'a>(
        &self,
        deserializer: toml::Value,
        assets: &lapiz_assets::store::AssetRegistry,
    ) -> Result<Self::AssociatedLiteralType, <toml::Value as serde::Deserializer<'a>>::Error> {
        let Some(asset) = deserializer.get("asset") else {
            return Ok(TextureReference::NULL);
        };
        let id = lapiz_assets::asset::AssetId::<lapiz_render::texture::Image>::deserialize(
            asset.clone(),
        )?;
        let handle = assets.handle(id).map_err(|e| {
            serde::de::Error::custom(format!("texture asset {id} is unavailable: {e:?}"))
        })?;
        Ok(TextureReference {
            texture: Some(handle),
        })
    }

    fn default_literal(&self) -> Self::AssociatedLiteralType {
        TextureReference::NULL
    }

    fn wgsl_type_name(&self) -> Option<&'static str> {
        None
    }

    fn view_literal(
        &self,
        data: &Self::AssociatedLiteralType,
        assets: &lapiz_assets::store::AssetRegistry,
    ) -> Element<'static, Self::Message, GraphTheme, GraphRenderer> {
        let options = assets
            .all_handles_of::<lapiz_render::texture::Image>()
            .unwrap_or_default()
            .into_iter()
            .map(|handle| TextureOption {
                name: handle
                    .get()
                    .map(|image| image.metadata.name.clone())
                    .unwrap_or_default(),
                handle,
            })
            .collect::<Vec<_>>();
        let selected = data.texture.as_ref().and_then(|handle| {
            options
                .iter()
                .find(|option| option.handle.id() == handle.id())
                .cloned()
        });
        ComboBox::new(options, selected, |option| TextureChanged(option.handle.clone()))
            .placeholder(t!("select_texture"))
            .width(iced_core::Length::Fill)
            .into()
    }

    fn update_literal(&self, data: &mut Self::AssociatedLiteralType, message: Self::Message) {
        data.texture = Some(message.0);
    }

    fn literal_to_code(&self, _data: &Self::AssociatedLiteralType) -> Option<Expression> {
        None
    }
}

#[derive(Clone)]
pub struct LayerType {
    pub texel_type: TexelType,
}

// Layers bind as resources; the literal is only a connectable placeholder.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct LayerReference;

pub struct PreparedLayer {
    pub storage: DynamicLayerStorage,
    pub pixel_bounds: IRect,
    // Only bound during eval; other passes read pixels through the storage.
    pub bounds: Buffer,
    dummy_texture: TextureView,
    dummy_tile_info: Buffer,
}

impl PreparedLayer {
    pub fn deep_clone(&self) -> Self {
        Self {
            storage: self.storage.deep_clone(),
            pixel_bounds: self.pixel_bounds,
            bounds: self.bounds.clone(),
            dummy_texture: self.dummy_texture.clone(),
            dummy_tile_info: self.dummy_tile_info.clone(),
        }
    }
}

impl GraphValueType for LayerType {
    type AssociatedLiteralType = LayerReference;
    type PreparedShaderType = PreparedLayer;
    type Message = ();

    fn hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(LayerType)
    }

    fn id(&self) -> GraphValueTypeId {
        GraphValueTypeId::new(match self.texel_type {
            TexelType::RGBA8 => "layer_rgba8",
            TexelType::A8 => "layer_a8",
        })
    }

    fn push_input_slots(&self) -> Vec<GraphDefaultInputSlot> {
        let color = match self.texel_type {
            TexelType::RGBA8 => GraphDefaultInputSlot::new::<ColorType>("color".into()),
            TexelType::A8 => GraphDefaultInputSlot::new::<F32Type>("alpha".into()),
        };
        vec![
            color,
            GraphDefaultInputSlot::new::<RectType>("bounds".into()),
        ]
    }

    fn push_output_slots(&self) -> Vec<GraphDefaultOutputSlot> {
        let color = match self.texel_type {
            TexelType::RGBA8 => GraphDefaultOutputSlot::new::<ColorType>("color".into()),
            TexelType::A8 => GraphDefaultOutputSlot::new::<F32Type>("alpha".into()),
        };
        vec![
            color,
            GraphDefaultOutputSlot::new::<RectType>("bounds".into()),
        ]
    }

    fn handle_input_values(
        &self,
        input_name: &str,
        base_index: usize,
        ctx: &mut GraphNodeCodeGenContext,
    ) -> Result<String> {
        let color = ctx.get_output(base_index)?;
        let bounds = ctx.get_output(base_index + 1)?;
        ctx.output_slot_idents
            .insert(ctx.outputs[base_index], ident_expression(color.clone()));
        ctx.output_slot_idents.insert(
            ctx.outputs[base_index + 1],
            ident_expression(bounds.clone()),
        );
        let load = Ident::new(layer_load_ident(input_name));
        let input_bounds = Ident::new(layer_bounds_ident(input_name));
        let color_stmt = quote_statement! {
            let #color = #load(dispatch_index);
        };
        let bounds_stmt = quote_statement! {
            let #bounds = render::math::Rect(vec2f(#input_bounds.xy), vec2f(#input_bounds.zw));
        };
        Ok(format!("{}\n{}", color_stmt, bounds_stmt))
    }

    fn handle_output_values(
        &self,
        output_name: &str,
        base_index: usize,
        ctx: &GraphNodeCodeGenContext,
    ) -> Result<String> {
        let color = ctx.get_input(base_index)?;
        let bounds = ctx.get_input(base_index + 1)?;
        let output_bounds = Ident::new(layer_bounds_ident(output_name));
        let store = Ident::new(layer_store_ident(output_name));
        let eval_stmt = quote_statement! {
            @if(EVAL) { #output_bounds = vec4i(vec2i(floor((#bounds).min)), vec2i(ceil((#bounds).max))); }
        };
        let main_stmt = quote_statement! {
            @if(!EVAL) {
                if all(dispatch_index >= vec2i(floor((#bounds).min))) && all(dispatch_index < vec2i(ceil((#bounds).max))) {
                    #store(dispatch_index, #color);
                }
            }
        };
        Ok(format!("{}\n{}", eval_stmt, main_stmt))
    }

    fn push_shader_layout(
        &self,
        name: &str,
        stage: GraphShaderStage,
        group: u32,
        binding: u32,
        bindings: DynamicBindGroupLayoutEntries,
        mut shader: String,
    ) -> Result<(u32, DynamicBindGroupLayoutEntries, String)> {
        match stage {
            GraphShaderStage::Input | GraphShaderStage::Main => {
                let (access, access_ident) = if stage == GraphShaderStage::Input {
                    (
                        wgpu::StorageTextureAccess::ReadOnly,
                        Ident::new("read".to_string()),
                    )
                } else {
                    (
                        wgpu::StorageTextureAccess::WriteOnly,
                        Ident::new("write".to_string()),
                    )
                };
                let texel = Ident::new(self.texel_type.wgpu_texel());
                shader.push_str(
                    &quote_declaration! {
                        @group(#group) @binding(#binding) var #name: texture_storage_2d_array<#texel, #access_ident>;
                    }
                    .to_string(),
                );
                shader.push('\n');
                let bindings = bindings.extend_with_indices(((
                    binding,
                    binding_types::texture_storage_2d_array(self.texel_type.wgpu_format(), access),
                ),));
                let tile_info_binding = binding + 1;
                let tile_info = Ident::new(layer_tile_info_ident(name));
                shader.push_str(
                    &quote_declaration! {
                        @group(#group) @binding(#tile_info_binding) var<storage, read> #tile_info: array<image::image_tiling::TileInfo>;
                    }
                    .to_string(),
                );
                shader.push('\n');
                let bindings = bindings.extend_with_indices(((
                    tile_info_binding,
                    binding_types::storage_buffer_read_only_sized(false, None),
                ),));
                if stage == GraphShaderStage::Input {
                    let bounds_binding = binding + 2;
                    let bounds = Ident::new(layer_bounds_ident(name));
                    shader.push_str(
                        &quote_declaration! {
                            @group(#group) @binding(#bounds_binding) var<storage, read> #bounds: vec4i;
                        }
                        .to_string(),
                    );
                    shader.push('\n');
                    let bindings = bindings.extend_with_indices(((
                        bounds_binding,
                        binding_types::storage_buffer_read_only_sized(false, None),
                    ),));
                    Ok((binding + 3, bindings, shader))
                } else {
                    Ok((binding + 2, bindings, shader))
                }
            }
            GraphShaderStage::Eval => {
                let bounds = Ident::new(layer_bounds_ident(name));
                shader.push_str(
                    &quote_declaration! {
                        @group(#group) @binding(#binding) var<storage, read_write> #bounds: vec4i;
                    }
                    .to_string(),
                );
                shader.push('\n');
                let bindings = bindings.extend_with_indices(((
                    binding,
                    binding_types::storage_buffer_sized(false, None),
                ),));
                Ok((binding + 1, bindings, shader))
            }
        }
    }

    fn push_shader_binding<'a>(
        &self,
        stage: GraphShaderStage,
        value: &'a PreparedLayer,
        binding: u32,
        bindings: DynamicBindGroupEntries<'a>,
    ) -> Result<(u32, DynamicBindGroupEntries<'a>)> {
        match stage {
            GraphShaderStage::Input | GraphShaderStage::Main => {
                let texture = value.storage.texture_view().unwrap_or(&value.dummy_texture);
                let tile_info = value
                    .storage
                    .tile_info_buffer()
                    .unwrap_or(&value.dummy_tile_info);
                let bindings = bindings.extend_with_indices(((binding, texture),));
                let bindings =
                    bindings.extend_with_indices(((binding + 1, tile_info.as_entire_binding()),));
                if stage == GraphShaderStage::Input {
                    let bindings = bindings
                        .extend_with_indices(((binding + 2, value.bounds.as_entire_binding()),));
                    Ok((binding + 3, bindings))
                } else {
                    Ok((binding + 2, bindings))
                }
            }
            GraphShaderStage::Eval => {
                let bindings =
                    bindings.extend_with_indices(((binding, value.bounds.as_entire_binding()),));
                Ok((binding + 1, bindings))
            }
        }
    }

    fn prepare_to_shader(
        &self,
        _data: &Self::AssociatedLiteralType,
        device: &Device,
        queue: &Queue,
    ) -> Result<Self::PreparedShaderType> {
        let dummy_texture = device.create_texture(&TextureDescriptor {
            label: Some("graph empty layer texture"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: self.texel_type.wgpu_format(),
            usage: TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        Ok(PreparedLayer {
            storage: DynamicLayerStorage::new(
                device.clone(),
                queue.clone(),
                GpuLayerInfo {
                    texel_type: self.texel_type,
                },
            ),
            pixel_bounds: IRect::EMPTY,
            bounds: device.create_buffer(&BufferDescriptor {
                label: Some("graph layer bounds"),
                size: 16,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            dummy_texture: dummy_texture.create_view(&TextureViewDescriptor {
                dimension: Some(TextureViewDimension::D2Array),
                ..Default::default()
            }),
            dummy_tile_info: device.create_buffer(&BufferDescriptor {
                label: Some("graph empty layer tile info"),
                size: 4,
                usage: BufferUsages::STORAGE,
                mapped_at_creation: false,
            }),
        })
    }

    fn requires_eval(&self) -> bool {
        true
    }

    fn post_eval(&self, value: &mut PreparedLayer, device: &Device, queue: &Queue) -> Result<()> {
        // TODO: async readback; block for now.
        let mut encoder = device.create_command_encoder(&Default::default());
        let staging =
            create_readback_buffer_and_schedule_copy_buffer(device, &mut encoder, &value.bounds);
        let readback = readback_buffer_on_submit_async::<IVec4, _>(&mut encoder, &staging, ..);
        let submission = queue.submit([encoder.finish()]);
        device.poll_indefinitely_for(submission)?;
        let mut readback = readback.into_inner();
        let bounds = loop {
            if let Some(bounds) = readback.try_recv()? {
                break bounds?;
            }
            device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })?;
        };
        value.pixel_bounds = IRect {
            min: IVec2::new(bounds.x, bounds.y),
            max: IVec2::new(bounds.z, bounds.w),
        };
        value.storage.allocate_pixels(value.pixel_bounds);
        Ok(())
    }

    fn literal_to_code(&self, data: &Self::AssociatedLiteralType) -> Option<Expression> {
        None
    }

    // Layer literals are connectable placeholders; hosts always override the
    // prepared value with a real layer, so they serialize as empty slots.
    fn serialize_literal(
        &self,
        _data: &Self::AssociatedLiteralType,
        _assets: &lapiz_assets::store::AssetRegistry,
    ) -> Result<toml::Value, toml::ser::Error> {
        Ok(toml::Value::Table(Default::default()))
    }

    fn deserialize_literal<'a>(
        &self,
        _deserializer: toml::Value,
        _assets: &lapiz_assets::store::AssetRegistry,
    ) -> Result<Self::AssociatedLiteralType, <toml::Value as serde::Deserializer<'a>>::Error> {
        Ok(LayerReference)
    }

    fn generate_extra_shader_body(&self, stage: GraphShaderStage, name: &str) -> Option<String> {
        let load = Ident::new(layer_load_ident(name));
        let store = Ident::new(layer_store_ident(name));
        let texture = Ident::new(name.to_string());
        let tile_info = Ident::new(layer_tile_info_ident(name));

        let (pack, unpack, default_value, color_ty) = match self.texel_type {
            TexelType::RGBA8 => (
                quote_expression! {
                    image::texture_unpack::pack_rgba8_texel(color)
                },
                quote_expression! {
                    image::texture_unpack::unpack_rgba8_texel(texel)
                },
                quote_expression! {
                    image::texture_unpack::pack_rgba8_texel(vec4f(0.0))
                },
                Ident::new("vec4f".into()),
            ),
            TexelType::A8 => (
                quote_expression! {
                    image::texture_unpack::pack_a8_texel(color)
                },
                quote_expression! {
                    image::texture_unpack::unpack_a8_texel(texel)
                },
                quote_expression! {
                    image::texture_unpack::pack_a8_texel(0.0)
                },
                Ident::new("f32".into()),
            ),
        };

        let mut helpers = String::new();
        match stage {
            GraphShaderStage::Input =>
            {
                helpers.push_str(
                                &quote_declaration! {
                                    fn #load(pixel: vec2i) -> #color_ty {
                                        var texel = #default_value;
                                        for (var tile = 0u; tile < arrayLength(&#tile_info); tile += 1u) {
                                            let info = #tile_info[tile];
                                            if (pixel.x >= info.origin.x) && (pixel.x < info.origin.x + i32(image::image_tiling::TILE_SIZE))
                                                && (pixel.y >= info.origin.y) && (pixel.y < info.origin.y + i32(image::image_tiling::TILE_SIZE)) {
                                                texel = textureLoad(#texture, pixel - info.origin, tile);
                                                break;
                                            }
                                        }
                                        return #unpack;
                                    }
                                }
                                .to_string(),
                            )
            }
,
            GraphShaderStage::Eval =>{},
            GraphShaderStage::Main =>{

                helpers.push_str(
                    &quote_declaration! {
                        @if(!EVAL) fn #store(pixel: vec2i, color: #color_ty) {
                            for (var tile = 0u; tile < arrayLength(&#tile_info); tile += 1u) {
                                let info = #tile_info[tile];
                                if (pixel.x >= info.origin.x) && (pixel.x < info.origin.x + i32(image::image_tiling::TILE_SIZE))
                                    && (pixel.y >= info.origin.y) && (pixel.y < info.origin.y + i32(image::image_tiling::TILE_SIZE)) {
                                    textureStore(#texture, pixel - info.origin, tile, #pack);
                                    return;
                                }
                            }
                        }
                    }
                    .to_string(),
                );
            },
        }
        helpers.push('\n');
        Some(helpers)
    }

    fn default_literal(&self) -> Self::AssociatedLiteralType {
        LayerReference
    }

    fn wgsl_type_name(&self) -> Option<&'static str> {
        None
    }

    fn view_literal(
        &self,
        _data: &Self::AssociatedLiteralType,
        _assets: &lapiz_assets::store::AssetRegistry,
    ) -> Element<'static, Self::Message, GraphTheme, GraphRenderer> {
        Element::new(space())
    }

    fn update_literal(&self, _data: &mut Self::AssociatedLiteralType, _message: Self::Message) {}
}

#[derive(Clone)]
pub struct ArrayType {
    pub element_type: Arc<dyn ErasedGraphValueType>,
    pub len: u32,
}

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
pub struct ArrayLiteral;

#[derive(Clone)]
pub struct PreparedArray {
    pub buffer: Buffer,
    pub len: u32,
}

impl GraphValueType for ArrayType {
    type AssociatedLiteralType = ArrayLiteral;
    type PreparedShaderType = PreparedArray;
    type Message = ();

    fn hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(ArrayType)
    }

    fn id(&self) -> GraphValueTypeId {
        GraphValueTypeId::new(format!("array_{}_{}", self.element_type.id().id, self.len))
    }

    fn push_input_slots(&self) -> Vec<GraphDefaultInputSlot> {
        vec![
            GraphDefaultInputSlot::new::<I32Type>("index".into()),
            GraphDefaultInputSlot::new_boxed("value".into(), self.element_type.clone()),
        ]
    }

    fn push_output_slots(&self) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new_boxed(
            "value".into(),
            self.element_type.clone(),
        )]
    }

    fn handle_input_values(
        &self,
        input_name: &str,
        base_index: usize,
        ctx: &mut GraphNodeCodeGenContext,
    ) -> Result<String> {
        ctx.get_output(base_index)?;
        let name = Ident::new(input_name.to_string());
        ctx.output_slot_idents.insert(
            ctx.outputs[base_index],
            quote_expression! { #name[dispatch_index] },
        );
        Ok(String::new())
    }

    fn handle_output_values(
        &self,
        output_name: &str,
        base_index: usize,
        ctx: &GraphNodeCodeGenContext,
    ) -> Result<String> {
        let index = ctx.get_input(base_index)?;
        let value = ctx.get_input(base_index + 1)?;
        Ok(format!(
            "@if(!EVAL) {{ {output_name}[{index}] = {value}; }}\n"
        ))
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
        let element = self
            .element_type
            .wgsl_type_name()
            .context("Array element type has no GPU representation")?
            .to_string();
        push_storage_layout(
            stage,
            &format!("array<{element}>"),
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
        value: &'a PreparedArray,
        binding: u32,
        bindings: DynamicBindGroupEntries<'a>,
    ) -> Result<(u32, DynamicBindGroupEntries<'a>)> {
        push_buffer_binding(&value.buffer, binding, bindings)
    }

    fn prepare_to_shader(
        &self,
        _data: &Self::AssociatedLiteralType,
        device: &Device,
        _queue: &Queue,
    ) -> Result<Self::PreparedShaderType> {
        let stride = self
            .element_type
            .wgsl_array_element_stride()
            .context("Array element type has no GPU representation")?;
        let size = u64::from(self.len)
            .checked_mul(stride)
            .context("Array too large")?;
        // Fresh wgpu buffers are zero-initialized.
        Ok(PreparedArray {
            buffer: device.create_buffer(&BufferDescriptor {
                label: Some("graph array literal"),
                size: size.max(4),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            len: self.len,
        })
    }

    fn default_literal(&self) -> Self::AssociatedLiteralType {
        ArrayLiteral
    }

    fn wgsl_type_name(&self) -> Option<&'static str> {
        None
    }

    fn view_literal(
        &self,
        _data: &Self::AssociatedLiteralType,
        _assets: &lapiz_assets::store::AssetRegistry,
    ) -> Element<'static, Self::Message, GraphTheme, GraphRenderer> {
        Element::new(space())
    }

    fn update_literal(&self, _data: &mut Self::AssociatedLiteralType, _message: Self::Message) {}

    fn literal_to_code(&self, _data: &Self::AssociatedLiteralType) -> Option<Expression> {
        None
    }
}
