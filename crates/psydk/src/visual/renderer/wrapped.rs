use std::str::FromStr;
use std::sync::{Arc, Mutex};

use super::color_formats::ColorEncoding;
use crate::visual::colors::IntoColor;
use crate::visual::geometry::{IntoSize, Size};
use crate::visual::renderer::colors::RGBA;
use crate::visual::renderer::lottie::PlaybackMode;
use crate::visual::renderer::styles::BlendMode;
use crate::visual::window::WindowStateSnapshot;
use crate::visual::{colors::Color, geometry::Shape, window::WindowState};
use pyo3::exceptions::PyValueError;
use skia::Point;
use skia::Scene as SkScene;

use super::skia;
use psydk_proc::DerefNewtype;
use pyo3::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    #[pyo3(signature = (window_state, shape, brush, draw_style, stroke_style=None, anti_alias=None, blend_mode=None))]
    pub fn draw_shape(
        &mut self,
        window_state: &WindowStateSnapshot,
        shape: &Shape,
        brush: &Brush,
        draw_style: DrawStyle,
        stroke_style: Option<StrokeStyle>,
        anti_alias: Option<bool>,
        blend_mode: Option<&str>,
    ) {
        let windows_size = window_state.size;
        let screen_props = window_state.physical_screen;
        let dc = &*window_state.display_characteristics;
        let skia_blend_mode = blend_mode
            .and_then(|bm| BlendMode::from_str(bm).ok())
            .unwrap_or(BlendMode::SourceOver)
            .into();

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
        }

        // if stroke width is 0, and the draw style is Stroke, we can stop here because we don't need to draw anything
        if skia_paint.stroke_width() == 0.0 && draw_style == DrawStyle::Stroke {
            return;
        }
        // if the stroke width is 0, and the draw style is FillAndStroke, we can set the draw style to Fill because we don't need to stroke anything
        if skia_paint.stroke_width() == 0.0 && draw_style == DrawStyle::FillAndStroke {
            skia_paint.set_style(skia_safe::paint::Style::Fill);
            skia_paint.set_stroke(false);
        }

        skia_paint.set_anti_alias(anti_alias.unwrap_or(false));
        skia_paint.set_blend_mode(skia_blend_mode);

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
    #[pyo3(signature = (window_state, shape, brush, anti_alias=None, blend_mode=None))]
    pub fn draw_shape_filled(
        &mut self,
        window_state: &WindowStateSnapshot,
        shape: &Shape,
        brush: &Brush,
        anti_alias: Option<bool>,
        blend_mode: Option<&str>,
    ) {
        self.draw_shape(
            window_state,
            shape,
            brush,
            DrawStyle::Fill,
            None,
            anti_alias,
            blend_mode,
        );
    }

    /// Draw a stroked shape.
    #[pyo3(signature = (window_state, shape, brush, stroke_style=None, anti_alias=None, blend_mode=None))]
    pub fn draw_shape_stroked(
        &mut self,
        window_state: &WindowStateSnapshot,
        shape: &Shape,
        brush: &Brush,
        stroke_style: Option<StrokeStyle>,
        anti_alias: Option<bool>,
        blend_mode: Option<&str>,
    ) {
        self.draw_shape(
            window_state,
            shape,
            brush,
            DrawStyle::Stroke,
            stroke_style,
            anti_alias,
            blend_mode,
        );
    }

    /// Draw a vector graphic.
    #[pyo3(signature = (window_state, vector_graphic, x, y, width, height, anti_alias=None))]
    pub fn draw_vector_graphic(
        &mut self,
        window_state: &WindowStateSnapshot,
        vector_graphic: &VectorGraphic,
        x: IntoSize,
        y: IntoSize,
        width: IntoSize,
        height: IntoSize,
        anti_alias: Option<bool>,
    ) -> PyResult<()> {
        let windows_size = window_state.size;
        let screen_props = window_state.physical_screen;

        let x = x.0.eval(windows_size, screen_props);
        let y = y.0.eval(windows_size, screen_props);
        let width = width.0.eval(windows_size, screen_props);
        let height = height.0.eval(windows_size, screen_props);
        // get canvas
        self.0.draw_vector_graphic(vector_graphic, (x, y).into(), width, height);
        Ok(())
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

    #[pyo3(name = "start_layer", signature = (opacity=None, blend_mode=None))]
    pub fn start_layer(&mut self, opacity: Option<f32>, blend_mode: Option<&str>) {
        let blend_mode = BlendMode::SourceAtop; // default blend mode

        self.0.start_layer(blend_mode, None, None, opacity.unwrap_or(1.0));
    }

    #[pyo3(name = "end_layer")]
    pub fn end_layer(&mut self) {
        self.0.end_layer();
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
        let color = color
            .0
            .to_display_rgba(&*window_state.display_characteristics, window_state.linear_blending);
        Self(super::brushes::Brush::Solid(color.into()))
    }

    #[staticmethod]
    #[pyo3(name = "gradient_linear")]
    pub fn new_gradient_linear(
        colors: Vec<IntoColor>,
        pos: Vec<f32>,
        start: (IntoSize, IntoSize),
        end: (IntoSize, IntoSize),
        window_state: &WindowStateSnapshot,
    ) -> Self {
        let linear_blending = window_state.linear_blending;
        let color_stops = colors
            .into_iter()
            .zip(pos.into_iter())
            .map(|(color, position)| {
                let color = color
                    .0
                    .to_display_rgba(&*window_state.display_characteristics, linear_blending);
                super::brushes::ColorStop {
                    color: color.into(),
                    offset: position,
                }
            })
            .collect();

        let start = Point {
            x: start.0 .0.eval(window_state.size, window_state.physical_screen),
            y: start.1 .0.eval(window_state.size, window_state.physical_screen),
        };

        let end = Point {
            x: end.0 .0.eval(window_state.size, window_state.physical_screen),
            y: end.1 .0.eval(window_state.size, window_state.physical_screen),
        };

        let gradient_kind = super::brushes::GradientKind::Linear { start, end };

        let gradient = super::brushes::Gradient {
            extend: super::brushes::Extend::Reflect,
            kind: gradient_kind,
            stops: color_stops,
        };

        Self(super::brushes::Brush::Gradient(gradient))
    }

    #[staticmethod]
    #[pyo3(name = "gradient_radial")]
    pub fn new_gradient_radial(
        colors: Vec<IntoColor>,
        pos: Vec<f32>,
        center: (IntoSize, IntoSize),
        radius: IntoSize,
        window_state: &WindowStateSnapshot,
    ) -> Brush {
        let linear_blending = window_state.linear_blending;
        let color_stops = colors
            .into_iter()
            .zip(pos.into_iter())
            .map(|(color, position)| {
                let color = color
                    .0
                    .to_display_rgba(&*window_state.display_characteristics, linear_blending);
                super::brushes::ColorStop {
                    color: color.into(),
                    offset: position,
                }
            })
            .collect();

        let center = Point {
            x: center.0 .0.eval(window_state.size, window_state.physical_screen),
            y: center.1 .0.eval(window_state.size, window_state.physical_screen),
        };

        let radius = radius.0.eval(window_state.size, window_state.physical_screen);

        let gradient_kind = super::brushes::GradientKind::Radial { center, radius };

        let gradient = super::brushes::Gradient {
            extend: super::brushes::Extend::Reflect,
            kind: gradient_kind,
            stops: color_stops,
        };

        Brush(super::brushes::Brush::Gradient(gradient))
    }

    #[staticmethod]
    #[pyo3(name = "image", signature = (bitmap, start, window_state, size=None, edge_mode=("clamp", "clamp"), sampling_mode="linear", alpha=1.0))]
    pub fn new_image(
        bitmap: Bitmap,
        start: (IntoSize, IntoSize),
        window_state: &WindowStateSnapshot,
        size: Option<(IntoSize, IntoSize)>,
        edge_mode: (&str, &str),
        sampling_mode: &str,
        alpha: f32,
    ) -> PyResult<Self> {
        let fit_mode = if let Some((width, height)) = size {
            super::styles::ImageFitMode::Exact {
                width: width.0.eval(window_state.size, window_state.physical_screen),
                height: height.0.eval(window_state.size, window_state.physical_screen),
            }
        } else {
            super::styles::ImageFitMode::Original
        };
        Ok(Self(super::brushes::Brush::Image {
            image: bitmap.0,
            start: Point {
                x: start.0 .0.eval(window_state.size, window_state.physical_screen),
                y: start.1 .0.eval(window_state.size, window_state.physical_screen),
            },
            fit_mode, // TODO: allow fit mode to be specified from python
            sampling: super::brushes::ImageSampling::from_str(sampling_mode).unwrap(),
            edge_mode: (
                super::brushes::Extend::from_str(edge_mode.0).unwrap(),
                super::brushes::Extend::from_str(edge_mode.1).unwrap(),
            ),
            alpha: Some(alpha),
            transform: None,
        }))
    }

    #[staticmethod]
    #[pyo3(name = "checkerboard", signature = (start, square_size, color1, color2, window_state))]
    pub fn new_checkerboard(
        start: (IntoSize, IntoSize),
        square_size: (IntoSize, IntoSize),
        color1: IntoColor,
        color2: IntoColor,
        window_state: &WindowStateSnapshot,
    ) -> Self {
        let color1 = color1
            .0
            .to_display_rgba(&*window_state.display_characteristics, window_state.linear_blending);
        let color2 = color2
            .0
            .to_display_rgba(&*window_state.display_characteristics, window_state.linear_blending);
        Self(super::brushes::Brush::Checkerboard(super::brushes::Checkerboard {
            start_x: start.0 .0.eval(window_state.size, window_state.physical_screen),
            start_y: start.1 .0.eval(window_state.size, window_state.physical_screen),
            square_size_x: square_size.0 .0.eval(window_state.size, window_state.physical_screen),
            square_size_y: square_size.1 .0.eval(window_state.size, window_state.physical_screen),
            color1: color1.into(),
            color2: color2.into(),
        }))
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

#[pyclass(unsendable)]
#[derive(DerefNewtype, Clone)]
pub struct Bitmap(pub super::skia::Bitmap);

#[pymethods]
impl Bitmap {
    #[staticmethod]
    #[pyo3(name = "from_file")]
    pub fn py_from_file(path: &str) -> PyResult<Self> {
        // load the image file using the image crate
        let image =
            image::open(path).map_err(|e| PyValueError::new_err(format!("Failed to load image file: {}", e)))?;

        // conver to f32 RGBA format
        let image = image.to_rgba8();

        Ok(Bitmap(super::skia::Bitmap::from_u8(image, ColorEncoding::Srgb)))
    }
}

#[pyclass(unsendable)]
#[derive(DerefNewtype, Clone)]
pub struct VectorGraphic(pub super::vector::VectorGraphic);

#[pymethods]
impl VectorGraphic {
    #[staticmethod]
    #[pyo3(name = "from_svg_path")]
    pub fn py_from_svg_path(svg_path: &str) -> PyResult<Self> {
        Ok(Self(super::vector::VectorGraphic::from_svg_path(svg_path).map_err(
            |e| PyValueError::new_err(format!("Failed to load SVG file: {}", e)),
        )?))
    }

    #[staticmethod]
    #[pyo3(name = "from_svg_str")]
    pub fn py_from_svg_str(file_contents: &str) -> PyResult<Self> {
        Ok(Self(
            super::vector::VectorGraphic::from_svg_str(file_contents)
                .map_err(|e| PyValueError::new_err(format!("Failed to parse SVG data: {}", e)))?,
        ))
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
