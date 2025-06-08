/// Defines color formats.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorFormat {
    /// 8-bit unsigned integer per channel
    UNorm8,
    /// 10-bit unsigned integer per channel
    UNorm10,
    /// 16-bit unsigned integer per channel
    UNorm16,
    /// 16-bit floating point per channel
    Float16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorEncoding {
    /// RGB color space without transfer function (linear).
    Linear,
    /// RGB color space with sRGB transfer function.
    Srgb,
}
