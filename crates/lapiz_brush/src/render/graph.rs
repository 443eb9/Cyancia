use std::sync::{Arc, LazyLock};

use encase::ShaderType;
use glam::Vec4;
use lapiz_effect::nodes::effect_nodes;
use lapiz_image::blend_modes::BlendMode;
use lapiz_shader_graph::{
    GraphElement,
    graph::{
        GraphResources,
        node::{
            GraphNode, GraphNodeCodeGenContext, GraphNodeCodeGenError, GraphNodeCreateSlotsContext,
            GraphNodeDefaultStateContext, GraphNodeRegistry, GraphNodeUpdateContext,
            GraphNodeViewContext, StatelessCommonGraphNode, stateless,
        },
        slot::{ErasedGraphLiteralUpdateMessage, GraphDefaultInputSlot, GraphDefaultOutputSlot},
    },
    wgsl_std::types::{ColorType, F32Type, I32Type, RectType, Vec2FType},
};
use lapiz_utils::random_oklch_hue_chroma;
use lapiz_widgets::combo_box::ComboBox;
use serde::{Deserialize, Serialize};

// TODO We may move to another crate.
#[derive(Debug, Default, Clone, ShaderType)]
pub struct CanvasResources {
    pub foreground_color: Vec4,
    pub background_color: Vec4,
}

// Brush effects expose their results through effect outputs with these
// conventional identifiers; the brush compiler wires the output variables into
// the template exit points. For now brushes are expected to keep them.
pub const SPACING_OUTPUT: &str = "spacing";
pub const DAB_COLOR_OUTPUT: &str = "dab_color";
pub const DAB_BOUNDS_OUTPUT: &str = "dab_bounds";
pub const STROKE_COLOR_OUTPUT: &str = "stroke_color";
pub const STROKE_BOUNDS_OUTPUT: &str = "stroke_bounds";

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
            "let {} = graph_input.position;\n",
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
            "let {} = graph_input.pressure;\n",
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
        Ok(format!(
            "let {} = graph_input.tilt;\n",
            ctx.get_output(0)?
        ))
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
            "let {} = graph_input.angle.x;\nlet {} = graph_input.angle.y;\n",
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
            "let {} = graph_input.draw_direction_angle;\nlet {} = graph_input.draw_direction_vec;\n",
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
            "let {} = i32(graph_input.dab_index);",
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
            "let {} = graph_input.stroke_distance;",
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
            "let {} = graph_input.time.now;\nlet {} = graph_input.time.stroke_begin;\n",
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
        Ok(format!("let {} = pixel_posf;", ctx.get_output(0)?))
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
            "let {output} = image::blend_modes::{}(vec4f({color}.rgb, {color}.a * {opacity}), current_input_color(pixel_pos));\n",
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
            "let {output} = image::blend_modes::{}(vec4f({color}.rgb, {color}.a * {opacity}), target_layer_color(pixel_pos));\n",
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
            "let {} = target_layer_color(vec2i({}));\n",
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
            "let {} = current_input_color(vec2i({}));\n",
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
            "let {} = Rect(vec2f(graph_input.accumulated_pixel_bound.min), vec2f(graph_input.accumulated_pixel_bound.max));",
            ctx.get_output(0)?
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
            "let {output} = load_selection_mask_value(vec2i({input}));\n"
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

pub static BRUSH_GRAPH_TYPES: LazyLock<Arc<lapiz_shader_graph::graph::variable::GraphTypeRegistry>> =
    LazyLock::new(|| Arc::new(brush_graph_types()));

fn brush_graph_types() -> lapiz_shader_graph::graph::variable::GraphTypeRegistry {
    lapiz_shader_graph::wgsl_std::builtin_types()
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
