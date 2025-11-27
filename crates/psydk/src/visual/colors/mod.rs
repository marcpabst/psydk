use nalgebra::{Matrix3, Vector3};
use palette::white_point::C;
use pyo3::prelude::*;
use pyo3::types::PyAny;
use pyo3::types::PyString;
use pyo3::types::PyType;
use pyo3::{exceptions::PyValueError, PyErr, PyResult};
use pyo3::{prelude::*, types::PyTuple};
use std::sync::Arc;

mod conversion;
pub mod display_characteristics;
use conversion::{hsl_to_rgb, rgb_to_hsl};
pub use display_characteristics::DisplayCharacteristics;
pub mod psydk_1;

#[pyclass]
/// RGBA color with floating point components.
#[derive(Debug, Clone, Copy)]
pub struct RGBA {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
    pub space: RGBColorSpace,
}

#[pyclass]
/// XYZA color with floating point components.
#[derive(Debug, Clone, Copy)]
pub struct XYZA {
    /// X component
    pub x: f32,
    /// Y component
    pub y: f32,
    /// Z component
    pub z: f32,
    /// Alpha component
    pub a: f32,
}

#[pyclass]
/// CIE 1976 L*u*v* color with alpha channel
#[derive(Debug, Clone, Copy)]
pub struct LuvA {
    /// L component
    pub l: f32,
    /// u component
    pub u: f32,
    /// v component
    pub v: f32,
    /// Alpha component
    pub a: f32,
    /// The white point in XYZ coordinates
    pub white_point: [f32; 3],
}

#[pyclass]
#[derive(Debug, Clone, Copy)]
pub struct LabA {
    /// L component
    pub l: f32,
    /// a component
    pub a: f32,
    /// b component
    pub b: f32,
    /// Alpha component
    pub alpha: f32,
    /// The white point in XYZ coordinates
    pub white_point: [f32; 3],
}

#[pyclass]
#[derive(Debug, Clone, Copy)]
pub enum Color {
    RGBA(RGBA),
    XYZA(XYZA),
    LuvA(LuvA),
    LabA(LabA),
}

#[derive(Clone)]
pub enum Display {
    /// A display with known characteristics
    DisplayCharacteristics(Arc<dyn DisplayCharacteristics>),
    /// A display defined by an ICC profile
    ICCProfile(Vec<u8>),
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum RGBColorSpace {
    /// The output device's native color space.
    Native,
    /// The output device's native color space (linearized).
    NativeLinear,
    /// Standard sRGB color space, with sRGB encoding.
    SRGB,
    /// Linear sRGB color space, no encoding.
    SRGBLinear,
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct GenericColor {
    pub c1: f32,
    pub c2: f32,
    pub c3: f32,
}

#[derive(Debug, Clone)]
/// RGBA color for display output (already in display color space)
pub struct DisplayRGBA {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for DisplayRGBA {
    fn default() -> Self {
        DisplayRGBA {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        }
    }
}

impl Default for &DisplayRGBA {
    fn default() -> Self {
        &DisplayRGBA {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        }
    }
}

impl DisplayRGBA {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

impl Color {
    pub fn new_rgba(r: f32, g: f32, b: f32, a: f32, space: RGBColorSpace) -> Self {
        Color::RGBA(RGBA { r, g, b, a, space })
    }

    pub fn new_srgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Color::RGBA(RGBA {
            r,
            g,
            b,
            a,
            space: RGBColorSpace::SRGB,
        })
    }

    pub fn new_xyza(x: f32, y: f32, z: f32, a: f32) -> Self {
        Color::XYZA(XYZA { x, y, z, a })
    }

    pub fn new_luva(l: f32, u: f32, v: f32, a: f32, white_point: [f32; 3]) -> Self {
        Color::LuvA(LuvA {
            l,
            u,
            v,
            a,
            white_point,
        })
    }

    pub fn new_laba(l: f32, a: f32, b: f32, alpha: f32, white_point: [f32; 3]) -> Self {
        Color::LabA(LabA {
            l,
            a,
            b,
            alpha,
            white_point,
        })
    }

    pub fn alpha(&self) -> f32 {
        match self {
            Color::RGBA(rgba) => rgba.a,
            Color::XYZA(xyza) => xyza.a,
            Color::LuvA(luva) => luva.a,
            Color::LabA(laba) => laba.alpha,
        }
    }

    /// Check if the color is in RGB color space
    pub fn is_rgb(&self) -> bool {
        matches!(self, Color::RGBA(_))
    }

    /// Check if the color is in CIE 1931 XYZ color space
    pub fn is_xyz(&self) -> bool {
        matches!(self, Color::XYZA(_))
    }

    /// Check if the color is in Luv color space
    pub fn is_luv(&self) -> bool {
        matches!(self, Color::LuvA(_))
    }

    /// Check if the color is in Lab color space
    pub fn is_lab(&self) -> bool {
        matches!(self, Color::LabA(_))
    }

    /// Convert the color to CIE 1931 XYZ color space.
    pub fn to_xyz(&self) -> Result<Vector3<f32>, String> {
        match self {
            Color::RGBA(rgba) => {
                // for RGB colors, conveersion depends on the color space
                match rgba.space {
                    // standard sRGB
                    RGBColorSpace::SRGB => {
                        // Convert sRGB to linear RGB
                        let r_lin = if rgba.r <= 0.04045 {
                            rgba.r / 12.92
                        } else {
                            ((rgba.r + 0.055) / 1.055).powf(2.4)
                        };
                        let g_lin = if rgba.g <= 0.04045 {
                            rgba.g / 12.92
                        } else {
                            ((rgba.g + 0.055) / 1.055).powf(2.4)
                        };
                        let b_lin = if rgba.b <= 0.04045 {
                            rgba.b / 12.92
                        } else {
                            ((rgba.b + 0.055) / 1.055).powf(2.4)
                        };
                        // Convert linear RGB to XYZ using the SRGB_TO_XYZ_DEBUG
                        let rgb = Vector3::new(r_lin, g_lin, b_lin);
                        let xyz = SRGB_TO_XYZ * rgb;
                        Ok([xyz.x, xyz.y, xyz.z])
                    }
                    // linear sRGB
                    RGBColorSpace::SRGBLinear => {
                        // Directly convert linear RGB to XYZ using the sRGB matrix
                        let rgb = Vector3::new(rgba.r, rgba.g, rgba.b);
                        let xyz = SRGB_TO_XYZ * rgb;
                        Ok([xyz.x, xyz.y, xyz.z])
                    }
                    _ => Err("Conversion from this RGB color space to XYZ is not implemented".to_string()),
                }
            }
            Color::XYZA(xyza) => Ok([xyza.x, xyza.y, xyza.z]),
            Color::LuvA(luva) => Ok(conversion::luv_to_xyz(&[luva.l, luva.u, luva.v], &luva.white_point)),
            Color::LabA(laba) => Ok(conversion::lab_to_xyz(&[laba.l, laba.alpha, laba.b], &laba.white_point)),
        }
        .map(|arr| Vector3::new(arr[0], arr[1], arr[2]))
    }

    pub fn to_display_rgba(&self, dc: &dyn DisplayCharacteristics) -> Result<DisplayRGBA, String> {
        /// If Native, just return the RGBA values directly
        if let Color::RGBA(rgba) = self {
            if rgba.space == RGBColorSpace::Native {
                return DisplayRGBA::new(rgba.r, rgba.g, rgba.b, rgba.a);
            }
        }

        let xyz = self.to_xyz()?;

        // Convert XYZ to display RGB using the display characteristics
        let display_rgb = dc.xyz_to_rgb(&xyz);

        DisplayRGBA::new(display_rgb.x, display_rgb.y, display_rgb.z, self.alpha())
    }
}

impl Default for Color {
    fn default() -> Self {
        Color::new_rgba(1.0, 1.0, 1.0, 1.0, RGBColorSpace::SRGB)
    }
}

impl Into<Vector3<f32>> for Color {
    fn into(self) -> Vector3<f32> {
        match self {
            Color::RGBA(rgba) => Vector3::new(rgba.r, rgba.g, rgba.b),
            Color::XYZA(xyza) => Vector3::new(xyza.x, xyza.y, xyza.z),
            Color::LuvA(luva) => Vector3::new(luva.l, luva.u, luva.v),
            Color::LabA(laba) => Vector3::new(laba.l, laba.a, laba.b),
        }
    }
}

impl From<DisplayRGBA> for renderer::colors::RGBA {
    fn from(rgba: DisplayRGBA) -> Self {
        Self::new_linear(rgba.r, rgba.g, rgba.b, rgba.a)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IntoColor(pub Color);

impl Default for IntoColor {
    fn default() -> Self {
        Self(Color::default())
    }
}

impl From<IntoColor> for Color {
    fn from(into_c: IntoColor) -> Self {
        into_c.0
    }
}

impl<'py> FromPyObject<'py> for IntoColor {
    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        // try to extract an existing Color object
        if let Ok(color) = ob.extract::<Color>() {
            Ok(Self(color))
        }
        // try to extract a tuple of 3 (alpha implicitly set to 1.0)
        // we assume native color space for tuples
        else if let Ok((r, g, b)) = ob.extract() {
            Ok(Self(Color::new_rgba(r, g, b, 1.0, RGBColorSpace::Native)))
        }
        // try to extract a tuple of 4
        // we assume b sRGB color space for tuples
        else if let Ok((r, g, b, a)) = ob.extract() {
            Ok(Self(Color::new_rgba(r, g, b, a, RGBColorSpace::Native)))
        }
        // try to extract from a string
        else if let Ok(color_str) = ob.extract::<String>() {
            name_to_color(&color_str)
                .map(|c| Self(c))
                .ok_or_else(|| PyErr::new::<PyValueError, _>(format!("Unknown color name: {}", color_str)))
        }
        // otherwise, raise an error
        else {
            Err(pyo3::exceptions::PyTypeError::new_err(
                "Expected a Color, a tuple of 3 or 4 floats, or a color name string",
            ))
        }
    }
}

// expose functons to python to create a Color
#[pyfunction]
#[pyo3(name = "rgb")]
#[pyo3(signature = (r, g, b, a = 1.0))]
/// A color in the display's RGB color space.
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
pub fn py_rgb(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color::new_rgba(r, g, b, a, RGBColorSpace::Native)
}

#[pyfunction]
#[pyo3(name = "linrgb")]
#[pyo3(signature = (r, g, b, a = 1.0))]
/// A color in the display's linear RGB color space.
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
pub fn py_linrgb(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color::new_rgba(r, g, b, a, RGBColorSpace::NativeLinear)
}

#[pyfunction]
#[pyo3(name = "srgb")]
#[pyo3(signature = (r, g, b, a = 1.0))]
/// A color in the standard sRGB color space.
pub fn py_srgb(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color::new_rgba(r, g, b, a, RGBColorSpace::SRGB)
}

#[pyfunction]
#[pyo3(name = "linsrgb")]
#[pyo3(signature = (r, g, b, a = 1.0))]
/// A color in the linear sRGB color space.
pub fn py_linsrgb(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color::new_rgba(r, g, b, a, RGBColorSpace::SRGBLinear)
}

#[pyfunction]
#[pyo3(name = "luv")]
#[pyo3(signature = (l, u, v, a = 1.0, white_point = [0.95047, 1.0, 1.08883]))]
/// A color in the CIE 1976 L*u*v* color space.
///
/// Parameters
/// ---------
/// l : float
///  The L* channel (0.0 to 100.0).
/// u : float
/// The u* channel.
/// v : float
/// The v* channel.
/// a : float, optional
///    The alpha channel (0.0 to 1.0).
/// white_point : list of 3 floats, optional
///    The white point in XYZ coordinates. Default is D65 ([0.95047, 1.0, 1.08883]).
pub fn py_luv(l: f32, u: f32, v: f32, a: f32, white_point: [f32; 3]) -> Color {
    Color::new_luva(l, u, v, a, white_point)
}

#[pyfunction]
#[pyo3(name = "xyz")]
#[pyo3(signature = (x, y, z, a = 1.0))]
/// A color in the CIE 1931 XYZ color space.
///
/// Parameters
/// ---------
/// x : float
/// The X channel.
/// y : float
/// The Y channel.
/// z : float
/// The Z channel.
/// a : float, optional
///   The alpha channel (0.0 to 1.0).
pub fn py_xyz(x: f32, y: f32, z: f32, a: f32) -> Color {
    Color::new_xyza(x, y, z, a)
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

const SRGB_TO_XYZ: Matrix3<f32> = Matrix3::new(
    0.4124564, 0.3575761, 0.1804375, // R
    0.2126729, 0.7151522, 0.0721750, // G
    0.0193339, 0.1191920, 0.9503041, // B
);

// DEBUG matrix (changes order of primary colors)
const SRGB_TO_XYZ_DEBUG: Matrix3<f32> = Matrix3::new(
    0.0193339, 0.1191920, 0.9503041, // B
    0.4124564, 0.3575761, 0.1804375, // R
    0.2126729, 0.7151522, 0.0721750, // G
);

/// convert CSS stadard color name to Color
fn name_to_color(name: &str) -> Option<Color> {
    match name.to_lowercase().as_str() {
        "transparent" => Some(Color::new_srgb(0.0, 0.0, 0.0, 0.0)),
        "black" => Some(Color::new_srgb(0.0, 0.0, 0.0, 1.0)),
        "silver" => Some(Color::new_srgb(0.75, 0.75, 0.75, 1.0)),
        "gray" | "grey" => Some(Color::new_srgb(0.5, 0.5, 0.5, 1.0)),
        "white" => Some(Color::new_srgb(1.0, 1.0, 1.0, 1.0)),
        "maroon" => Some(Color::new_srgb(0.5, 0.0, 0.0, 1.0)),
        "red" => Some(Color::new_srgb(1.0, 0.0, 0.0, 1.0)),
        "purple" => Some(Color::new_srgb(0.5, 0.0, 0.5, 1.0)),
        "fuchsia" => Some(Color::new_srgb(1.0, 0.0, 1.0, 1.0)),
        "green" => Some(Color::new_srgb(0.0, 0.5, 0.0, 1.0)),
        "lime" => Some(Color::new_srgb(0.0, 1.0, 0.0, 1.0)),
        "olive" => Some(Color::new_srgb(0.5, 0.5, 0.0, 1.0)),
        "yellow" => Some(Color::new_srgb(1.0, 1.0, 0.0, 1.0)),
        "navy" => Some(Color::new_srgb(0.0, 0.0, 0.5, 1.0)),
        "blue" => Some(Color::new_srgb(0.0, 0.0, 1.0, 1.0)),
        "teal" => Some(Color::new_srgb(0.0, 0.5, 0.5, 1.0)),
        "aqua" => Some(Color::new_srgb(0.0, 1.0, 1.0, 1.0)),
        _ => {
            // try to parse as hex color
            if let Ok((r, g, b)) = parse_hex_color(name) {
                Some(Color::new_srgb(
                    r as f32 / 255.0,
                    g as f32 / 255.0,
                    b as f32 / 255.0,
                    1.0,
                ))
            } else {
                None
            }
        }
    }
}

// parse a hex color string (#RRGGBB or #RGB) into (r, g, b) u8 values
fn parse_hex_color(hex: &str) -> Result<(u8, u8, u8), &'static str> {
    let hex = hex.trim_start_matches('#');

    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).map_err(|_| "Invalid hex")?;
            let g = u8::from_str_radix(&hex[1..2], 16).map_err(|_| "Invalid hex")?;
            let b = u8::from_str_radix(&hex[2..3], 16).map_err(|_| "Invalid hex")?;
            // Multiply by 17 to expand 0xF to 0xFF (15 * 17 = 255)
            Ok((r * 17, g * 17, b * 17))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| "Invalid hex")?;
            let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| "Invalid hex")?;
            let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| "Invalid hex")?;
            Ok((r, g, b))
        }
        _ => Err("Invalid hex color"),
    }
}
