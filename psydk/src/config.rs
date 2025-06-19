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
}

impl ExperimentConfig {
    /// Create a new `ExperimentConfig` with the specified color depth and encoding.
    pub fn new_from_string(
        pedantic: bool,
        debug: bool,
        color_depth: &str,
        color_encoding: &str,
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

        Ok(Self {
            pedantic,
            debug,
            internal_color_depth,
            internal_color_encoding,
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
    ) -> PyResult<Self> {
        Self::new_from_string(pedantic, debug, internal_color_depth, internal_color_encoding)
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
    #[default]
    /// 8-bit unsigned integer for red, green, blue.
    Rgb888Unorm,
    /// 10-bit unsigned integer for red, green, blue.
    Rgb101010Unorm,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
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
