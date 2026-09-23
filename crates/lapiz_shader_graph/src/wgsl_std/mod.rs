use crate::graph::{GraphData, node::GraphNodeRegistry, variable::GraphTypeRegistry};

pub mod casters;
pub mod nodes;
pub mod types;

pub fn builtin_nodes<Data: GraphData>() -> GraphNodeRegistry<Data> {
    use nodes::{
        ClampNode, ColorMixNode, CombineColorComponentsNode, CombineComponentsNode, CompareNode,
        CurveNode, CustomExpressionNode, ExternalVariableNode, GetPixelColorNode,
        GraphFunctionNode, RandomNode, RectMathNode, RepeatNode, ScalarMathNode, ScalarSelectNode,
        SmoothStepNode, SplitColorComponentsNode, SplitComponentsNode, StepNode, TextureNode,
        TextureSizeNode, VectorMathNode, VectorSelectNode,
    };

    let mut nodes = GraphNodeRegistry::with_capacity();

    nodes.register::<ScalarMathNode>();
    nodes.register::<VectorMathNode>();
    nodes.register::<RectMathNode>();
    nodes.register::<CompareNode>();
    nodes.register::<ScalarSelectNode>();
    nodes.register::<VectorSelectNode>();
    nodes.register::<ClampNode>();
    nodes.register::<RandomNode>();
    nodes.register::<StepNode>();
    nodes.register::<SmoothStepNode>();
    nodes.register::<SplitComponentsNode>();
    nodes.register::<CombineComponentsNode>();
    nodes.register::<SplitColorComponentsNode>();
    nodes.register::<CombineColorComponentsNode>();
    nodes.register::<GetPixelColorNode>();
    nodes.register::<ColorMixNode>();
    nodes.register::<TextureNode>();
    nodes.register::<TextureSizeNode>();
    nodes.register::<GraphFunctionNode>();
    nodes.register::<ExternalVariableNode>();
    nodes.register::<CurveNode>();
    nodes.register::<RepeatNode>();
    nodes.register::<CustomExpressionNode>();

    nodes
}

pub fn builtin_types() -> GraphTypeRegistry {
    use casters::{
        BoolToI32Caster, F32ToI32Caster, F32ToVec2FCaster, I32ToBoolCaster, I32ToF32Caster,
        I32ToVec2FCaster, Vec2FToF32Caster, Vec2FToI32Caster,
    };
    use types::{BoolType, ColorType, F32Type, I32Type, RectType, TextureType, Vec2FType};

    let mut types = GraphTypeRegistry::default();

    types.register_type::<F32Type>();
    types.register_type::<I32Type>();
    types.register_type::<BoolType>();
    types.register_type::<Vec2FType>();
    types.register_type::<ColorType>();
    types.register_type::<TextureType>();
    types.register_type::<RectType>();

    types.register_caster::<F32ToVec2FCaster>();
    types.register_caster::<Vec2FToF32Caster>();
    types.register_caster::<BoolToI32Caster>();
    types.register_caster::<I32ToBoolCaster>();
    types.register_caster::<F32ToI32Caster>();
    types.register_caster::<I32ToF32Caster>();
    types.register_caster::<Vec2FToI32Caster>();
    types.register_caster::<I32ToVec2FCaster>();

    types
}
