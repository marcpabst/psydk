use crate::visual::colors::display::GenericDisplayCharacteristics;
use atomic_float::AtomicF64;
use derive_debug::Dbg;
use pyo3::{
    pyclass, pyfunction,
    types::{PyDict, PyTuple},
    Py, PyAny, Python,
};

#[cfg(target_os = "ios")]
use winit::platform::ios::EventLoopBuilderExtIOS;

use crate::visual::renderer::{
    color_formats::{ColorEncoding, ColorFormat},
    cosmic_text,
    wgpu::TextureFormat,
    Renderer,
};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU32, AtomicU64},
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use crate::visual::colors::Color;
use wgpu::MemoryHints;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    monitor::MonitorHandle,
    window::{Window as WinitWindow, WindowAttributes, WindowId},
};

use crate::{
    config::{ColorType, ExperimentConfig, WindowConfig},
    context::{EventLoopAction, ExperimentContext, Monitor, WindowOptions},
    errors,
    input::Event,
    visual::window::{PhysicalScreen, Window, WindowState},
    EventTryFrom,
};

pub type ArcMutex<T> = Arc<Mutex<T>>;

#[derive(Debug)]
/// Holds the GPU state (instance, adapter, device, queue)s
pub struct GPUState {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

#[derive(Dbg)]
/// The main experiment (which serves as the wini nt application handler)
pub struct Experiment {
    pub windows: Vec<Window>,
    pub gpu_state: ArcMutex<GPUState>,
    pub action_receiver: Receiver<EventLoopAction>,
    pub action_sender: Sender<EventLoopAction>,
    pub dummy_window: Option<Window>,
    #[dbg(placeholder = "[[ RendererFactory ]]")]
    pub renderer: Arc<Renderer>,
    pub font_manager: ArcMutex<crate::visual::renderer::cosmic_text::FontSystem>,
}

impl Experiment {
    /// Create a new experiment with the given configuration.
    pub fn new(config: ExperimentConfig) -> Result<Self, errors::PsydkError> {
        // the main event loop is running on the main thread, but our experiment code will run in a separate thread
        // so we need to create a channel to communicate between the two threads
        let (action_sender, action_receiver) = std::sync::mpsc::channel();

        // ESSENTIAL GPU SETUP

        // we currently only support Metal and DX12 backends
        let backend = wgpu::Backends::METAL | wgpu::Backends::DX12;
        // we need to set some backend options to enable low latency rendering on Windows with DX12
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
        .map_err(|e| errors::PsydkError::GPUError(format!("Failed to request graphics adapter: {}", e)))?;

        log::debug!("Selected graphics adapter: {:?}", adapter.get_info());

        let mut limits = wgpu::Limits::downlevel_defaults();
        limits.max_storage_buffers_per_shader_stage = 16; // do we need to request this

        // we want to use higher-bit color formats if possible
        let features = wgpu::Features::TEXTURE_FORMAT_16BIT_NORM
            | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
            | wgpu::Features::FLOAT32_FILTERABLE;

        // Create the logical device and command queue
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: features,
            // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
            required_limits: limits.using_resolution(adapter.limits()),
            memory_hints: MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| errors::PsydkError::GPUError(format!("Failed to create graphics device: {}", e)))?;

        log::debug!("Created graphics device: {:?}", device);

        let gpu_state = GPUState {
            instance,
            adapter,
            device,
            queue,
        };

        // set the pixel format/texture format used for internal rendering
        let internal_color_format = match config.internal_color_type {
            ColorType::EightBit => crate::visual::renderer::color_formats::ColorFormat::Rgba8,
            ColorType::TenBit => crate::visual::renderer::color_formats::ColorFormat::Rgba10,
            ColorType::SixteenBitFloat => crate::visual::renderer::color_formats::ColorFormat::RgbaF16,
            ColorType::ThirtyTwoBitFloat => panic!("32F color format not supported in renderer"),
        };

        // create a renderer
        let renderer = crate::visual::renderer::Renderer::new(
            &gpu_state.adapter,
            &gpu_state.device,
            &gpu_state.queue,
            crate::visual::renderer::color_formats::ColorEncoding::Linear,
            internal_color_format,
        );

        // create a font system
        let empty_db = cosmic_text::fontdb::Database::new();
        let mut font_system = cosmic_text::FontSystem::new_with_locale_and_db("en".to_string(), empty_db);

        // load Noto Sans as the default/fallback font
        let noto_sans_regular = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");
        font_system.db_mut().load_font_data(noto_sans_regular.to_vec());
        let noto_sans_bold = include_bytes!("../assets/fonts/NotoSans-Bold.ttf");
        font_system.db_mut().load_font_data(noto_sans_bold.to_vec());
        let noto_sans_italic = include_bytes!("../assets/fonts/NotoSans-Italic.ttf");
        font_system.db_mut().load_font_data(noto_sans_italic.to_vec());
        let noto_sans_bold_italic = include_bytes!("../assets/fonts/NotoSans-BoldItalic.ttf");
        font_system.db_mut().load_font_data(noto_sans_bold_italic.to_vec());

        Ok(Self {
            windows: vec![],
            gpu_state: Arc::new(Mutex::new(gpu_state)),
            action_receiver,
            action_sender,
            dummy_window: None,
            renderer: Arc::new(renderer),
            font_manager: Arc::new(Mutex::new(font_system)),
        })
    }

    /// Create a new window with the given options.
    pub fn create_window(
        &self,
        window_options: &WindowOptions,
        display_config: WindowConfig,
        experiment_config: &ExperimentConfig,
        event_loop: &dyn ActiveEventLoop,
    ) -> Window {
        let window_attributes = WindowAttributes::default();

        let winit_window = event_loop.create_window(window_attributes).unwrap();

        winit_window.focus_window();

        log::debug!("Window created: {:?}", winit_window);

        let winit_window = Arc::new(winit_window);

        let gpu_state = self.gpu_state.lock().unwrap();

        let instance = &gpu_state.instance;
        let adapter = &gpu_state.adapter;
        let device = &gpu_state.device;
        let queue = &gpu_state.queue;

        log::debug!("Creating WGPU surface...");

        let surface = instance
            .create_surface(winit_window.clone())
            .expect("Failed to create surface. This is likely a bug, please report it.");

        // print supported swapchain formats
        let swapchain_formats = surface.get_capabilities(adapter).formats;
        log::debug!("Supported swapchain formats: {:?}", swapchain_formats);

        let size = winit_window.surface_size();

        let swapchain_capabilities = surface.get_capabilities(adapter);

        // depending on the provided internal color format, there are multiple possible swapchain formats
        // not all formats are supported on all platforms, so we pick the first one that is supported
        let possible_swapchain_formats = match display_config.surface_color_depth {
            ColorType::EightBit => vec![TextureFormat::Bgra8Unorm, TextureFormat::Rgba8Unorm],
            ColorType::TenBit => vec![TextureFormat::Rgb10a2Unorm],
            ColorType::SixteenBitFloat => vec![TextureFormat::Rgba16Float],
            ColorType::ThirtyTwoBitFloat => vec![TextureFormat::Rgba32Float],
        };

        let swapchain_format = possible_swapchain_formats
            .into_iter()
            .find(|f| swapchain_formats.contains(f))
            .expect(&format!(
                "No supported swapchain format found for the requested display color format: {:?}. Supported formats on this adapter are: {:?}",
                display_config.surface_color_depth, swapchain_formats
            ));

        log::debug!("Selected swapchain format: {:?}", swapchain_format);

        // create surface configuration for wgpu
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: swapchain_capabilities.alpha_modes[0],
            view_formats: vec![swapchain_format],
            desired_maximum_frame_latency: 1,
        };

        log::debug!("Surface configuration: {:?}", config);

        // configure the surface
        surface.configure(device, &config);

        // set fullscreen mode
        let mon_handle = window_options.monitor().unwrap().handle();
        let mon_name = mon_handle.name().unwrap_or("Unnamed monitor".to_string().into());

        // Borderless fullscreen is preferred, as it allows for better compatibility with different display setups
        winit_window.set_fullscreen(Some(winit::monitor::Fullscreen::Borderless(Some(mon_handle.clone()))));

        // on macOS, we request the device's native color space
        #[cfg(target_os = "macos")]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};

            let view = match winit_window.window_handle().unwrap().as_raw() {
                RawWindowHandle::AppKit(handle) => {
                    assert!(objc2::MainThreadMarker::new().is_some());
                    unsafe { handle.ns_view.cast::<objc2_app_kit::NSView>().as_ref() }
                }
                _ => panic!("Not running on macOS!"),
            };

            unsafe {
                view.window()
                    .unwrap()
                    .setColorSpace(Some(&objc2_app_kit::NSColorSpace::deviceRGBColorSpace()));
            }
        }

        // chose an internal color format based on internal_color_depth
        let internal_color_format = match experiment_config.internal_color_type {
            ColorType::EightBit => ColorFormat::Rgba8,
            ColorType::TenBit => ColorFormat::Rgba10,
            ColorType::SixteenBitFloat => ColorFormat::RgbaF16,
            ColorType::ThirtyTwoBitFloat => panic!("32F color format not supported in renderer"),
        };

        // create a dymmy (identity) LUT for now, we will replace this with a real LUT later
        // LUT structure is (&[f32, 16 384], &[f32, 16 384], &[f32, 16 384]])
        let identity_lut = (
            (0..16_384).rev().map(|i| i as f32 / 16_383.0).collect::<Vec<_>>(),
            (0..16_384).rev().map(|i| i as f32 / 16_383.0).collect::<Vec<_>>(),
            (0..16_384).rev().map(|i| i as f32 / 16_383.0).collect::<Vec<_>>(),
        );

        let wgpu_renderer = pollster::block_on(crate::visual::renderer::wgpu_renderer::WgpuRenderer::new(
            winit_window.clone(),
            instance,
            device,
            queue,
            swapchain_format,
            internal_color_format,
            Some((
                identity_lut.0.as_slice(),
                identity_lut.1.as_slice(),
                identity_lut.2.as_slice(),
            )),
        ));

        // create the skia renderer
        let mut renderer = crate::visual::renderer::Renderer::new(
            adapter,
            device,
            queue,
            ColorEncoding::Linear,
            internal_color_format,
        );

        let winit_id = winit_window.id();

        // set width of the screen to 30 cm
        let width_mm = 300.0;
        let viewing_distance = 1000.0;

        // create a pwindow
        let window_state = WindowState {
            winit_window: winit_window.clone(),
            surface,
            config,
            wgpu_renderer,
            renderer: self.renderer.clone(),
            display_characteristics: display_config.display_characteristics.clone(),
            mouse_cursor_visible: true,
            mouse_position: None,
            size: size.into(),
            physical_screen: PhysicalScreen::new(size.width, width_mm, viewing_distance),
            event_handlers: HashMap::new(), // TODO this should be a weak reference
            bg_color: Color::new_srgba(0.5, 0.5, 0.5, 1.0),
            frame_callbacks: HashMap::new(),
            frame_queue: Vec::new(),
            last_frame_id: 0,
            current_frame: None,
        };

        // create channel for physical input
        let (mut event_broadcast_sender, physical_input_receiver) = async_broadcast::broadcast(10_000);
        event_broadcast_sender.set_overflow(true);
        // deactivate the receiver
        let event_broadcast_receiver = physical_input_receiver.deactivate();

        let vblank_times = Arc::new(Mutex::new(VecDeque::with_capacity(100_000_000)));
        let flip_count_presented_at = Arc::new(AtomicU32::new(0));
        let created_at_time = std::time::Instant::now();

        #[cfg(all(feature = "dx12", target_os = "windows"))]
        let win32_scanline_samples = Arc::new(Mutex::new((
            VecDeque::with_capacity(1000),
            VecDeque::with_capacity(1000),
        )));

        #[cfg(all(feature = "dx12", target_os = "windows"))]
        {
            use crate::visual::utils::win::get_adapter_and_vidpn_source;

            let swap_chain = unsafe {
                window_state
                    .surface
                    .as_hal::<wgpu::hal::api::Dx12>()
                    .unwrap()
                    .swap_chain()
                    .unwrap()
            };

            let waitable_handle = unsafe {
                window_state
                    .surface
                    .as_hal::<wgpu::hal::api::Dx12>()
                    .unwrap()
                    .waitable_handle()
                    .unwrap()
            };

            // this is waiting for the frame latency waitable object to be signaled
            unsafe { windows::Win32::System::Threading::WaitForSingleObject(waitable_handle, 10000) };

            let win32_scanline_samples_clone = win32_scanline_samples.clone();
            let vblank_times_clone = vblank_times.clone();
            let flip_count_presented_at = flip_count_presented_at.clone();

            let (adapter_handle, pid) = get_adapter_and_vidpn_source(&swap_chain).unwrap();

            thread::spawn(move || {
                // Vector to store scan line data points

                use crate::visual::utils::win::HighPrecisionTimer;

                // use timeBeginPeriod(1) to set the timer resolution to 1ms
                unsafe { windows::Win32::Media::timeBeginPeriod(1) };

                // Create a high precision timer with 0.5 ms interval
                let timer = HighPrecisionTimer::new(1).expect("Failed to create high precision timer");

                let mut i = 0;

                loop {
                    use windows::Wdk::Graphics::Direct3D::D3DKMT_GETSCANLINE;

                    use crate::utils;
                    let mut scan_line_info = D3DKMT_GETSCANLINE {
                        hAdapter: adapter_handle,
                        VidPnSourceId: 1, // Primary display
                        InVerticalBlank: false.into(),
                        ScanLine: 0,
                    };

                    // Call D3DKMTGetScanLine);
                    unsafe {
                        use windows::Wdk::Graphics::Direct3D::D3DKMTGetScanLine;

                        D3DKMTGetScanLine(&mut scan_line_info)
                            .ok()
                            .expect("Failed to call D3DKMTGetScanLine");

                        // if we're in vertical blank, we skip this frame
                        if scan_line_info.InVerticalBlank == false {
                            // push the scan line data into the vector
                            use std::time::Instant;
                            let mut win32_scanline_samples = win32_scanline_samples_clone.lock().unwrap();
                            win32_scanline_samples.0.push_back(Instant::now());
                            win32_scanline_samples.1.push_back(scan_line_info.ScanLine);

                            // remove old scan lines and times
                            if win32_scanline_samples.0.len() >= 1000 {
                                win32_scanline_samples.0.pop_front();
                                win32_scanline_samples.1.pop_front();
                            }

                            if i % 500 == 0 {
                                // find vblanks
                                // we run our vblank finding algorithm here, which will yield a list of timestamps where the vblanks occured
                                // we can then use this to estimate the refresh rate and figure out when the last flip occurred

                                use crate::visual::utils::find_zero_crossings;
                                let xs = win32_scanline_samples
                                    .0
                                    .iter()
                                    .map(|x| x.duration_since(created_at_time.clone()).as_secs_f64() * 1000.0)
                                    .collect::<Vec<_>>();
                                let ys = win32_scanline_samples.1.iter().map(|y| *y as f64).collect::<Vec<_>>();
                                let vblanks = find_zero_crossings(&xs, &ys);

                                // we add the vblank times to the vblank_times vector
                                // hoowever, we only add the vblank times if they are not already in the vector
                                // to do this efficiently, we take the first NEW vblank time and check where it is in the vector
                                // we then remove all vblank times that are before this time and add the new vblank time to the vector
                                let mut vblank_times = vblank_times_clone.lock().unwrap();
                                if let Some(first_new_vblank) = vblanks.first() {
                                    // find the index of the first new vblank time
                                    let index = vblank_times.iter().position(|x| *x >= *first_new_vblank);
                                    if let Some(index) = index {
                                        // remove all vblank times before this index
                                        vblank_times.drain(0..index);
                                    }
                                    // add the new vblank time to the vector
                                    vblank_times.push_back(*first_new_vblank);
                                }
                            }
                            i += 1;
                        }
                    }

                    timer.wait().expect("Failed to wait on timer");
                }
            });
        }

        // create handle
        let window = Window {
            winit_id,
            state: Arc::new(Mutex::new(Some(window_state))),
            gpu_state: self.gpu_state.clone(),
            event_broadcast_sender,
            event_broadcast_receiver,
            config: Arc::new(Mutex::new(ExperimentConfig::default())),
            created_at: created_at_time,
            #[cfg(all(feature = "dx12", target_os = "windows"))]
            win32_scanline_samples,
            vblank_times: vblank_times.clone(),
            estimated_refresh_rate: Arc::new(AtomicF64::new(0.0)),
            last_vblank_time_presented_at: Arc::new(AtomicF64::new(0.0)),
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
    pub fn run_experiment<F>(self, experiment_fn: F, config: ExperimentConfig) -> Result<(), errors::PsydkError>
    where
        F: FnOnce(ExperimentContext) -> Result<(), errors::PsydkError> + 'static + Send,
    {
        log::debug!("Main task is running on thread {:?}", std::thread::current().id());

        // for most platforms, we can just create the event loop and run it
        #[cfg(not(target_os = "ios"))]
        let event_loop = EventLoop::new().unwrap();
        // on iOS, we need to use the iOS-specific event loop
        #[cfg(target_os = "ios")]
        let event_loop = {
            unsafe {
                EventLoop::builder()
                    .with_main_thread_marker(objc2::MainThreadMarker::new_unchecked())
                    .build()
                    .unwrap()
            }
        };

        let event_loop_proxy = event_loop.create_proxy();
        let event_loop_proxy2 = event_loop.create_proxy();

        let action_sender = self.action_sender.clone();

        let audio_host = psydk_audio::cpal::default_host().into();

        let exp_manager = ExperimentContext::new(
            self.gpu_state.clone(),
            event_loop_proxy,
            action_sender.clone(),
            self.renderer.clone(),
            audio_host,
            self.font_manager.clone(),
            config,
        );

        // create mutex to hold potential error
        let error_mutex = Arc::new(Mutex::new(None));
        let error_mutex_clone = error_mutex.clone();

        // start experiment
        thread::spawn(move || {
            log::debug!("Experiment thread started on {:?}", std::thread::current().id());
            let res = experiment_fn(exp_manager);

            // send Exit event to the event loop, then wake it up
            action_sender.send(EventLoopAction::Exit(None)).unwrap();
            event_loop_proxy2.wake_up();

            // panic if the experiment function returns an error
            if let Err(e) = res {
                // put the error in the mutex
                let mut error = error_mutex_clone.lock().unwrap();
                *error = Some(e);
            }
        });

        #[cfg(target_os = "ios")]
        {
            // put self in a Box and leak it to get a 'static reference
            let slf = Box::new(self);
            let slf: &'static mut Experiment = Box::leak(slf);

            // start event loop
            let _ = event_loop.run_app(slf);
        }
        #[cfg(not(target_os = "ios"))]
        {
            // start event loop
            let _ = event_loop.run_app(self);
        }

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

impl ApplicationHandler for Experiment {
    // fn war_what_is_it_good_for(&self) {}
    fn resumed(&mut self, event_loop: &dyn winit::event_loop::ActiveEventLoop) {}

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        log::debug!("Proxy wake up called");
        // check if we need to create a new window
        self.action_receiver.try_recv().map(|action| match action {
            EventLoopAction::CreateNewWindow(options, experiment_config, display_config, sender) => {
                let window = self.create_window(&options, display_config, &experiment_config, event_loop);
                self.windows.push(window.clone());
                sender.send(window).unwrap();
            }
            EventLoopAction::GetAvailableMonitors(sender) => {
                let monitors = event_loop.available_monitors();

                // convert into a vector of monitors
                let monitors: Vec<Monitor> = monitors
                    .map(|monitor| {
                        Monitor::new(
                            monitor.name().unwrap_or("Unnamed monitor".to_string().into()).into(),
                            (0, 0),
                            monitor,
                        )
                    })
                    .collect();
                sender.send(monitors).unwrap();
            }
            EventLoopAction::RunInEventLoop(task) => {
                // run the task in the event loop
                task();
            }
            EventLoopAction::Exit(..) => {
                event_loop.exit();
            }
        });
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
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
            WindowEvent::SurfaceResized(size) => {
                // find the window
                let window = self.windows.iter().find(|w| w.winit_id == window_id);

                if let Some(window) = window {
                    // update the window size
                    window.resize(size);
                }
            }
            WindowEvent::KeyboardInput { .. }
            | WindowEvent::PointerMoved { .. }
            | WindowEvent::PointerEntered { .. }
            | WindowEvent::PointerLeft { .. }
            | WindowEvent::PointerButton { .. }
            | WindowEvent::MouseWheel { .. } => {
                // find the window
                let window = self.windows.iter().find(|w| w.winit_id == window_id);

                // if this was a cursor moved event, update the mouse position
                if let WindowEvent::PointerMoved { position, .. } = event {
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

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {}
}
