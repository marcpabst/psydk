use std::{
    any::Any,
    sync::{Arc, Mutex},
};

use cosmic_text::{
    Attrs, Buffer as CosmicBuffer, Family as CosmicFamily, Metrics as CosmicMetrics, Stretch as CosmicStretch,
    Style as CosmicStyle, Weight as CosmicWeight,
};
use image::DynamicImage;

use super::scenes::{DynamicScene, Scene};
use crate::{
    bitmaps::DynamicBitmap,
    font::{DynamicFontFace, FontStyle, FontWidth},
    shapes::Point,
};

pub struct DynamicRenderer {
    backend: Box<dyn Renderer>,
}

pub struct DynamicRenderResources {
    pub resources: Box<dyn SharedRendererState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
    LinearSrgb,
    Srgb,
}

impl DynamicRenderer {
    pub(crate) fn new(backend_renderer: Box<dyn Renderer>) -> Self {
        DynamicRenderer {
            backend: backend_renderer,
        }
    }
    pub fn render_to_texture(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
        scene: &mut DynamicScene,
    ) {
        self.backend
            .render_to_texture(device, queue, texture, width, height, scene.inner().as_mut());
    }

    pub fn create_scene(&self, width: u32, heigth: u32) -> DynamicScene {
        let scene = self.backend.create_scene(width, heigth);
        DynamicScene::new(scene)
    }

    pub fn create_bitmap_u8(&self, data: image::RgbaImage, color_space: ColorSpace) -> DynamicBitmap {
        self.backend.create_bitmap_u8(data, color_space)
    }

    pub fn create_bitmap_f32(
        &self,
        data: image::ImageBuffer<image::Rgba<f32>, Vec<f32>>,
        color_space: ColorSpace,
    ) -> DynamicBitmap {
        self.backend.create_bitmap_f32(data, color_space)
    }

    pub fn create_bitmap_from_path(&self, path: &str) -> DynamicBitmap {
        self.backend.create_bitmap_from_path(path)
    }
}

/// A Renderer is responsible for rendering scenes to textures. There is one renderer per window or surface.
pub trait Renderer {
    fn render_to_texture(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
        scene: &mut dyn Scene,
    );

    fn create_scene(&self, width: u32, heigth: u32) -> Box<dyn Scene>;

    fn load_font_face(
        &mut self,
        face_info: &cosmic_text::fontdb::FaceInfo,
        font_data: &[u8],
        index: usize,
    ) -> DynamicFontFace;

    fn create_bitmap_u8(&self, data: image::RgbaImage, color_space: ColorSpace) -> DynamicBitmap;
    fn create_bitmap_f32(
        &self,
        data: image::ImageBuffer<image::Rgba<f32>, Vec<f32>>,
        color_space: ColorSpace,
    ) -> DynamicBitmap;

    fn create_bitmap_from_path(&self, path: &str) -> DynamicBitmap {
        let image = image::open(path).unwrap().to_rgba8();
        self.create_bitmap_u8(image, ColorSpace::Srgb)
    }

    fn create_bitmap_from_wgpu_texture(
        &self,
        texture: wgpu::Texture,
        color_space: crate::renderer::ColorSpace,
    ) -> DynamicBitmap;
}

/// A SharedRendererState is a trait that provides methods to create renderers, bitmaps, and font faces.
/// Structs implementing this trait can implement caches and other shared resources that are used across multiple renderers.
pub trait SharedRendererState: Send + Sync {
    fn create_renderer(&self, surface_format: wgpu::TextureFormat, width: u32, height: u32) -> DynamicRenderer;

    fn create_bitmap_u8(&self, data: image::RgbaImage, color_space: ColorSpace) -> DynamicBitmap;

    fn create_bitmap_f32(
        &self,
        data: image::ImageBuffer<image::Rgba<f32>, Vec<f32>>,
        color_space: ColorSpace,
    ) -> DynamicBitmap;

    fn create_bitmap_from_path(&self, path: &str) -> DynamicBitmap {
        let image = image::open(path).unwrap();
        // convert to RGBA
        let image = image.to_rgba8();
        self.create_bitmap_u8(image, ColorSpace::Srgb)
    }

    fn create_font_face(&self, font_data: &[u8], index: u32) -> DynamicFontFace;

    fn as_any(&self) -> &dyn Any;

    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn create_bitmap_from_wgpu_texture(
        &self,
        texture: wgpu::Texture,
        color_space: crate::renderer::ColorSpace,
    ) -> DynamicBitmap;

    // Returns the render resources
    fn render_resources(&self) -> Option<DynamicRenderResources>;

    fn cloned(&self) -> Box<dyn SharedRendererState>;
}
