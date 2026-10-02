use lapiz_shader_graph_derive::stateless;
use lapiz_utils::random_oklch_hue_chroma;
use wesl::syntax::{
    BinaryExpression, BinaryOperator, Declaration, DeclarationKind, Expression, FunctionCall,
    Ident, IndexingExpression, NamedComponentExpression, Span, Spanned, Statement, TypeExpression,
};
use wesl_quote::quote_statement;

use crate::{
    graph::{
        node::{
            GraphNodeCodeGenContext, GraphNodeCodeGenError, GraphNodeCreateSlotsContext,
            StatelessCommonGraphNode,
        },
        slot::{GraphDefaultInputSlot, GraphDefaultOutputSlot},
    },
    wgsl_std::types::{
        compound::ColorType, handle::TextureType, primitive::F32Type, vector::Vec2FType,
    },
};

#[derive(Default, Clone)]
pub struct GetPixelColorNode;

#[stateless]
impl StatelessCommonGraphNode for GetPixelColorNode {
    fn id(&self) -> &'static str {
        "get_pixel_color_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(GetPixelColorNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![
            GraphDefaultInputSlot::new::<TextureType>("texture".into()),
            GraphDefaultInputSlot::new::<Vec2FType>("position".into()),
        ]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<ColorType>("color".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let input_texture = ctx.get_input(0)?;
        let input_position = ctx.get_input(1)?;
        let output_color = ctx.get_output(0)?;

        // TODO: sample_local_texture is only defined in `brush_template.wesl`
        Ok(format!(
            "{}\n",
            quote_statement! {
                let #output_color = sample_local_texture(#input_texture, vec2u(#input_position));
            }
        ))
    }
}

// TODO: Mixing in different color spaces.
#[derive(Default, Clone)]
pub struct ColorMixNode;

#[stateless]
impl StatelessCommonGraphNode for ColorMixNode {
    fn id(&self) -> &'static str {
        "color_mix_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(ColorMixNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![
            GraphDefaultInputSlot::new::<ColorType>("color_a".into()),
            GraphDefaultInputSlot::new::<ColorType>("color_b".into()),
            GraphDefaultInputSlot::new::<F32Type>("factor".into()),
        ]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<ColorType>("result".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let input_color_a = ctx.get_input(0)?;
        let input_color_b = ctx.get_input(1)?;
        let input_factor = ctx.get_input(2)?;
        let output = ctx.get_output(0)?;

        Ok(format!(
            "{}\n",
            quote_statement! { let #output = mix(#input_color_a, #input_color_b, #input_factor); }
        ))
    }
}

#[derive(Default, Clone)]
pub struct TextureSizeNode;

#[stateless]
impl StatelessCommonGraphNode for TextureSizeNode {
    fn id(&self) -> &'static str {
        "texture_size_node"
    }

    fn header_hue_chroma(&self) -> (f32, f32) {
        random_oklch_hue_chroma!(TextureSizeNode)
    }

    fn create_inputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultInputSlot> {
        vec![GraphDefaultInputSlot::new::<TextureType>("texture".into())]
    }

    fn create_outputs(&self, _: GraphNodeCreateSlotsContext<'_>) -> Vec<GraphDefaultOutputSlot> {
        vec![GraphDefaultOutputSlot::new::<Vec2FType>("size".into())]
    }

    fn generate_code(
        &self,
        mut ctx: GraphNodeCodeGenContext<'_>,
    ) -> Result<String, GraphNodeCodeGenError> {
        let input_texture = ctx.get_input(0)?;
        let output_size = ctx.get_output(0)?;

        Ok(format!(
            "{}\n",
            quote_statement! {
                let #output_size =
                    vec2f(texture_bounds[#input_texture].max - texture_bounds[#input_texture].min);
            }
        ))
    }
}
