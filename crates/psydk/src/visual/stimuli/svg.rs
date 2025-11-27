use crate::visual::colors::Color;
use crate::visual::colors::IntoColor;
use num_traits::Bounded;
use numpy::{PyReadonlyArray2, PyReadonlyArray3, PyReadonlyArray4, PyUntypedArrayMethods};
use psydk_proc::StimulusParams;
use pyo3::ffi::c_str;
use renderer::image::{ImageBuffer, Pixel, RgbImage, Rgba, RgbaImage};
use renderer::svg::DynamicSVG;
use renderer::{
    brushes::{Brush, Extend, ImageSampling},
    image::Rgb,
    shapes::Shape,
    styles::ImageFitMode,
    DynamicBitmap, DynamicScene,
};
use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

use super::{
    animations::Animation,
    helpers::{self, get_experiment_context},
    impl_pystimulus_for_wrapper, PyStimulus, Stimulus, StimulusParamValue, StimulusParams,
};
use crate::{
    context::{ExperimentContext, PyRendererFactory},
    visual::{
        geometry::{Anchor, Size, Transformation2D},
        window::{Frame, WindowState},
    },
};

#[derive(StimulusParams, Clone, Debug)]
/// Parameters for the ImageStimulus.
pub struct SVGParams {
    /// x position of the stimulus.
    pub x: Size,
    /// y position of the stimulus.
    pub y: Size,
    /// Width of the stimulus.
    pub width: Size,
    /// Height of the stimulus.
    pub height: Size,
    /// Rotation of the stimulus in degrees.
    pub rotation: f64,
    /// Opacity of the stimulus, from 0.0 (transparent) to 1.0 (opaque).
    pub opacity: f64,
}

#[derive(Debug)]
pub struct SVGStimulus {
    /// Unique identifier for the stimulus.
    id: uuid::Uuid,
    /// Parameters for the image stimulus.
    params: SVGParams,
    /// The image to be displayed.
    svg: DynamicSVG,
    /// The anchor point of the image stimulus for positioning.
    anchor: Anchor,
    /// The transformation applied to the image stimulus.
    transformation: Transformation2D,
    /// List of animations associated with the stimulus.
    animations: Vec<Animation>,
    /// Whether the image stimulus is currently visible.
    visible: bool,
}

unsafe impl Send for SVGStimulus {}

impl SVGStimulus {
    /// Creates a new `ImageStimulus` from an image and parameters.
    pub fn from_svg(svg: DynamicSVG, params: SVGParams, transform: Option<Transformation2D>, anchor: Anchor) -> Self {
        Self {
            id: Uuid::new_v4(),
            transformation: transform.unwrap_or_else(|| Transformation2D::Identity()),
            animations: Vec::new(),
            visible: true,
            svg,
            anchor,
            params,
        }
    }
}

#[derive(Debug, Clone)]
#[pyclass(name = "SVGStimulus", extends=PyStimulus)]
pub struct PySVGStimulus();

#[pymethods]
impl PySVGStimulus {
    #[new]
    #[pyo3(signature = (
        src,
        x,
        y,
        width,
        height,
        rotation = 0.0,
        opacity = 1.0,
        anchor = Anchor::Center,
        transform = None,
        srgb = true,
        context = None,
    ))]
    /// Creates a new `ImageStimulus` from a file path.
    ///
    /// Parameters
    /// ----------
    /// src : str
    ///     The file path to the svg image.
    /// x : Size, num, or str
    ///     The x position of the stimulus.
    /// y : Size, num, or str
    ///     The y position of the stimulus.
    /// width : Size, num, or str
    ///     The width of the stimulus.
    /// height : Size, num, or str
    /// The height of the stimulus.
    /// rotation : float, optional
    ///
    fn __new__(
        py: Python,
        src: String,
        x: IntoSize,
        y: IntoSize,
        width: IntoSize,
        height: IntoSize,
        rotation: f64,
        opacity: f64,
        anchor: Anchor,
        transform: Option<Transformation2D>,
        srgb: bool,
        context: Option<ExperimentContext>,
    ) -> PyResult<(Self, PyStimulus)> {
        let ctx = get_experiment_context(context, py)?;

        // ignore srgb for SVGs for now
        let svg_str = std::fs::read_to_string(&src)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to read SVG file: {}", e)))?;

        // Load the SVG from the provided source
        let svg = ctx.renderer_factory().create_svg(&svg_str);

        Ok((
            Self(),
            PyStimulus::new(SVGStimulus::from_svg(
                svg,
                SVGParams {
                    x: x.into(),
                    y: y.into(),
                    width: width.into(),
                    height: height.into(),
                    rotation,
                    opacity,
                },
                transform,
                anchor,
            )),
        ))
    }
}

impl_pystimulus_for_wrapper!(PySVGStimulus, SVGStimulus);

impl Stimulus for SVGStimulus {
    fn uuid(&self) -> Uuid {
        self.id
    }

    fn draw(&mut self, scene: &mut DynamicScene, window_state: &WindowState) {
        if !self.visible {
            return;
        }

        let window_size = window_state.size;
        let screen_props = window_state.physical_screen;

        // convert physical units to pixels
        let x = self.params.x.eval(window_size, screen_props);
        let y = self.params.y.eval(window_size, screen_props);

        let width = self.params.width.eval(window_size, screen_props);
        let height = self.params.height.eval(window_size, screen_props);

        let (x, y) = self.anchor.to_top_left(x, y, width, height);

        // let image_offset_x = self.params.image_x.eval(window_size, screen_props);
        // let image_offset_y = self.params.image_y.eval(window_size, screen_props);

        // rotate the transformation matrix around x,y
        let trans_mat = self.transformation.clone()
            * Transformation2D::RotationPoint(
                self.params.rotation as f32,
                self.params.x.clone(),
                self.params.y.clone(),
            );

        let trans_mat = trans_mat.eval(window_size, screen_props);

        scene.draw_svg(
            &self.svg,
            renderer::shapes::Point {
                x: x as f64,
                y: y as f64,
            },
            width as f32,
            height as f32,
            None,
        );
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn animations(&mut self) -> &mut Vec<Animation> {
        &mut self.animations
    }

    fn add_animation(&mut self, animation: Animation) {
        self.animations.push(animation);
    }

    fn set_transformation(&mut self, transformation: crate::visual::geometry::Transformation2D) {
        self.transformation = transformation;
    }

    fn add_transformation(&mut self, transformation: crate::visual::geometry::Transformation2D) {
        self.transformation = transformation * self.transformation.clone();
    }

    fn transformation(&self) -> crate::visual::geometry::Transformation2D {
        self.transformation.clone()
    }

    fn contains(&self, x: Size, y: Size, window_state: &WindowState) -> bool {
        let window_size = window_state.size;
        let screen_props = window_state.physical_screen;

        let ix = self.params.x.eval(window_size, screen_props);
        let iy = self.params.y.eval(window_size, screen_props);
        let width = self.params.width.eval(window_size, screen_props);
        let height = self.params.height.eval(window_size, screen_props);

        let trans_mat = self.transformation.eval(window_size, screen_props);

        let x = x.eval(window_size, screen_props);
        let y = y.eval(window_size, screen_props);

        // apply transformation by multiplying the point with the transformation matrix
        let p = nalgebra::Vector3::new(x, y, 1.0);
        let p_new = trans_mat * p;

        // check if the point is inside the rectangle
        p_new[0] >= ix && p_new[0] <= ix + width && p_new[1] >= iy && p_new[1] <= iy + height
    }

    fn get_param(&self, name: &str) -> Option<StimulusParamValue> {
        self.params.get_param(name)
    }

    fn set_param(&mut self, name: &str, value: StimulusParamValue) {
        self.params.set_param(name, value)
    }
}

fn numpy3_to_image<P, S>(py_array: PyReadonlyArray3<S>) -> ImageBuffer<P, Vec<S>>
where
    P: Pixel<Subpixel = S> + 'static,
    S: numpy::Element + Clone + Default + Bounded + 'static,
    Vec<S>: std::ops::Deref<Target = [S]>,
{
    let shape = py_array.shape();
    let (height, width, channels) = (shape[0], shape[1], shape[2]);

    // Verify channel count matches pixel type
    assert_eq!(
        channels,
        P::CHANNEL_COUNT as usize,
        "Channel count mismatch: expected {}, got {}",
        P::CHANNEL_COUNT,
        channels
    );

    // Convert to ndarray and ensure contiguous layout
    let py_array = py_array.as_array();
    let py_array = py_array.as_standard_layout();
    let data = py_array.as_slice().unwrap();

    let mut img = ImageBuffer::new(width as u32, height as u32);

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * channels;
            let pixel_data = &data[idx..idx + channels];

            // Create pixel from slice
            let pixel = *P::from_slice(pixel_data);
            img.put_pixel(x as u32, y as u32, pixel);
        }
    }

    img
}

fn numpy4_to_image<P, S>(py_array: PyReadonlyArray4<S>) -> ImageBuffer<P, Vec<S>>
where
    P: Pixel<Subpixel = S> + 'static,
    S: numpy::Element + Clone + Default + Bounded + 'static,
    Vec<S>: std::ops::Deref<Target = [S]>,
{
    let shape = py_array.shape();
    let (height, width, channels) = (shape[0], shape[1], shape[2]);

    // Verify channel count matches pixel type
    assert_eq!(
        channels,
        P::CHANNEL_COUNT as usize,
        "Channel count mismatch: expected {}, got {}",
        P::CHANNEL_COUNT,
        channels
    );

    // Convert to ndarray and ensure contiguous layout
    let py_array = py_array.as_array();
    let py_array = py_array.as_standard_layout();
    let data = py_array.as_slice().unwrap();

    let mut img = ImageBuffer::new(width as u32, height as u32);

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) * channels;
            let pixel_data = &data[idx..idx + channels];

            // Create pixel from slice
            let pixel = *P::from_slice(pixel_data);
            img.put_pixel(x as u32, y as u32, pixel);
        }
    }

    img
}
