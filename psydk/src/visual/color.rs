use csscolorparser;
use pyo3::{prelude::*, types::PyTuple};

use crate::visual::geometry::IntoSize;

#[derive(Debug, Clone, Copy)]
/// Create a new linear RGBA color.
/// The alpha channel defaults to 1.0.
///
/// Parameters
/// ----------
/// r : float
///    The red channel (0.0 to 1.0).
/// g : float
///   The green channel (0.0 to 1.0).
/// b : float
///  The blue channel.
/// a : float, optional
///   The alpha channel (0.0 to 1.0).
///
/// Returns
/// -------
/// LinRgba
///  The linear RGBA color.
///
/// Examples
/// --------
/// >>> black = LinRgba(0.0, 0.0, 0.0)
/// >>> blue = LinRgba.from_str("blue")
pub struct LinRgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for LinRgba {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }
}

impl LinRgba {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    #[inline]
    fn srgb_to_lin_rgb(c: f32) -> f32 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    pub fn from_srgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        // Decode sRGB to linear RGB
        Self {
            r: Self::srgb_to_lin_rgb(r),
            g: Self::srgb_to_lin_rgb(g),
            b: Self::srgb_to_lin_rgb(b),
            a,
        }
    }

    pub fn from_str(css_color_str: &str) -> Self {
        let color = csscolorparser::parse(css_color_str).expect("Failed to parse color");
        Self {
            r: Self::srgb_to_lin_rgb(color.r),
            g: Self::srgb_to_lin_rgb(color.g),
            b: Self::srgb_to_lin_rgb(color.b),
            a: color.a,
        }
    }

    pub fn r(&self) -> f32 {
        self.r
    }

    pub fn g(&self) -> f32 {
        self.g
    }

    pub fn b(&self) -> f32 {
        self.b
    }

    pub fn a(&self) -> f32 {
        self.a
    }

    pub fn r_u8(&self) -> u8 {
        (self.r * 255.0).round() as u8
    }

    pub fn g_u8(&self) -> u8 {
        (self.g * 255.0).round() as u8
    }

    pub fn b_u8(&self) -> u8 {
        (self.b * 255.0).round() as u8
    }

    pub fn a_u8(&self) -> u8 {
        (self.a * 255.0).round() as u8
    }
}

impl From<LinRgba> for renderer::colors::RGBA {
    fn from(rgba: LinRgba) -> Self {
        Self::new_linear(rgba.r, rgba.g, rgba.b, rgba.a)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IntoLinRgba(pub LinRgba);

impl IntoLinRgba {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self(LinRgba::new(r, g, b, a))
    }
}

impl From<IntoLinRgba> for LinRgba {
    fn from(into_lin_rgba: IntoLinRgba) -> Self {
        into_lin_rgba.0
    }
}

impl<'py> FromPyObject<'py> for IntoLinRgba {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        // try to extract a LinRgba from the object
        if let Ok(rgba) = ob.extract::<LinRgba>() {
            Ok(Self(rgba))
        }
        // try to extract a tuple of 3 (alpha implicitly set to 1.0)
        else if let Ok((r, g, b)) = ob.extract() {
            Ok(Self(LinRgba::new(r, g, b, 1.0)))
        }
        // try to extract a tuple of 4
        else if let Ok((r, g, b, a)) = ob.extract() {
            Ok(Self(LinRgba::new(r, g, b, a)))
        }
        // try to extract from a string
        else if let Ok(css_color_str) = ob.extract::<String>() {
            Ok(Self(LinRgba::from_str(&css_color_str)))
        }
        // otherwise, raise an error
        else {
            Err(pyo3::exceptions::PyTypeError::new_err(
                "Expected a tuple of 3 or 4 floats, a LinRgba, or a CSS color string",
            ))
        }
    }
}

// expose functons to python to create a LinRgba
#[pyfunction]
#[pyo3(name = "rgb")]
#[pyo3(signature = (r, g, b, a = 1.0))]
/// Specify an RGB with all channels being sRGB encoded.
/// This is the default color space for most colors.
/// Note: The returned color is in linear RGB space.
///
/// Parameters
/// ---------
/// r : float
///   The red channel (0.0 to 1.0).
/// g : float
///  The green channel (0.0 to 1.0).
/// b : float
///     The blue channel (0.0 to 1.0).
/// a : float, optional
///     The alpha channel (0.0 to 1.0).
///
/// Returns
/// -------
/// (r, g, b, a) : tuple
///   The RGB color as a tuple of 4 floats.
pub fn py_rgb(r: f32, g: f32, b: f32, a: f32) -> LinRgba {
    LinRgba::from_srgba(r, g, b, a)
}

#[pyfunction]
#[pyo3(name = "linrgb")]
#[pyo3(signature = (r, g, b, a = 1.0))]
/// Specify a color in linear RGB space.
///
/// Parameters
/// ---------
/// r : float
///   The red channel (0.0 to 1.0).
/// g : float
///  The green channel (0.0 to 1.0).
/// b : float
///     The blue channel (0.0 to 1.0).
/// a : float, optional
///     The alpha channel (0.0 to 1.0).
///
/// Returns
/// -------
/// (r, g, b, a) : tuple
///   The linear RGB color as a tuple of 4 floats.
pub fn py_linrgb(r: f32, g: f32, b: f32, a: f32) -> LinRgba {
    LinRgba::new(r, g, b, a)
}

// implement IntoPyObject for LinRgba
impl<'py> IntoPyObject<'py> for LinRgba {
    type Target = PyTuple;
    type Output = Bound<'py, Self::Target>;
    type Error = pyo3::PyErr;

    #[inline]
    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        PyTuple::new(py, &[self.r, self.g, self.b, self.a])
    }
}

// allow Python tuples to be converted to LinRgba
impl<'py> FromPyObject<'py> for LinRgba {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        // try to extract a tuple of 3 (alpha implicitly set to 1.0)
        if let Ok((r, g, b)) = ob.extract() {
            Ok(LinRgba::new(r, g, b, 1.0))
        }
        // try to extract a tuple of 4
        else if let Ok((r, g, b, a)) = ob.extract() {
            Ok(LinRgba::new(r, g, b, a))
        }
        // otherwise, raise an error
        else {
            Err(pyo3::exceptions::PyTypeError::new_err(
                "Expected a tuple of 3 or 4 floats",
            ))
        }
    }
}
