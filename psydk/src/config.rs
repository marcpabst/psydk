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
    /// display color format
    pub display_color_format: DisplayColorFormat,
    /// display color encoding
    pub display_color_encoding: DisplayColorEncoding,
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        Self {
            pedantic: true,
            debug: false,
            internal_color_depth: InternalColorDepth::default(),
            internal_color_encoding: InternalColorEncoding::default(),
            display_color_format: DisplayColorFormat::default(),
            display_color_encoding: DisplayColorEncoding::default(),
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
    /// 16-bit unsigned integer per channel
    UNorm16,
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
    /// RGB color space with sRGB transfer function.
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
