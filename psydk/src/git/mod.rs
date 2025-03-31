use std::sync::{Arc, Mutex};

use gix::{open, Repository};
use pyo3::prelude::{pyclass, pymethods};

#[derive(Debug)]
#[pyclass]
#[pyo3(name = "Repository")]
/// A repository.
pub struct PyRepository {
    pub repo: Arc<Mutex<Repository>>,
}

impl PyRepository {
    pub fn new(path: &str) -> Self {
        let repo = open(path).unwrap();
        PyRepository {
            repo: Arc::new(Mutex::new(repo)),
        }
    }
}

#[pymethods]
impl PyRepository {
    #[new]
    fn py_new(path: &str) -> Self {
        PyRepository::new(path)
    }

    #[pyo3(name = "is_dirty")]
    fn py_is_dirty(&self) -> bool {
        self.repo.lock().unwrap().is_dirty().unwrap()
    }

    #[pyo3(name = "get_current_branch")]
    fn py_get_current_branch(&self) -> String {
        self.repo
            .lock()
            .unwrap()
            .head_ref()
            .unwrap()
            .unwrap()
            .name()
            .shorten()
            .to_string()
    }

    /// Returns the current commit.
    #[pyo3(name = "get_current_commit")]
    fn py_get_current_commit(&self) -> String {
        self.repo.lock().unwrap().head_commit().unwrap().id().to_string()
    }
}

impl From<Repository> for PyRepository {
    fn from(repo: Repository) -> Self {
        PyRepository {
            repo: Arc::new(Mutex::new(repo)),
        }
    }
}
