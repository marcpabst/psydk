use std::{
    collections::{HashMap, VecDeque},
    ops::Deref,
    pin::Pin,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc, Mutex, MutexGuard, RwLock,
    },
    time::{Duration, Instant},
};

use async_channel::{bounded, Receiver, Sender};
use atomic_float::AtomicF64;
use derive_debug::Dbg;
use futures_lite::{future::block_on, Future};
use nalgebra;
use palette::IntoColor;
use pyo3::prelude::*;
use renderer::{
    renderer::{DynamicRenderResources, SharedRendererState},
    wgpu_renderer::WgpuRenderer,
    DynamicRenderer, DynamicScene,
};
use send_wrapper::SendWrapper;
use uuid::Uuid;
use wgpu::TextureFormat;
use winit::{dpi::PhysicalSize, window::WindowId};

use super::{
    color::LinRgba,
    geometry::Size,
    stimuli::{DynamicStimulus, Stimulus},
};
use crate::{
    app::GPUState,
    context::Monitor,
    errors::{PsydkError, PsydkResult},
    input::{Event, EventHandler, EventHandlerId, EventHandlingExt, EventKind, EventReceiver},
    time::Timestamp,
    visual::utils::find_zero_crossings,
};

#[derive(Debug, Clone, Copy)]
pub struct PhysicalScreen {
    /// Pixel/mm of the screen.
    pub pixel_density: f32,
    /// Viewing distance in meters.
    pub viewing_distance: f32,
}

impl PhysicalScreen {
    /// Creates a new physical screen given width in pixels and millimeters.
    pub fn new(width_px: u32, width_mm: f32, viewing_distance: f32) -> Self {
        let pixel_density = width_px as f32 / width_mm;
        Self {
            pixel_density,
            viewing_distance,
        }
    }

    /// Returns the size of the screen in millimeters.
    pub fn size(&self, width_px: u32, height_px: u32) -> (f32, f32) {
        let width_mm = width_px as f32 / self.pixel_density;
        let height_mm = height_px as f32 / self.pixel_density;
        (width_mm, height_mm)
    }

    /// Returns the width of the screen in millimeters.
    pub fn width(&self, width_px: u32) -> f32 {
        width_px as f32 / self.pixel_density
    }

    /// Returns the height of the screen in millimeters.
    pub fn height(&self, height_px: u32) -> f32 {
        height_px as f32 / self.pixel_density
    }

    /// Sets the pixel density of the screen based on the width of the screen in pixels and millimeters.
    pub fn set_pixel_density(&mut self, width_px: u32, width_mm: f32) {
        self.pixel_density = width_px as f32 / width_mm;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PixelSize {
    pub width: u32,
    pub height: u32,
}

impl From<(u32, u32)> for PixelSize {
    fn from((width, height): (u32, u32)) -> Self {
        Self { width, height }
    }
}

impl From<PhysicalSize<u32>> for PixelSize {
    fn from(size: PhysicalSize<u32>) -> Self {
        Self {
            width: size.width,
            height: size.height,
        }
    }
}

impl From<PixelSize> for (u32, u32) {
    fn from(val: PixelSize) -> Self {
        (val.width, val.height)
    }
}

pub type FrameId = u64;

/// Internal window state. This is used to store the winit window, the wgpu
/// device, the wgpu queue, etc.
#[derive(Dbg)]
pub struct WindowState {
    /// the winit window
    pub winit_window: Arc<winit::window::Window>,
    /// the wgpu surface
    pub surface: wgpu::Surface<'static>,
    /// the wgpu surface configuration
    pub config: wgpu::SurfaceConfiguration,
    /// the renderers
    #[dbg(placeholder = "[[ WgpuRenderer ]]")]
    pub wgpu_renderer: WgpuRenderer,
    #[dbg(placeholder = "[[ DynamicRenderer ]]")]
    pub renderer: DynamicRenderer,
    /// The shared renderer state. This is used to share the renderer state
    #[dbg(placeholder = "[[ DynamicRenderer ]]")]
    pub shared_renderer_state: Arc<dyn SharedRendererState>,
    // The current mouse position. None if the mouse has left the window.
    pub mouse_position: Option<(f32, f32)>,
    /// Stores if the mouse cursor is currently visible.
    pub mouse_cursor_visible: bool,
    /// The size of the window in pixels.
    pub size: PixelSize,
    /// Physical properties of the screen.
    pub physical_screen: PhysicalScreen,
    /// Event handlers for the window.
    #[dbg(placeholder = "...")]
    pub event_handlers: HashMap<EventHandlerId, (EventKind, EventHandler)>,
    /// Background color of the window.
    pub bg_color: LinRgba,
    /// The frame callbacks that maps the frame number to the callback.
    #[dbg(placeholder = "...")]
    pub frame_callbacks: HashMap<FrameId, Box<dyn FnOnce() + Send>>,
    /// Queue of frames that have been submitted.
    #[dbg(placeholder = "...")]
    pub frame_queue: Vec<FrameId>,
    pub last_frame_id: FrameId,
    /// The frame currently being shown (used to dispatch events).
    pub current_frame: Option<Frame>,
}

unsafe impl Send for WindowState {}

impl WindowState {
    /// Resize the window's renders
    pub fn resize(&mut self, size: PixelSize, gpu_state: &mut GPUState) {
        self.size = size;
        self.config.width = size.width;
        self.config.height = size.height;

        self.surface.configure(&gpu_state.device, &self.config);

        self.wgpu_renderer
            .resize(size.width, size.height, &self.surface, &gpu_state.device);
    }
}

/// How to block when presenting a frame.
/// A Window represents a window on the screen. It is used to create stimuli and
/// to submit them to the screen for rendering. Each window has a render task
/// that is responsible for rendering stimuli to the screen.
#[derive(Dbg, Clone)]
#[pyclass]
pub struct Window {
    /// Window ID
    pub winit_id: WindowId,
    /// The window state. Shared between all clones of the window.
    pub state: Arc<Mutex<Option<WindowState>>>,
    /// gpu state for the window
    pub gpu_state: Arc<Mutex<GPUState>>,
    /// The global configuration for the experiment.
    pub config: Arc<Mutex<crate::config::ExperimentConfig>>,
    /// Broadcast sender for keyboard events.
    pub event_broadcast_sender: async_broadcast::Sender<Event>,
    /// Broadcast receiver for keyboard events.
    pub event_broadcast_receiver: async_broadcast::InactiveReceiver<Event>,
    /// The Instant at which the window was created.
    pub created_at: Instant,
    #[cfg(target_os = "windows")]
    /// A Buffer that holds recent presentation (or vsync) timestamps.
    pub win32_scanline_samples: Arc<Mutex<(VecDeque<Instant>, VecDeque<u32>)>>,
    /// The refresh rate as estimated at the last presentation.
    pub estimated_refresh_rate: Arc<AtomicF64>,
    /// A running list of vblanl timestamps
    pub vblank_times: Arc<Mutex<VecDeque<f64>>>,
    /// The last vblank time at which a frame was presented.
    pub last_vblank_time_presented_at: Arc<AtomicF64>,
}

impl Window {
    /// Creates a new physical input receiver that will receive physical input
    /// events from the window.
    pub fn create_event_receiver(&self) -> EventReceiver {
        EventReceiver {
            receiver: self.event_broadcast_receiver.activate_cloned(),
        }
    }

    /// Resizes the window's surface to the given size.
    pub fn resize(&self, size: impl Into<PixelSize>) {
        let size = size.into();
        let mut gpu_state = self.gpu_state.lock().unwrap();
        let mut win_state = self.state.lock().unwrap();
        let mut win_state = win_state.as_mut().unwrap();

        win_state.resize(size, &mut gpu_state);
    }

    /// Present a frame on the window. Returns the timestamp of the vblank at which the frame was presented.
    pub fn present(&self, frame: &mut Frame) -> PsydkResult<()> {
        // lock the gpu state and window state
        let gpu_state = &mut self.gpu_state.lock().unwrap();
        let mut win_state = &mut self.state.lock().unwrap();
        let mut win_state = win_state.as_mut().unwrap();

        let device = &gpu_state.device;
        let queue = &gpu_state.queue;
        let width = win_state.size.width;
        let height = win_state.size.height;

        let config = win_state.config.clone();

        // push frame id
        let new_frame_id = win_state.last_frame_id + 1;
        win_state.frame_queue.push(new_frame_id);

        // find and take all onset events and copy them
        let frame_onset_events = frame
            .event_handlers
            .iter()
            .filter(|(_, (kind, _))| *kind == EventKind::Onset)
            .map(|(id, (_, handler))| (*id, handler.clone()))
            .collect::<Vec<_>>();

        // push onset event from frame to the event queue
        let onset_callback_fn = move || {
            for (id, handler) in frame_onset_events.iter() {
                // create a new event
                let onset_event = Event::Onset {
                    timestamp: Instant::now().into(),
                };
                // call the handler
                handler(onset_event);
            }
        };

        win_state
            .frame_callbacks
            .insert(new_frame_id, Box::new(onset_callback_fn));

        let suface_texture = win_state
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");

        let width = suface_texture.texture.size().width;
        let height = suface_texture.texture.size().height;

        let texture = win_state.wgpu_renderer.texture();

        let mut scene = win_state.renderer.create_scene(width, height);

        for stimulus in &frame.stimuli {
            let now = Instant::now();
            let mut stimulus = (&stimulus).lock();
            stimulus.update_animations(now, &win_state);
            stimulus.draw(&mut scene, &win_state);
        }

        win_state
            .renderer
            .render_to_texture(device, queue, texture, width, height, &mut scene);

        let surface_texture_view = suface_texture.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(config.format),
            ..wgpu::TextureViewDescriptor::default()
        });

        // render the texture to the surface
        win_state
            .wgpu_renderer
            .render_to_texture(device, queue, &surface_texture_view);

        // on metal, we will don't need to use the frame queue as we can tell metal to run the callback
        // #[cfg(all(target_os = "macos", feature = "metal"))]
        // unsafe {
        //     // if let Some(on_present) = frame.on_present.take() {
        //     //     let drawable = unsafe {
        //             suface_texture.texture
        //                 .as_hal::<wgpu::hal::api::Metal, _, _>(|suface_texture| {

        //                     if let Some(suface_texture) = suface_texture {

        //                     }
        //                 });
        //     //     };
        //     // }
        // }
        //

        // write frame to window_state
        win_state.current_frame = Some(frame.clone());

        // present the frame
        suface_texture.present();

        let t0 = Instant::now();

        // on dx12, get the frame id and add it to the frame queue
        // then wait for the frame to be presented
        #[cfg(all(feature = "dx12", target_os = "windows"))]
        {
            let swap_chain = unsafe {
                win_state
                    .surface
                    .as_hal::<wgpu::hal::api::Dx12>()
                    .unwrap()
                    .swap_chain()
                    .unwrap()
            };

            let waitable_handle = unsafe {
                win_state
                    .surface
                    .as_hal::<wgpu::hal::api::Dx12>()
                    .unwrap()
                    .waitable_handle()
                    .unwrap()
            };

            // let frame_id = unsafe { swap_chain.GetLastPresentCount() }.expect("Failed to get frame id");
            // win_state.frame_queue.push(frame_id.into());
            // this is waiting for the frame latency waitable object to be signaled
            unsafe { windows::Win32::System::Threading::WaitForSingleObject(waitable_handle, 10000) };

            // now we can run the callback
            if let Some(callback) = win_state.frame_callbacks.remove(&new_frame_id) {
                callback();
            }
        }

        // TODO wait for the frame to be presented
        // TODO on Windows, we will run the callback here
        // TODO on MacOS we will let Metal run the callback

        let onset_time = Instant::now();

        // we run our vblank finding algorithm here, which will yield a list of timestamps where the vblanks occured
        // we can then use this to estimate the refresh rate and figure out when the last flip occurred
        let xs = self
            .win32_scanline_samples
            .lock()
            .unwrap()
            .0
            .iter()
            .map(|x| x.duration_since(self.created_at).as_secs_f64() * 1000.0)
            .collect::<Vec<_>>();
        let ys = self
            .win32_scanline_samples
            .lock()
            .unwrap()
            .1
            .iter()
            .map(|y| *y as f64)
            .collect::<Vec<_>>();
        let vblanks = find_zero_crossings(&xs, &ys);

        // use vblanks to estimate the refresh rate
        let estimated_refresh_rate = super::utils::estimate_refresh_rate(&vblanks);

        if let Some((refresh_rate, last_recorded_time)) = estimated_refresh_rate {
            // update the estimated refresh rate
            self.estimated_refresh_rate.store(refresh_rate, Ordering::Relaxed);
            let current_time = Instant::now().duration_since(self.created_at).as_secs_f64() * 1000.0; // in milliseconds
            let diff = current_time - (last_recorded_time as f64); //+ TOLARENCE; // in milliseconds

            let expected_flips = (diff * (refresh_rate / 1000.0)).floor() as u32;
            // println!(
            //     "Estimated refresh rate: {refresh_rate:.2} Hz, last recorded time: {last_recorded_time:.2} ms, current time: {current_time:.2} ms, diff: {diff:.2} ms, expected flips: {expected_flips}",
            // );

            let estimated_last_t = last_recorded_time + (expected_flips as f64 * (1000.0 / refresh_rate));

            // update the last vsync timestamp
            self.last_vblank_time_presented_at
                .store(estimated_last_t, Ordering::Relaxed);

            let diff_est = current_time - estimated_last_t;
            // println!("Time since last estimated flip: {diff_est:.3} ms");
        }

        Ok(())
    }

    pub fn close(&self) {
        // close the window
        let mut win_state = self.state.lock().unwrap();
        // set the state to None
        *win_state = None;
    }

    pub fn get_current_refresh_rate(&self) -> Option<f64> {
        let winit_window = {
            let win_state = self.state.lock().unwrap();
            let win_state = win_state.as_ref().unwrap();
            win_state.winit_window.clone()
        };

        let monitor = winit_window.current_monitor();

        if let Some(monitor) = monitor {
            monitor.refresh_rate_millihertz().map(|x| x as f64 / 1000.0)
        } else {
            None
        }
    }

    pub fn get_current_monitor(&self) -> Option<Monitor> {
        let winit_window = {
            let win_state = self.state.lock().unwrap();
            let win_state = win_state.as_ref().unwrap();
            win_state.winit_window.clone()
        };
        let monitor = winit_window.current_monitor();

        if let Some(monitor) = monitor {
            Some(Monitor {
                name: monitor.name().unwrap_or_default(),
                resolution: monitor.size().into(),
                handle: monitor.clone(),
            })
        } else {
            None
        }
    }

    /// Set the visibility of the mouse cursor.
    pub fn set_cursor_visible(&self, visible: bool) {
        let mut win_state = self.state.lock().unwrap();
        let mut win_state = win_state.as_mut().unwrap();
        win_state.mouse_cursor_visible = visible;
        win_state.winit_window.set_cursor_visible(false);
    }

    /// Returns true if the mouse cursor is currently visible.
    pub fn cursor_visible(&self) -> bool {
        let win_state = self.state.lock().unwrap();
        let win_state = win_state.as_ref().unwrap();

        win_state.mouse_cursor_visible
    }

    /// Returns the mouse position. None if cursor not in window.
    pub fn mouse_position(&self) -> Option<(f32, f32)> {
        let win_state = self.state.lock().unwrap();
        let win_state = win_state.as_ref().unwrap();
        win_state.mouse_position.clone()
    }

    /// Returns the 4x4 matrix than when applied to pixel coordinates will transform
    /// them to normalized device coordinates. Pixel coordinates are in a
    /// coordinate system with (0.0,0.0) in the center of the screen and
    /// (half of screen width in px, half of screen height in px) in the top right
    /// corner of the screen.
    #[rustfmt::skip]
    pub fn transformation_matrix_to_ndc(width_px: u32, height_px: u32) -> nalgebra::Matrix3<f64> {
        nalgebra::Matrix3::new(
            2.0 / width_px as f64,0.0, 0.0,
            0.0, 2.0 / height_px as f64, 0.0,
            0.0, 0.0, 1.0,
        )
    }

    /// Returns the size of the window in pixels.
    pub fn size(&self) -> PixelSize {
        let win_state = self.state.lock().unwrap();
        let win_state = win_state.as_ref().unwrap();
        win_state.size
    }

    /// Return a new frame for the window.
    pub fn get_frame(&self) -> Frame {
        let win_state = self.state.lock().unwrap();
        let win_state = win_state.as_ref().unwrap();

        let mut frame = Frame {
            stimuli: Vec::new(),
            window: self.clone(),
            event_handlers: HashMap::new(),
        };

        frame.set_bg_color(win_state.bg_color);

        frame
    }
    fn remove_event_handler(&self, id: EventHandlerId) {
        let mut state = self.state.lock().unwrap();
        let state = state.as_mut().unwrap();
        state.event_handlers.remove(&id);
    }

    pub fn dispatch_event(&self, event: Event) -> bool {
        let mut handled = false;

        let event_handlers = {
            let state = self.state.lock().unwrap();
            let state = state.as_ref().unwrap();

            // clone the event handlers
            let event_handlers = &state.event_handlers;

            let mut new_event_handlers: HashMap<EventHandlerId, (EventKind, EventHandler)> = HashMap::new();
            for (id, (kind, handler)) in event_handlers.iter() {
                new_event_handlers.insert(*id, (*kind, handler.clone()));
            }

            new_event_handlers
        };

        for (id, (kind, handler)) in event_handlers.iter() {
            // println!("Checking handler with id: {} for event kind: {:?}", id, kind);
            if kind == &event.kind() {
                // println!("Dispatching event to handler with id: {}", id);
                handled |= handler(event.clone());
                // println!("Handler with id: {} returned: {}", id, handled);
            }
        }

        // dispatch to frame
        let state = self.state.lock().unwrap();
        let state = state.as_ref().unwrap();

        if let Some(frame) = &state.current_frame {
            handled |= frame.dispatch_event_to_stimuli(&event, &state);
        }
        handled
    }

    fn add_event_handler<F>(&self, kind: EventKind, handler: F) -> EventHandlerId
    where
        F: Fn(Event) -> bool + 'static + Send + Sync,
    {
        let mut state = self.state.lock().unwrap();
        let mut state = state.as_mut().unwrap();
        let mut event_handlers = &mut state.event_handlers;

        // find a free id
        let id = loop {
            let id = rand::random::<EventHandlerId>();
            if !event_handlers.contains_key(&id) {
                break id;
            }
        };

        // add handler
        event_handlers.insert(id, (kind, Arc::new(handler)));

        id
    }

    fn set_soft_keyboard_shown(&self, allowed: bool) {
        let win = self
            .state
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .winit_window
            .set_ime_allowed(allowed);
    }
}

#[pymethods]
impl Window {
    #[pyo3(name = "get_frame")]
    fn py_get_frame(&self, py: Python) -> Frame {
        let self_wrapper = SendWrapper::new(self.clone());
        let d = py.allow_threads(move || SendWrapper::new(self_wrapper.get_frame()));
        d.take()
    }

    #[pyo3(name = "get_frames")]
    fn py_get_frames(&self, py: Python, n: u32) -> FrameIterator {
        FrameIterator {
            window: self.clone(),
            mode: FrameIterationMode::Number(n),
            current_index: 0,
            first_presentation_time: None,
            handle_missed_frames: false,
            start_time: Instant::now(),
        }
    }

    #[pyo3(name = "get_frames_for_duration")]
    fn py_get_frames_for_duration(&self, py: Python, duration: f64) -> FrameIterator {
        FrameIterator {
            window: self.clone(),
            mode: FrameIterationMode::Time(Duration::from_secs_f64(duration)),
            current_index: 0,
            first_presentation_time: None,
            handle_missed_frames: false,
            start_time: Instant::now(),
        }
    }

    #[pyo3(name = "present")]
    #[pyo3(signature = (frame))]
    /// Present a frame on the window.
    fn py_present(&self, frame: &mut Frame, py: Python) -> PyResult<()> {
        let self_wrapper = SendWrapper::new(self.clone());
        let frame_wrapper = SendWrapper::new(frame);
        py.allow_threads(move || self_wrapper.present(frame_wrapper.take()))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    #[getter(cursor_visible)]
    fn py_cursor_visible(&self) -> bool {
        self.cursor_visible()
    }

    #[setter(cursor_visible)]
    fn py_set_cursor_visible(&self, visible: bool) {
        self.set_cursor_visible(visible);
    }

    #[pyo3(name = "get_current_monitor")]
    fn py_get_current_monitor(&self, py: Python) -> Option<Monitor> {
        let self_wrapper = SendWrapper::new(self);
        py.allow_threads(move || self_wrapper.get_current_monitor())
    }

    #[pyo3(name = "get_size")]
    fn py_get_size(&self, py: Python) -> (u32, u32) {
        self.size().into()
    }

    #[pyo3(name = "bg_color")]
    #[getter]
    fn py_get_bg_color(&self, py: Python) -> LinRgba {
        let self_wrapper = SendWrapper::new(self);
        py.allow_threads(move || {
            let state = self_wrapper.state.lock().unwrap();
            let state = state.as_ref().unwrap();
            state.bg_color
        })
    }

    #[pyo3(name = "bg_color")]
    #[setter]
    fn py_set_bg_color(&self, py: Python, bg_color: LinRgba) {
        let self_wrapper = SendWrapper::new(self);
        py.allow_threads(move || {
            let mut state = self_wrapper.state.lock().unwrap();
            let mut state = state.as_mut().unwrap();
            state.bg_color = bg_color
        })
    }

    /// Add an event handler to the window. The event handler will be called
    /// whenever an event of the specified kind occurs.
    ///
    /// Parameters
    /// ----------
    /// kind : EventKind
    ///   The kind of event to listen for.
    /// callback : callable
    ///  The callback that will be called when the event occurs. The callback should take a single argument, an instance of `Event`.
    #[pyo3(name = "add_event_handler")]
    fn py_add_event_handler(&self, kind: EventKind, callback: Py<PyAny>, py: Python<'_>) -> EventHandlerId {
        // let kind = EventKind::from_str(&kind).expect("Invalid event kind");

        let rust_callback_fn = move |event: Event| -> bool {
            Python::with_gil(|py| -> PyResult<()> {

                    callback.call1(py, (event,))
                            .expect("Error calling callback in event handler. Make sure the callback takes a single argument of type Event. Error");
                    Ok(())
                }).unwrap();
            false
        };

        let self_wrapper = SendWrapper::new(self);

        let id = py.allow_threads(move || self_wrapper.add_event_handler(kind, rust_callback_fn));

        id
    }

    /// Remove an event handler from the window.
    #[pyo3(name = "remove_event_handler")]
    fn py_remove_event_handler(&self, id: EventHandlerId, py: Python) {
        let self_wrapper = SendWrapper::new(self);
        py.allow_threads(move || self_wrapper.remove_event_handler(id));
    }

    /// Create a new EventReceiver that will receive events from the window.
    #[pyo3(name = "create_event_receiver")]
    fn py_create_event_receiver(&self) -> EventReceiver {
        self.create_event_receiver()
    }

    /// Set the visibility of the soft keyboard.
    #[pyo3(name = "set_soft_keyboard_shown")]
    fn py_set_soft_keyboard_shown(&self, allowed: bool, py: Python) {
        let self_wrapper = SendWrapper::new(self);
        py.allow_threads(move || self_wrapper.set_soft_keyboard_shown(allowed));
    }

    // allows Window to be used as a context manager
    fn __enter__(slf: PyRef<Self>) -> PyResult<Py<Self>> {
        // return self
        Ok(slf.into())
    }

    fn __exit__(
        slf: PyRef<Self>,
        _exc_type: Option<PyObject>,
        _exc_value: Option<PyObject>,
        _traceback: Option<PyObject>,
    ) -> PyResult<()> {
        // slf.close();
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FrameIterationMode {
    Number(u32),
    Time(Duration),
    Forever,
}

/// FrameIterator is an iterator that yields frames.
#[derive(Debug, Clone)]
#[pyclass]
pub struct FrameIterator {
    /// The window that the frames are associated with.
    window: Window,
    /// How long to iterate over frames.
    mode: FrameIterationMode,
    /// The current frame index.
    current_index: u32,
    /// First time at which a frame from the iterator was presented.
    first_presentation_time: Option<f64>,
    /// If the iterator should handle missed frames.
    handle_missed_frames: bool,
    /// The time at which the iterator was started.
    start_time: Instant,
}

#[pymethods]
impl FrameIterator {
    fn __iter__(slf: PyRef<Self>) -> PyResult<Py<FrameIterator>> {
        Ok(slf.into())
    }

    fn __next__(mut slf: PyRefMut<Self>, py: Python) -> PyResult<Option<(u32, Frame)>> {
        if slf.handle_missed_frames {
            if slf.current_index == 0 {
                slf.first_presentation_time = Some(slf.window.last_vblank_time_presented_at.load(Ordering::Relaxed));
            } else if let Some(first_presentation_time) = slf.first_presentation_time {
                // we just count the number of vblanks that have occurred since the first presentation time
                // for this, we iterate over the vblank times and count how many are greater than the first presentation time
                let expected_vblanks = {
                    let vblank_times = slf.window.vblank_times.lock().unwrap();
                    vblank_times.iter().filter(|&&t| t > first_presentation_time).count() as u32
                };
                slf.current_index = expected_vblanks;
            }
        }

        match slf.mode {
            FrameIterationMode::Number(n) => {
                if slf.current_index < n {
                    let slf_wrapper = SendWrapper::new(slf.window.clone());
                    let frame = py.allow_threads(move || slf_wrapper.get_frame());
                    let frame_id = slf.current_index;
                    slf.current_index += 1;
                    Ok(Some((frame_id, frame)))
                } else {
                    Ok(None)
                }
            }
            FrameIterationMode::Time(duration) => {
                if slf.start_time.elapsed() < duration {
                    let slf_wrapper = SendWrapper::new(slf.window.clone());
                    let frame = py.allow_threads(move || slf_wrapper.get_frame());
                    let frame_id = slf.current_index;
                    slf.current_index += 1;
                    Ok(Some((frame_id, frame)))
                } else {
                    Ok(None)
                }
            }
            FrameIterationMode::Forever => {
                let slf_wrapper = SendWrapper::new(slf.window.clone());
                let frame = py.allow_threads(move || slf_wrapper.get_frame());
                let frame_id = slf.current_index;
                slf.current_index += 1;
                Ok(Some((frame_id, frame)))
            }
        }
    }
}

#[derive(Dbg, Clone)]
#[pyclass]
pub struct Frame {
    #[dbg(placeholder = "...")]
    /// The vector of stimuli that will be drawn upon presentation.
    stimuli: Vec<DynamicStimulus>,
    /// The window that the frame is associated with.
    window: Window,
    /// An optional callback that will be called when the frame is presented.
    #[dbg(placeholder = "...")]
    pub event_handlers: HashMap<EventHandlerId, (EventKind, EventHandler)>,
}

impl Frame {
    /// Set the background color of the frame.
    pub fn set_bg_color(&mut self, bg_color: LinRgba) {
        // TODO
    }

    /// Draw onto the frame.
    pub fn add(&mut self, stimulus: &DynamicStimulus) {
        self.stimuli.push(stimulus.clone());

        // let now = Instant::now();
        // {
        //     // this needs to be scoped so that the mutable borrow of self is released
        //     let window_state = self.window.state.lock().unwrap();
        //     stimulus.update_animations(now, &window_state);
        // }

        // stimulus.draw(self);
    }

    fn add_event_handler<F>(&mut self, kind: EventKind, handler: F) -> EventHandlerId
    where
        F: Fn(Event) -> bool + 'static + Send + Sync,
    {
        let mut event_handlers = &mut self.event_handlers;

        // find a free id
        let id = loop {
            let id = rand::random::<EventHandlerId>();
            if !event_handlers.contains_key(&id) {
                break id;
            }
        };

        // add handler
        event_handlers.insert(id, (kind, Arc::new(handler)));

        id
    }

    pub fn window(&self) -> Window {
        self.window.clone()
    }

    /// Dispatch an even to the Frame's stimuli.
    pub fn dispatch_event_to_stimuli(&self, event: &Event, window_state: &WindowState) -> bool {
        let mut handled = false;

        for stimulus in &self.stimuli {
            // dispatch the event to the stimulus
            handled |= stimulus.lock().dispatch_event(&event, window_state);
            if handled {
                // if the event was handled, we can stop dispatching it
                break;
            }
        }

        handled
    }
}

#[pymethods]
impl Frame {
    #[pyo3(name = "add")]
    fn py_add(&mut self, stimulus: crate::visual::stimuli::PyStimulus, py: Python) {
        let mut self_wrapper = SendWrapper::new(self);
        let stimulus_wrapper = SendWrapper::new(stimulus);
        py.allow_threads(move || self_wrapper.add(stimulus_wrapper.as_super()));
    }

    #[setter(bg_color)]
    fn py_set_bg_color(&mut self, bg_color: super::color::LinRgba) {
        self.set_bg_color(bg_color);
    }

    #[pyo3(name = "add_event_handler")]
    fn py_add_event_handler(&mut self, kind: EventKind, callback: Py<PyAny>, py: Python<'_>) -> EventHandlerId {
        let rust_callback_fn = move |event: Event| -> bool {
            Python::with_gil(|py| -> PyResult<()> {
                callback.call1(py, (event,))
                    .expect("Error calling callback in event handler. Make sure the callback takes a single argument of type Event. Error");
                Ok(())
            }).unwrap();
            false
        };

        let mut self_wrapper = SendWrapper::new(self);

        let id = py.allow_threads(move || self_wrapper.add_event_handler(kind, rust_callback_fn));

        id
    }
}
