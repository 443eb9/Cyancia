use std::sync::Arc;

use cyancia_image::CImage;

pub mod action;
pub mod render;
pub mod resource;
pub mod widget;

#[derive(Debug)]
pub struct CCanvas {
    pub image: Arc<CImage>,
}
