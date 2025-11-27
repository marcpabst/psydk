pub mod affine;
pub mod bitmaps;
pub mod brushes;
pub mod color_formats;
pub mod colors;
pub mod effects;
pub mod font;
pub mod renderer;
pub mod scenes;
pub mod shapes;
pub mod skia_backend;
pub mod styles;
pub mod svg;
mod utils;

pub mod wgpu_renderer;

pub use cosmic_text;

// re-export the image crate
// re-export the renderer
pub use bitmaps::DynamicBitmap;
pub use image;
pub use renderer::DynamicRenderer;
pub use scenes::DynamicScene;
// re-export wgpu crate
pub use wgpu;

pub enum Backend {
    Vello,
}
