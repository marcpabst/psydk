use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};

use sysinfo::System;

use derive_debug::Dbg;
use pyo3::{
    pyclass, pyfunction, pymethods,
    types::{PyAnyMethods, PyDict, PyList, PyListMethods, PySequenceMethods, PyTuple, PyTupleMethods},
    IntoPy, Py, PyAny, PyResult, Python,
};
use renderer::{cosmic_text, renderer::SharedRendererState};
use winit::event_loop::EventLoopProxy;

use crate::{
    app::{App, ArcMutex, GPUState},
    audio::{PyDevice, PyHost, PyStream},
    errors::{self, PsydkError, PsydkResult},
    git::PyRepository,
    visual::window::Window,
};

#[derive(Dbg)]
pub enum EventLoopAction {
    CreateNewWindow(WindowOptions, GammaOptions, Sender<Window>),
    GetAvailableMonitors(Sender<Vec<Monitor>>),
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

    /// Create a new window with the given options. This function will dispatch
    /// a new UserEvent to the event loop and wait until the winit window
    /// has been created. Then it will setup the wgpu device and surface and
    /// return a new Window object.
    pub fn create_window(&self, window_options: &WindowOptions, gamma_options: GammaOptions) -> Window {
        // set up window by dispatching a new CreateNewWindow action
        let (sender, receiver) = channel();
        let action = EventLoopAction::CreateNewWindow(window_options.clone(), gamma_options, sender);

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
    #[pyo3(signature = (device = None))]
    fn py_create_audio_stream(&self, device: Option<&PyDevice>) -> PyStream {
        PyStream::new(&self.audio_host, device)
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
}

/// Runs your experiment function. This function will block the current thread
/// until the experiment function returns!
///
/// Parameters
/// ----------
/// experiment_fn : callable
///    The function that runs your experiment. This function should take a single argument, an instance of `ExperimentManager`, and should not return nothing.
#[pyfunction]
#[pyo3(name = "run_experiment", signature = (py_experiment_fn, *args, **kwargs))]
pub fn py_run_experiment(
    py: Python,
    py_experiment_fn: Py<PyAny>,
    args: Py<PyTuple>,
    kwargs: Option<Py<PyDict>>,
) -> PyResult<()> {
    // create app
    let mut app = App::new();

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
