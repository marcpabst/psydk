use std::sync::Arc;
use std::time::{Duration, Instant};

use pyo3::ffi::c_str;
use pyo3::types::PyAnyMethods;
use pyo3::{pyclass, pyfunction, pymethods, PyResult, Python};

#[derive(Debug, Clone)]
#[pyclass]
#[pyo3(name = "Timestamp")]
/// A timestamp represents a point in time.
///
/// Timestamps are opaque and can only be compared to other timestamps or floats. This
/// means that they can be compared to each other, added to or subtracted from, and used to
/// calculate the time since another timestamp. However, they are not meaningful on their own.
pub struct PyTimestamp {
    pub(crate) timestamp: Instant,
}

#[pymethods]
impl PyTimestamp {
    #[new]
    fn new() -> Self {
        PyTimestamp {
            timestamp: Instant::now(),
        }
    }

    fn elapsed(&self) -> PyResult<f64> {
        Ok(self.timestamp.elapsed().as_secs_f64())
    }

    // allow subtracting a float from the timestamp
    fn __sub__(&self, other: f64) -> PyResult<PyTimestamp> {
        Ok(PyTimestamp {
            timestamp: self.timestamp - Duration::from_secs_f64(other),
        })
    }

    // allow adding a float to the timestamp
    fn __add__(&self, other: f64) -> PyResult<PyTimestamp> {
        Ok(PyTimestamp {
            timestamp: self.timestamp + Duration::from_secs_f64(other),
        })
    }

    // allow comparing the timestamp to a float
    fn __lt__(&self, other: f64) -> PyResult<bool> {
        Ok(self.timestamp.elapsed().as_secs_f64() < other)
    }

    // allow comparing the timestamp to a float
    fn __gt__(&self, other: f64) -> PyResult<bool> {
        Ok(self.timestamp.elapsed().as_secs_f64() > other)
    }

    /// Get the time since another timestamp in seconds.
    /// May be negative if the other timestamp is in the future.
    fn seconds_since(&self, other: PyTimestamp) -> PyResult<f64> {
        Ok(self.timestamp.duration_since(other.timestamp).as_secs_f64())
    }

    // implement the __str__ method
    fn __str__(&self) -> PyResult<String> {
        Ok("Opaque timestamp (use the `seconds_since` method to get the time since another timestamp)".to_string())
    }
}

#[pyfunction]
#[pyo3(name = "now")]
pub fn py_now() -> PyTimestamp {
    PyTimestamp {
        timestamp: Instant::now(),
    }
}
