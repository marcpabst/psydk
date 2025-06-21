use std::sync::Arc;

use numpy::{IntoPyArray, PyReadonlyArrayDyn};
use pyo3::ffi::c_str;
use pyo3::types::PyAnyMethods;
use pyo3::{pyclass, pyfunction, pymethods, Bound, PyAny, PyObject, PyRef, PyRefMut, PyResult, Python};
use timed_audio::cpal::traits::{DeviceTrait, HostTrait};
use timed_audio::cpal::{default_host, Device, Host, StreamConfig};
use timed_audio::{AudioObject, Stream};

use crate::time::Timestamp;

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
    stream: Option<Stream>,
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
    pub fn new(host: &Host, sampling_rate: Option<u32>, device: Option<&PyDevice>) -> Self {
        let device = match device {
            Some(device) => &device.device,
            None => &host.default_output_device().unwrap(),
        };

        let config = device.default_output_config().unwrap();
        let sample_format = config.sample_format();

        let mut config: StreamConfig = config.into();

        // If a specific sample rate is requested, override the default
        if let Some(rate) = sampling_rate {
            config.sample_rate = timed_audio::cpal::SampleRate(rate);
        }

        Self {
            stream: Some(Stream::new(&device, &config, sample_format)),
        }
    }
}

#[pymethods]
impl PyStream {
    fn play(&self, audio_object: PyAudioObject) {
        self.stream.as_ref().unwrap().play_now(audio_object.audio_object);
    }

    fn play_at(&self, audio_object: PyAudioObject, timestamp: Timestamp) {
        self.stream
            .as_ref()
            .unwrap()
            .play_at(audio_object.audio_object, timestamp.timestamp);
    }

    #[getter]
    fn sample_rate(&self) -> u32 {
        self.stream.as_ref().unwrap().sample_rate()
    }

    // allow stream to be used as a context manager
    fn __enter__(slf: PyRef<Self>) -> PyRef<Self> {
        slf
    }

    fn __exit__(
        mut slf: PyRefMut<Self>,
        exc_type: Bound<'_, crate::PyAny>,
        exc_value: Bound<'_, crate::PyAny>,
        traceback: Bound<'_, crate::PyAny>,
    ) -> PyResult<()> {
        // drop the stream
        slf.stream = None;
        Ok(())
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

    #[staticmethod]
    #[pyo3(signature = (path, track = None, sampling_rate = None))]
    fn from_file(path: &str, track: Option<usize>, sampling_rate: Option<u32>) -> PyResult<Self> {
        let audio_object = AudioObject::from_file(path, track, sampling_rate)
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("Failed to load audio file: {}", e)))?;
        Ok(Self { audio_object })
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

#[pyfunction]
#[pyo3(name = "create_silence")]
pub fn py_create_silence(py: Python, duration: f32) -> PyAudioObject {
    PyAudioObject::silence(std::time::Duration::from_secs_f32(duration))
}

#[pyfunction]
#[pyo3(name = "create_white_noise")]
pub fn py_create_white_noise(py: Python, amplitude: f32, duration: f32) -> PyAudioObject {
    PyAudioObject::white_noise(amplitude, duration)
}

#[pyfunction]
#[pyo3(name = "create_sine_wave")]
pub fn py_create_sine_wave(py: Python, frequency: f32, volume: f32, duration: f32) -> PyAudioObject {
    PyAudioObject::sine_wave(frequency, volume, std::time::Duration::from_secs_f32(duration))
}

#[pyfunction]
#[pyo3(name = "create_from_samples")]
pub fn py_create_from_samples(py: Python, samples: PyReadonlyArrayDyn<'_, f32>, sample_rate: u32) -> PyAudioObject {
    PyAudioObject::from_samples(samples, sample_rate)
}

#[pyfunction]
#[pyo3(name = "create_from_file")]
#[pyo3(signature = (path, track = None, sampling_rate = None))]
pub fn py_create_from_file(
    py: Python,
    path: &str,
    track: Option<usize>,
    sampling_rate: Option<u32>,
) -> PyResult<PyAudioObject> {
    PyAudioObject::from_file(path, track, sampling_rate)
        .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("Failed to load audio file: {}", e)))
}
