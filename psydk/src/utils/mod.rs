use fs4::FileExt;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::thread;

use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyclass, pymethods, Bound, Py, PyObject, PyRef, PyRefMut, PyResult};

#[derive(Debug, Clone)]
pub struct CSVWriter {
    pub path: PathBuf,
    pub delimiter: char,
    pub headers: Vec<String>,
    pub record_sender: Option<Sender<Vec<String>>>,
}

impl CSVWriter {
    pub fn new(
        path: String,
        delimiter: char,
        headers: Vec<String>,
        write_headers: bool,
        append: bool,
    ) -> Result<Self, std::io::Error> {
        // check if directory exists
        let path = std::path::Path::new(&path).to_path_buf();
        if !path.parent().map_or(false, |p| p.exists()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Directory {} does not exist", path.display()),
            ));
        }

        // check if we have write permissions
        if !path.parent().map_or(false, |p| p.is_dir()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("No write permissions for directory {}", path.display()),
            ));
        }

        // check if the file path exists and is writable
        if !append && std::path::Path::new(&path).exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("File {} already exists", path.display()),
            ));
        }

        // Create the thread that will write to the CSV file
        let (tx, rx) = channel::<Vec<String>>();
        let path_clone = path.clone();
        let delimiter_clone = delimiter;
        let headers_clone = headers.clone();

        thread::spawn(move || {
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .append(append)
                .open(path_clone)
                .expect("Unable to open file");

            // Lock the file for writing
            file.try_lock_exclusive().expect("Unable to lock file for writing");

            let mut writer = BufWriter::new(file);

            // Write headers if they are provided
            if !headers_clone.is_empty() && write_headers {
                let header_line = headers_clone.join(&delimiter_clone.to_string());
                writeln!(writer, "{}", header_line).expect("Unable to write headers");
                // Flush the writer to ensure headers are written to the file
                writer.flush().expect("Unable to flush writer");
            }

            // Write records received from the channel
            loop {
                match rx.recv() {
                    Ok(record) => {
                        let record_line = record.join(&delimiter_clone.to_string());
                        writeln!(writer, "{}", record_line).expect("Unable to write record");
                        // Flush the writer to ensure data is written to the file
                        writer.flush().expect("Unable to flush writer");
                    }
                    Err(_) => {
                        // Channel closed, exit the loop
                        break;
                    }
                }
            }
            // Unlock the file after writing
            writer.get_ref().unlock().expect("Unable to unlock file");
        });

        Ok(Self {
            path,
            delimiter,
            headers,
            record_sender: Some(tx),
        })
    }
    pub fn write_record(&self, record: Vec<String>) -> Result<(), std::io::Error> {
        if let Some(sender) = &self.record_sender {
            sender.send(record).expect("Unable to send record");
            Ok(())
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::Other, "CSV writer is closed"))
        }
    }

    pub fn close(&mut self) {
        // Close the channel to signal the writing thread to exit
        if let Some(sender) = self.record_sender.take() {
            drop(sender);
        }
    }
}

#[pyclass]
#[derive(Clone)]
#[pyo3(name = "CSVWriter")]
pub struct PyCSVWriter(pub CSVWriter);

#[pymethods]
impl PyCSVWriter {
    #[new]
    pub fn new(
        path: String,
        delimiter: char,
        headers: Vec<String>,
        write_headers: bool,
        append: bool,
    ) -> PyResult<Self> {
        Ok(PyCSVWriter(
            CSVWriter::new(path, delimiter, headers, write_headers, append)
                .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("Failed to create CSV writer: {}", e)))?,
        ))
    }

    pub fn write_record(&self, record: Vec<String>) -> PyResult<()> {
        self.0
            .write_record(record)
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(format!("Failed to write record to CSV: {}", e)))
    }

    pub fn write_dict(&self, record: Bound<PyDict>) -> PyResult<()> {
        let mut record_vec = Vec::new();

        // check if all provided keys are in the headers
        for key in record.keys() {
            let key_str = key.to_string();
            if !self.0.headers.contains(&key_str) {
                return Err(pyo3::exceptions::PyKeyError::new_err(format!(
                    "Key '{}' not found in headers",
                    key_str
                )));
            }
        }

        // create a vector of values in the same order as the headers, append empty strings for missing keys
        for header in &self.0.headers {
            if let Ok(Some(value)) = record.get_item(header) {
                record_vec.push(value.to_string());
            } else {
                record_vec.push("".to_string());
            }
        }

        self.write_record(record_vec)
    }

    pub fn close(&mut self) {
        self.0.close();
    }

    // allows Window to be used as a context manager
    fn __enter__(slf: PyRef<Self>) -> PyResult<Py<Self>> {
        // return self
        Ok(slf.into())
    }

    fn __exit__(
        mut slf: PyRefMut<Self>,
        exc_type: Bound<'_, crate::PyAny>,
        exc_value: Bound<'_, crate::PyAny>,
        traceback: Bound<'_, crate::PyAny>,
    ) -> PyResult<()> {
        // close the CSV writer
        slf.0.close();
        Ok(())
    }
}
