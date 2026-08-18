use anyhow::Result;
use image::ImageFormat;
use lapiz_abr::Pattern;
use lapiz_render::texture::{Image, ImageMetadata};

pub fn parse_patt(patt: &Pattern) -> Result<Image> {
    let content = patt.as_image()?;
    Ok(Image {
        metadata: ImageMetadata {
            name: format!("patt-{}", patt.id),
        },
        image: content,
        format: ImageFormat::Png,
    })
}
