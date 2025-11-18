/// Defines color formats.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorFormat {
    /// 8-bit unsigned integer per channel
    Rgba8,
    /// 8-bit unsigned integer per channel in brga order
    /// 10-bit unsigned integer per channel + 2-bit alpha
    Rgba10,
    /// 16-bit floating point per channel
    RgbaF16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorEncoding {
    /// RGB color space without transfer function (linear).
    Linear,
    /// RGB color space with sRGB transfer function.
    Srgb,
}
