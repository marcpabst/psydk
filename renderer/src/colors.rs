use crate::color_formats::ColorEncoding;

#[derive(Debug, Clone, Copy)]
/// A color with red, green, blue, and alpha components.
pub struct RGBA {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
    encoding: ColorEncoding,
}

impl RGBA {
    pub fn new_linear(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r,
            g,
            b,
            a,
            encoding: ColorEncoding::Linear,
        }
    }

    pub fn new(r: f32, g: f32, b: f32, a: f32, encoding: ColorEncoding) -> Self {
        Self { r, g, b, a, encoding }
    }

    /// Convert to an RGBA color with sRGB encoding.
    pub fn as_srgba(&self) -> (f32, f32, f32, f32) {
        (lin2srgb(self.r), lin2srgb(self.g), lin2srgb(self.b), self.a)
    }

    pub fn color_encoding(&self) -> ColorEncoding {
        self.encoding
    }

    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
        encoding: ColorEncoding::Srgb,
    };

    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
        encoding: ColorEncoding::Srgb,
    };

    pub const RED: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
        encoding: ColorEncoding::Srgb,
    };

    pub const GREEN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
        encoding: ColorEncoding::Srgb,
    };

    pub const BLUE: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
        encoding: ColorEncoding::Srgb,
    };

    pub const YELLOW: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
        encoding: ColorEncoding::Srgb,
    };

    pub const CYAN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
        encoding: ColorEncoding::Srgb,
    };

    pub const MAGENTA: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
        encoding: ColorEncoding::Srgb,
    };

    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
        encoding: ColorEncoding::Srgb,
    };

    pub const GRAY: Self = Self {
        r: 0.5,
        g: 0.5,
        b: 0.5,
        a: 1.0,
        encoding: ColorEncoding::Srgb,
    };
}

fn lin2srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}
