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
