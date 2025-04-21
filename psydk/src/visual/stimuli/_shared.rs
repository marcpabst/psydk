use pyo3::pyclass;
use renderer::DynamicBitmap;

#[pyclass]
pub struct Image(DynamicBitmap);

pub struct IntoImage(pub Image);
