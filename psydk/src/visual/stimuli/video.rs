use derive_debug::Dbg;
use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};

use crate::{app::GPUState, errors::PsydkError};

use byte_slice_cast::*;
use gstreamer::{element_error, element_warning, prelude::*};
use psydk_proc::StimulusParams;
use pyo3::ffi::c_str;
use renderer::{
    brushes::{Brush, Extend, ImageSampling},
    renderer::ColorSpace,
    shapes::Shape,
    styles::ImageFitMode,
    DynamicBitmap, DynamicScene,
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
/// Parameters for the VideoStimulus.
pub struct VideoParams {
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
    /// The x offset of the video within the stimulus.
    pub image_x: Size,
    /// The y offset of the video within the stimulus.
    pub image_y: Size,
}

#[derive(Debug, Clone)]
struct SwappableValue<T> {
    value: Arc<arc_swap::ArcSwap<T>>,
}

impl<T> SwappableValue<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: Arc::new(arc_swap::ArcSwap::from_pointee(value)),
        }
    }

    pub fn swap(&self, new_value: T) {
        self.value.swap(Arc::new(new_value));
    }

    pub fn get(&self) -> Arc<T> {
        self.value.load_full()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VideoState {
    NotReady,
    Ready { duration: f64, width: u32, height: u32 },
    Playing(usize, f64),
    Paused(f64),
    Stopped(f64),
    Errored(),
}

#[derive(Dbg)]
pub struct VideoStimulus {
    /// Unique identifier for the stimulus.
    id: uuid::Uuid,
    /// Parameters for the video stimulus.
    params: VideoParams,
    /// The current frame image to be displayed.
    current_frame: DynamicBitmap,
    /// Buffer for receiving new frames from GStreamer.
    buffer: Arc<Mutex<Option<renderer::image::RgbaImage>>>,
    /// GPU queue
    queue: wgpu::Queue,
    /// Texture for the video frame.
    texture: wgpu::Texture,
    /// GStreamer pipeline for video decoding.
    pipeline: gstreamer::Pipeline,
    /// Channel for receiving video state updates.
    status: SwappableValue<VideoState>,
    /// Index of the current frame in the video.
    current_frame_index: usize,
    /// Timestamp of the last displayed frame.
    current_frame_time: f64,
    /// The total duration as reported by GStreamer.
    duration: f64,
    /// The anchor point of the video stimulus for positioning.
    anchor: Anchor,
    /// The transformation applied to the video stimulus.
    transformation: Transformation2D,
    /// List of animations associated with the stimulus.
    animations: Vec<Animation>,
    /// Whether the video stimulus is currently visible.
    visible: bool,
}

unsafe impl Send for VideoStimulus {}

impl VideoStimulus {
    /// Creates a new `VideoStimulus` from a file path.
    pub fn from_path(
        path: &str,
        params: VideoParams,
        transform: Option<Transformation2D>,
        anchor: Anchor,
        context: ExperimentContext,
    ) -> Self {
        // get gpu_state
        let gpu_state = context.gpu_state.lock().unwrap();
        let renderer_factory = context.renderer_factory().deref();
        let device = gpu_state.device.clone();
        let queue = gpu_state.queue.clone();

        let status = SwappableValue::new(VideoState::NotReady);

        let buffer = Arc::new(Mutex::new(None));
        println!("Creating video pipeline for path: {}", path);
        let pipeline = Self::create_pipeline(path, status.clone(), buffer.clone()).unwrap();

        // set the pipeline to paused state to prepare it for playback
        pipeline.set_state(gstreamer::State::Paused).unwrap();

        let (duration, width, height) = loop {
            match *(status.get()) {
                VideoState::Ready {
                    duration,
                    width,
                    height,
                } => {
                    println!(
                        "Video is ready with duration: {} seconds, dimensions: {}x{}",
                        duration, width, height
                    );
                    break (duration, width, height);
                }
                VideoState::Errored() => {
                    panic!("Video pipeline error.")
                }
                _ => continue,
            }
        };

        // Create a new wgpu texture for the video frame
        let texture_desc = wgpu::TextureDescriptor {
            label: Some("VideoStimulus Texture"),
            size: wgpu::Extent3d {
                width: width,
                height: height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
        };

        let texture = device.create_texture(&texture_desc);

        // upload a red image to the texture as a placeholder
        let red_image = renderer::image::RgbaImage::from_raw(
            width as u32,
            height as u32,
            [255, 255, 255, 0].repeat(width as usize * height as usize),
        )
        .expect("Failed to create red image buffer");

        let red_image_data = red_image.as_raw();

        println!("Uploaded red image to texture for video stimulus");

        let frame = renderer_factory.create_bitmap_from_wgpu_texture(texture.clone(), ColorSpace::Srgb);

        println!("Video pipeline created for path: {}", path);

        let slf = Self {
            id: Uuid::new_v4(),
            params,
            current_frame: frame,
            buffer,
            queue: queue.clone(),
            texture,
            pipeline,
            status: status,
            current_frame_index: 0,
            current_frame_time: -1.0,
            duration,
            anchor,
            transformation: transform.unwrap_or_else(|| Transformation2D::Identity()),
            animations: Vec::new(),
            visible: true,
        };

        // upload the red image to the texture
        slf.update_texture(red_image_data, &queue);

        slf
    }

    pub fn is_playing(&self) -> bool {
        self.pipeline.current_state() == gstreamer::State::Playing
    }

    pub fn play(&self) {
        self.pipeline.set_state(gstreamer::State::Playing).unwrap();
    }

    pub fn pause(&self) {
        self.pipeline.set_state(gstreamer::State::Paused).unwrap();
    }

    pub fn toggle(&self) {
        if self.is_playing() {
            self.pause();
        } else {
            self.play();
        }
    }

    pub fn stop(&self) {
        self.pipeline.set_state(gstreamer::State::Ready).unwrap();
    }

    fn update_texture(&self, data: &[u8], queue: &wgpu::Queue) {
        let width = self.texture.size().width as u32;
        let height = self.texture.size().height as u32;

        // Create a new wgpu texture for the video frame
        let texture_desc = wgpu::TextureDescriptor {
            label: Some("VideoStimulus Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
        };

        let texture = self.texture.clone();
        queue.write_texture(
            // Tells wgpu where to copy the pixel data
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            // The actual pixel data
            data,
            // The layout of the texture
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width), // 4 bytes per pixel (RGBA)
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        // submit the queue to ensure the texture is updated
        queue.submit(std::iter::empty());
    }

    pub fn seek(&self, to: f64, accurate: bool, flush: bool, block: bool) {
        let mut flags = gstreamer::SeekFlags::empty();
        if accurate {
            flags |= gstreamer::SeekFlags::ACCURATE;
        }
        if flush && self.is_playing() {
            flags |= gstreamer::SeekFlags::FLUSH;
        }

        self.pipeline
            .seek_simple(flags, gstreamer::ClockTime::from_seconds(to as u64))
            .expect("Failed to seek in video pipeline");

        if block {
            self.pipeline
                .state(gstreamer::ClockTime::from_seconds(5))
                .0
                .expect("Failed to block until seek is done");
        }
    }

    pub fn current_time(&self) -> f64 {
        self.current_frame_time
    }

    pub fn current_frame(&self) -> i64 {
        match *self.status.get() {
            VideoState::Playing(frame_index, _) => frame_index as i64,
            VideoState::Paused(frame_index) | VideoState::Stopped(frame_index) => frame_index as i64,
            VideoState::Ready { .. } => 0,
            VideoState::NotReady | VideoState::Errored() => -1, // Not ready or errored
            _ => -1,                                            // Not playing or not ready
        }
    }

    /// Returns the current progress of the video from 0.0 to 1.0.
    pub fn current_progress(&self) -> f64 {
        let time = self.current_time();
        let durartion = self.duration;
        if durartion > 0.0 {
            (time / durartion).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    fn create_pipeline(
        path: &str,
        status: SwappableValue<VideoState>,
        buffer: Arc<Mutex<Option<renderer::image::RgbaImage>>>,
    ) -> Result<gstreamer::Pipeline, PsydkError> {
        gstreamer::init()?;

        let pipeline = gstreamer::Pipeline::default();
        let src = gstreamer::ElementFactory::make("filesrc")
            .property("location", path)
            .build()
            .expect("Failed to create filesrc element");

        let decodebin = gstreamer::ElementFactory::make("decodebin").build()?;

        let appsink = gstreamer_app::AppSink::builder()
            .caps(
                &gstreamer_video::VideoCapsBuilder::new()
                    .format(gstreamer_video::VideoFormat::Rgba)
                    .build(),
            )
            .build();

        let r_status = status.clone();

        appsink.set_callbacks(
            gstreamer_app::AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink.pull_sample().map_err(|_| gstreamer::FlowError::Eos)?;
                    let gst_buffer = sample.buffer().ok_or_else(|| {
                        element_error!(
                            appsink,
                            gstreamer::ResourceError::Failed,
                            ("Failed to get buffer from appsink")
                        );
                        gstreamer::FlowError::Error
                    })?;

                    let caps = sample.caps().expect("caps on appsink");
                    let structure = caps.structure(0).expect("structure in caps");
                    let width = structure.get::<i32>("width").expect("width in caps");
                    let height = structure.get::<i32>("height").expect("height in caps");

                    let u_time = gst_buffer.pts().expect("timestamp").useconds();
                    println!("Received new sample with timestamp: {}", u_time);
                    let time = u_time as f64 / 1_000_000.0; // Convert microseconds to seconds

                    let frame_index = structure.get::<i64>("pos_frames").unwrap_or(-1);

                    let map = gst_buffer.map_readable().map_err(|_| {
                        element_error!(
                            appsink,
                            gstreamer::ResourceError::Failed,
                            ("Failed to map buffer readable")
                        );
                        gstreamer::FlowError::Error
                    })?;

                    let samples = map.as_slice_of::<u8>().map_err(|_| {
                        element_error!(
                            appsink,
                            gstreamer::ResourceError::Failed,
                            ("Failed to interpret buffer as array of u8")
                        );
                        gstreamer::FlowError::Error
                    })?;

                    let new_buffer =
                        renderer::image::RgbaImage::from_raw(width as u32, height as u32, samples.to_vec())
                            .expect("Failed to create image buffer from raw data");

                    let mut buffer = buffer.lock().unwrap();
                    *buffer = Some(new_buffer);

                    r_status.swap(VideoState::Playing(frame_index as usize, time));

                    Ok(gstreamer::FlowSuccess::Ok)
                })
                .build(),
        );

        pipeline.add_many([&src, &decodebin])?;
        src.link(&decodebin)?;

        let status2 = status.clone();

        let pipeline_weak = pipeline.downgrade();
        decodebin.connect_pad_added(move |dbin, src_pad| {
            let Some(pipeline) = pipeline_weak.upgrade() else {
                return;
            };

            let (is_audio, is_video) = {
                let media_type = src_pad.current_caps().and_then(|caps| {
                    caps.structure(0).map(|s| {
                        let name = s.name();
                        (name.starts_with("audio/"), name.starts_with("video/"))
                    })
                });

                match media_type {
                    None => {
                        element_warning!(
                            dbin,
                            gstreamer::CoreError::Negotiation,
                            ("Failed to get media type from pad {}", src_pad.name())
                        );
                        return;
                    }
                    Some(media_type) => media_type,
                }
            };

            let insert_sink = |is_audio, is_video| -> Result<(), PsydkError> {
                if is_audio {
                    let queue = gstreamer::ElementFactory::make("queue").build()?;
                    let convert = gstreamer::ElementFactory::make("audioconvert").build()?;
                    let resample = gstreamer::ElementFactory::make("audioresample").build()?;
                    let sink = gstreamer::ElementFactory::make("autoaudiosink").build()?;

                    let elements = &[&queue, &convert, &resample, &sink];
                    pipeline.add_many(elements)?;
                    gstreamer::Element::link_many(elements)?;

                    for e in elements {
                        e.sync_state_with_parent()?;
                    }

                    let sink_pad = queue.static_pad("sink").expect("queue has no sinkpad");
                    src_pad.link(&sink_pad)?;
                } else if is_video {
                    let queue = gstreamer::ElementFactory::make("queue").build()?;
                    let convert = gstreamer::ElementFactory::make("videoconvert").build()?;
                    let scale = gstreamer::ElementFactory::make("videoscale").build()?;

                    let elements = &[&queue, &convert, &scale, &appsink.upcast_ref()];
                    pipeline.add_many(elements)?;
                    gstreamer::Element::link_many(elements)?;

                    for e in elements {
                        e.sync_state_with_parent()?;
                    }

                    let sink_pad = queue.static_pad("sink").expect("queue has no sinkpad");
                    src_pad.link(&sink_pad)?;

                    println!("Video pad linked successfully");

                    // get duration of the video
                    let duration = pipeline
                        .query_duration::<gstreamer::ClockTime>()
                        .expect("Failed to query duration")
                        .seconds() as f64;

                    // print dimensions of the video
                    let caps = src_pad.current_caps().expect("src pad has caps");
                    let structure = caps.structure(0).expect("structure in caps");
                    let width = structure.get::<i32>("width").expect("width in caps");
                    let height = structure.get::<i32>("height").expect("height in caps");

                    status2.swap(VideoState::Ready {
                        duration,
                        width: width as u32,
                        height: height as u32,
                    });
                }

                Ok(())
            };

            if let Err(err) = insert_sink(is_audio, is_video) {
                println!("Error: {err}");
            }
        });

        Self::start_pipeline(pipeline.clone(), status.clone());
        Ok(pipeline)
    }

    fn start_pipeline(pipeline: gstreamer::Pipeline, status: SwappableValue<VideoState>) {
        let bus = pipeline.bus().expect("Pipeline without bus. Shouldn't happen!");

        std::thread::spawn(move || {
            for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
                use gstreamer::MessageView;

                // get the status of the video
                let pipeline_status = pipeline.current_state();

                // // Update the status based on pipeline status
                // if pipeline_status == gstreamer::State::Playing {
                //     let res = pipeline.query_position::<gstreamer::ClockTime>();
                //     let def = pipeline
                //         .query_position_generic(gstreamer::Format::Default)
                //         .expect("Failed to query position")
                //         .value();
                //     println!("Pipeline is playing at position: {:?}", def);

                //     if let Some(position) = res {
                //         let time = position.useconds() as f64 / 1_000_000.0;
                //         let state = VideoState::Playing(def as usize, time);
                //         // status.swap(state);
                //     } else {
                //         status.swap(VideoState::Errored());
                //     }
                // }

                match msg.view() {
                    MessageView::Eos(..) => break,
                    MessageView::Error(err) => {
                        pipeline.set_state(gstreamer::State::Null).unwrap();
                        println!(
                            "Error from element {}: {}",
                            msg.src().map(|s| s.path_string()).as_deref().unwrap_or("None"),
                            err.error().to_string()
                        );
                    }
                    _ => (),
                }
            }

            pipeline.set_state(gstreamer::State::Null).unwrap();
        });
    }

    fn update_frame(&self, queue: &wgpu::Queue) -> bool {
        let buffer = self.buffer.lock().unwrap();
        // get as slice of u8
        if let Some(ref frame) = *buffer {
            let data = frame.as_raw();
            // update the texture with the new frame data
            self.update_texture(data, queue);
        }

        false
    }
}

#[derive(Debug, Clone)]
#[pyclass(name = "VideoStimulus", extends=PyStimulus)]
pub struct PyVideoStimulus();

#[pymethods]
impl PyVideoStimulus {
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
        context = None,
    ))]
    /// Creates a new `VideoStimulus` from a file path.
    ///
    /// Parameters
    /// ----------
    /// src : str
    ///     The file path to the video.
    /// x : Size, num, or str
    ///     The x position of the stimulus.
    /// y : Size, num, or str
    ///     The y position of the stimulus.
    /// width : Size, num, or str
    ///     The width of the stimulus.
    /// height : Size, num, or str
    ///     The height of the stimulus.
    /// rotation : float, optional
    ///     The rotation of the stimulus in degrees. Default is 0.0.
    /// opacity : float, optional
    ///     The opacity of the stimulus. Default is 1.0.
    /// anchor : Anchor, optional
    ///     The anchor point for positioning. Default is Center.
    /// transform : Transformation2D, optional
    ///     Additional transformation to apply.
    /// context : ExperimentContext, optional
    ///     The experiment context.
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
        context: Option<ExperimentContext>,
    ) -> PyResult<(Self, PyStimulus)> {
        let ctx = get_experiment_context(context, py)?;

        Ok((
            Self(),
            PyStimulus::new(VideoStimulus::from_path(
                &src,
                VideoParams {
                    x: x.into(),
                    y: y.into(),
                    width: width.into(),
                    height: height.into(),
                    image_x: 0.0.into(),
                    image_y: 0.0.into(),
                    rotation,
                    opacity,
                },
                transform,
                anchor,
                ctx,
            )),
        ))
    }

    /// Start playing the video.
    fn play(slf: PyRef<'_, Self>) {
        let mut stim = slf.as_ref().0.lock();
        if let Some(video) = stim.downcast_mut::<VideoStimulus>() {
            video.play();
        }
    }

    /// Pause the video.
    fn pause(slf: PyRef<'_, Self>) {
        let mut stim = slf.as_ref().0.lock();
        if let Some(video) = stim.downcast_mut::<VideoStimulus>() {
            video.pause();
        }
    }

    /// Toggle the video playback state (play/pause).
    fn toggle(slf: PyRef<'_, Self>) {
        let mut stim = slf.as_ref().0.lock();
        if let Some(video) = stim.downcast_mut::<VideoStimulus>() {
            video.toggle();
        }
    }

    /// Stop the video.
    fn stop(slf: PyRef<'_, Self>) {
        let mut stim = slf.as_ref().0.lock();
        if let Some(video) = stim.downcast_mut::<VideoStimulus>() {
            video.stop();
        }
    }

    /// Seek to a specific time in the video.
    ///
    /// Parameters
    /// ----------
    /// to : float
    ///     The time to seek to in seconds.
    /// accurate : bool, optional
    ///     Whether to seek accurately. Default is True.
    /// flush : bool, optional
    ///     Whether to flush the pipeline. Default is True.
    /// block : bool, optional
    ///     Whether to block until the seek is complete. Default is True.
    #[pyo3(signature = (to, accurate = true, flush = true, block = true))]
    fn seek(slf: PyRef<'_, Self>, to: f64, accurate: bool, flush: bool, block: bool) {
        let mut stim = slf.as_ref().0.lock();
        if let Some(video) = stim.downcast_mut::<VideoStimulus>() {
            video.seek(to, accurate, flush, block);
        }
    }

    /// Check if the video is currently playing.
    fn is_playing(slf: PyRef<'_, Self>) -> bool {
        todo!("Implement is_playing method for VideoStimulus")
    }

    /// Return the current time of the video.
    fn get_current_time(slf: PyRef<'_, Self>) -> f64 {
        let stim = slf.as_ref().0.lock();
        if let Some(video) = stim.downcast_ref::<VideoStimulus>() {
            video.current_time()
        } else {
            unreachable!()
        }
    }

    fn get_current_frame(slf: PyRef<'_, Self>) -> i64 {
        let stim = slf.as_ref().0.lock();
        if let Some(video) = stim.downcast_ref::<VideoStimulus>() {
            video.current_frame()
        } else {
            unreachable!()
        }
    }

    fn get_current_progress(slf: PyRef<'_, Self>) -> f64 {
        let stim = slf.as_ref().0.lock();
        if let Some(video) = stim.downcast_ref::<VideoStimulus>() {
            video.current_progress()
        } else {
            unreachable!()
        }
    }
}

impl_pystimulus_for_wrapper!(PyVideoStimulus, VideoStimulus);

impl Stimulus for VideoStimulus {
    fn uuid(&self) -> Uuid {
        self.id
    }

    fn draw(&mut self, scene: &mut DynamicScene, window_state: &WindowState) {
        if !self.visible {
            return;
        }

        self.update_frame(&self.queue);

        // update current_frame_time
        self.current_frame_time = match *self.status.get() {
            VideoState::Playing(_, time) => time,
            VideoState::Paused(time) | VideoState::Stopped(time) => time,
            _ => -1.0, // Not ready or errored
        };

        let frame = &self.current_frame;

        let window_size = window_state.size;
        let screen_props = window_state.physical_screen;

        // Convert physical units to pixels
        let x = self.params.x.eval(window_size, screen_props);
        let y = self.params.y.eval(window_size, screen_props);
        let width = self.params.width.eval(window_size, screen_props);
        let height = self.params.height.eval(window_size, screen_props);

        let (x, y) = self.anchor.to_top_left(x, y, width, height);

        let image_offset_x = self.params.image_x.eval(window_size, screen_props);
        let image_offset_y = self.params.image_y.eval(window_size, screen_props);

        // Apply rotation transformation
        let trans_mat = self.transformation.clone()
            * Transformation2D::RotationPoint(
                self.params.rotation as f32,
                self.params.x.clone(),
                self.params.y.clone(),
            );

        let trans_mat = trans_mat.eval(window_size, screen_props);

        scene.draw_shape_fill(
            Shape::Rectangle {
                a: (x, y).into(),
                w: width as f64,
                h: height as f64,
            },
            Brush::Image {
                image: frame,
                start: (x + image_offset_x, y + image_offset_y).into(),
                fit_mode: ImageFitMode::Exact { width, height },
                sampling: ImageSampling::Linear,
                edge_mode: (Extend::Pad, Extend::Pad),
                transform: None,
                alpha: Some(self.params.opacity as f32),
            },
            Some(trans_mat.into()),
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

    fn set_transformation(&mut self, transformation: Transformation2D) {
        self.transformation = transformation;
    }

    fn add_transformation(&mut self, transformation: Transformation2D) {
        self.transformation = transformation * self.transformation.clone();
    }

    fn transformation(&self) -> Transformation2D {
        self.transformation.clone()
    }

    fn contains(&self, x: Size, y: Size, window: &Window) -> bool {
        let window_state = window.state.lock().unwrap();
        let window_state = window_state.as_ref().unwrap();
        let window_size = window_state.size;
        let screen_props = window_state.physical_screen;

        let ix = self.params.x.eval(window_size, screen_props);
        let iy = self.params.y.eval(window_size, screen_props);
        let width = self.params.width.eval(window_size, screen_props);
        let height = self.params.height.eval(window_size, screen_props);

        let trans_mat = self.transformation.eval(window_size, screen_props);

        let x = x.eval(window_size, screen_props);
        let y = y.eval(window_size, screen_props);

        // Apply transformation by multiplying the point with the transformation matrix
        let p = nalgebra::Vector3::new(x, y, 1.0);
        let p_new = trans_mat * p;

        // Check if the point is inside the rectangle
        p_new[0] >= ix && p_new[0] <= ix + width && p_new[1] >= iy && p_new[1] <= iy + height
    }

    fn get_param(&self, name: &str) -> Option<StimulusParamValue> {
        self.params.get_param(name)
    }

    fn set_param(&mut self, name: &str, value: StimulusParamValue) {
        self.params.set_param(name, value)
    }
}
