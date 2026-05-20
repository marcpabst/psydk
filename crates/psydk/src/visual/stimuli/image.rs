use crate::visual::colors::Color;
use crate::visual::colors::IntoColor;
use crate::visual::renderer::image::{ImageBuffer, Pixel, RgbImage, Rgba, RgbaImage};
use crate::visual::renderer::{
    brushes::{Brush, Extend, ImageSampling},
    image::Rgb,
    styles::ImageFitMode,
    Bitmap, Scene,
};
use num_traits::Bounded;
use numpy::{PyReadonlyArray2, PyReadonlyArray3, PyReadonlyArray4, PyUntypedArrayMethods};
use psydk_proc::StimulusParams;
use pyo3::ffi::c_str;
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
    context::ExperimentContext,
    visual::{
        geometry::{Anchor, Size, Transformation2D},
        window::{Frame, WindowState, WindowStateSnapshot},
    },
};

#[derive(StimulusParams, Clone, Debug)]
/// Parameters for the ImageStimulus.
pub struct ImageParams {
    /// x position of the stimulus.
    pub x: Size,
    /// y position of the stimulus.
    pub y: Size,
    /// Width of the stimulus.
    pub width: Size,
    /// Height of the stimulus.
    pub height: Size,
    /// Rotation of the stimulus in degrees.
    pub rotation: f32,
    /// Opacity of the stimulus, from 0.0 (transparent) to 1.0 (opaque).
    pub opacity: f32,
    /// The x offset of the image within the stimulus.
    pub image_x: Size,
    /// The y offset of the image within the stimulus.
    pub image_y: Size,
}

#[derive(Debug)]
pub struct ImageStimulus {
    /// Unique identifier for the stimulus.
    id: uuid::Uuid,
    /// Parameters for the image stimulus.
    params: ImageParams,
    /// The image to be displayed.
    image: Bitmap,
    /// The anchor point of the image stimulus for positioning.
    anchor: Anchor,
    /// The transformation applied to the image stimulus.
    transformation: Transformation2D,
    /// List of animations associated with the stimulus.
    animations: Vec<Animation>,
    /// Whether the image stimulus is currently visible.
    visible: bool,
}

unsafe impl Send for ImageStimulus {}

impl ImageStimulus {
    /// Creates a new `ImageStimulus` from an image and parameters.
    pub fn from_image(image: Bitmap, params: ImageParams, transform: Option<Transformation2D>, anchor: Anchor) -> Self {
        Self {
            id: Uuid::new_v4(),
            transformation: transform.unwrap_or_else(|| Transformation2D::Identity()),
            animations: Vec::new(),
            visible: true,
            image,
            anchor,
            params,
        }
    }
}

#[derive(Debug, Clone)]
#[pyclass(name = "ImageStimulus", extends=PyStimulus)]
pub struct PyImageStimulus();

// #[pymethods]
// impl PyImageStimulus {
//     #[new]
//     #[pyo3(signature = (
//         src,
//         x,
//         y,
//         width,
//         height,
//         rotation = 0.0,
//         opacity = 1.0,
//         anchor = Anchor::Center,
//         transform = None,
//         srgb = true,
//         context = None,
//     ))]
//     /// Creates a new `ImageStimulus` from a file path.
//     ///
//     /// Parameters
//     /// ----------
//     /// src : str
//     ///     The file path to the image.
//     /// x : Size, num, or str
//     ///     The x position of the stimulus.
//     /// y : Size, num, or str
//     ///     The y position of the stimulus.
//     /// width : Size, num, or str
//     ///     The width of the stimulus.
//     /// height : Size, num, or str
//     /// The height of the stimulus.
//     /// rotation : float, optional
//     ///
//     fn __new__(
//         py: Python,
//         src: pyo3::PyObject,
//         x: IntoSize,
//         y: IntoSize,
//         width: IntoSize,
//         height: IntoSize,
//         rotation: f32,
//         opacity: f32,
//         anchor: Anchor,
//         transform: Option<Transformation2D>,
//         srgb: bool,
//         context: Option<ExperimentContext>,
//     ) -> PyResult<(Self, PyStimulus)> {
//         let ctx = get_experiment_context(context, py)?;

//         // try to extract a string from the src parameter
//         let bitmap = if let Ok(path) = src.extract::<String>(py) {
//             todo!()
//             // ctx.renderer().create_bitmap_from_path(&path)
//         } else if let Ok(path) = src.extract::<&str>(py) {
//             todo!()
//             // ctx.renderer().create_bitmap_from_path(path)
//         } else if let Ok(array) = src.extract::<PyReadonlyArray3<u8>>(py) {
//             // Convert the Numpy array to a image::RgbImageBuffer
//             let array = numpy3_to_image::<Rgba<u8>, u8>(array);

//             ctx.renderer().create_bitmap_from_image_u8(
//                 array,
//                 if srgb {
//                     crate::visual::renderer::color_formats::ColorEncoding::Srgb
//                 } else {
//                     crate::visual::renderer::color_formats::ColorEncoding::Linear
//                 },
//             )
//         // } else if let Ok(array) = src.extract::<PyReadonlyArray3<f32>>(py) {
//         //     let array = numpy3_to_image::<Rgba<f32>, f32>(array);
//         //     ctx.renderer().create_bitmap_from_image_f32(
//         //         array,
//         //         if srgb {
//         //             crate::visual::renderer::color_formats::ColorEncoding::Srgb
//         //         } else {
//         //             crate::visual::renderer::color_formats::ColorEncoding::Linear
//         //         },
//         //     )
//         } else if let Ok(array) = src.extract::<PyReadonlyArray4<u8>>(py) {
//             let array = numpy4_to_image::<Rgba<u8>, u8>(array);
//             ctx.renderer().create_bitmap_from_image_u8(
//                 array,
//                 if srgb {
//                     crate::visual::renderer::color_formats::ColorEncoding::Srgb
//                 } else {
//                     crate::visual::renderer::color_formats::ColorEncoding::Linear
//                 },
//             )
//         // } else if let Ok(array) = src.extract::<PyReadonlyArray4<f32>>(py) {
//         //     let array = numpy4_to_image::<Rgba<f32>, f32>(array);
//         //     ctx.renderer().create_bitmap_from_image_f32(
//         //         array,
//         //         if srgb {
//         //             crate::visual::renderer::color_formats::ColorEncoding::Srgb
//         //         } else {
//         //             crate::visual::renderer::color_formats::ColorEncoding::Linear
//         //         },
//         //     )
//         } else {
//             return Err(pyo3::exceptions::PyTypeError::new_err(
//                 "src must be a string, PathBuf, or a Numpy array",
//             ));
//         };

//         let bitmap =
//             bitmap.map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("Failed to create bitmap: {}", e)))?;

//         Ok((
//             Self(),
//             PyStimulus::new(ImageStimulus::from_image(
//                 bitmap,
//                 ImageParams {
//                     x: x.into(),
//                     y: y.into(),
//                     width: width.into(),
//                     height: height.into(),
//                     image_x: 0.0.into(),
//                     image_y: 0.0.into(),
//                     rotation,
//                     opacity,
//                 },
//                 transform,
//                 anchor,
//             )),
//         ))
//     }

//     // Creates a new `ImageStimulus` from a Numpy array.
//     // #[pyo3(signature = (
//     //     array,
//     //     x,
//     //     y,
//     //     width,
//     //     height,
//     //     rotation = 0.0,
//     //     opacity = 1.0,
//     //     anchor = Anchor::Center,
//     //     transform = None,
//     //     context = None,
//     // ))]
//     // #[staticmethod]
//     // fn fromarray_u8(
//     //     py: Python,
//     //     array: PyReadonlyArray3<u8>,
//     //     x: IntoSize,
//     //     y: IntoSize,
//     //     width: IntoSize,
//     //     height: IntoSize,
//     //     rotation: f64,
//     //     opacity: f64,
//     //     anchor: Anchor,
//     //     transform: Option<Transformation2D>,
//     //     context: Option<ExperimentContext>,
//     // ) -> (Self, PyStimulus) {
//     //     let ctx = get_experiment_context(context, py)?;

//     //     // Convert the Numpy array to a image::RgbImageBuffer
//     //     let array = numpy_to_rgbimage(array);

//     //     let bitmap = ctx
//     //         .renderer_factory()
//     //         .create_bitmap_u8(array, crate::visual::renderer::color_formats::ColorEncoding::Srgb);

//     //     Ok((
//     //         Self(),
//     //         PyStimulus::new(ImageStimulus::from_image(
//     //             bitmap,
//     //             ImageParams {
//     //                 x: x.into(),
//     //                 y: y.into(),
//     //                 width: width.into(),
//     //                 height: height.into(),
//     //                 image_x: 0.0.into(),
//     //                 image_y: 0.0.into(),
//     //                 rotation,
//     //                 opacity,
//     //             },
//     //             transform,
//     //             anchor,
//     //         )),
//     //     ))
//     // }
// }

impl_pystimulus_for_wrapper!(PyImageStimulus, ImageStimulus);

impl Stimulus for ImageStimulus {
    fn uuid(&self) -> Uuid {
        self.id
    }

    fn draw(&mut self, scene: &mut crate::visual::renderer::wrapped::Scene, window_state: &WindowStateSnapshot) {
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

        let image_offset_x = self.params.image_x.eval(window_size, screen_props);
        let image_offset_y = self.params.image_y.eval(window_size, screen_props);

        // rotate the transformation matrix around x,y
        let trans_mat = self.transformation.clone()
            * Transformation2D::RotationPoint(
                self.params.rotation as f32,
                self.params.x.clone(),
                self.params.y.clone(),
            );

        let trans_mat = trans_mat.eval(window_size, screen_props);

        // scene.draw_shape_filled(
        //     Shape::Rectangle {
        //         a: (x, y).into(),
        //         w: width,
        //         h: height,
        //     },
        //     Brush::Image {
        //         image: self.image.clone(),
        //         start: (x + image_offset_x, y + image_offset_y).into(),
        //         fit_mode: ImageFitMode::Exact { width, height },
        //         sampling: ImageSampling::Linear,
        //         edge_mode: (Extend::Pad, Extend::Pad),
        //         transform: None,
        //         alpha: Some(self.params.opacity as f32),
        //     },
        //     Some(trans_mat.into()),
        //     None,
        // );
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn animations(&mut self) -> Option<&mut Vec<Animation>> {
        Some(&mut self.animations)
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

    fn contains(&self, x: Size, y: Size, window_state: &WindowStateSnapshot) -> bool {
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
