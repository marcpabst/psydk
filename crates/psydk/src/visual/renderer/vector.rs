use skia_safe::FontMgr;

use crate::visual::window::WindowStateSnapshot;

#[derive(Clone)]
pub struct VectorGraphic {
    pub vector: skia_safe::svg::Dom,
}

impl VectorGraphic {
    pub fn from_svg_str(svg_str: &str) -> Result<Self, String> {
        let vector = skia_safe::svg::Dom::from_str(svg_str, FontMgr::default())
            .map_err(|e| format!("Failed to parse SVG string: {}", e))?;
        Ok(Self { vector })
    }

    pub fn from_svg_path(svg_path: &str) -> Result<Self, String> {
        let svg_str =
            std::fs::read_to_string(svg_path).map_err(|e| format!("Failed to read SVG file '{}': {}", svg_path, e))?;
        Self::from_svg_str(&svg_str)
    }
}
