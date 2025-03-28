use std::sync::Arc;

use numpy::{IntoPyArray, PyReadonlyArrayDyn};
use pyo3::ffi::c_str;
use pyo3::types::PyAnyMethods;
use pyo3::{pyclass, pymethods, PyResult, Python};
use timed_audio::cpal::traits::{DeviceTrait, HostTrait};
use timed_audio::cpal::{default_host, Device, Host};
use timed_audio::{AudioObject, Stream};

use crate::time::PyTimestamp;

#[derive(Clone)]
#[pyclass]
#[pyo3(name = "Host")]
pub struct PyHost {
    pub(crate) host: Arc<Host>,
}

impl Default for PyHost {
    fn default() -> Self {
        Self {
            host: Arc::new(default_host()),
        }
    }
}

#[derive(Clone)]
#[pyclass]
#[pyo3(name = "Stream")]
pub struct PyStream {
    stream: Stream,
}

#[derive(Clone)]
#[pyclass]
#[pyo3(name = "Device")]
pub struct PyDevice {
    pub(crate) device: Device,
}

#[derive(Debug, Clone)]
#[pyclass]
#[pyo3(name = "AudioObject")]
pub struct PyAudioObject {
    pub(crate) audio_object: AudioObject,
}

impl PyStream {
    pub fn new(host: &Host, device: Option<&PyDevice>) -> Self {
        let device = match device {
            Some(device) => &device.device,
            None => &host.default_output_device().unwrap(),
        };

        let config = device.default_output_config().unwrap();
        let sample_format = config.sample_format();
        Self {
            stream: Stream::new(&device, &config.into(), sample_format),
        }
    }
}

#[pymethods]
impl PyStream {
    fn play(&self, audio_object: PyAudioObject) {
        self.stream.play_now(audio_object.audio_object);
    }

    fn play_at(&self, audio_object: PyAudioObject, timestamp: PyTimestamp) {
        self.stream.play_at(audio_object.audio_object, timestamp.timestamp);
    }

    #[getter]
    fn sample_rate(&self) -> u32 {
        self.stream.sample_rate()
    }
}

#[pymethods]
impl PyAudioObject {
    #[staticmethod]
    fn white_noise(amplitude: f32, duration: f32) -> Self {
        let duration = std::time::Duration::from_secs_f32(duration);
        Self {
            audio_object: AudioObject::white_noise(amplitude, None, duration),
        }
    }

    #[staticmethod]
    fn sine_wave(frequency: f32, volume: f32, duration: std::time::Duration) -> Self {
        Self {
            audio_object: AudioObject::sine_wave(frequency, volume, duration),
        }
    }

    #[staticmethod]
    fn silence(duration: std::time::Duration) -> Self {
        Self {
            audio_object: AudioObject::silence(duration),
        }
    }

    #[staticmethod]
    fn from_samples(samples: PyReadonlyArrayDyn<'_, f32>, sample_rate: u32) -> Self {
        let buffer = samples.as_array().into_owned();

        Self {
            audio_object: AudioObject::from_samples(buffer, sample_rate),
        }
    }
}

pub(crate) fn get_host(py: Python) -> PyResult<PyHost> {
    // first, try to get __renderer_factory from the __globals__
    let host = py
        .eval(c_str!("__audio_host"), None, None)
        .expect("No audio host found in function scope. Are you calling this function from a stimulus callback?");

    // covert to Rust type
    // let renderer_factory = PyRendererFactory::extract_bound(renderer_factory).unwrap();
    let host: PyHost = host.extract().unwrap();
    Ok(host)
}
