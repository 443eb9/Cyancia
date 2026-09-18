use std::io::Cursor;

use lapiz_assets::loader::AssetSerializer;
use lapiz_effect::asset::{EffectAssetSerializer, EffectPassDispatchStrategy};
use lapiz_shader_graph::wgsl_std::builtin_types;

#[test]
fn manual_lef_loads_and_round_trips() {
    assert_eq!(EffectAssetSerializer::file_extension(), "lef");

    let serializer = EffectAssetSerializer;
    let mut source = Cursor::new(include_bytes!("fixtures/manual.lef"));
    let asset = serializer.read(&mut source).unwrap();

    assert_eq!(asset.name, "Manual Effect");
    assert_eq!(asset.passes.len(), 1);
    assert_eq!(asset.passes[0].name, "Empty Pass");
    assert_eq!(
        asset.passes[0].dispatch_strategy,
        EffectPassDispatchStrategy::Once
    );
    assert_eq!(asset.inputs.len(), 2);
    assert_eq!(asset.inputs[0].ty, "layer_rgba8");
    assert_eq!(asset.inputs[1].ty, "texture_a8");
    assert_eq!(asset.outputs.len(), 1);
    assert_eq!(asset.outputs[0].ty, "layer_a8");

    let types = builtin_types();
    for id in ["layer_rgba8", "layer_a8", "texture_rgba8", "texture_a8"] {
        assert!(types.resolve_type(id).is_some(), "missing graph type {id}");
    }

    let mut encoded = Vec::new();
    serializer.write(&asset, &mut encoded).unwrap();
    let round_tripped = serializer.read(&mut Cursor::new(encoded)).unwrap();

    assert_eq!(round_tripped.name, asset.name);
    assert_eq!(round_tripped.passes.len(), asset.passes.len());
    assert_eq!(round_tripped.inputs.len(), asset.inputs.len());
    assert_eq!(round_tripped.outputs.len(), asset.outputs.len());
}
