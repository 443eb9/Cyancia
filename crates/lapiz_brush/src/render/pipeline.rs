use std::borrow::Cow;

use lapiz_render::{
    bind_group_entries::BindGroupEntries,
    bind_group_layout_entries::{BindGroupLayoutEntries, binding_types},
    buffer::DynamicBuffer,
};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupLayout, BindGroupLayoutDescriptor, ComputePass,
    ComputePipeline, ComputePipelineDescriptor, Device, PipelineLayoutDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages,
};

use crate::render::{ComputedPenInput, InputSampler, OutputSamples, PenInputBatch};

pub struct PreparedInputSamplingPipelineData {
    bind_group: BindGroup,
}

pub struct BrushInputSamplingPipeline {
    layout: BindGroupLayout,
    pipeline: ComputePipeline,
}

impl BrushInputSamplingPipeline {
    pub fn new(
        device: &Device,
        resource_layout: &BindGroupLayout,
        compiled_shader: Cow<'_, str>,
    ) -> Self {
        let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("brush input sampling layout"),
            entries: BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    binding_types::storage_buffer_read_only::<PenInputBatch>(false),
                    binding_types::storage_buffer::<InputSampler>(false),
                    binding_types::storage_buffer::<OutputSamples>(false),
                    binding_types::storage_buffer::<ComputedPenInput>(false),
                ),
            )
            .as_ref(),
        });
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("brush input sampling shader"),
            source: ShaderSource::Wgsl(compiled_shader),
        });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("brush input sampling pipeline layout"),
            bind_group_layouts: &[Some(&layout), Some(resource_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("brush input sampling pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        Self { layout, pipeline }
    }

    #[must_use]
    pub fn prepare(
        &self,
        device: &Device,
        input_batch: &DynamicBuffer<PenInputBatch>,
        input_sampler: &DynamicBuffer<InputSampler>,
        output_samples: &DynamicBuffer<OutputSamples>,
        initial_pen_input: &DynamicBuffer<ComputedPenInput>,
    ) -> PreparedInputSamplingPipelineData {
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("brush input sampling bind group"),
            layout: &self.layout,
            entries: BindGroupEntries::sequential((
                input_batch.inner_buffer().unwrap().as_entire_binding(),
                input_sampler.binding().unwrap(),
                output_samples.inner_buffer().unwrap().as_entire_binding(),
                initial_pen_input.binding().unwrap(),
            ))
            .as_ref(),
        });
        PreparedInputSamplingPipelineData { bind_group }
    }

    pub fn dispatch(
        &self,
        pass: &mut ComputePass,
        data: &PreparedInputSamplingPipelineData,
        resource_group: &BindGroup,
    ) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &data.bind_group, &[]);
        pass.set_bind_group(1, resource_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
}
