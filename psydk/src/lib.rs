#![allow(unused)]
#[macro_use]
use std::collections::HashMap;
use std::{
    pin::Pin,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

use async_channel::{bounded, Receiver, Sender};
use context::{py_run_experiment, ExperimentContext};
use derive_debug::Dbg;
use futures_lite::{future::block_on, Future};
use pyo3::{prelude::*, py_run};
use renderer::wgpu_renderer;
use visual::geometry::Size;
use wgpu::{MemoryHints, TextureFormat};
use winit::{
    event::{Event as WinitEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder},
    monitor::VideoMode,
};

use crate::input::{Event, EventHandlingExt, EventKind, EventTryFrom};

pub mod app;
pub mod audio;
pub mod config;
pub mod errors;
pub mod git;
pub mod input;
pub mod time;
pub mod utils;
pub mod visual;

pub mod context;

// re-export wgpu
pub use wgpu;

// types to make the code more readable
pub(crate) type RenderThreadChannelPayload = Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>;

use std::thread;

use pyo3::types::{PyDict, PyList, PyTuple, PyType};

use crate::visual::window::Frame;

// macro that adds a sub-module to the current module
// example usage:
//
macro_rules! new_submodule {
    ($supermodule:ident, $supermodule_name:literal, $name:literal) => {{
        let m = PyModule::new($supermodule.py(), $name)?;
        m.setattr("__module__", concat!($supermodule_name, ".", $name))?;
        m.py()
            .import("sys")?
            .getattr("modules")?
            .set_item(concat!($supermodule_name, ".", $name), &m)?;
        $supermodule.add_submodule(&m)?;
        m
    }};
}

/// This module is implemented in Rust.
#[pymodule]
fn psydk(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(py_run_experiment, m)?);
    m.add_class::<ExperimentContext>()?;

    let m_visual = {
        let m = new_submodule!(m, "psydk", "visual");

        let m_stimuli = {
            let m = new_submodule!(m, "psydk.visual", "stimuli");
            m.add_class::<visual::stimuli::PyStimulus>()?;
            m.add_class::<visual::stimuli::gabor::PyGaborStimulus>()?;
            m.add_class::<visual::stimuli::image::PyImageStimulus>()?;
            m.add_class::<visual::stimuli::pattern::PyPatternStimulus>()?;
            m.add_class::<visual::stimuli::text::PyTextStimulus>()?;
            m
        };

        m.add_submodule(&m_stimuli)?;

        let m_geometry = {
            let m = new_submodule!(m, "psydk.visual", "geometry");
            m.add_class::<visual::geometry::Transformation2D>()?;
            m.add_class::<visual::geometry::Shape>()?;
            m.add_class::<visual::geometry::Size>()?;
            m.add_function(wrap_pyfunction!(visual::geometry::px, &m)?)?;
            m.add_function(wrap_pyfunction!(visual::geometry::vw, &m)?)?;
            m.add_function(wrap_pyfunction!(visual::geometry::vh, &m)?)?;
            m.add_function(wrap_pyfunction!(visual::geometry::deg, &m)?)?;
            m.add_function(wrap_pyfunction!(visual::geometry::mm, &m)?)?;
            m.add_function(wrap_pyfunction!(visual::geometry::cm, &m)?)?;
            m.add_function(wrap_pyfunction!(visual::geometry::py_in, &m)?)?;

            m.add_function(wrap_pyfunction!(visual::geometry::rectangle, &m)?)?;
            m.add_function(wrap_pyfunction!(visual::geometry::circle, &m)?)?;
            m.add_function(wrap_pyfunction!(visual::geometry::ellipse, &m)?)?;
            m.add_function(wrap_pyfunction!(visual::geometry::line, &m)?)?;
            m.add_function(wrap_pyfunction!(visual::geometry::polygon, &m)?)?;
            m.add_function(wrap_pyfunction!(visual::geometry::path, &m)?)?;

            m
        };

        m.add_submodule(&m_geometry)?;

        let m_color = {
            let m = new_submodule!(m, "psydk.visual", "color");
            m.add_function(wrap_pyfunction!(visual::color::py_rgb, &m)?)?;
            m.add_function(wrap_pyfunction!(visual::color::py_rgba, &m)?)?;
            m.add_function(wrap_pyfunction!(visual::color::py_linrgb, &m)?)?;
            m.add_function(wrap_pyfunction!(visual::color::py_linrgba, &m)?)?;
            m
        };

        m.add_submodule(&m_color)?;

        m
    };

    m.add_submodule(&m_visual)?;

    let m_audio = {
        let m = new_submodule!(m, "psydk", "audio");
        m.add_class::<audio::PyStream>()?;
        m.add_class::<audio::PyDevice>()?;
        m.add_class::<audio::PyHost>()?;
        m.add_class::<audio::PyAudioObject>()?;
        m.add_function(wrap_pyfunction!(audio::py_create_silence, &m)?)?;
        m.add_function(wrap_pyfunction!(audio::py_create_sine_wave, &m)?)?;
        m.add_function(wrap_pyfunction!(audio::py_create_white_noise, &m)?)?;
        m.add_function(wrap_pyfunction!(audio::py_create_from_samples, &m)?)?;
        m
    };

    m.add_submodule(&m_audio)?;

    let m_time = {
        let m = new_submodule!(m, "psydk", "time");
        m.add_class::<time::Timestamp>()?;
        m.add_function(wrap_pyfunction!(time::py_now, &m)?)?;
        m
    };

    m.add_submodule(&m_time)?;

    let m_utils = {
        let m = new_submodule!(m, "psydk", "utils");
        m.add_class::<utils::PyCSVWriter>()?;
        m.add_function(wrap_pyfunction!(time::py_now, &m)?)?;
        m
    };

    m.add_submodule(&m_utils)?;

    Ok(())
}
