use std::sync::{Arc, LazyLock};

use anyhow::{Result, bail};
use encase::ShaderType;
use glam::Vec4;
use lapiz_effect::nodes::effect_nodes;
use lapiz_image::{blend_modes::BlendMode, texel::TexelType, tile::LayerBinding};
use lapiz_render::{
    bind_group_entries::DynamicBindGroupEntries,
    bind_group_layout_entries::DynamicBindGroupLayoutEntries,
};
use lapiz_shader_graph::{
    GraphElement,
    graph::{
        GraphResources,
        node::{
            GraphNode, GraphNodeCodeGenContext, GraphNodeCodeGenError, GraphNodeCreateSlotsContext,
            GraphNodeDefaultStateContext, GraphNodeRegistry, GraphNodeUpdateContext,
            GraphNodeViewContext, StatelessCommonGraphNode, stateless,
        },
        slot::{
            ErasedGraphLiteralUpdateMessage, ErasedGraphValueType, GraphDefaultInputSlot,
            GraphDefaultOutputSlot, GraphShaderStage, GraphValueType,
        },
        variable::GraphTypeRegistry,
    },
    save::GraphValueTypeId,
    wgsl_std::types::{ColorType, F32Type, I32Type, RectType, Vec2FType},
};
use lapiz_utils::random_oklch_hue_chroma;
use lapiz_widgets::combo_box::ComboBox;
use serde::{Deserialize, Serialize};
use wesl::syntax::Expression;

use iced_core::widget::Void;
use wgpu::util::DeviceExt as _;
// TODO We may move to another crate.
#[derive(Debug, Default, Clone, ShaderType, Serialize, Deserialize)]
pub struct CanvasResources {
    pub foreground_color: Vec4,
    pub background_color: Vec4,
}

// Brush effects expose their results through effect outputs with these
// conventional identifiers; the brush compiler wires the output variables into
// the template exit points. For now brushes are expected to keep them.
pub const SPACING_OUTPUT: &str = "spacing";
// Intermediate output accumulated across dabs.
pub const MAIN_ACCUMULATE_BUFFER: &str = "main_accumulate";
// Final stroke output, already blended with the target layer.
pub const STROKE_RESULT: &str = "stroke_result";

// The builtin literals brush hosts inject into compiled effects. The shader
// declarations use the original names, so brush nodes and the templates can
// reference them directly.
pub const CANVAS_RESOURCES_BUILTIN: &str = "canvas_resources";
pub const TARGET_LAYER_BUILTIN: &str = "target_layer";
pub const SELECTION_BUILTIN: &str = "selection";
pub const HAS_SELECTION_BUILTIN: &str = "has_selection";
pub const BRUSH_SAMPLE_BUILTIN: &str = "brush_sample";
pub const INITIAL_PEN_INPUT_BUILTIN: &str = "initial_pen_input";
pub const STROKE_DATA_BUILTIN: &str = "stroke_data";

#[derive(Default, Clone)]
pub struct CanvasResourcesValueType;

impl GraphValueType for CanvasResourcesValueType {
    type AssociatedLiteralType = CanvasResources;
    type PreparedShaderType = wgpu::Buffer;
    type Message = ();

    fn id(&self) -> GraphValueTypeId {
        GraphValueTypeId::new("brush_canvas_resources")
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
            bail!("canvas resources can only be a shader input");
        }
        if !shader.contains("struct CanvasResources") {
            shader.push_str(
                "struct CanvasResources { foreground_color: vec4f, background_color: vec4f }\n",
            );
        }
        shader.push_str(&format!(
            "@group({group}) @binding({binding}) var<storage, read> {name}: CanvasResources;\n"
        ));
        let bindings = bindings.extend_with_indices(((
            binding,
            lapiz_render::bind_group_layout_entries::binding_types::storage_buffer_read_only_sized(
                false, None,
            ),
        ),));
        Ok((binding + 1, bindings, shader))
    }

    fn push_shader_binding<'a>(
        &self,
        _stage: GraphShaderStage,
        value: &'a wgpu::Buffer,
        binding: u32,
        bindings: DynamicBindGroupEntries<'a>,
    ) -> Result<(u32, DynamicBindGroupEntries<'a>)> {
        let bindings = bindings.extend_with_indices(((binding, value.as_entire_binding()),));
        Ok((binding + 1, bindings))
    }

    fn prepare_to_shader(
        &self,
        data: &CanvasResources,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Result<wgpu::Buffer> {
        let mut storage = encase::StorageBuffer::new(Vec::new());
        storage.write(data)?;
        Ok(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("brush canvas resources buffer"),
                contents: storage.as_ref(),
                usage: wgpu::BufferUsages::STORAGE,
            }),
        )
    }

    fn default_literal(&self) -> CanvasResources {
        CanvasResources::default()
    }

    fn wgsl_type_name(&self) -> Option<&'static str> {
        Some("CanvasResources")
    }

    fn hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(CanvasResourcesValueType)
    }

    fn view_literal(&self, _data: &CanvasResources) -> GraphElement<'static, Self::Message> {
        Void.into()
    }

    fn update_literal(&self, _data: &mut CanvasResources, _message: Self::Message) {}

    fn literal_to_code(&self, _data: &CanvasResources) -> Option<Expression> {
        None
    }
}

// A canvas layer bound as read-only storage texture pairs (packed texel
// texture + tile info buffer). Hosts inject the actual LayerBinding.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct BrushLayerReference;

#[derive(Clone)]
pub struct BrushLayerType {
    pub texel_type: TexelType,
}

impl GraphValueType for BrushLayerType {
    type AssociatedLiteralType = BrushLayerReference;
    type PreparedShaderType = LayerBinding;
    type Message = ();

    fn id(&self) -> GraphValueTypeId {
        GraphValueTypeId::new(match self.texel_type {
            TexelType::RGBA8 => "brush_layer_rgba8",
            TexelType::A8 => "brush_layer_a8",
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
            bail!("brush layers can only be shader inputs");
        }
        let texel = self.texel_type.wgpu_texel();
        shader.push_str(&format!(
            "@group({group}) @binding({binding}) var {name}: texture_storage_2d_array<{texel}, read>;\n"
        ));
        let bindings = bindings.extend_with_indices(((
            binding,
            lapiz_render::bind_group_layout_entries::binding_types::texture_storage_2d_array(
                self.texel_type.wgpu_format(),
                wgpu::StorageTextureAccess::ReadOnly,
            ),
        ),));
        let tile_info_binding = binding + 1;
        let tile_info = lapiz_shader_graph::wgsl_std::types::layer_tile_info_ident(name);
        shader.push_str(&format!(
            "@group({group}) @binding({tile_info_binding}) var<storage, read> {tile_info}: array<image::image_tiling::TileInfo>;\n"
        ));
        let bindings = bindings.extend_with_indices(((
            tile_info_binding,
            lapiz_render::bind_group_layout_entries::binding_types::storage_buffer_read_only_sized(
                false, None,
            ),
        ),));
        Ok((binding + 2, bindings, shader))
    }

    fn push_shader_binding<'a>(
        &self,
        _stage: GraphShaderStage,
        value: &'a LayerBinding,
        binding: u32,
        bindings: DynamicBindGroupEntries<'a>,
    ) -> Result<(u32, DynamicBindGroupEntries<'a>)> {
        let bindings = bindings.extend_with_indices(((binding, &value.texture),));
        let bindings = bindings
            .extend_with_indices(((binding + 1, value.tile_info_buffer.as_entire_binding()),));
        Ok((binding + 2, bindings))
    }

    fn prepare_to_shader(
        &self,
        _data: &BrushLayerReference,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Result<LayerBinding> {
        bail!("brush layers are host-bound and never prepared from literals")
    }

    fn default_literal(&self) -> BrushLayerReference {
        BrushLayerReference
    }

    fn wgsl_type_name(&self) -> Option<&'static str> {
        None
    }

    fn hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(BrushLayerType)
    }

    fn view_literal(&self, _data: &BrushLayerReference) -> GraphElement<'static, Self::Message> {
        Void.into()
    }

    fn update_literal(&self, _data: &mut BrushLayerReference, _message: Self::Message) {}

    fn literal_to_code(&self, _data: &BrushLayerReference) -> Option<Expression> {
        None
    }

    fn generate_extra_shader_body(&self, stage: GraphShaderStage, name: &str) -> Option<String> {
        if stage != GraphShaderStage::Input {
            return None;
        }
        let tile_info = lapiz_shader_graph::wgsl_std::types::layer_tile_info_ident(name);
        let (return_type, unpack, default) = match self.texel_type {
            TexelType::RGBA8 => (
                "vec4f",
                "image::texture_unpack::unpack_rgba8_texel",
                "vec4f(0.0)",
            ),
            TexelType::A8 => ("f32", "image::texture_unpack::unpack_a8_texel", "0.0"),
        };
        Some(format!(
            "fn brush_layer_load_{name}(pixel: vec2i) -> {return_type} {{\n\
                 for (var tile = 0u; tile < arrayLength(&{tile_info}); tile += 1u) {{\n\
                     let info = {tile_info}[tile];\n\
                     if all(pixel >= info.origin) && all(pixel < info.origin + i32(image::image_tiling::TILE_SIZE)) {{\n\
                         return {unpack}(textureLoad({name}, pixel - info.origin, tile));\n\
                     }}\n\
                 }}\n\
                 return {default};\n\
             }}\n"
        ))
    }
}

#[derive(Default, Clone)]
pub struct ComputedPenInputValueType;

impl GraphValueType for ComputedPenInputValueType {
    type AssociatedLiteralType = crate::render::ComputedPenInput;
    type PreparedShaderType = wgpu::Buffer;
    type Message = ();

    fn id(&self) -> GraphValueTypeId {
        GraphValueTypeId::new("brush_computed_pen_input")
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
            bail!("computed pen input can only be a shader input");
        }
        if !shader.contains("struct BrushComputedPenInput") {
            shader.push_str(
                "struct BrushTime { now: f32, stroke_begin: f32 }\n\
                 struct BrushComputedPenInput {\n\
                     position: vec2f, draw_direction_vec: vec2f, tilt: vec2f, angle: vec2f,\n\
                     draw_direction_angle: f32, pressure: f32, dab_index: u32, stroke_distance: f32,\n\
                     time: BrushTime\n\
                 }\n",
            );
        }
        shader.push_str(&format!(
            "@group({group}) @binding({binding}) var<storage, read> {name}: BrushComputedPenInput;\n"
        ));
        let bindings = bindings.extend_with_indices(((
            binding,
            lapiz_render::bind_group_layout_entries::binding_types::storage_buffer_read_only_sized(
                false, None,
            ),
        ),));
        Ok((binding + 1, bindings, shader))
    }

    fn push_shader_binding<'a>(
        &self,
        _stage: GraphShaderStage,
        value: &'a wgpu::Buffer,
        binding: u32,
        bindings: DynamicBindGroupEntries<'a>,
    ) -> Result<(u32, DynamicBindGroupEntries<'a>)> {
        Ok((
            binding + 1,
            bindings.extend_with_indices(((binding, value.as_entire_binding()),)),
        ))
    }

    fn prepare_to_shader(
        &self,
        data: &crate::render::ComputedPenInput,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Result<wgpu::Buffer> {
        let mut storage = encase::StorageBuffer::new(Vec::new());
        storage.write(data)?;
        Ok(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("brush computed pen input"),
                contents: storage.as_ref(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            }),
        )
    }

    fn default_literal(&self) -> crate::render::ComputedPenInput {
        crate::render::ComputedPenInput::default()
    }

    fn wgsl_type_name(&self) -> Option<&'static str> {
        None
    }
    fn hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(ComputedPenInputValueType)
    }
    fn view_literal(&self, _: &crate::render::ComputedPenInput) -> GraphElement<'static, ()> {
        Void.into()
    }
    fn update_literal(&self, _: &mut crate::render::ComputedPenInput, _: ()) {}
    fn literal_to_code(&self, _: &crate::render::ComputedPenInput) -> Option<Expression> {
        None
    }

    fn generate_extra_shader_body(&self, _stage: GraphShaderStage, name: &str) -> Option<String> {
        (name == BRUSH_SAMPLE_BUILTIN).then(||
            "struct BrushMask { bounds: render::math::Rect, value: f32 }\n\
             fn elliptical_mask(pixel: vec2f, center: vec2f, radii: vec2f, rotation: f32) -> BrushMask {\n\
                 let relative = pixel - center;\n\
                 let rotated = render::math::rotate_mat2x2(rotation) * relative;\n\
                 let extent = vec2f(max(radii.x, radii.y));\n\
                 let distance = render::math::sdf_ellipse(rotated, vec2f(0.0), radii);\n\
                 return BrushMask(render::math::Rect(center - extent, center + extent), smoothstep(1.0, 0.0, distance));\n\
             }\n"
                .into()
        )
    }
}

#[derive(Default, Clone)]
pub struct StrokeDataValueType;

impl GraphValueType for StrokeDataValueType {
    type AssociatedLiteralType = crate::render::StrokePostprocessData;
    type PreparedShaderType = wgpu::Buffer;
    type Message = ();

    fn id(&self) -> GraphValueTypeId {
        GraphValueTypeId::new("brush_stroke_data")
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
            bail!("stroke data can only be a shader input");
        }
        if !shader.contains("struct BrushStrokeData") {
            if !shader.contains("struct BrushTime") {
                shader.push_str("struct BrushTime { now: f32, stroke_begin: f32 }\n");
            }
            shader.push_str(
                "struct BrushStrokeData { accumulated_pixel_bounds: render::math::IRect, time: BrushTime }\n",
            );
        }
        shader.push_str(&format!(
            "@group({group}) @binding({binding}) var<storage, read> {name}: BrushStrokeData;\n"
        ));
        let bindings = bindings.extend_with_indices(((
            binding,
            lapiz_render::bind_group_layout_entries::binding_types::storage_buffer_read_only_sized(
                false, None,
            ),
        ),));
        Ok((binding + 1, bindings, shader))
    }

    fn push_shader_binding<'a>(
        &self,
        _stage: GraphShaderStage,
        value: &'a wgpu::Buffer,
        binding: u32,
        bindings: DynamicBindGroupEntries<'a>,
    ) -> Result<(u32, DynamicBindGroupEntries<'a>)> {
        Ok((
            binding + 1,
            bindings.extend_with_indices(((binding, value.as_entire_binding()),)),
        ))
    }

    fn prepare_to_shader(
        &self,
        data: &crate::render::StrokePostprocessData,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Result<wgpu::Buffer> {
        let mut storage = encase::StorageBuffer::new(Vec::new());
        storage.write(data)?;
        Ok(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("brush stroke data"),
                contents: storage.as_ref(),
                usage: wgpu::BufferUsages::STORAGE,
            }),
        )
    }

    fn default_literal(&self) -> crate::render::StrokePostprocessData {
        Default::default()
    }
    fn wgsl_type_name(&self) -> Option<&'static str> {
        None
    }
    fn hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(StrokeDataValueType)
    }
    fn view_literal(&self, _: &crate::render::StrokePostprocessData) -> GraphElement<'static, ()> {
        Void.into()
    }
    fn update_literal(&self, _: &mut crate::render::StrokePostprocessData, _: ()) {}
    fn literal_to_code(&self, _: &crate::render::StrokePostprocessData) -> Option<Expression> {
        None
    }
}

#[derive(Default, Clone)]
pub struct PenPositionNode;

#[stateless]
impl StatelessCommonGraphNode for PenPositionNode {
    fn id(&self) -> &'static str {
        "pen_position_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(PenPositionNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<Vec2FType>("position".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = brush_sample.position;\n",
            ctx.get_output(0)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct PenPressureNode;

#[stateless]
impl StatelessCommonGraphNode for PenPressureNode {
    fn id(&self) -> &'static str {
        "pen_pressure_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(PenPressureNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<F32Type>("pressure".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = brush_sample.pressure;\n",
            ctx.get_output(0)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct PenTiltNode;

#[stateless]
impl StatelessCommonGraphNode for PenTiltNode {
    fn id(&self) -> &'static str {
        "pen_tilt_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(PenTiltNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<Vec2FType>("tilt".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!("let {} = brush_sample.tilt;\n", ctx.get_output(0)?))
    }
}

#[derive(Default, Clone)]
pub struct PenAngleNode;

#[stateless]
impl StatelessCommonGraphNode for PenAngleNode {
    fn id(&self) -> &'static str {
        "pen_angle_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(PenAngleNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![
            GraphDefaultOutputSlot::new::<F32Type>("altitude".into()),
            GraphDefaultOutputSlot::new::<F32Type>("azimuth".into()),
        ]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = brush_sample.angle.x;\nlet {} = brush_sample.angle.y;\n",
            ctx.get_output(0)?,
            ctx.get_output(1)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct DrawDirectionNode;

#[stateless]
impl StatelessCommonGraphNode for DrawDirectionNode {
    fn id(&self) -> &'static str {
        "draw_direction_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(DrawDirectionNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![
            GraphDefaultOutputSlot::new::<F32Type>("angle".into()),
            GraphDefaultOutputSlot::new::<Vec2FType>("direction".into()),
        ]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = brush_sample.draw_direction_angle;\nlet {} = brush_sample.draw_direction_vec;\n",
            ctx.get_output(0)?,
            ctx.get_output(1)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct DabIndexNode;

#[stateless]
impl StatelessCommonGraphNode for DabIndexNode {
    fn id(&self) -> &'static str {
        "dab_index_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(DabIndexNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<I32Type>("index".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = i32(brush_sample.dab_index);",
            ctx.get_output(0)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct StrokeDistanceNode;

#[stateless]
impl StatelessCommonGraphNode for StrokeDistanceNode {
    fn id(&self) -> &'static str {
        "stroke_distance_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(StrokeDistanceNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<F32Type>("distance".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = brush_sample.stroke_distance;",
            ctx.get_output(0)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct InitialPenPositionNode;

#[stateless]
impl StatelessCommonGraphNode for InitialPenPositionNode {
    fn id(&self) -> &'static str {
        "initial_pen_position_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(InitialPenPositionNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<Vec2FType>("position".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = initial_pen_input.position;",
            ctx.get_output(0)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct InitialPenPressureNode;

#[stateless]
impl StatelessCommonGraphNode for InitialPenPressureNode {
    fn id(&self) -> &'static str {
        "initial_pen_pressure_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(InitialPenPressureNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<F32Type>("pressure".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = initial_pen_input.pressure;\n",
            ctx.get_output(0)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct InitialPenTiltNode;

#[stateless]
impl StatelessCommonGraphNode for InitialPenTiltNode {
    fn id(&self) -> &'static str {
        "initial_pen_tilt_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(InitialPenTiltNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<Vec2FType>("tilt".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = initial_pen_input.tilt;\n",
            ctx.get_output(0)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct InitialPenAngleNode;

#[stateless]
impl StatelessCommonGraphNode for InitialPenAngleNode {
    fn id(&self) -> &'static str {
        "initial_pen_angle_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(InitialPenAngleNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![
            GraphDefaultOutputSlot::new::<F32Type>("altitude".into()),
            GraphDefaultOutputSlot::new::<F32Type>("azimuth".into()),
        ]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = initial_pen_input.angle.x;\nlet {} = initial_pen_input.angle.y;\n",
            ctx.get_output(0)?,
            ctx.get_output(1)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct InitialDrawDirectionNode;

#[stateless]
impl StatelessCommonGraphNode for InitialDrawDirectionNode {
    fn id(&self) -> &'static str {
        "initial_draw_direction_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(InitialDrawDirectionNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![
            GraphDefaultOutputSlot::new::<F32Type>("angle".into()),
            GraphDefaultOutputSlot::new::<Vec2FType>("direction".into()),
        ]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = initial_pen_input.draw_direction_angle;\nlet {} = initial_pen_input.draw_direction_vec;\n",
            ctx.get_output(0)?,
            ctx.get_output(1)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct TimeNode;

#[stateless]
impl StatelessCommonGraphNode for TimeNode {
    fn id(&self) -> &'static str {
        "time_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(TimeNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![
            GraphDefaultOutputSlot::new::<F32Type>("now".into()),
            GraphDefaultOutputSlot::new::<F32Type>("stroke_begin".into()),
        ]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = brush_sample.time.now;\nlet {} = brush_sample.time.stroke_begin;\n",
            ctx.get_output(0)?,
            ctx.get_output(1)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct PixelPositionNode;

#[stateless]
impl StatelessCommonGraphNode for PixelPositionNode {
    fn id(&self) -> &'static str {
        "pixel_position_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(PixelPositionNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<Vec2FType>("position".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = vec2f(dispatch_index);",
            ctx.get_output(0)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct FilterWithinBoundsNode;

#[stateless]
impl StatelessCommonGraphNode for FilterWithinBoundsNode {
    fn id(&self) -> &'static str {
        "filter_within_bounds_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(FilterWithinBoundsNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![
            GraphDefaultInputSlot::new::<ColorType>("color".into()),
            GraphDefaultInputSlot::new::<RectType>("bounds".into()),
        ]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![
            GraphDefaultOutputSlot::new::<ColorType>("color".into()),
            GraphDefaultOutputSlot::new::<RectType>("bounds".into()),
        ]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let color = ctx.get_input(0)?;
        let bounds = ctx.get_input(1)?;
        Ok(format!(
            "let {} = filter_within_bounds(pixel_pos, {color}, {bounds});\nlet {} = {bounds};\n",
            ctx.get_output(0)?,
            ctx.get_output(1)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct BlendColorNode;

#[derive(Clone, Serialize, Deserialize)]
pub struct BlendColorNodeState {
    pub blend_mode: BlendMode,
}

#[derive(Clone)]
pub enum BlendModeNodeMessage {
    ModeChanged(BlendMode),
    LiteralUpdate(ErasedGraphLiteralUpdateMessage),
}

impl GraphNode for BlendColorNode {
    type State = BlendColorNodeState;
    type Message = BlendModeNodeMessage;

    fn id(&self) -> &'static str {
        "blend_color_node"
    }

    fn default_state(&self, _: GraphNodeDefaultStateContext<'_>) -> Self::State {
        BlendColorNodeState {
            blend_mode: BlendMode::Normal,
        }
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(BlendColorNode)
    }

    fn create_inputs(
        &self,
        _: &Self::State,
        _: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultInputSlot> {
        vec![
            GraphDefaultInputSlot::new::<ColorType>("src_color".into()),
            GraphDefaultInputSlot::new::<ColorType>("dst_color".into()),
        ]
    }

    fn create_outputs(
        &self,
        _: &Self::State,
        _: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<ColorType>("color".into())]
    }

    fn view(
        &self,
        state: &Self::State,
        ctx: GraphNodeViewContext<'_>,
    ) -> GraphElement<'static, Self::Message> {
        ctx.view_all_slots_with_header(
            ComboBox::new(
                BlendMode::ALL
                    .to_vec()
                    .into_iter()
                    .map(BlendModeOption)
                    .collect::<Vec<_>>(),
                Some(BlendModeOption(state.blend_mode)),
                |option| BlendModeNodeMessage::ModeChanged(option.into_inner()),
            )
            .width(iced_core::Length::Fill),
            BlendModeNodeMessage::LiteralUpdate,
        )
    }

    fn update(
        &self,
        state: &mut Self::State,
        message: Self::Message,
        mut ctx: GraphNodeUpdateContext<'_>,
    ) {
        match message {
            BlendModeNodeMessage::ModeChanged(mode) => state.blend_mode = mode,
            BlendModeNodeMessage::LiteralUpdate(message) => ctx.update_literal(message),
        }
    }

    fn generate_code(
        &self,
        state: &Self::State,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let src = ctx.get_input(0)?;
        let dst = ctx.get_input(1)?;
        let output = ctx.get_output(0)?;
        Ok(format!(
            "let {output} = image::blend_modes::{}({src}, {dst});\n",
            state.blend_mode.shader_func(),
        ))
    }
}

#[derive(Default, Clone)]
pub struct BlendWithInputNode;

#[derive(Clone, Serialize, Deserialize)]
pub struct BlendWithBufferNodeState {
    pub blend_mode: BlendMode,
}

impl GraphNode for BlendWithInputNode {
    type State = BlendWithBufferNodeState;
    type Message = BlendModeNodeMessage;

    fn id(&self) -> &'static str {
        "blend_with_input_node"
    }

    fn default_state(&self, _: GraphNodeDefaultStateContext<'_>) -> Self::State {
        BlendWithBufferNodeState {
            blend_mode: BlendMode::Normal,
        }
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(BlendWithInputNode)
    }

    fn create_inputs(
        &self,
        _: &Self::State,
        _: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultInputSlot> {
        vec![
            GraphDefaultInputSlot::new::<ColorType>("color".into()),
            GraphDefaultInputSlot::new::<F32Type>("opacity".into()),
        ]
    }

    fn create_outputs(
        &self,
        _: &Self::State,
        _: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<ColorType>("color".into())]
    }

    fn view(
        &self,
        state: &Self::State,
        ctx: GraphNodeViewContext<'_>,
    ) -> GraphElement<'static, Self::Message> {
        ctx.view_all_slots_with_header(
            ComboBox::new(
                BlendMode::ALL
                    .to_vec()
                    .into_iter()
                    .map(BlendModeOption)
                    .collect::<Vec<_>>(),
                Some(BlendModeOption(state.blend_mode)),
                |option| BlendModeNodeMessage::ModeChanged(option.into_inner()),
            )
            .width(iced_core::Length::Fill),
            BlendModeNodeMessage::LiteralUpdate,
        )
    }

    fn update(
        &self,
        state: &mut Self::State,
        message: Self::Message,
        mut ctx: GraphNodeUpdateContext<'_>,
    ) {
        match message {
            BlendModeNodeMessage::ModeChanged(mode) => state.blend_mode = mode,
            BlendModeNodeMessage::LiteralUpdate(message) => ctx.update_literal(message),
        }
    }

    fn generate_code(
        &self,
        state: &Self::State,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let color = ctx.get_input(0)?;
        let opacity = ctx.get_input(1)?;
        let output = ctx.get_output(0)?;
        Ok(format!(
            "let {output} = image::blend_modes::{}(vec4f({color}.rgb, {color}.a * {opacity}), main_accumulate_load(dispatch_index));\n",
            state.blend_mode.shader_func(),
        ))
    }
}

#[derive(Default, Clone)]
pub struct BlendWithLayerNode;

impl GraphNode for BlendWithLayerNode {
    type State = BlendWithBufferNodeState;
    type Message = BlendModeNodeMessage;

    fn id(&self) -> &'static str {
        "blend_with_layer_node"
    }

    fn default_state(&self, _: GraphNodeDefaultStateContext<'_>) -> Self::State {
        BlendWithBufferNodeState {
            blend_mode: BlendMode::Normal,
        }
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(BlendWithLayerNode)
    }

    fn create_inputs(
        &self,
        _: &Self::State,
        _: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultInputSlot> {
        vec![
            GraphDefaultInputSlot::new::<ColorType>("color".into()),
            GraphDefaultInputSlot::new::<F32Type>("opacity".into()),
        ]
    }

    fn create_outputs(
        &self,
        _: &Self::State,
        _: GraphNodeCreateSlotsContext<'_>,
    ) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<ColorType>("color".into())]
    }

    fn view(
        &self,
        state: &Self::State,
        ctx: GraphNodeViewContext<'_>,
    ) -> GraphElement<'static, Self::Message> {
        ctx.view_all_slots_with_header(
            ComboBox::new(
                BlendMode::ALL
                    .to_vec()
                    .into_iter()
                    .map(BlendModeOption)
                    .collect::<Vec<_>>(),
                Some(BlendModeOption(state.blend_mode)),
                |option| BlendModeNodeMessage::ModeChanged(option.into_inner()),
            )
            .width(iced_core::Length::Fill),
            BlendModeNodeMessage::LiteralUpdate,
        )
    }

    fn update(
        &self,
        state: &mut Self::State,
        message: Self::Message,
        mut ctx: GraphNodeUpdateContext<'_>,
    ) {
        match message {
            BlendModeNodeMessage::ModeChanged(mode) => state.blend_mode = mode,
            BlendModeNodeMessage::LiteralUpdate(message) => ctx.update_literal(message),
        }
    }

    fn generate_code(
        &self,
        state: &Self::State,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let color = ctx.get_input(0)?;
        let opacity = ctx.get_input(1)?;
        let output = ctx.get_output(0)?;
        Ok(format!(
            "let {output} = image::blend_modes::{}(vec4f({color}.rgb, {color}.a * {opacity}), brush_layer_load_target_layer(dispatch_index));\n",
            state.blend_mode.shader_func(),
        ))
    }
}

#[derive(Clone)]
pub struct BlendModeOption(BlendMode);

impl BlendModeOption {
    fn into_inner(self) -> BlendMode {
        self.0
    }
}

impl std::fmt::Display for BlendModeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl PartialEq for BlendModeOption {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

#[derive(Default, Clone)]
pub struct LayerPixelColorNode;

#[stateless]
impl StatelessCommonGraphNode for LayerPixelColorNode {
    fn id(&self) -> &'static str {
        "layer_pixel_color_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(LayerPixelColorNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![GraphDefaultInputSlot::new::<Vec2FType>("position".into())]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<ColorType>("color".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = brush_layer_load_target_layer(vec2i({}));\n",
            ctx.get_output(0)?,
            ctx.get_input(0)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct CurrentPixelColorNode;

#[stateless]
impl StatelessCommonGraphNode for CurrentPixelColorNode {
    fn id(&self) -> &'static str {
        "current_pixel_color_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(CurrentPixelColorNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![GraphDefaultInputSlot::new::<Vec2FType>("position".into())]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<ColorType>("color".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = main_accumulate_load(vec2i({}));\n",
            ctx.get_output(0)?,
            ctx.get_input(0)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct StrokeBoundsNode;

#[stateless]
impl StatelessCommonGraphNode for StrokeBoundsNode {
    fn id(&self) -> &'static str {
        "stroke_bounds_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(StrokeBoundsNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<RectType>("bounds".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = render::math::Rect(vec2f(stroke_data.accumulated_pixel_bounds.min), vec2f(stroke_data.accumulated_pixel_bounds.max));",
            ctx.get_output(0)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct AccumulateBoundsNode;

#[stateless]
impl StatelessCommonGraphNode for AccumulateBoundsNode {
    fn id(&self) -> &'static str {
        "accumulate_bounds_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(AccumulateBoundsNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![GraphDefaultInputSlot::new::<RectType>("dab bounds".into())]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<RectType>(
            "stroke bounds".into(),
        )]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let bounds = ctx.get_input(0)?;
        let output = ctx.get_output(0)?;
        Ok(format!(
            "let {output} = render::math::Rect(min(({bounds}).min, vec2f(main_accumulate_bounds.xy)), max(({bounds}).max, vec2f(main_accumulate_bounds.zw)));"
        ))
    }
}

#[derive(Default, Clone)]
pub struct EllipticalMaskNode;

#[stateless]
impl StatelessCommonGraphNode for EllipticalMaskNode {
    fn id(&self) -> &'static str {
        "elliptical_mask_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(EllipticalMaskNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![
            GraphDefaultInputSlot::new::<Vec2FType>("sample_position".into()),
            GraphDefaultInputSlot::new::<Vec2FType>("center".into()),
            GraphDefaultInputSlot::new::<Vec2FType>("radii".into()),
            GraphDefaultInputSlot::new::<F32Type>("rotation".into()),
        ]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![
            GraphDefaultOutputSlot::new::<F32Type>("mask_value".into()),
            GraphDefaultOutputSlot::new::<RectType>("bounds".into()),
        ]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let mask = ctx.ident_generator.next_output();
        Ok(format!(
            "let {mask} = elliptical_mask({}, {}, {}, {});\nlet {} = {mask}.value;\nlet {} = {mask}.bounds;\n",
            ctx.get_input(0)?,
            ctx.get_input(1)?,
            ctx.get_input(2)?,
            ctx.get_input(3)?,
            ctx.get_output(0)?,
            ctx.get_output(1)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct SelectionMaskNode;

#[stateless]
impl StatelessCommonGraphNode for SelectionMaskNode {
    fn id(&self) -> &'static str {
        "selection_mask_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(SelectionMaskNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![GraphDefaultInputSlot::new::<Vec2FType>("position".into())]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<F32Type>("value".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let input = ctx.get_input(0)?;
        let output = ctx.get_output(0)?;
        Ok(format!(
            "let {output} = brush_layer_load_selection(vec2i({input}));\n"
        ))
    }
}

#[derive(Default, Clone)]
pub struct ForegroundColorNode;

#[stateless]
impl StatelessCommonGraphNode for ForegroundColorNode {
    fn id(&self) -> &'static str {
        "foreground_color_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(ForegroundColorNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<ColorType>("color".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = canvas_resources.foreground_color;\n",
            ctx.get_output(0)?
        ))
    }
}

#[derive(Default, Clone)]
pub struct BackgroundColorNode;

#[stateless]
impl StatelessCommonGraphNode for BackgroundColorNode {
    fn id(&self) -> &'static str {
        "background_color_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(BackgroundColorNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<ColorType>("color".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        Ok(format!(
            "let {} = canvas_resources.background_color;\n",
            ctx.get_output(0)?
        ))
    }
}

pub static BRUSH_GRAPH_TYPES: LazyLock<Arc<GraphTypeRegistry>> =
    LazyLock::new(|| Arc::new(brush_graph_types()));

fn brush_graph_types() -> GraphTypeRegistry {
    let mut types = lapiz_shader_graph::wgsl_std::builtin_types();
    types.register_type::<CanvasResourcesValueType>();
    types.register_type::<ComputedPenInputValueType>();
    types.register_type::<StrokeDataValueType>();
    types
}

pub fn spacing_graph_nodes() -> GraphNodeRegistry {
    let mut nodes = effect_nodes();
    nodes.register::<PenPositionNode>();
    nodes.register::<PenPressureNode>();
    nodes.register::<PenAngleNode>();
    nodes.register::<PenTiltNode>();
    nodes.register::<DrawDirectionNode>();
    nodes.register::<DabIndexNode>();
    nodes.register::<InitialPenPositionNode>();
    nodes.register::<InitialPenPressureNode>();
    nodes.register::<InitialPenAngleNode>();
    nodes.register::<InitialPenTiltNode>();
    nodes.register::<InitialDrawDirectionNode>();
    nodes.register::<TimeNode>();
    nodes.register::<ForegroundColorNode>();
    nodes.register::<BackgroundColorNode>();
    nodes
}

pub fn main_graph_nodes() -> GraphNodeRegistry {
    let mut nodes = effect_nodes();
    nodes.register::<PenPositionNode>();
    nodes.register::<PenPressureNode>();
    nodes.register::<PenAngleNode>();
    nodes.register::<PenTiltNode>();
    nodes.register::<DrawDirectionNode>();
    nodes.register::<DabIndexNode>();
    nodes.register::<StrokeDistanceNode>();
    nodes.register::<InitialPenPositionNode>();
    nodes.register::<InitialPenPressureNode>();
    nodes.register::<InitialPenAngleNode>();
    nodes.register::<InitialPenTiltNode>();
    nodes.register::<InitialDrawDirectionNode>();
    nodes.register::<TimeNode>();
    nodes.register::<PixelPositionNode>();
    nodes.register::<FilterWithinBoundsNode>();
    nodes.register::<BlendColorNode>();
    nodes.register::<LayerPixelColorNode>();
    nodes.register::<CurrentPixelColorNode>();
    nodes.register::<StrokeBoundsNode>();
    nodes.register::<AccumulateBoundsNode>();
    nodes.register::<EllipticalMaskNode>();
    nodes.register::<BlendWithInputNode>();
    nodes.register::<BlendWithLayerNode>();
    nodes.register::<SelectionMaskNode>();
    nodes.register::<ForegroundColorNode>();
    nodes.register::<BackgroundColorNode>();
    nodes
}

pub fn postprocess_graph_nodes() -> GraphNodeRegistry {
    let mut nodes = effect_nodes();
    nodes.register::<PixelPositionNode>();
    nodes.register::<FilterWithinBoundsNode>();
    nodes.register::<BlendColorNode>();
    nodes.register::<LayerPixelColorNode>();
    nodes.register::<CurrentPixelColorNode>();
    nodes.register::<StrokeBoundsNode>();
    nodes.register::<EllipticalMaskNode>();
    nodes.register::<BlendWithInputNode>();
    nodes.register::<BlendWithLayerNode>();
    nodes.register::<SelectionMaskNode>();
    nodes.register::<ForegroundColorNode>();
    nodes.register::<BackgroundColorNode>();
    nodes
}

// The builtin literal types brush hosts inject, keyed by the original names
// the compiled shaders reference.
pub fn brush_builtin_types(
    target_layer_format: TexelType,
    selection_layer_format: TexelType,
) -> std::collections::BTreeMap<
    String,
    Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType>,
> {
    std::collections::BTreeMap::from([
        (
            CANVAS_RESOURCES_BUILTIN.to_string(),
            Arc::new(CanvasResourcesValueType)
                as Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType>,
        ),
        (
            HAS_SELECTION_BUILTIN.to_string(),
            Arc::new(lapiz_shader_graph::wgsl_std::types::U32Type)
                as Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType>,
        ),
        (
            SELECTION_BUILTIN.to_string(),
            Arc::new(BrushLayerType {
                texel_type: selection_layer_format,
            }) as Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType>,
        ),
        (
            TARGET_LAYER_BUILTIN.to_string(),
            Arc::new(BrushLayerType {
                texel_type: target_layer_format,
            }) as Arc<dyn lapiz_shader_graph::graph::slot::ErasedGraphValueType>,
        ),
    ])
}

pub fn main_builtin_types(
    target_layer_format: TexelType,
    selection_layer_format: TexelType,
) -> std::collections::BTreeMap<String, Arc<dyn ErasedGraphValueType>> {
    let mut types = brush_builtin_types(target_layer_format, selection_layer_format);
    types.insert(
        BRUSH_SAMPLE_BUILTIN.into(),
        Arc::new(ComputedPenInputValueType),
    );
    types.insert(
        INITIAL_PEN_INPUT_BUILTIN.into(),
        Arc::new(ComputedPenInputValueType),
    );
    types.insert(
        MAIN_ACCUMULATE_BUFFER.into(),
        Arc::new(lapiz_shader_graph::wgsl_std::types::LayerType {
            texel_type: target_layer_format,
        }),
    );
    types
}

pub fn postprocess_builtin_types(
    target_layer_format: TexelType,
    selection_layer_format: TexelType,
) -> std::collections::BTreeMap<String, Arc<dyn ErasedGraphValueType>> {
    let mut types = brush_builtin_types(target_layer_format, selection_layer_format);
    types.insert(
        MAIN_ACCUMULATE_BUFFER.into(),
        Arc::new(lapiz_shader_graph::wgsl_std::types::LayerType {
            texel_type: target_layer_format,
        }),
    );
    types.insert(STROKE_DATA_BUILTIN.into(), Arc::new(StrokeDataValueType));
    types
}

pub fn brush_graph_resources(
    registry: Arc<GraphNodeRegistry>,
    assets: lapiz_assets::store::AssetRegistry,
) -> GraphResources {
    GraphResources {
        type_registry: BRUSH_GRAPH_TYPES.clone(),
        node_registry: registry,
        assets,
    }
}
