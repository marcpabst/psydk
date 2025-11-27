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
//! - ICCParametric7: 7-parameter ICC curve
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
use nalgebra as na;

pub trait DisplayCharacteristics {
    /// Returns the name of the display device
    fn name(&self) -> &str;
    /// Convert XYZ to device RGB
    fn xyz_to_rgb(&self, xyz: &Vector3<f32>) -> Vector3<f32>;
    /// Convert XYZ to linear device RGB
    fn xyz_to_linear_rgb(&self, xyz: &Vector3<f32>) -> Vector3<f32>;
    /// Convert linear device RGB to device RGB
    fn linear_rgb_to_rgb(&self, linear_rgb: &Vector3<f32>) -> Vector3<f32>;
    /// Returns true if supports inverse transformation from RGB to XYZ
    fn supports_inverse(&self) -> bool;
    /// Convert device RGB to XYZ (if supported)
    fn rgb_to_xyz(&self, rgb: &Vector3<f32>) -> Option<Vector3<f32>>;
    /// Returns the seperable EOTF for the display (or None if the characteristics do not support it)
    fn eotf(&self) -> Option<[EOTF; 3]>;
    /// Returns the white point of the display in CIE xy chromaticity coordinates
    fn white_point(&self) -> (f32, f32);
    /// Returns the absolute luminance of the display's white point in cd/m^2 (Y value in CIE xyY/XYZ)
    fn white_point_luminance(&self) -> Option<f32>;
    /// Returns the spectral power distribution of the display's primaries (if available)
    /// If the display is reasonably well-behaved, this can be used for LMS->RGB conversion
    fn spectral_primaries(&self) -> Option<[(f32, f32); 3]>;
}

#[derive(Debug, Clone)]
pub enum EOTF {
    /// Standard sRGB transfer function
    SRGB,
    /// Linear transfer function (gamma = 1.0)
    Linear,
    /// Pure power function with specified gamma
    Gamma(f32),
    /// ICC 7-parameter transfer function
    ICCParametric7(f32, f32, f32, f32, f32, f32, f32),
    /// Custom transfer function defined by a lookup table
    LookUpTable(Vec<f32>),
}

pub struct GenericDisplayCharacteristics {
    pub transform: Matrix3<f32>,
    pub gamma: f32,
    pub white_point: (f32, f32),
    pub max_luminance: f32,
    pub min_luminance: f32,
}

impl Default for GenericDisplayCharacteristics {
    fn default() -> Self {
        Self::new_srgb()
    }
}

impl GenericDisplayCharacteristics {
    /// Standard sRGB display (gamma = 2.2)
    pub fn new_srgb() -> Self {
        Self {
            /// XYZ to RGB transform matrix
            transform: Matrix3::new(
                3.2406, -1.5372, -0.4986, // R
                -0.9689, 1.8758, 0.0415, // G
                0.0557, -0.2040, 1.0570, // B
            ),
            white_point: (0.3127, 0.3290), // D65
            gamma: 2.2,
            max_luminance: 100.0,
            min_luminance: 0.1,
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
                2.4935, -0.9314, -0.4027, // R
                -0.8295, 1.7627, 0.0236, // G
                0.0357, -0.0762, 0.9569, // B
            ),
            white_point: (0.3127, 0.3290), // D65
            gamma: 2.2,
            max_luminance: 100.0,
            min_luminance: 0.1,
        }
    }

    /// Create a custom display with specified parameters
    pub fn new_custom(
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
}

impl DisplayCharacteristics for GenericDisplayCharacteristics {
    fn name(&self) -> &str {
        "Generic RGB Display"
    }

    fn xyz_to_rgb(&self, xyz: &Vector3<f32>) -> Vector3<f32> {
        let linear_rgb = self.transform * xyz;
        self.linear_rgb_to_rgb(&linear_rgb)
    }

    fn xyz_to_linear_rgb(&self, xyz: &Vector3<f32>) -> Vector3<f32> {
        self.transform * xyz
    }

    fn linear_rgb_to_rgb(&self, linear_rgb: &Vector3<f32>) -> Vector3<f32> {
        Vector3::new(
            linear_rgb.x.powf(1.0 / self.gamma),
            linear_rgb.y.powf(1.0 / self.gamma),
            linear_rgb.z.powf(1.0 / self.gamma),
        )
    }

    fn supports_inverse(&self) -> bool {
        true
    }

    fn rgb_to_xyz(&self, rgb: &Vector3<f32>) -> Option<Vector3<f32>> {
        let linear_rgb = Vector3::new(rgb.x.powf(self.gamma), rgb.y.powf(self.gamma), rgb.z.powf(self.gamma));
        let inv_transform = self.transform.try_inverse()?;
        Some(inv_transform * linear_rgb)
    }

    fn eotf(&self) -> Option<[EOTF; 3]> {
        Some([
            EOTF::Gamma(self.gamma),
            EOTF::Gamma(self.gamma),
            EOTF::Gamma(self.gamma),
        ])
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32;

    #[test]
    fn test_srgb_display_creation() {
        let display = GenericDisplayCharacteristics::new_srgb();
        assert_eq!(display.name(), "Generic RGB Display");
        assert_eq!(display.white_point(), (0.3127, 0.3290)); // D65 white point
        assert!((display.gamma - 2.2).abs() < f32::EPSILON);
        assert_eq!(display.white_point_luminance(), Some(100.0));
        assert!(display.supports_inverse());
        assert!(display.eotf().is_some());
        assert!(display.spectral_primaries().is_none());
    }

    #[test]
    fn test_linear_srgb_display_creation() {
        let display = GenericDisplayCharacteristics::new_linear_srgb();
        assert_eq!(display.name(), "Generic RGB Display");
        assert_eq!(display.white_point(), (0.3127, 0.3290)); // D65 white point
        assert!((display.gamma - 1.0).abs() < f32::EPSILON);
        assert_eq!(display.white_point_luminance(), Some(100.0));
    }

    #[test]
    fn test_display_p3_display_creation() {
        let display = GenericDisplayCharacteristics::new_display_p3();
        assert_eq!(display.name(), "Generic RGB Display");
        assert_eq!(display.white_point(), (0.3127, 0.3290)); // D65 white point
        assert!((display.gamma - 2.2).abs() < f32::EPSILON);
        assert_eq!(display.white_point_luminance(), Some(100.0));
    }

    #[test]
    fn test_custom_display_creation() {
        let custom_transform = Matrix3::new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
        let display = GenericDisplayCharacteristics::new_custom(custom_transform, 2.4, (0.33, 0.33), 80.0, 0.05);
        assert_eq!(display.white_point(), (0.33, 0.33));
        assert!((display.gamma - 2.4).abs() < f32::EPSILON);
        assert_eq!(display.white_point_luminance(), Some(80.0));
    }

    #[test]
    fn test_xyz_to_linear_rgb() {
        let display = GenericDisplayCharacteristics::new_srgb();
        let xyz = Vector3::new(0.5, 0.5, 0.5);
        let linear_rgb = display.xyz_to_linear_rgb(&xyz);

        // Just check that we get a result - exact values depend on the transform matrix
        assert!(linear_rgb.x >= 0.0);
        assert!(linear_rgb.y >= 0.0);
        assert!(linear_rgb.z >= 0.0);
    }

    #[test]
    fn test_xyz_to_rgb() {
        let display = GenericDisplayCharacteristics::new_srgb();
        let xyz = Vector3::new(0.5, 0.5, 0.5);
        let rgb = display.xyz_to_rgb(&xyz);

        // Check that we get a valid RGB value
        assert!(rgb.x >= 0.0);
        assert!(rgb.y >= 0.0);
        assert!(rgb.z >= 0.0);
    }

    #[test]
    fn test_linear_rgb_to_rgb() {
        let display = GenericDisplayCharacteristics::new_srgb();
        let linear_rgb = Vector3::new(0.5, 0.5, 0.5);
        let rgb = display.linear_rgb_to_rgb(&linear_rgb);

        // For gamma=2.2, the result should be approximately 0.5^(1/2.2)
        let expected = 0.5f32.powf(1.0 / 2.2);
        assert!((rgb.x - expected).abs() < 1e-6);
        assert!((rgb.y - expected).abs() < 1e-6);
        assert!((rgb.z - expected).abs() < 1e-6);
    }

    #[test]
    fn test_rgb_to_xyz() {
        let display = GenericDisplayCharacteristics::new_srgb();
        let rgb = Vector3::new(0.5, 0.5, 0.5);
        let xyz = display.rgb_to_xyz(&rgb);

        // Just check that we get a valid result
        assert!(xyz.is_some());
        let xyz = xyz.unwrap();
        assert!(xyz.x >= 0.0);
        assert!(xyz.y >= 0.0);
        assert!(xyz.z >= 0.0);
    }

    #[test]
    fn test_roundtrip_xyz_rgb_xyz() {
        let display = GenericDisplayCharacteristics::new_srgb();
        let original_xyz = Vector3::new(0.3, 0.5, 0.2);

        // Convert XYZ to RGB and back
        let rgb = display.xyz_to_rgb(&original_xyz);
        let converted_xyz = display.rgb_to_xyz(&rgb).unwrap();

        // The values should be close but not exactly equal due to gamma corrections
        assert!((original_xyz.x - converted_xyz.x).abs() < 0.01);
        assert!((original_xyz.y - converted_xyz.y).abs() < 0.01);
        assert!((original_xyz.z - converted_xyz.z).abs() < 0.01);
    }

    #[test]
    fn test_roundtrip_rgb_xyz_rgb() {
        let display = GenericDisplayCharacteristics::new_srgb();
        let original_rgb = Vector3::new(0.3, 0.5, 0.2);

        // Convert RGB to XYZ and back
        let xyz = display.rgb_to_xyz(&original_rgb).unwrap();
        let converted_rgb = display.xyz_to_rgb(&xyz);

        // The values should be close but not exactly equal due to gamma corrections
        assert!((original_rgb.x - converted_rgb.x).abs() < 0.01);
        assert!((original_rgb.y - converted_rgb.y).abs() < 0.01);
        assert!((original_rgb.z - converted_rgb.z).abs() < 0.01);
    }

    #[test]
    fn test_linear_roundtrip() {
        let display = GenericDisplayCharacteristics::new_srgb();
        let original_xyz = Vector3::new(0.3, 0.5, 0.2);

        // Convert XYZ to linear RGB and back
        let linear_rgb = display.xyz_to_linear_rgb(&original_xyz);
        let inv_transform = display.transform.try_inverse().unwrap();
        let converted_xyz = inv_transform * linear_rgb;

        // For linear transformations, the roundtrip should be exact
        assert!((original_xyz.x - converted_xyz.x).abs() < f32::EPSILON);
        assert!((original_xyz.y - converted_xyz.y).abs() < f32::EPSILON);
        assert!((original_xyz.z - converted_xyz.z).abs() < f32::EPSILON);
    }

    #[test]
    fn test_eotf() {
        let display = GenericDisplayCharacteristics::new_srgb();
        let eotf = display.eotf().unwrap();

        // For sRGB, all three channels should have the same gamma
        if let (EOTF::Gamma(gamma1), EOTF::Gamma(gamma2), EOTF::Gamma(gamma3)) = (&eotf[0], &eotf[1], &eotf[2]) {
            assert!((gamma1 - 2.2).abs() < f32::EPSILON);
            assert!((gamma2 - 2.2).abs() < f32::EPSILON);
            assert!((gamma3 - 2.2).abs() < f32::EPSILON);
        } else {
            panic!("Expected Gamma EOTF");
        }
    }

    #[test]
    fn test_white_point_luminance() {
        let display = GenericDisplayCharacteristics::new_srgb();
        assert_eq!(display.white_point_luminance(), Some(100.0));

        let linear_display = GenericDisplayCharacteristics::new_linear_srgb();
        assert_eq!(linear_display.white_point_luminance(), Some(100.0));
    }

    #[test]
    fn test_default() {
        let display = GenericDisplayCharacteristics::default();
        assert_eq!(display.white_point(), (0.3127, 0.3290)); // D65
        assert!((display.gamma - 2.2).abs() < f32::EPSILON);
    }
}
