//! Display Characteristics Module
//!
//! This module defines the traits and structures for representing and working with
//! display color characteristics. It provides functionality for converting between
//! different color spaces (XYZ, RGB) with various display types.
//!
//! ## Key Components
//!
//! ### DisplayCharacteristics Trait
//! The core trait that defines the interface for all display types. It provides methods for:
//! - Converting between XYZ and RGB color spaces
//! - Accessing display properties like white point, luminance, and transfer functions
//! - Checking if inverse transformations are supported
//!
//! ### EOTF Enum
//! Represents various electro-optical transfer functions (gamma curves):
//! - SRGB: Standard sRGB transfer function
//! - Linear: No gamma correction
//! - Gamma: Pure power function
//! - LookUpTable: Custom curve defined by values
//!
//! ### GenericDisplayCharacteristics
//! A concrete implementation of DisplayCharacteristics that supports:
//! - Standard sRGB display (D65 white point, 2.2 gamma)
//! - Linear sRGB display (D65 white point, 1.0 gamma)
//! - Display P3 (D65 white point, 2.2 gamma)
//! - Custom displays with user-defined parameters

use na::Matrix3;
use na::Vector3;
use na::Vector4;
use nalgebra as na;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::default::Default;
use std::sync::Arc;

#[derive(Clone)]
pub struct Display {
    pub characteristics: Arc<dyn DisplayCharacteristics>,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            characteristics: Arc::new(GenericDisplayCharacteristics::default()),
        }
    }
}

impl Display {
    /// Create a new display with given name and characteristics
    pub fn new(characteristics: Arc<dyn DisplayCharacteristics>) -> Self {
        Self { characteristics }
    }

    /// Load display characteristics from a file
    pub fn from_file(path: &str) -> Result<Self, std::io::Error> {
        let characteristics = CustomDisplayCharacteristics::from_file(path)?;
        Ok(Self {
            characteristics: Arc::new(characteristics),
        })
    }
}

pub trait DisplayCharacteristics {
    /// Returns the name of the display device
    fn name(&self) -> &str;
    /// Convert XYZ to device RGB
    fn xyza_to_device_rgba(&self, xyz: &Vector4<f32>) -> Vector4<f32>;
    /// Convert XYZ to linear device RGB
    fn xyza_to_linear_device_rgba(&self, xyz: &Vector4<f32>) -> Vector4<f32>;
    /// Convert linear device RGB to device RGB
    fn linear_device_rgba_to_device_rgba(&self, linear_rgba: &Vector4<f32>) -> Vector4<f32>;
    /// Returns true if supports inverse transformation from RGB to XYZ
    fn supports_inverse(&self) -> bool;
    /// Convert device RGB to XYZ (if supported)
    fn device_rgba_to_xyza(&self, rgba: Vector4<f32>) -> Option<Vector4<f32>>;
    /// Returns the seperable EOTF for the display (or None if the characteristics do not support it)
    fn eotf(&self) -> Option<[EOTF; 3]>;
    /// Applies the EOTF to an RGBA vector
    fn apply_eotf(&self, rgba: &Vector4<f32>) -> Vector4<f32>;
    /// Applies the inverse EOTF to an RGBA vector
    fn apply_inverse_eotf(&self, rgba: &Vector4<f32>) -> Option<Vector4<f32>>;
    /// Returns the white point of the display in CIE xy chromaticity coordinates
    fn white_point(&self) -> (f32, f32);
    /// Returns the absolute luminance of the display's white point in cd/m^2 (Y value in CIE xyY/XYZ)
    fn white_point_luminance(&self) -> Option<f32>;
    /// Returns the spectral power distribution of the display's primaries (if available)
    /// If the display is reasonably well-behaved, this can be used for LMS->RGB conversion
    fn spectral_primaries(&self) -> Option<[(f32, f32); 3]>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum EOTF {
    /// Standard sRGB transfer function
    #[serde(rename = "srgb")]
    SRGB,
    /// Linear transfer function (gamma = 1.0)
    #[serde(rename = "linear")]
    Linear,
    /// Pure power function with specified gamma
    #[serde(rename = "gamma")]
    Gamma(f32),
    /// Custom transfer function defined by a lookup table + an inverse lookup table (for inverse EOTF)
    #[serde(rename = "lut")]
    LookUpTable((Vec<f32>, Vec<f32>)),
}

impl EOTF {
    /// Apply the EOTF to a linear component
    pub fn apply(&self, linear_component: f32) -> f32 {
        match self {
            EOTF::SRGB => {
                if linear_component <= 0.0031308 {
                    12.92 * linear_component
                } else {
                    1.055 * linear_component.powf(1.0 / 2.4) - 0.055
                }
            }
            EOTF::Linear => linear_component,
            EOTF::Gamma(gamma) => linear_component.powf(1.0 / gamma),
            EOTF::LookUpTable(lut) => {
                let index = (linear_component.clamp(0.0, 1.0) * (lut.0.len() - 1) as f32).round() as usize;
                lut.0[index]
            }
        }
    }

    pub fn apply_inverse(&self, component: f32) -> Option<f32> {
        match self {
            EOTF::SRGB => {
                if component <= 0.04045 {
                    Some(component / 12.92)
                } else {
                    Some(((component + 0.055) / 1.055).powf(2.4))
                }
            }
            EOTF::Linear => Some(component),
            EOTF::Gamma(gamma) => Some(component.powf(*gamma)),
            EOTF::LookUpTable(lut) => {
                let index = (component.clamp(0.0, 1.0) * (lut.1.len() - 1) as f32).round() as usize;
                Some(lut.1[index])
            }
        }
    }

    pub fn create_lut(&self, n: usize) -> Vec<f32> {
        // generate a LUT with n entries for the EOTF by applying the EOTF to n evenly spaced linear values
        (0..n).map(|i| self.apply(i as f32 / (n - 1) as f32)).collect()
    }

    pub fn create_inverse_lut(&self, n: usize) -> Vec<f32> {
        // generate an inverse LUT with n entries for the EOTF by applying the inverse EOTF to n evenly spaced non-linear values
        (0..n)
            .map(|i| self.apply_inverse(i as f32 / (n - 1) as f32).unwrap_or(0.0))
            .collect()
    }
}

// We provide `GenericDisplayCharacteristics` as a flexible implementation of `DisplayCharacteristics`
// that can represent a variety of common display types as well as custom configurations.

#[derive(Debug, Clone)]
/// A generic RGB display with configurable characteristics.
/// Assumes a 3x3 matrix for XYZ to RGB conversion and a simple gamma curve.
pub struct GenericDisplayCharacteristics {
    pub transform: Matrix3<f32>,
    pub gamma: f32,
    pub white_point: (f32, f32),
    pub max_luminance: f32,
    pub min_luminance: f32,
}

impl Default for GenericDisplayCharacteristics {
    fn default() -> Self {
        Self::new_srgb(0.1, 100.0)
    }
}

impl GenericDisplayCharacteristics {
    /// Create a custom display with specified parameters
    pub fn new(
        transform: Matrix3<f32>,
        gamma: f32,
        white_point: (f32, f32),
        max_luminance: f32,
        min_luminance: f32,
    ) -> Self {
        Self {
            transform,
            gamma,
            white_point,
            max_luminance,
            min_luminance,
        }
    }
    /// Standard sRGB display (gamma = 2.2)
    pub fn new_srgb(min_luminance: f32, max_luminance: f32) -> Self {
        Self {
            /// XYZ to RGB transform matrix
            transform: Matrix3::new(
                3.2406, -1.5372, -0.4986, // R
                -0.9689, 1.8758, 0.0415, // G
                0.0557, -0.2040, 1.0570, // B
            ),
            white_point: (0.3127, 0.3290), // D65
            gamma: 2.2,
            max_luminance,
            min_luminance,
        }
    }

    /// Standard linear sRGB display (gamma = 1.0)
    pub fn new_linear_srgb() -> Self {
        Self {
            transform: Matrix3::new(
                3.2406, -1.5372, -0.4986, // R
                -0.9689, 1.8758, 0.0415, // G
                0.0557, -0.2040, 1.0570, // B
            ),
            white_point: (0.3127, 0.3290), // D65
            gamma: 1.0,
            max_luminance: 100.0,
            min_luminance: 0.1,
        }
    }

    /// Standard display with Display P3 primaries (gamma = 2.2)
    pub fn new_display_p3() -> Self {
        Self {
            transform: Matrix3::new(
                2.4935, -0.9313, -0.4027, // R
                -0.8295, 1.7627, 0.0236, // G
                0.0357, -0.0762, 0.9569, // B
            ),
            white_point: (0.3127, 0.3290), // D65
            gamma: 2.2,
            max_luminance: 100.0,
            min_luminance: 0.1,
        }
    }
}

impl DisplayCharacteristics for GenericDisplayCharacteristics {
    fn name(&self) -> &str {
        "Generic RGB Display"
    }

    fn xyza_to_device_rgba(&self, xyz: &Vector4<f32>) -> Vector4<f32> {
        let linear_rgb = self.transform * Vector3::new(xyz.x, xyz.y, xyz.z);
        let rgb =
            self.linear_device_rgba_to_device_rgba(&Vector4::new(linear_rgb.x, linear_rgb.y, linear_rgb.z, xyz.w));
        Vector4::new(rgb.x, rgb.y, rgb.z, xyz.w)
    }

    fn xyza_to_linear_device_rgba(&self, xyz: &Vector4<f32>) -> Vector4<f32> {
        let linear_rgb = self.transform * Vector3::new(xyz.x, xyz.y, xyz.z);
        Vector4::new(linear_rgb.x, linear_rgb.y, linear_rgb.z, xyz.w)
    }

    fn linear_device_rgba_to_device_rgba(&self, linear_rgba: &Vector4<f32>) -> Vector4<f32> {
        let linear_rgb = Vector3::new(linear_rgba.x, linear_rgba.y, linear_rgba.z);
        Vector4::new(
            linear_rgb.x.powf(1.0 / self.gamma),
            linear_rgb.y.powf(1.0 / self.gamma),
            linear_rgb.z.powf(1.0 / self.gamma),
            linear_rgba.w,
        )
    }

    fn supports_inverse(&self) -> bool {
        true
    }

    fn eotf(&self) -> Option<[EOTF; 3]> {
        Some([
            EOTF::Gamma(self.gamma),
            EOTF::Gamma(self.gamma),
            EOTF::Gamma(self.gamma),
        ])
    }

    fn apply_eotf(&self, rgba: &Vector4<f32>) -> Vector4<f32> {
        Vector4::new(
            rgba.x.powf(1.0 / self.gamma),
            rgba.y.powf(1.0 / self.gamma),
            rgba.z.powf(1.0 / self.gamma),
            rgba.w,
        )
    }

    fn apply_inverse_eotf(&self, rgba: &Vector4<f32>) -> Option<Vector4<f32>> {
        Some(Vector4::new(
            rgba.x.powf(self.gamma),
            rgba.y.powf(self.gamma),
            rgba.z.powf(self.gamma),
            rgba.w,
        ))
    }

    fn white_point(&self) -> (f32, f32) {
        self.white_point
    }

    fn white_point_luminance(&self) -> Option<f32> {
        Some(self.max_luminance)
    }

    fn spectral_primaries(&self) -> Option<[(f32, f32); 3]> {
        None
    }

    fn device_rgba_to_xyza(&self, rgba: Vector4<f32>) -> Option<Vector4<f32>> {
        let linear_rgb = Vector3::new(
            rgba.x.powf(self.gamma),
            rgba.y.powf(self.gamma),
            rgba.z.powf(self.gamma),
        );
        let inv_transform = self.transform.try_inverse()?;
        let xyz = inv_transform * linear_rgb;
        Some(Vector4::new(xyz.x, xyz.y, xyz.z, rgba.w))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Custom display characteristics defined by user parameters
pub struct CustomDisplayCharacteristics {
    pub name: String,
    pub transform: Matrix3<f32>,
    pub eotf: [EOTF; 3],
    pub white_point: (f32, f32),
}

impl CustomDisplayCharacteristics {
    /// Create a new custom display with specified parameters
    pub fn new(name: String, transform: Matrix3<f32>, eotf: [EOTF; 3], white_point: (f32, f32)) -> Self {
        Self {
            name,
            transform,
            eotf,
            white_point,
        }
    }

    // Create a dummy custom display with identity transform and linear LUT
    pub fn dummy() -> Self {
        let linear_lut_r = EOTF::LookUpTable((
            (0..16384).map(|i| i as f32 / 16383.0).collect(),
            (0..16384).map(|i| 1.0 - (i as f32 / 16383.0)).collect(),
        ));
        let linear_lut_g = EOTF::LookUpTable((
            (0..16384).map(|i| i as f32 / 16383.0).collect(),
            (0..16384).map(|i| 1.0 - (i as f32 / 16383.0)).collect(),
        ));
        let linear_lut_b = EOTF::LookUpTable((
            (0..16384).map(|i| i as f32 / 16383.0).collect(),
            (0..16384).map(|i| 1.0 - (i as f32 / 16383.0)).collect(),
        ));

        let lut = [linear_lut_r, linear_lut_g, linear_lut_b];

        Self {
            name: "Dummy Display".to_string(),
            transform: Matrix3::identity(),
            eotf: lut,
            white_point: (0.3127, 0.3290), // D65
        }
    }

    /// Load custom display characteristics from a JSON file
    pub fn from_file(path: &str) -> Result<Self, std::io::Error> {
        let file_content = std::fs::read_to_string(path)?;
        let characteristics: CustomDisplayCharacteristics = serde_json::from_str(&file_content)?;
        Ok(characteristics)
    }
}

impl DisplayCharacteristics for CustomDisplayCharacteristics {
    fn name(&self) -> &str {
        &self.name
    }

    fn xyza_to_device_rgba(&self, xyz: &Vector4<f32>) -> Vector4<f32> {
        let linear_rgb = self.transform * Vector3::new(xyz.x, xyz.y, xyz.z);
        let rgb = self.apply_eotf(&Vector4::new(linear_rgb.x, linear_rgb.y, linear_rgb.z, xyz.w));
        Vector4::new(rgb.x, rgb.y, rgb.z, xyz.w)
    }

    fn xyza_to_linear_device_rgba(&self, xyz: &Vector4<f32>) -> Vector4<f32> {
        let linear_rgb = self.transform * Vector3::new(xyz.x, xyz.y, xyz.z);
        Vector4::new(linear_rgb.x, linear_rgb.y, linear_rgb.z, xyz.w)
    }

    fn linear_device_rgba_to_device_rgba(&self, linear_rgba: &Vector4<f32>) -> Vector4<f32> {
        self.apply_eotf(linear_rgba)
    }

    fn supports_inverse(&self) -> bool {
        true
    }

    fn device_rgba_to_xyza(&self, _rgba: Vector4<f32>) -> Option<Vector4<f32>> {
        None
    }

    fn eotf(&self) -> Option<[EOTF; 3]> {
        Some(self.eotf.clone())
    }

    fn apply_eotf(&self, rgba: &Vector4<f32>) -> Vector4<f32> {
        Vector4::new(
            self.eotf[0].apply(rgba.x),
            self.eotf[1].apply(rgba.y),
            self.eotf[2].apply(rgba.z),
            rgba.w,
        )
    }

    fn apply_inverse_eotf(&self, rgba: &Vector4<f32>) -> Option<Vector4<f32>> {
        Some(Vector4::new(
            self.eotf[0].apply_inverse(rgba.x)?,
            self.eotf[1].apply_inverse(rgba.y)?,
            self.eotf[2].apply_inverse(rgba.z)?,
            rgba.w,
        ))
    }

    fn white_point(&self) -> (f32, f32) {
        self.white_point
    }
    fn white_point_luminance(&self) -> Option<f32> {
        None
    }
    fn spectral_primaries(&self) -> Option<[(f32, f32); 3]> {
        None
    }
}
