use anyhow::Result;
use image::{DynamicImage, ImageFormat};
use lapiz_abr::{Sample, SampleImage};
use lapiz_render::texture::{Image, ImageMetadata};

pub fn parse_samp(samp: &Sample) -> Result<Image> {
    let content = match samp.as_image()? {
        SampleImage::Bit8(img) => DynamicImage::ImageLuma8(img),
        SampleImage::Bit16(img) => DynamicImage::ImageLuma16(img),
    };

    Ok(Image {
        metadata: ImageMetadata {
            name: format!("samp-{}", samp.id),
        },
        image: content,
        format: ImageFormat::Png,
    })
}
