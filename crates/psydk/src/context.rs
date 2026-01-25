use crate::{
    audio::{PyDevice, PyHost, PyStream},
    config::{ExperimentConfig, WindowConfig},
    errors::{self, PsydkError, PsydkResult},
    experiment::{ArcMutex, Experiment, GPUState},
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
use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};
use sysinfo::System;
use winit::event_loop::EventLoopProxy;

pub enum EventLoopAction {
    CreateNewWindow(WindowOptions, ExperimentConfig, WindowConfig, Sender<Window>),
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

#[derive(Debug, Clone, PartialEq, Eq)]
#[pyclass]
/// A physical monitor connected to the system.
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
        self.handle.current_video_mode().and_then(|mode| {
            // Convert the refresh rate from Hz to f64
            mode.refresh_rate_millihertz().map(|r| r.get() as f64 / 1000.0)
        })
    }
}

#[pymethods]
impl Monitor {
    #[getter]
    #[pyo3(name = "refresh_rate")]
    // Return refresh rate reported by the monitor in Hz.
    fn py_refresh_rate(&self) -> PyResult<f64> {
        self.refresh_rate()
            .map(|r| r as f64)
            .ok_or_else(|| PsydkError::MonitorError("Monitor does not have a refresh rate".to_string()).into())
    }
}

#[derive(Debug, Clone, PartialEq)]
#[pyclass]
pub enum WindowOptions {
    Windowed {
        /// The width and height of the window in pixels.
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
    /// Returns the monitor associated with the window options, if any.
    pub fn monitor(&self) -> Option<&Monitor> {
        match self {
            WindowOptions::Windowed { .. } => None,
            WindowOptions::FullscreenExact { monitor, .. } => monitor.as_ref(),
            WindowOptions::FullscreenHighestRefreshRate { monitor, .. } => monitor.as_ref(),
            WindowOptions::FullscreenHighestResolution { monitor, .. } => monitor.as_ref(),
        }
    }
}

#[derive(Clone)]
#[pyclass]
/// Contains context information about the experiment, including access to
/// the GPU state, event loop proxy, renderer factory, audio host, font manager,
pub struct ExperimentContext {
    pub gpu_state: ArcMutex<GPUState>,
    event_loop_proxy: EventLoopProxy,
    action_sender: Sender<EventLoopAction>,
    renderer_factory: Arc<dyn SharedRendererState>,
    audio_host: Arc<psydk_audio::cpal::Host>,
    font_manager: Arc<Mutex<cosmic_text::FontSystem>>,
    config: Arc<Mutex<crate::config::ExperimentConfig>>,
}

impl ExperimentContext {
    pub fn new(
        gpu_state: ArcMutex<GPUState>,
        event_loop_proxy: EventLoopProxy,
        action_sender: Sender<EventLoopAction>,
        renderer_factory: Arc<dyn SharedRendererState>,
        audio_host: Arc<psydk_audio::cpal::Host>,
        font_manager: Arc<Mutex<cosmic_text::FontSystem>>,
        config: ExperimentConfig,
    ) -> Self {
        Self {
            gpu_state,
            event_loop_proxy,
            action_sender,
            renderer_factory,
            audio_host,
            font_manager,
            config: Arc::new(Mutex::new(config)),
        }
    }

    pub fn font_manager(&self) -> &Arc<Mutex<cosmic_text::FontSystem>> {
        &self.font_manager
    }

    /// Load system fonts into the font manager. Behavior is platform-specific.
    pub fn load_system_fonts(&self) {
        let mut font_manager = self.font_manager.lock().unwrap();
        font_manager.db_mut().load_system_fonts();
    }

    /// Load a font file into the font manager.
    pub fn load_font_file(&self, path: &str) -> Result<(), errors::PsydkError> {
        let mut font_manager = self.font_manager.lock().unwrap();
        font_manager.db_mut().load_font_file(path)?;
        Ok(())
    }

    /// Load all font files in a directory into the font manager.
    pub fn load_font_directory(&self, path: &str) -> Result<(), errors::PsydkError> {
        println!("Loading font directory: {}", path);
        let mut font_manager = self.font_manager.lock().unwrap();
        font_manager.db_mut().load_fonts_dir(path);
        Ok(())
    }

    pub fn set_idle_timer_disabled(&self, disabled: bool) {
        #[cfg(target_os = "ios")]
        {
            // let app = unsafe { objc2_ui_kit::UIApplication::sharedApplication() };
            // app.setIdleTimerDisabled(true);

            // this needs to be called on the main thread
            self.run_in_event_loop(move || {
                use objc2::MainThreadMarker;
                use objc2_ui_kit::UIApplication;
                let main_thread = MainThreadMarker::new().unwrap();
                let app = unsafe { UIApplication::sharedApplication(main_thread) };
                app.setIdleTimerDisabled(disabled);
            });
        }
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
        self.event_loop_proxy.wake_up();
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
            // On desktop platforms, just return the current working directory.
            std::env::current_dir().unwrap().to_str().unwrap().to_string()
        }
        #[cfg(target_os = "ios")]
        {
            // On iOS, return the app's documents directory.
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
    }

    #[cfg(target_os = "macos")]
    /// Show an alert dialog with the given message.
    pub fn show_alert(&self, message: &str) {
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
        self.event_loop_proxy.wake_up();

        // wait for the result
        receiver.recv().unwrap();
    }

    /// Show a prompt dialog with the given title and optional subtitle.
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
            self.event_loop_proxy.wake_up();

            // wait for the result
            let (text_input_value, confirmed) = receiver.recv().unwrap();
            if confirmed {
                return text_input_value;
            } else {
                // if the user cancelled, return an empty string
                return String::new();
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
        display_config: WindowConfig,
    ) -> Window {
        // set up window by dispatching a new CreateNewWindow action
        let (sender, receiver) = channel();
        let action =
            EventLoopAction::CreateNewWindow(window_options.clone(), experiment_config, display_config, sender);

        println!("Sending CreateNewWindow action to event loop");

        // send action and wake up the event loop
        self.action_sender.send(action).unwrap();
        self.event_loop_proxy.wake_up();

        println!("Waiting for window to be created");

        // wait for response
        let mut window = receiver.recv().expect("Failed to create window");

        // set the config
        window.config = self.config.clone();
        log::debug!("New window successfully created");

        window
    }

    /// Create a new window in fullscreen mode with the highest resolution
    pub fn create_default_window(
        &self,
        fullscreen: bool,
        monitor: Option<u32>,
        display_config: Option<WindowConfig>,
    ) -> Window {
        let monitors = self.get_available_monitors();
        // get the first monitor if available, otherwise use the first one
        let monitor = monitors
            .get(monitor.unwrap_or(0) as usize)
            .unwrap_or(monitors.first().expect("No monitor found - this should not happen"));

        let display_config = display_config.unwrap_or_default();
        let experiment_config = self.config.lock().unwrap().clone();

        self.create_window(
            &WindowOptions::FullscreenHighestResolution {
                monitor: Some(monitor.clone()),
                refresh_rate: None,
            },
            experiment_config,
            display_config,
        )
    }

    /// Retrive available monitors.
    pub fn get_available_monitors(&self) -> Vec<Monitor> {
        println!("Getting available monitors");
        log::debug!("Requesting available monitors from event loop");
        let (sender, receiver) = channel();
        self.action_sender
            .send(EventLoopAction::GetAvailableMonitors(sender.clone()))
            .unwrap();

        // wake up the event loop
        self.event_loop_proxy.wake_up();

        receiver.recv().unwrap()
    }

    /// Returns the git repository in the current directory or any of its parent directories.
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

    /// Returns generic system information as a dictionary.
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
    #[pyo3(signature = (fullscreen = false, monitor = None, config = None))]
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
    /// config : WindowConfig, optional
    ///  The display configuration to use. If not provided, the default configuration will be used.
    ///
    /// Returns
    /// -------
    /// Window
    ///  The new window.
    fn py_create_default_window(&self, fullscreen: bool, monitor: Option<u32>, config: Option<WindowConfig>) -> Window {
        self.create_default_window(fullscreen, monitor, config)
    }

    // Create a new audio stream
    #[pyo3(name = "create_audio_stream")]
    #[pyo3(signature = (sampling_rate=None, device=None))]
    /// Create a new audio stream with the given sampling rate and device.
    ///
    /// Parameters
    /// ----------
    /// sampling_rate : int, optional
    ///   The sampling rate of the audio stream. If not provided, the default sampling rate of the device will be used.
    /// device : PyDevice, optional
    ///   The audio device to use. If not provided, the default device will be used.
    ///
    /// Returns
    /// -------
    /// Stream
    ///   The new audio stream.
    fn py_create_audio_stream(&self, sampling_rate: Option<u32>, device: Option<&PyDevice>) -> PyStream {
        PyStream::new(&self.audio_host, sampling_rate, device)
    }

    #[pyo3(name = "get_available_monitors")]
    /// Get a list of available monitors.
    ///
    /// Returns
    /// -------
    /// List[Monitor]
    ///  A list of available monitors.
    fn py_get_available_monitors(&self) -> Vec<Monitor> {
        self.get_available_monitors()
    }

    #[pyo3(name = "get_repository")]
    /// Returns the git repository in the current directory or any of its parent directories.
    /// If no repository is found, `None` is returned.
    ///
    /// Returns
    /// -------
    /// Optional[PyRepository]
    ///  The git repository or `None` if no repository was found.
    fn py_get_repository(&self) -> PsydkResult<Option<PyRepository>> {
        self.get_repository().map(|r| r.map(|r| r.into()))
    }

    #[pyo3(name = "system_info")]
    /// Returns generic system information as a dictionary.
    ///
    /// The information includes:
    /// - `os_name`: The name of the operating system.
    /// - `os_version`: The version of the operating system.
    /// - `os_kernel_version`: The kernel version of the operating system.
    /// - `cpu_architecture`: The architecture of the CPU.
    ///
    /// Returns
    /// -------
    /// Dict[str, str]
    ///  A dictionary containing system information.
    fn py_system_info(&self) -> PyResult<HashMap<String, String>> {
        Ok(self.system_info())
    }

    #[pyo3(name = "load_system_fonts")]
    /// Load system fonts into the font manager. Behavior is platform-specific.
    fn py_load_system_fonts(&self) -> PyResult<()> {
        self.load_system_fonts();
        Ok(())
    }

    #[pyo3(name = "load_font_file")]
    /// Load a font file into the font manager.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///  The path to the font file.
    fn py_load_font_file(&self, path: &str) -> PyResult<()> {
        self.load_font_file(path)?;
        Ok(())
    }

    #[pyo3(name = "set_idle_timer_disabled")]
    /// Disable the idle timer on mobile platforms to prevent the screen from
    /// dimming or locking during the experiment. This function has no effect on
    /// desktop platforms.
    ///
    /// Parameters
    /// ----------
    /// disabled : bool
    ///  Whether to disable the idle timer. Set to `True` to disable, `False` to enable.
    fn py_set_idle_timer_disabled(&self, disabled: bool) -> PyResult<()> {
        self.set_idle_timer_disabled(disabled);
        Ok(())
    }

    #[pyo3(name = "load_font_directory")]
    /// Load all font files in a directory into the font manager.
    /// Behavior is platform-specific.
    ///
    /// Parameters
    /// ----------
    /// path : str
    ///  The path to the directory containing font files.
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

    #[cfg(target_os = "macos")]
    #[pyo3(name = "show_alert")]
    /// Show an alert dialog with the given message.
    ///
    /// Parameters
    /// ----------
    /// message : str
    ///  The message to display in the alert dialog.
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
    /// Show a prompt dialog with the given title and optional subtitle.
    ///
    /// Parameters
    /// ----------
    /// title : str
    ///  The title of the prompt dialog.
    /// subtitle : str, optional
    ///  The subtitle of the prompt dialog.
    /// show_text_input : bool, optional
    ///  Whether to show a text input field. Defaults to `True`.
    /// text_input_placeholder : str, optional
    ///  The placeholder text for the text input field.
    /// text_input_value : str, optional
    ///  The initial value for the text input field.
    /// show_cancel_button : bool, optional
    ///  Whether to show a cancel button. Defaults to `False`.
    ///
    /// Returns
    /// ------
    /// str
    ///  The text input value if the user confirmed, or an empty string if the user cancelled.
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

#[pyfunction]
#[pyo3(name = "run_experiment", signature = (py_experiment_fn, config=None, *args, **kwargs))]
/// Runs your experiment. This function will block the current thread until
/// the experiment function returns.
///
/// Parameters
/// ----------
/// experiment_fn : callable
///    The function that runs your experiment. This function should take an instance
///    of `ExperimentContext` as its first argument. Additional arguments can be passed
///    using `*args` and `**kwargs`.
/// config : ExperimentConfig, optional
///   The configuration for the experiment. If not provided, the default configuration
///   will be used.
/// *args : tuple
///   Additional positional arguments to pass to the experiment function.
/// **kwargs : dict
///   Additional keyword arguments to pass to the experiment function.
///
/// Example
/// ---------
/// ```python
/// from psydk import run_experiment, ExperimentContext
///
/// def my_experiment(context: ExperimentContext, subject_id: str):
///     window = context.create_default_window(fullscreen=True)
///     # Your experiment code here
///
/// run_experiment(my_experiment, config=None, "subject_01")
/// ```
pub fn py_run_experiment(
    py: Python,
    py_experiment_fn: Py<PyAny>,
    config: Option<ExperimentConfig>,
    args: Py<PyTuple>,
    kwargs: Option<Py<PyDict>>,
) -> PsydkResult<()> {
    let config = config.unwrap_or_default();

    // create the app
    let mut experiment = Experiment::new(config.clone())?;

    // make app static by leaking it into a static variable
    // todo: is this necessary?
    // let app = Box::leak(Box::new(app));

    // ** black magic ahead **

    // set the __globals__ to make "_renderer_factory" available
    // this will allow functions to create renderer-specific objects
    // without having to pass the renderer object
    let globals = PyDict::new(py);
    let renderer_factory = PyRendererFactory(experiment.shared_renderer_state.cloned());

    // create the Rust function that will be passed to the experiment thread
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

    // actually run the experiment in a separate thread
    py.allow_threads(move || experiment.run_experiment(rust_experiment_fn, config))?; // run the experiment
    Ok(())
}
