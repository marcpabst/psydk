use crate::color_formats::ColorEncoding;

use super::skia;
use psydk_proc::DerefNewtype;
use pyo3::prelude::*;

#[pyclass]
#[derive(DerefNewtype)]
pub struct Renderer(pub skia::Renderer);

#[pyclass]
#[derive(DerefNewtype, Clone)]
pub struct Scene(pub skia::Scene);

#[pymethods]
impl Scene {
    pub fn set_bg_color(&mut self, color: (f32, f32, f32, f32)) {
        self.0.set_bg_color(super::colors::RGBA::new(
            color.0,
            color.1,
            color.2,
            color.3,
            ColorEncoding::Linear,
        ));
    }
}

#[pyclass]
#[derive(DerefNewtype, Clone)]
pub struct Shape(pub super::shapes::Shape);

#[pymethods]
impl Shape {
    #[new]
    pub fn new_circle(x: f32, y: f32, radius: f32) -> Self {
        Self(super::shapes::Shape::circle((x, y), radius))
    }
    #[staticmethod]
    pub fn new_rectangle(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self(super::shapes::Shape::rectangle((x, y), width, height))
    }
    #[staticmethod]
    pub fn new_rounded_rectangle(x: f32, y: f32, width: f32, height: f32, corner_radius: f32) -> Self {
        Self(super::shapes::Shape::rounded_rectangle(
            (x, y),
            width,
            height,
            corner_radius,
        ))
    }
}
