use lapiz_image::texel::TexelType;

use crate::graph::{node::GraphNodeRegistry, variable::GraphTypeRegistry};

pub mod casters;
pub mod nodes;
pub mod types;

pub fn builtin_nodes() -> GraphNodeRegistry {
    use nodes::*;

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
    nodes.register::<TextureSizeNode>();
    nodes.register::<GraphFunctionNode>();
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
    types.register_type::<U32Type>();
    types.register_type::<BoolType>();
    types.register_type::<AtomicI32Type>();
    types.register_type::<AtomicU32Type>();
    types.register_type::<Vec2FType>();
    types.register_type::<Vec3FType>();
    types.register_type::<Vec4FType>();
    types.register_type::<Vec2IType>();
    types.register_type::<Vec3IType>();
    types.register_type::<Vec4IType>();
    types.register_type::<Vec2UType>();
    types.register_type::<Vec3UType>();
    types.register_type::<Vec4UType>();
    types.register_type::<ColorType>();
    types.register_type::<RectType>();

    for texel_type in TexelType::ALL_POSSIBLE_FORMATS {
        types.register_type_value(TextureType { texel_type });
        types.register_type_value(LayerType { texel_type });
    }

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
