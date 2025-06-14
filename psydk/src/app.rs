use std::{
    collections::HashMap,
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use derive_debug::Dbg;
use pyo3::{
    pyclass, pyfunction,
    types::{PyDict, PyTuple},
    Py, PyAny, Python,
};
use renderer::{cosmic_text, renderer::SharedRendererState, wgpu::TextureFormat};
use wgpu::MemoryHints;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    monitor::MonitorHandle,
    window::{Window as WinitWindow, WindowId},
};

use crate::{
    config::ExperimentConfig,
    context::{EventLoopAction, ExperimentContext, GammaOptions, Monitor, WindowOptions},
    errors,
    input::Event,
    visual::{
        color::LinRgba,
        window::{PhysicalScreen, Window, WindowState},
    },
    EventTryFrom,
};

pub type ArcMutex<T> = Arc<Mutex<T>>;

#[derive(Debug)]
pub struct GPUState {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

#[derive(Dbg)]
pub struct App {
    pub windows: Vec<Window>,
    pub gpu_state: ArcMutex<GPUState>,
    pub action_receiver: Receiver<EventLoopAction>,
    pub action_sender: Sender<EventLoopAction>,
    pub dummy_window: Option<Window>,
    #[dbg(placeholder = "[[ RendererFactory ]]")]
    pub shared_renderer_state: Arc<dyn SharedRendererState>,
    pub font_manager: ArcMutex<renderer::cosmic_text::FontSystem>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let (action_sender, action_receiver) = std::sync::mpsc::channel();

        let backend = wgpu::Backends::METAL | wgpu::Backends::DX12;
        let backend_options = wgpu::BackendOptions {
            gl: wgpu::GlBackendOptions::default(),
            dx12: wgpu::Dx12BackendOptions {
                latency_waitable_object: wgpu::wgt::Dx12UseFrameLatencyWaitableObject::DontWait,
                ..Default::default()
            },
            noop: wgpu::NoopBackendOptions::default(),
        };
        let instance_desc = wgpu::InstanceDescriptor {
            backends: backend,
            backend_options,
            // use defaults for the rest
            ..Default::default()
        };

        let instance = wgpu::Instance::new(&instance_desc);

        // request an adapter
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None, // idealy we would use the surface here, but we don't have it yet
        }))
        .expect("Failed to find an suitable graphics adapter. This is likely a bug, please report it.");

        log::debug!("Selected graphics adapter: {:?}", adapter.get_info());

        let mut limits = wgpu::Limits::downlevel_defaults();
        limits.max_storage_buffers_per_shader_stage = 16;

        let features =
            wgpu::Features::TEXTURE_FORMAT_16BIT_NORM | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;

        // Create the logical device and command queue
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: features,
            // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
            required_limits: limits.using_resolution(adapter.limits()),
            memory_hints: MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .expect("Failed to create device. This is likely a bug, please report it.");

        let gpu_state = GPUState {
            instance,
            adapter,
            device,
            queue,
        };

        // create font manager
        // create a font system (=font manager)
        let empty_db = cosmic_text::fontdb::Database::new();
        let mut font_manager = cosmic_text::FontSystem::new_with_locale_and_db("en".to_string(), empty_db);

        // load Noto Sans
        let noto_sans_regular = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");
        font_manager.db_mut().load_font_data(noto_sans_regular.to_vec());
        let noto_sans_bold = include_bytes!("../assets/fonts/NotoSans-Bold.ttf");
        font_manager.db_mut().load_font_data(noto_sans_bold.to_vec());
        let noto_sans_italic = include_bytes!("../assets/fonts/NotoSans-Italic.ttf");
        font_manager.db_mut().load_font_data(noto_sans_italic.to_vec());
        let noto_sans_bold_italic = include_bytes!("../assets/fonts/NotoSans-BoldItalic.ttf");
        font_manager.db_mut().load_font_data(noto_sans_bold_italic.to_vec());

        // create shared renderer state
        let renderer = renderer::skia_backend::SkiaSharedRendererState::new(
            &gpu_state.adapter,
            &gpu_state.device,
            &gpu_state.queue,
        );

        Self {
            windows: vec![],
            gpu_state: Arc::new(Mutex::new(gpu_state)),
            action_receiver,
            action_sender,
            dummy_window: None,
            shared_renderer_state: Arc::new(renderer),
            font_manager: Arc::new(Mutex::new(font_manager)),
        }
    }

    /// Create a new window with the given options.
    pub fn create_window(
        &self,
        window_options: &WindowOptions,
        gamma_options: GammaOptions,
        event_loop: &ActiveEventLoop,
    ) -> Window {
        let window_attributes = WinitWindow::default_attributes()
            .with_title("Winit window")
            .with_transparent(false);

        let winit_window = event_loop.create_window(window_attributes).unwrap();

        // make sure cursor is visible (for normlisation across platforms)
        winit_window.set_cursor_visible(true);

        winit_window.focus_window();

        // log::debug!("Window created: {:?}", winit_window);

        let winit_window = Arc::new(winit_window);

        let gpu_state = self.gpu_state.lock().unwrap();

        let instance = &gpu_state.instance;
        let adapter = &gpu_state.adapter;
        let device = &gpu_state.device;
        let queue = &gpu_state.queue;

        log::debug!("Creating wgup surface...");

        let surface = instance
            .create_surface(winit_window.clone())
            .expect("Failed to create surface. This is likely a bug, please report it.");

        // print supported swapchain formats
        let swapchain_formats = surface.get_capabilities(adapter).formats;
        log::debug!("Supported swapchain formats: {:?}", swapchain_formats);

        let size = winit_window.inner_size();

        let _swapchain_formats = adapter.get_texture_format_features(TextureFormat::Bgra8Unorm);

        let swapchain_capabilities = surface.get_capabilities(adapter);
        let swapchain_format = TextureFormat::Bgra8Unorm;
        let swapchain_view_format = vec![TextureFormat::Bgra8Unorm];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: swapchain_view_format,
            desired_maximum_frame_latency: 1,
        };

        log::debug!("Surface configuration: {:?}", config);

        surface.configure(device, &config);

        // set fullscreen mode
        let mon_handle = window_options.monitor().unwrap().handle();
        let mon_name = mon_handle.name().unwrap_or("Unnamed monitor".to_string());

        winit_window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(Some(mon_handle.clone()))));

        let wgpu_renderer = pollster::block_on(renderer::wgpu_renderer::WgpuRenderer::new(
            winit_window.clone(),
            instance,
            device,
            queue,
            swapchain_format,
            gamma_options.lut,
            gamma_options.encode_gamma,
        ));

        // create the renderer
        let mut renderer = self
            .shared_renderer_state
            .create_renderer(swapchain_format, size.width, size.height);

        let winit_id = winit_window.id();

        // set width of the screen to 30 cm
        let width_mm = 300.0;
        let viewing_distance = 1000.0;

        // create a pwindow
        let window_state = WindowState {
            winit_window: winit_window.clone(),
            surface,
            config,
            renderer,
            wgpu_renderer,
            shared_renderer_state: self.shared_renderer_state.clone(),
            mouse_cursor_visible: true,
            mouse_position: None,
            size: size.into(),
            physical_screen: PhysicalScreen::new(size.width, width_mm, viewing_distance),
            event_handlers: HashMap::new(), // TODO this should be a weak reference
            bg_color: LinRgba::new(0.5, 0.5, 0.5, 1.0),
            frame_callbacks: HashMap::new(),
            frame_queue: Vec::new(),
            last_frame_id: 0,
        };

        // create channel for physical input
        let (mut event_broadcast_sender, physical_input_receiver) = async_broadcast::broadcast(10_000);
        event_broadcast_sender.set_overflow(true);
        // deactivate the receiver
        let event_broadcast_receiver = physical_input_receiver.deactivate();

        #[cfg(all(feature = "dx12", target_os = "windows"))]
        {
            let swap_chain = unsafe {
                window_state
                    .surface
                    .as_hal::<wgpu::hal::api::Dx12, _, _>(|surface| surface.unwrap().swap_chain().unwrap())
            };

            let waitable_handle = unsafe {
                window_state
                    .surface
                    .as_hal::<wgpu::hal::api::Dx12, _, _>(|surface| surface.unwrap().waitable_handle().unwrap())
            };

            // this is waiting for the frame latency waitable object to be signaled
            unsafe { windows::Win32::System::Threading::WaitForSingleObject(waitable_handle, 10000) };
        }

        // create handle
        let window = Window {
            winit_id,
            state: Arc::new(Mutex::new(Some(window_state))),
            gpu_state: self.gpu_state.clone(),
            event_broadcast_sender,
            event_broadcast_receiver,
            config: Arc::new(Mutex::new(ExperimentConfig::default())),
        };

        let win_clone = window.clone();

        // add a default event handler for mouse move events, which updates the mouse
        // position
        // window.add_event_handler(EventKind::CursorMoved, move |event| {
        //     if let Some(pos) = event.position() {
        //         win_clone.inner().mouse_position = Some(pos.clone());
        //     };
        //     false
        // });

        window
    }

    // /// Run the app
    // pub fn run(&mut self) {
    //     // create event loop
    //     let event_loop = EventLoop::new().unwrap();
    //     event_loop.set_control_flow(ControlFlow::Poll);
    //     let _ = event_loop.run_app(self);
    // }

    /// Starts the experiment. This will block until the experiment is finished.
    pub fn run_experiment<F>(&mut self, experiment_fn: F) -> Result<(), errors::PsydkError>
    where
        F: FnOnce(ExperimentContext) -> Result<(), errors::PsydkError> + 'static + Send,
    {
        log::debug!("Main task is running on thread {:?}", std::thread::current().id());

        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(ControlFlow::Poll);

        let event_loop_proxy = event_loop.create_proxy();
        let event_loop_proxy2 = event_loop.create_proxy();

        let action_sender = self.action_sender.clone();

        let audio_host = timed_audio::cpal::default_host().into();

        let exp_manager = ExperimentContext::new(
            self.gpu_state.clone(),
            event_loop_proxy,
            action_sender.clone(),
            self.shared_renderer_state.clone(),
            audio_host,
            self.font_manager.clone(),
        );

        // create mutex to hold potential error
        let error_mutex = Arc::new(Mutex::new(None));
        let error_mutex_clone = error_mutex.clone();

        // start experiment
        thread::spawn(move || {
            let res = experiment_fn(exp_manager);

            // send Exit event to the event loop, then wake it up
            action_sender.send(EventLoopAction::Exit(None)).unwrap();
            event_loop_proxy2.send_event(()).unwrap();

            // panic if the experiment function returns an error
            if let Err(e) = res {
                // put the error in the mutex
                let mut error = error_mutex_clone.lock().unwrap();
                *error = Some(e);
            }
        });

        // start event loop
        let _ = event_loop.run_app(self);

        // check if there was an error
        let error = error_mutex.lock().unwrap().take();
        match error {
            Some(e) => {
                return Err(e);
            }
            None => {
                return Ok(());
            }
        }
    }

    // Start a thread that will dispath
}

impl ApplicationHandler<()> for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {}

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: ()) {
        // check if we need to create a new window
        self.action_receiver.try_recv().map(|action| match action {
            EventLoopAction::CreateNewWindow(options, gamma_options, sender) => {
                let window = self.create_window(&options, gamma_options, event_loop);
                self.windows.push(window.clone());
                sender.send(window).unwrap();
            }
            EventLoopAction::GetAvailableMonitors(sender) => {
                let monitors = event_loop.available_monitors();

                // convert into a vector of monitors
                let monitors: Vec<Monitor> = monitors
                    .map(|monitor| {
                        Monitor::new(monitor.name().unwrap_or("Unnamed monitor".to_string()), (0, 0), monitor)
                    })
                    .collect();
                sender.send(monitors).unwrap();
            }
            EventLoopAction::Exit(..) => {
                event_loop.exit();
            }
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                // for now, exit the program
                std::process::exit(0);
                // find the window
                let window = self.windows.iter().find(|w| w.winit_id == window_id);

                if let Some(window) = window {
                    // remove the window
                    self.windows.retain(|w| w.winit_id != window_id);
                }
            }
            WindowEvent::Resized(size) => {
                // find the window
                let window = self.windows.iter().find(|w| w.winit_id == window_id);

                if let Some(window) = window {
                    // update the window size
                    window.resize(size);
                }
            }
            WindowEvent::KeyboardInput { .. }
            | WindowEvent::CursorMoved { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::Touch { .. } => {
                // find the window
                let window = self.windows.iter().find(|w| w.winit_id == window_id);

                // if this was a cursor moved event, update the mouse position
                if let WindowEvent::CursorMoved { position, .. } = event {
                    if let Some(window) = window {
                        let mut window_state = window.state.lock().unwrap();
                        let window_state = window_state.as_mut().unwrap();
                        let win_size = window_state.size;
                        let shifted_position = (
                            position.x as f32 - win_size.width as f32 / 2.0,
                            position.y as f32 - win_size.height as f32 / 2.0,
                        );
                        window_state.mouse_position = Some(shifted_position);
                    }
                }

                if let Some(window) = window {
                    if let Some(input) = Event::try_from_winit(event.clone(), &window).ok() {
                        // if escape key was pressed, close window
                        if input.key_pressed("\u{1b}") {
                            // for now, just exit the program
                            std::process::exit(0);
                        }

                        // broadcast the event
                        window.event_broadcast_sender.try_broadcast(input.clone()); //.unwrap();

                        // send the event to the window
                        window.dispatch_event(input);
                    }
                }
            }
            _ => {}
        }
    }
}
