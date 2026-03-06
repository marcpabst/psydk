use std::str::FromStr;
use std::sync::{Arc, Mutex};

use super::color_formats::ColorEncoding;
use crate::visual::colors::IntoColor;
use crate::visual::geometry::{IntoSize, Size};
use crate::visual::renderer::colors::RGBA;
use crate::visual::renderer::lottie::PlaybackMode;
use crate::visual::window::WindowStateSnapshot;
use crate::visual::{colors::Color, geometry::Shape, window::WindowState};
use pyo3::exceptions::PyValueError;
use skia::Point;
use skia::Scene as SkScene;

use super::skia;
use psydk_proc::DerefNewtype;
use pyo3::prelude::*;

#[derive(Debug, Clone, Copy)]
#[pyclass]
pub enum DrawStyle {
    Fill,
    Stroke,
    FillAndStroke,
}

#[pyclass]
#[derive(DerefNewtype)]
pub struct Renderer(pub skia::Renderer);

#[pyclass]
#[derive(DerefNewtype, Clone, Debug)]
pub struct Scene(pub skia::Scene);

#[pyclass]
#[derive(DerefNewtype, Clone, Debug)]
pub struct Brush(pub super::brushes::Brush);

#[pyclass]
#[derive(DerefNewtype, Clone, Debug)]
pub struct StrokeStyle(pub super::styles::StrokeStyle);

#[pymethods]
impl Scene {
    pub fn set_bg_color(&mut self, color: (f32, f32, f32, f32)) {
        self.0
            .set_bg_color(super::colors::RGBA::new(color.0, color.1, color.2, color.3));
    }

    /// Draw a shape.
    #[pyo3(signature = (window_state, shape, brush, draw_style, stroke_style=None, anti_alias=None))]
    pub fn draw_shape(
        &mut self,
        window_state: &WindowStateSnapshot,
        shape: &Shape,
        brush: &Brush,
        draw_style: DrawStyle,
        stroke_style: Option<StrokeStyle>,
        anti_alias: Option<bool>,
    ) {
        let windows_size = window_state.size;
        let screen_props = window_state.physical_screen;
        let dc = &*window_state.display_characteristics;

        // get canvas
        let mut binding = self.picture_recorder.lock().unwrap();
        let mut canvas = binding.recording_canvas().unwrap();

        // map brush to skia brush
        let mut skia_paint: skia_safe::Paint = brush.0.clone().into();

        match draw_style {
            DrawStyle::Fill => {
                skia_paint.set_style(skia_safe::paint::Style::Fill);
                skia_paint.set_stroke(false);
            }
            DrawStyle::Stroke => {
                skia_paint.set_style(skia_safe::paint::Style::Stroke);
                skia_paint.set_stroke(true);
            }
            DrawStyle::FillAndStroke => {
                skia_paint.set_style(skia_safe::paint::Style::StrokeAndFill);
                skia_paint.set_stroke(true);
            }
        };

        if let Some(stroke_style) = stroke_style {
            skia_paint.set_stroke_width(stroke_style.0.width);
        } else {
            skia_paint.set_stroke_width(1.0);
        }

        skia_paint.set_anti_alias(anti_alias.unwrap_or(false));

        match shape {
            Shape::Rectangle { x, y, width, height } => {
                // resolve the x, y, width, and height using the window state
                let x = x.eval(windows_size, screen_props);
                let y = y.eval(windows_size, screen_props);
                let width = width.eval(windows_size, screen_props);
                let height = height.eval(windows_size, screen_props);

                SkScene::draw_rectangle(&canvas, skia_paint, (x, y).into(), width, height, None);
            }
            Shape::RectangleRounded {
                x,
                y,
                width,
                height,
                radii,
            } => {
                let x = x.eval(windows_size, screen_props);
                let y = y.eval(windows_size, screen_props);
                let width = width.eval(windows_size, screen_props);
                let height = height.eval(windows_size, screen_props);

                let radius_x = radii.0.eval(windows_size, screen_props);
                let radius_y = radii.1.eval(windows_size, screen_props);

                SkScene::draw_rounded_rectangle(
                    &canvas,
                    skia_paint,
                    (x, y).into(),
                    width,
                    height,
                    radius_x,
                    radius_y,
                    None,
                );
            }
            Shape::Circle { x, y, radius } => {
                let x = x.eval(windows_size, screen_props);
                let y = y.eval(windows_size, screen_props);
                let radius = radius.eval(windows_size, screen_props);
                SkScene::draw_circle(canvas, skia_paint, (x, y).into(), radius, None);
            }
            Shape::Ellipse {
                x,
                y,
                radius_x,
                radius_y,
            } => {
                let x = x.eval(windows_size, screen_props);
                let y = y.eval(windows_size, screen_props);
                let radius_x = radius_x.eval(windows_size, screen_props);
                let radius_y = radius_y.eval(windows_size, screen_props);

                SkScene::draw_ellipse(canvas, skia_paint, (x, y).into(), radius_x, radius_y, 0.0, None);
            }
            Shape::Line { x1, y1, x2, y2 } => {
                let x1 = x1.eval(windows_size, screen_props);
                let y1 = y1.eval(windows_size, screen_props);
                let x2 = x2.eval(windows_size, screen_props);
                let y2 = y2.eval(windows_size, screen_props);

                SkScene::draw_line(canvas, skia_paint, (x1, y1).into(), (x2, y2).into(), None);
            }
            _ => {
                println!("Shape type not supported yet: {:?}", shape);
            }
        }
    }

    /// Draw a filled shape.
    #[pyo3(signature = (window_state, shape, brush, anti_alias=None))]
    pub fn draw_shape_filled(
        &mut self,
        window_state: &WindowStateSnapshot,
        shape: &Shape,
        brush: &Brush,
        anti_alias: Option<bool>,
    ) {
        self.draw_shape(window_state, shape, brush, DrawStyle::Fill, None, anti_alias);
    }

    /// Draw a stroked shape.
    #[pyo3(signature = (window_state, shape, brush, stroke_style=None, anti_alias=None))]
    pub fn draw_shape_stroked(
        &mut self,
        window_state: &WindowStateSnapshot,
        shape: &Shape,
        brush: &Brush,
        stroke_style: Option<StrokeStyle>,
        anti_alias: Option<bool>,
    ) {
        self.draw_shape(window_state, shape, brush, DrawStyle::Stroke, stroke_style, anti_alias);
    }

    /// Draw a Lottie animation.
    #[pyo3(signature = (window_state, animation, bounding_rect, anti_alias=None))]
    pub fn draw_lottie_animation(
        &mut self,
        window_state: &WindowStateSnapshot,
        animation: &mut LottieAnimation,
        bounding_rect: Option<Shape>,
        anti_alias: Option<bool>,
    ) -> PyResult<()> {
        let windows_size = window_state.size;
        let screen_props = window_state.physical_screen;
        // get canvas
        let mut binding = self.picture_recorder.lock().unwrap();
        let mut canvas = binding.recording_canvas().unwrap();

        // make sure the bounding rect is a rectangle
        match bounding_rect {
            Some(Shape::Rectangle { x, y, width, height }) => {
                let x = x.eval(windows_size, screen_props);
                let y = y.eval(windows_size, screen_props);
                let width = width.eval(windows_size, screen_props);
                let height = height.eval(windows_size, screen_props);
                SkScene::draw_lottie_animation(canvas, &mut animation.0, Some((x, y, width, height)));
            }
            None => {
                SkScene::draw_lottie_animation(canvas, &mut animation.0, None);
            }
            _ => {
                return Err(PyValueError::new_err("Bounding rect must be a rectangle shape"));
            }
        };

        Ok(())
    }

    #[pyo3(signature = (window_state, text, paint=None))]
    fn build_text(&mut self, window_state: &WindowStateSnapshot, text: &mut Text, paint: Option<&Brush>) {
        let windows_size = window_state.size;
        let screen_props = window_state.physical_screen;

        let skia_paint = paint.map(|b| b.0.clone().into());

        // get canvas
        let mut binding = self.picture_recorder.lock().unwrap();
        let mut canvas = binding.recording_canvas().unwrap();

        text.build(
            &FontCollection::new(self.0.font_collection.clone()),
            skia_paint,
            window_state,
        );
    }

    #[pyo3(signature = (window_state, text, x = IntoSize(crate::visual::geometry::Size::Pixels(0.0)), y = IntoSize(crate::visual::geometry::Size::Pixels(0.0))))]
    fn draw_text(&mut self, window_state: &WindowStateSnapshot, text: &mut Text, x: IntoSize, y: IntoSize) {
        let windows_size = window_state.size;
        let screen_props = window_state.physical_screen;

        let x = x.0.eval(windows_size, screen_props);
        let y = y.0.eval(windows_size, screen_props);

        // let mut skia_paint: skia_safe::Paint = brush.0.clone().into();

        let mut binding = self.picture_recorder.lock().unwrap();
        let mut canvas = binding.recording_canvas().unwrap();

        // update font size and color of the text style based on the provided brush and font size
        text.inner
            .set_font_size(text.font_size.eval(windows_size, screen_props));

        text.inner.draw(&canvas, x, y);
    }
}

#[pymethods]
impl Renderer {
    pub fn font_collection(&self) -> FontCollection {
        FontCollection::new(self.0.font_collection.clone())
    }
}

#[pymethods]
impl Brush {
    #[staticmethod]
    #[pyo3(name = "solid")]
    pub fn new_solid(color: IntoColor, window_state: &WindowStateSnapshot) -> Self {
        let color = color.0.to_display_rgba(&*window_state.display_characteristics);
        Self(super::brushes::Brush::Solid(color.into()))
    }
}

#[pymethods]
impl StrokeStyle {
    #[new]
    pub fn new(width: IntoSize, window_state: &WindowStateSnapshot) -> Self {
        let width = width.0.eval(window_state.size, window_state.physical_screen);
        Self(super::styles::StrokeStyle::new(width))
    }
}

#[pyclass]
#[derive(DerefNewtype, Clone)]
pub struct LottieAnimation(pub super::lottie::LottieAnimation);

#[pymethods]
impl LottieAnimation {
    #[staticmethod]
    #[pyo3(name = "from_file")]
    pub fn py_from_file(path: &str, playback_mode: &str, speed: f32) -> PyResult<Self> {
        let playback_mode =
            PlaybackMode::from_str(playback_mode).map_err(|_| PyValueError::new_err("Invalid playback mode"))?;

        Ok(Self(super::lottie::LottieAnimation::from_file(
            path,
            playback_mode,
            speed,
        )))
    }

    pub fn play(&mut self) {
        self.0.play();
    }

    pub fn pause(&mut self) {
        self.0.pause();
    }

    pub fn is_playing(&self) -> bool {
        self.0.is_playing()
    }
}

#[pyclass(unsendable)]
pub struct Text {
    inner: super::text::Text,
    font_size: Size,
    layout_width: Option<Size>,
}

#[pyclass(unsendable)]
#[derive(DerefNewtype, Clone)]
pub struct FontCollection(Arc<Mutex<skia_safe::textlayout::FontCollection>>);

impl FontCollection {
    pub fn new(font_collection: Arc<Mutex<skia_safe::textlayout::FontCollection>>) -> Self {
        Self(font_collection)
    }
}

#[pymethods]
impl Text {
    #[new]
    #[pyo3(signature = (text, font_family=None, font_size=None, layout_width=None))]
    pub fn new(
        text: &str,
        font_family: Option<String>,
        font_size: Option<IntoSize>,
        layout_width: Option<IntoSize>,
    ) -> Self {
        let font_size = font_size.map(|fs| fs.0);
        let layout_width = layout_width.map(|lw| lw.0);

        Self {
            inner: super::text::Text::new(text, font_family, None, None),
            font_size: (font_size.unwrap_or(Size::Pixels(16.0))),
            layout_width,
        }
    }

    pub fn build(
        &mut self,
        font_collection: &FontCollection,
        paint: Option<Brush>,
        window_state: &WindowStateSnapshot,
    ) {
        let skia_paint = paint.map(|b| b.0.clone().into());

        // update paint, font size, and layout width of the text style based on the provided brush, font size, and layout width
        self.inner
            .set_font_size(self.font_size.eval(window_state.size, window_state.physical_screen));
        self.inner
            .set_paint(&skia_paint.unwrap_or_else(|| skia_safe::Paint::default()));

        if let Some(layout_width) = &self.layout_width {
            let layout_width = layout_width.eval(window_state.size, window_state.physical_screen);
            self.inner.layout_width = Some(layout_width);
        }

        self.inner.build(&font_collection.0.lock().unwrap());
    }

    pub fn measure(&mut self) -> Option<(f32, f32)> {
        self.inner.measure()
    }

    #[setter]
    pub fn set_layout_width(&mut self, layout_width: IntoSize) {
        self.layout_width = Some(layout_width.0);
    }
}
