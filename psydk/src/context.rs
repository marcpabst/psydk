use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};

use sysinfo::System;

use crate::{
    app::{App, ArcMutex, GPUState},
    audio::{PyDevice, PyHost, PyStream},
    config::ExperimentConfig,
    errors::{self, PsydkError, PsydkResult},
    git::PyRepository,
    visual::window::Window,
};
use derive_debug::Dbg;
use pyo3::types::{PyModule, PyString};
use pyo3::{
    pyclass, pyfunction, pymethods,
    types::{PyAnyMethods, PyDict, PyList, PyListMethods, PySequenceMethods, PyTuple, PyTupleMethods},
    IntoPy, Py, PyAny, PyResult, Python,
};
use renderer::{cosmic_text, renderer::SharedRendererState};
use winit::event_loop::EventLoopProxy;

pub enum EventLoopAction {
    CreateNewWindow(WindowOptions, ExperimentConfig, GammaOptions, Sender<Window>),
    GetAvailableMonitors(Sender<Vec<Monitor>>),
    RunInEventLoop(Box<dyn FnOnce() + Send>),
    Exit(Option<errors::PsydkError>),
}

#[pyclass]
pub struct PyRendererFactory(pub Box<dyn SharedRendererState>);

// impl Clone for PyRendererFactory
impl Clone for PyRendererFactory {
    fn clone(&self) -> Self {
        Self(self.0.cloned())
    }
}

// deref for PyRendererFactory
impl std::ops::Deref for PyRendererFactory {
    type Target = dyn SharedRendererState;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl PyRendererFactory {
    pub fn inner(&self) -> &dyn SharedRendererState {
        self.0.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[pyclass]
pub struct Monitor {
    #[pyo3(get)]
    pub name: String,
    pub resolution: (u32, u32),
    pub handle: winit::monitor::MonitorHandle,
}

impl Monitor {
    pub fn new(name: String, resolution: (u32, u32), handle: winit::monitor::MonitorHandle) -> Self {
        Self {
            name,
            resolution,
            handle,
        }
    }

    pub fn handle(&self) -> &winit::monitor::MonitorHandle {
        &self.handle
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn refresh_rate(&self) -> Option<f64> {
        self.handle.refresh_rate_millihertz().map(|r| r as f64 / 1000.0)
    }
}

#[pymethods]
impl Monitor {
    #[getter]
    #[pyo3(name = "refresh_rate")]
    // Refresh rate in Hz
    fn py_refresh_rate(&self) -> PyResult<f64> {
        self.refresh_rate()
            .map(|r| r as f64)
            .ok_or_else(|| PsydkError::MonitorError("Monitor does not have a refresh rate".to_string()).into())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GammaOptions {
    pub encode_gamma: bool,
    pub lut: Option<renderer::image::RgbImage>,
}

/// Options for creating a window. The ExperimentManager will try to find a
/// video mode that satisfies the provided constraints. See documentation of the
/// variants for more information.
#[derive(Debug, Clone, PartialEq)]
#[pyclass]
pub enum WindowOptions {
    Windowed {
        /// The width and height of the window in pixels. Defaults to 800x600
        /// (px).
        resolution: Option<(u32, u32)>,
    },
    /// Match the given constraints exactly. You can set any of the constraints
    /// to `None` to use the default value.
    FullscreenExact {
        /// The monitor to use. Defaults to the primary monitor.
        monitor: Option<Monitor>,
        /// The width and height of the window in pixels. Defaults to the width
        /// of the first supported video mode of the selected monitor.
        resolution: Option<(u32, u32)>,
        /// The refresh rate to use in Hz. Defaults to the refresh rate of the
        /// first supported video mode of the selected monitor.
        refresh_rate: Option<f64>,
    },
    /// Select window configuration that satisfies the given constraints and has
    /// the highest refresh rate.
    FullscreenHighestRefreshRate {
        monitor: Option<Monitor>,
        resolution: Option<(u32, u32)>,
    },
    /// Select the highest resolution that satisfies the given constraints and
    /// has the highest resolution.
    FullscreenHighestResolution {
        monitor: Option<Monitor>,
        refresh_rate: Option<f64>,
    },
}

impl WindowOptions {
    pub fn monitor(&self) -> Option<&Monitor> {
        match self {
            WindowOptions::Windowed { .. } => None,
            WindowOptions::FullscreenExact { monitor, .. } => monitor.as_ref(),
            WindowOptions::FullscreenHighestRefreshRate { monitor, .. } => monitor.as_ref(),
            WindowOptions::FullscreenHighestResolution { monitor, .. } => monitor.as_ref(),
        }
    }
}

/// The ExperimentManager is available to the user in the experiment function.
#[derive(Clone)]
#[pyclass]
pub struct ExperimentContext {
    pub gpu_state: ArcMutex<GPUState>,
    event_loop_proxy: EventLoopProxy<()>,
    action_sender: Sender<EventLoopAction>,
    renderer_factory: Arc<dyn SharedRendererState>,
    audio_host: Arc<timed_audio::cpal::Host>,
    font_manager: Arc<Mutex<cosmic_text::FontSystem>>,
    config: Arc<Mutex<crate::config::ExperimentConfig>>,
}

impl ExperimentContext {
    pub fn new(
        gpu_state: ArcMutex<GPUState>,
        event_loop_proxy: EventLoopProxy<()>,
        action_sender: Sender<EventLoopAction>,
        renderer_factory: Arc<dyn SharedRendererState>,
        audio_host: Arc<timed_audio::cpal::Host>,
        font_manager: Arc<Mutex<cosmic_text::FontSystem>>,
    ) -> Self {
        Self {
            gpu_state,
            event_loop_proxy,
            action_sender,
            renderer_factory,
            audio_host,
            font_manager,
            config: Arc::new(Mutex::new(crate::config::ExperimentConfig::default())),
        }
    }

    // pub fn exit(&self) {
    //     // send exit action
    //     self.action_sender.send(EventLoopAction::Exit(None)).unwrap();
    //     // wake up the event loop
    //     self.event_loop_proxy.send_event(()).unwrap();
    // }

    pub fn font_manager(&self) -> &Arc<Mutex<cosmic_text::FontSystem>> {
        &self.font_manager
    }

    pub fn load_system_fonts(&self) {
        let mut font_manager = self.font_manager.lock().unwrap();
        font_manager.db_mut().load_system_fonts();
    }

    pub fn load_font_file(&self, path: &str) -> Result<(), errors::PsydkError> {
        let mut font_manager = self.font_manager.lock().unwrap();
        font_manager.db_mut().load_font_file(path)?;
        Ok(())
    }

    pub fn load_font_directory(&self, path: &str) -> Result<(), errors::PsydkError> {
        println!("Loading font directory: {}", path);
        let mut font_manager = self.font_manager.lock().unwrap();
        font_manager.db_mut().load_fonts_dir(path);
        Ok(())
    }

    pub fn renderer_factory(&self) -> &Arc<dyn SharedRendererState> {
        &self.renderer_factory
    }

    /// Run some code in the event loop thread.
    pub fn run_in_event_loop<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        // create a channel to send the result back
        let (sender, receiver) = channel();
        // send the action to the event loop
        self.action_sender
            .send(EventLoopAction::RunInEventLoop(Box::new(move || {
                let result = f();
                sender.send(result).unwrap();
            })))
            .unwrap();
        // wake up the event loop
        self.event_loop_proxy.send_event(()).unwrap();
        // wait for the result
        receiver.recv().unwrap()
    }

    /// Get a writable directory. This is platform-specific and will return a
    /// directory that is suitable for writing files. On dekstop platforms, this is just
    /// the current working directory. On mobile platforms, this is the app's
    /// sandboxed writable directory.
    pub fn writable_directory(&self) -> String {
        #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
        {
            std::env::current_dir().unwrap().to_str().unwrap().to_string()
        }
        #[cfg(target_os = "ios")]
        {
            use objc2::rc::Id;
            use objc2::{msg_send, ClassType};
            use objc2_foundation::{
                NSArray, NSFileManager, NSSearchPathDirectory, NSSearchPathDomainMask, NSString, NSURL,
            };

            let path = unsafe {
                let file_manager = NSFileManager::defaultManager();
                let paths = file_manager.URLsForDirectory_inDomains(
                    NSSearchPathDirectory::DocumentDirectory,
                    NSSearchPathDomainMask::UserDomainMask,
                );
                let url = paths.firstObject().unwrap();
                url.path().unwrap_or_default()
            };

            path.to_string()
        }
        #[cfg(target_os = "android")]
        {
            panic!("Android writable directory is not implemented yet");
        }
    }

    /// Show an alert dialog with the given message.
    ///
    pub fn show_alert(&self, message: &str) {
        #[cfg(target_os = "macos")]
        {
            // create a channel to send the result back
            let (sender, receiver) = channel();
            let message = message.to_string();
            // the closure to run in the event loop (macos)
            #[cfg(target_os = "macos")]
            let closure = move || {
                use objc2::MainThreadMarker;
                #[cfg(target_os = "macos")]
                use objc2_app_kit::NSAlert;
                use objc2_foundation::{ns_string, NSString};
                let main_thread = MainThreadMarker::new().unwrap();
                let alert = unsafe { NSAlert::new(main_thread) };
                unsafe { alert.setMessageText(&*NSString::from_str(&message)) };
                unsafe { alert.runModal() };

                sender.send(()).unwrap();
            };

            // send the action to the event loop
            self.action_sender
                .send(EventLoopAction::RunInEventLoop(Box::new(closure)))
                .unwrap();
            // wake up the event loop
            self.event_loop_proxy.send_event(()).unwrap();

            // wait for the result
            receiver.recv().unwrap();
        }
        #[cfg(target_os = "ios")]
        todo!();
    }

    /// Show an alert dialog with the given message.
    pub fn show_prompt(
        &self,
        title: String,
        subtitle: Option<String>,
        show_text_input: bool,
        text_input_placeholder: Option<String>,
        text_input_value: Option<String>,
        show_cancel_button: bool,
    ) -> String {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            // create a channel to send the result back
            let (sender, receiver) = channel();
            // the closure to run in the event loop (macos or ios)
            #[cfg(target_os = "macos")]
            let closure = move || {
                use objc2::MainThreadMarker;
                use objc2::MainThreadOnly;
                use objc2_app_kit::{NSAlert, NSImage, NSTextField};
                use objc2_core_foundation::{CGPoint, CGSize};
                use objc2_foundation::{ns_string, NSRect, NSString};
                let main_thread = MainThreadMarker::new().unwrap();
                let alert = unsafe { NSAlert::new(main_thread) };
                unsafe {
                    use objc2::AnyThread;

                    let icon = NSImage::alloc();
                    let icon = NSImage::initWithSize(icon, CGSize::new(1.0, 1.0));
                    alert.setIcon(Some(&*icon));
                }
                unsafe { alert.setMessageText(&*NSString::from_str(&title)) };
                unsafe { alert.addButtonWithTitle(&*NSString::from_str("OK")) };

                let text_field = if show_text_input {
                    unsafe {
                        let frame = NSRect::new(CGPoint::new(0.0, 0.0), CGSize::new(300.0, 24.0));
                        let text_field = NSTextField::alloc(main_thread);
                        let text_field = NSTextField::initWithFrame(text_field, frame);

                        if let Some(subtitle) = subtitle {
                            alert.setInformativeText(&*NSString::from_str(&subtitle));
                        }

                        if let Some(placeholder) = text_input_placeholder {
                            text_field.setPlaceholderString(Some(&*NSString::from_str(&placeholder)));
                        }

                        if let Some(value) = text_input_value {
                            text_field.setStringValue(&*NSString::from_str(&value));
                        }

                        // text_field.setStringValue(&*NSString::from_str(""));
                        alert.setAccessoryView(Some(&*text_field));

                        Some(text_field)
                    }
                } else {
                    None
                };

                if show_cancel_button {
                    unsafe { alert.addButtonWithTitle(&*NSString::from_str("Cancel")) };
                }

                let response = unsafe { alert.runModal() };

                // get the text input if available
                let text_input_value = if let Some(text_field) = text_field {
                    unsafe { text_field.stringValue().to_string() }
                } else {
                    String::new()
                };

                if response == 1000 {
                    // NSAlertFirstButtonReturn
                    sender.send((text_input_value, true)).unwrap();
                } else {
                    // NSAlertSecondButtonReturn or NSAlertThirdButtonReturn
                    sender.send((text_input_value, false)).unwrap();
                }
            };
            #[cfg(target_os = "ios")]
            let closure = move || {
                use objc2::rc::Retained;
                use objc2::MainThreadMarker;
                use objc2_foundation::NSString;
                use objc2_ui_kit::{
                    UIAlertAction, UIAlertActionStyle, UIAlertController, UIAlertControllerStyle, UIApplication,
                    UITextField,
                };
                use std::ptr::NonNull;

                let main_thread = MainThreadMarker::new().unwrap();

                let subtitle = subtitle.unwrap_or_else(|| "".to_string());

                // Create alert controller
                let alert = unsafe {
                    UIAlertController::alertControllerWithTitle_message_preferredStyle(
                        Some(&*NSString::from_str(&title)),
                        Some(&*NSString::from_str(&subtitle)),
                        UIAlertControllerStyle::Alert,
                        main_thread,
                    )
                };

                // Store text field reference
                let text_field_ref: Option<Retained<UITextField>> = if show_text_input {
                    unsafe {
                        alert.addTextFieldWithConfigurationHandler(Some(&block2::ConcreteBlock::new(
                            move |text_field_ptr: NonNull<UITextField>| {
                                let text_field = text_field_ptr.as_ref();

                                if let Some(placeholder) = &text_input_placeholder {
                                    text_field.setPlaceholder(Some(&*NSString::from_str(placeholder)));
                                }

                                if let Some(value) = &text_input_value {
                                    text_field.setText(Some(&*NSString::from_str(value)));
                                }
                            },
                        )));

                        // Get reference to the text field we just added
                        alert
                            .textFields()
                            .map(|fields| fields.firstObject().map(|tf| tf))
                            .flatten()
                    }
                } else {
                    None
                };

                // Add OK button
                let sender_ok = sender.clone();
                let text_field_ok = text_field_ref.clone();
                unsafe {
                    let ok_action = UIAlertAction::actionWithTitle_style_handler(
                        Some(&*NSString::from_str("OK")),
                        UIAlertActionStyle::Default,
                        Some(&block2::ConcreteBlock::new(move |_action: NonNull<UIAlertAction>| {
                            let text_value = if let Some(ref tf) = text_field_ok {
                                tf.text().map(|s| s.to_string()).unwrap_or_default()
                            } else {
                                String::new()
                            };
                            sender_ok.send((text_value, true)).unwrap();
                        })),
                        main_thread,
                    );
                    alert.addAction(&ok_action);
                }

                // Add Cancel button if requested
                if show_cancel_button {
                    let sender_cancel = sender.clone();
                    let text_field_cancel = text_field_ref.clone();
                    unsafe {
                        let cancel_action = UIAlertAction::actionWithTitle_style_handler(
                            Some(&*NSString::from_str("Cancel")),
                            UIAlertActionStyle::Cancel,
                            Some(&block2::ConcreteBlock::new(move |_action: NonNull<UIAlertAction>| {
                                let text_value = if let Some(ref tf) = text_field_cancel {
                                    tf.text().map(|s| s.to_string()).unwrap_or_default()
                                } else {
                                    String::new()
                                };
                                sender_cancel.send((text_value, false)).unwrap();
                            })),
                            main_thread,
                        );
                        alert.addAction(&cancel_action);
                    }
                }

                // Present the alert
                // Note: You'll need to get the root view controller to present from
                // This is a simplified example - in practice you'd need the actual view controller
                unsafe {
                    if let Some(window) = UIApplication::sharedApplication(main_thread).keyWindow() {
                        if let Some(root_vc) = window.rootViewController() {
                            root_vc.presentViewController_animated_completion(&alert, true, None);
                        }
                    }
                }
            };

            // send the action to the event loop
            self.action_sender
                .send(EventLoopAction::RunInEventLoop(Box::new(closure)))
                .unwrap();
            // wake up the event loop
            self.event_loop_proxy.send_event(()).unwrap();

            // wait for the result
            let (text_input_value, confirmed) = receiver.recv().unwrap();
            if confirmed {
                text_input_value
            } else {
                // if the user cancelled, return an empty string
                String::new()
            }
        }
        return String::new(); // fallback for unsupported platforms
    }

    /// Create a new window with the given options. This function will dispatch
    /// a new UserEvent to the event loop and wait until the winit window
    /// has been created. Then it will setup the wgpu device and surface and
    /// return a new Window object.
    pub fn create_window(
        &self,
        window_options: &WindowOptions,
        experiment_config: ExperimentConfig,
        gamma_options: GammaOptions,
    ) -> Window {
        // set up window by dispatching a new CreateNewWindow action
        let (sender, receiver) = channel();
        let action = EventLoopAction::CreateNewWindow(window_options.clone(), experiment_config, gamma_options, sender);

        // send action
        self.action_sender.send(action).unwrap();
        self.event_loop_proxy.send_event(());

        // wait for response
        let mut window = receiver.recv().expect("Failed to create window");

        // set the config (this could be done in the event loop, should we need it there)
        window.config = self.config.clone();
        log::debug!("New window successfully created");

        window
    }

    /// Create a new window. This is a convenience function that creates a
    /// window with the default options.
    pub fn create_default_window(&self, fullscreen: bool, monitor: Option<u32>, gamma: Option<GammaOptions>) -> Window {
        // select monitor 1 if available
        // find all monitors available

        let monitors = self.get_available_monitors();
        // get the second monitor if available, otherwise use the first one
        let monitor = monitors
            .get(monitor.unwrap_or(0) as usize)
            .unwrap_or(monitors.first().expect("No monitor found - this should not happen"));

        let gamma_options = gamma.unwrap_or_else(|| GammaOptions {
            encode_gamma: true,
            lut: None,
        });

        self.create_window(
            &WindowOptions::FullscreenHighestResolution {
                monitor: Some(monitor.clone()),
                refresh_rate: None,
            },
            self.config.lock().unwrap().clone(),
            gamma_options,
        )
    }

    /// Retrive available monitors.
    pub fn get_available_monitors(&self) -> Vec<Monitor> {
        let (sender, receiver) = channel();
        self.action_sender
            .send(EventLoopAction::GetAvailableMonitors(sender.clone()))
            .unwrap();

        // wake up the event loop
        self.event_loop_proxy.send_event(());

        receiver.recv().unwrap()
    }

    pub fn get_repository(&self) -> PsydkResult<Option<gix::Repository>> {
        // get the current directory
        let mut current_dir = std::env::current_dir().map_err(|e| errors::PsydkError::IOError(e))?;
        // try to open the repository, otherwise traverse the directory tree
        while current_dir.parent().is_some() {
            let repo = gix::open(current_dir.clone()).ok();
            if let Some(repo) = repo {
                return Ok(Some(repo));
            }
            current_dir.pop();
        }
        Ok(None)
    }

    pub fn system_info(&self) -> HashMap<String, String> {
        let mut info = HashMap::new();
        info.insert("os_name".to_string(), System::name().unwrap_or("unknown".to_string()));
        info.insert(
            "os_version".to_string(),
            System::os_version().unwrap_or("unknown".to_string()),
        );
        info.insert(
            "os_kernel_version".to_string(),
            System::kernel_version().unwrap_or("unknown".to_string()),
        );
        info.insert(
            "cpu_architecture".to_string(),
            System::cpu_arch().unwrap_or("unknown".to_string()),
        );
        info
    }
}

#[pymethods]
impl ExperimentContext {
    #[pyo3(name = "create_default_window")]
    #[pyo3(signature = (fullscreen = false, monitor = None, encode_gamma=true, lut_img_path = None))]
    /// Create a new window. This is a convenience function that creates a
    /// window with the default options.
    ///
    /// Even when `fullscreen` is set to `True`, no video mode changes will be
    /// initiated. The window will be created with the highest resolution
    /// changes. When `fullscreen` is set to `true`,
    /// `monitor` can be used to select the monitor to use. Monitor enumeration
    /// is OS-specific and the primary monitor may not always be at index 0.
    ///
    /// Parameters
    /// ----------
    /// fullscreen : bool, optional
    ///   Whether to create a fullscreen window. Defaults to `false`.
    /// monitor : int, optional
    ///   The index of the monitor to use. Defaults to 0.
    ///
    /// Returns
    /// -------
    /// Window
    ///  The new window.
    fn py_create_default_window(
        &self,
        fullscreen: bool,
        monitor: Option<u32>,
        encode_gamma: bool,
        lut_img_path: Option<String>,
    ) -> Window {
        let gamma_options = if let Some(path) = lut_img_path {
            let img = renderer::image::io::Reader::open(path)
                .unwrap()
                .decode()
                .unwrap()
                .into_rgb8();
            GammaOptions {
                encode_gamma,
                lut: Some(img),
            }
        } else {
            GammaOptions {
                encode_gamma: encode_gamma,
                lut: None,
            }
        };

        self.create_default_window(fullscreen, monitor, Some(gamma_options))
    }

    // Create a new audio stream
    #[pyo3(name = "create_audio_stream")]
    #[pyo3(signature = (sampling_rate=None, device=None))]
    fn py_create_audio_stream(&self, sampling_rate: Option<u32>, device: Option<&PyDevice>) -> PyStream {
        PyStream::new(&self.audio_host, sampling_rate, device)
    }

    #[pyo3(name = "get_available_monitors")]
    fn py_get_available_monitors(&self) -> Vec<Monitor> {
        self.get_available_monitors()
    }

    #[pyo3(name = "get_repository")]
    fn py_get_repository(&self) -> PsydkResult<Option<PyRepository>> {
        self.get_repository().map(|r| r.map(|r| r.into()))
    }

    #[pyo3(name = "system_info")]
    fn py_system_info(&self) -> PyResult<HashMap<String, String>> {
        Ok(self.system_info())
    }

    #[pyo3(name = "load_system_fonts")]
    fn py_load_system_fonts(&self) -> PyResult<()> {
        self.load_system_fonts();
        Ok(())
    }

    #[pyo3(name = "load_font_file")]
    fn py_load_font_file(&self, path: &str) -> PyResult<()> {
        self.load_font_file(path)?;
        Ok(())
    }

    #[pyo3(name = "load_font_directory")]
    fn py_load_font_directory(&self, path: &str) -> PyResult<()> {
        self.load_font_directory(path)?;
        Ok(())
    }

    #[pyo3(name = "get_writable_directory")]
    /// Get a writable directory. This is platform-specific and will return a
    /// directory that is suitable for writing files. On desktop platforms, this is just
    // the current working directory. On mobile platforms, this is the app's
    /// sandboxed writable directory.
    fn py_get_writable_directory(&self) -> String {
        self.writable_directory()
    }

    #[pyo3(name = "show_alert")]
    fn py_show_alert(&self, message: &str, py: Python) -> PyResult<()> {
        self.show_alert(message);
        Ok(())
    }

    #[pyo3(name = "show_prompt")]
    #[pyo3(signature = (title,
        subtitle=None,
        show_text_input=true,
        text_input_placeholder=None,
        text_input_value=None,
        show_cancel_button=false))]
    fn py_show_prompt(
        &self,
        title: String,
        subtitle: Option<String>,
        show_text_input: bool,
        text_input_placeholder: Option<String>,
        text_input_value: Option<String>,
        show_cancel_button: bool,
        py: Python,
    ) -> PyResult<String> {
        Ok(self.show_prompt(
            title,
            subtitle,
            show_text_input,
            text_input_placeholder,
            text_input_value,
            show_cancel_button,
        ))
    }
}

/// Runs your experiment function. This function will block the current thread
/// until the experiment function returns!
///
/// Parameters
/// ----------
/// experiment_fn : callable
///    The function that runs your experiment. This function should take a single argument, an instance of `ExperimentManager`, and should not return nothing.
#[pyfunction]
#[pyo3(name = "run_experiment", signature = (py_experiment_fn, options=None, *args, **kwargs))]
pub fn py_run_experiment(
    py: Python,
    py_experiment_fn: Py<PyAny>,
    options: Option<ExperimentConfig>,
    args: Py<PyTuple>,
    kwargs: Option<Py<PyDict>>,
) -> PyResult<()> {
    // create app
    let mut app = App::new(options.unwrap_or_default());

    // set the __globals__ to make "_renderer_factory" available
    // this will allow functions to create renderer-specific objects
    // without having to pass the renderer object around

    let globals = PyDict::new(py);
    let renderer_factory = PyRendererFactory(app.shared_renderer_state.cloned());

    let rust_experiment_fn = move |em: ExperimentContext| -> Result<(), errors::PsydkError> {
        Python::with_gil(|py| -> _ {
            // bind kwargs
            let kwargs = if let Some(kwargs) = kwargs {
                kwargs.into_bound(py)
            } else {
                PyDict::new(py)
            };

            py_experiment_fn
                .getattr(py, "__globals__")?
                .bind(py)
                .downcast::<PyDict>()?
                .set_item("_experiment_context", em.clone())?;

            // TODO: There must be a better way to do this!
            let args = args.bind(py);
            let args_as_seq = args.to_list();
            let args_as_seq = args_as_seq.as_sequence();
            let em = em.into_py(py);
            let em_as_seq = PyList::new(py, vec![em])?;
            let em_as_seq = em_as_seq.as_sequence();

            let args = em_as_seq.concat(args_as_seq).unwrap();
            let args = args.to_tuple().unwrap();

            py_experiment_fn.call_bound(py, args, Some(&kwargs))
        })?;
        Ok(())
    };

    py.allow_threads(move || app.run_experiment(rust_experiment_fn))?; // run the experiment
    Ok(())
}
