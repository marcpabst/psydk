use pyo3::prelude::*;

#[pyclass]
#[derive(Debug, Clone)]
pub struct ExperimentConfig {
    /// pedantic mode
    pub pedantic: bool,
    /// debug mode
    pub debug: bool,
    /// internal color format
    pub internal_color_depth: InternalColorDepth,
    /// internal color encoding
    pub internal_color_encoding: InternalColorEncoding,
    // output color format
    pub display_color_format: DisplayColorFormat,
}

impl ExperimentConfig {
    /// Create a new `ExperimentConfig` with the specified color depth and encoding.
    pub fn new_from_string(
        pedantic: bool,
        debug: bool,
        color_depth: &str,
        color_encoding: &str,
        output_color_format: &str,
    ) -> Result<Self, String> {
        let internal_color_depth = match color_depth {
            "unorm8" => InternalColorDepth::UNorm8,
            "unorm10" => InternalColorDepth::UNorm10,
            "f16" => InternalColorDepth::F16,
            _ => {
                return Err(format!(
                    "Unknown color depth: {}. Supported values are: unorm8, unorm10, f16",
                    color_depth
                ))
            }
        };

        let internal_color_encoding = match color_encoding {
            "linear" => InternalColorEncoding::Linear,
            "srgb" => InternalColorEncoding::Srgb,
            _ => {
                return Err(format!(
                    "Unknown color encoding: {}. Supported values are: linear, srgb",
                    color_encoding
                ))
            }
        };

        let output_color_format = match output_color_format {
            "rgb888unorm" => DisplayColorFormat::Rgba8Unorm,
            "bgra8888unorm" => DisplayColorFormat::Bgra8Unorm,
            "rgb101010unorm" => DisplayColorFormat::Rgb101010Unorm,
            _ => {
                return Err(format!(
                    "Unknown output color format: {}. Supported values are: rgb888unorm, rgb101010unorm",
                    output_color_format
                ))
            }
        };

        Ok(Self {
            pedantic,
            debug,
            internal_color_depth,
            internal_color_encoding,
            display_color_format: output_color_format,
        })
    }
}

#[pymethods]
impl ExperimentConfig {
    #[new]
    pub fn new(
        pedantic: bool,
        debug: bool,
        internal_color_depth: &str,
        internal_color_encoding: &str,
        display_color_format: &str,
    ) -> PyResult<Self> {
        Self::new_from_string(
            pedantic,
            debug,
            internal_color_depth,
            internal_color_encoding,
            display_color_format,
        )
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e))
    }
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        Self {
            pedantic: true,
            debug: false,
            internal_color_depth: InternalColorDepth::default(),
            internal_color_encoding: InternalColorEncoding::default(),
            display_color_format: DisplayColorFormat::default(),
        }
    }
}

/// Color formats used in the internal representations.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalColorDepth {
    /// 8-bit unsigned integer per channel
    UNorm8,
    /// 10-bit unsigned integer per channel
    UNorm10,
    #[default]
    /// 16-bit floating point per channel
    F16,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayColorFormat {
    /// 8-bit unsigned integer for red, green, blue.
    Rgba8Unorm,
    #[default]
    /// 8-bit unsigned integer in BGRA order.
    Bgra8Unorm,
    /// 10-bit unsigned integer for red, green, blue.
    Rgb101010Unorm,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalColorEncoding {
    #[default]
    /// RGB color space without transfer function (linear).
    Linear,
    /// RGB color space with sRGB transfer function
    Srgb,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub enum DisplayColorEncoding {
    /// Linear encoding.
    Linear,
    #[default]
    /// Colors encoded with sRGB transfer function.
    Srgb,
    /// Custom LUT encoding. Requires the internal encoding to be `Linear`.
    CustomLut(GammaLUT),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GammaLUT {
    /// Mapping from float -> 8-bit unsigned integer
    EightBit(Vec<u8>),
    /// Mapping from float -> 10-bit unsigned integer
    TenBit(Vec<u16>),
}

impl Into<renderer::color_formats::ColorEncoding> for InternalColorEncoding {
    fn into(self) -> renderer::color_formats::ColorEncoding {
        match self {
            InternalColorEncoding::Linear => renderer::color_formats::ColorEncoding::Linear,
            InternalColorEncoding::Srgb => renderer::color_formats::ColorEncoding::Srgb,
        }
    }
}

impl Into<renderer::color_formats::ColorFormat> for InternalColorDepth {
    fn into(self) -> renderer::color_formats::ColorFormat {
        match self {
            InternalColorDepth::UNorm8 => renderer::color_formats::ColorFormat::Rgba8,
            InternalColorDepth::UNorm10 => renderer::color_formats::ColorFormat::Rgba1010102,
            InternalColorDepth::F16 => renderer::color_formats::ColorFormat::RgbaF16,
        }
    }
}

impl Into<renderer::color_formats::ColorFormat> for DisplayColorFormat {
    fn into(self) -> renderer::color_formats::ColorFormat {
        match self {
            DisplayColorFormat::Rgba8Unorm => renderer::color_formats::ColorFormat::Rgba8,
            DisplayColorFormat::Bgra8Unorm => renderer::color_formats::ColorFormat::Bgra8,
            DisplayColorFormat::Rgb101010Unorm => renderer::color_formats::ColorFormat::Rgba1010102,
        }
    }
}

impl Into<wgpu::TextureFormat> for DisplayColorFormat {
    fn into(self) -> wgpu::TextureFormat {
        match self {
            DisplayColorFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            DisplayColorFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
            DisplayColorFormat::Rgb101010Unorm => wgpu::TextureFormat::Rgb10a2Unorm,
        }
    }
}
