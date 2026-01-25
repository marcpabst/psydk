use crate::visual::colors::display::{
    CustomDisplayCharacteristics, DisplayCharacteristics, GenericDisplayCharacteristics,
};
use pyo3::prelude::*;
use renderer::color_formats::ColorFormat;
use std::{fs::File, io::BufReader, sync::Arc};

#[pyclass]
#[derive(Debug, Clone)]
/// Configuration for an experiment.
pub struct ExperimentConfig {
    /// pedantic mode
    pub pedantic: bool,
    /// debug mode
    pub debug: bool,
    /// internel color depth
    pub internal_color_type: ColorType,
    /// are internal colors linear?
    pub internal_colors_are_linear: bool,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorType {
    /// 8-bit unsigned integer per channel
    #[default]
    EightBit,
    /// 10-bit unsigned integer per channel
    TenBit,
    /// 16-bit floating point per channel
    SixteenBitFloat,
    /// 32-bit floating point per channel
    ThirtyTwoBitFloat,
}

impl ColorType {
    /// Convert to renderer ColorFormat
    pub fn to_color_format(&self) -> ColorFormat {
        match self {
            ColorType::EightBit => ColorFormat::Rgba8,
            ColorType::TenBit => ColorFormat::Rgba10,
            ColorType::SixteenBitFloat => ColorFormat::RgbaF16,
            ColorType::ThirtyTwoBitFloat => panic!("32F color format not supported in renderer"),
        }
    }
}

impl std::str::FromStr for ColorType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "8U" => Ok(ColorType::EightBit),
            "10U" => Ok(ColorType::TenBit),
            "16F" => Ok(ColorType::SixteenBitFloat),
            "32F" => Ok(ColorType::ThirtyTwoBitFloat),
            _ => Err(format!("Invalid color type: {}", s)),
        }
    }
}

#[pyclass]
#[derive(Clone)]
/// Configuration for the display.
pub struct WindowConfig {
    /// The surface color depth to use (normally 8, 10, or 12 bit). Psydk will
    /// throw an error if you try to use a color depth that is not
    /// supported. Note that on some systems, chosing a higher bit depth
    /// than supported by the display will result in temporal dithering being
    /// applied by the operating system/graphics driver.
    pub surface_color_depth: ColorType,
    // /// How to tag the surface color space. Depending on your
    // /// operating system and display, this might affect rendering.
    // /// If not set, psydk will try to make sure that the display's
    // /// native color space is used. Note that pdysk by default
    // /// does not make any provision for high dynamic range.
    // pub display_color_space: ColorSpace,
    /// Display characteristics. This defines transformations to accurately
    /// display colors and luminance on the display.
    pub display_characteristics: Arc<dyn DisplayCharacteristics + Send + Sync>,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            surface_color_depth: ColorType::EightBit,
            display_characteristics: Arc::new(GenericDisplayCharacteristics::default()),
        }
    }
}

#[pymethods]
impl ExperimentConfig {
    #[new]
    #[pyo3(signature = (pedantic=true, debug=false, internal_color_type=None, internal_colors_are_linear=false))]
    /// Defines how your experiment will be run.
    ///
    /// # Parameters
    /// - `pedantic`: If true, psydk will be more strict about checking for errors.
    /// - `debug`: If true, psydk will print debug information.
    /// - `internal_color_type`: The internal color depth to use. Must be one of '8U', '10U', or '16F'. Defaults to '8U'.
    ///    If you use a display with a higher color depth than 8-bit, you should set this to '10U' or '16F'.
    pub fn new(
        pedantic: bool,
        debug: bool,
        internal_color_type: Option<&str>,
        internal_colors_are_linear: bool,
    ) -> PyResult<Self> {
        Ok(Self {
            pedantic,
            debug,
            internal_color_type: if let Some(color_type_str) = internal_color_type {
                color_type_str
                    .parse()
                    .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e))?
            } else {
                ColorType::default()
            },
            internal_colors_are_linear,
        })
    }
}

#[pymethods]
impl WindowConfig {
    #[new]
    #[pyo3(signature = (
        surface_color_type=None,
        calibration_file=None
    ))]
    /// Defines the the properties the colour/luminance output of the display.
    /// You can either provide a calibration file or assume a generic display.
    ///
    /// # Parameters
    /// - `surface_color_type`: The surface color depth to use. Must be one of '8U', '10U', or '12U'. Defaults to '8U'.
    /// - `calibration_file`: Path to a display calibration file in JSON format.
    pub fn new(surface_color_type: Option<&str>, calibration_file: Option<&str>) -> PyResult<Self> {
        let display_characteristics: Arc<dyn DisplayCharacteristics + Send + Sync> =
            if let Some(file_path) = calibration_file {
                Arc::new(CustomDisplayCharacteristics::from_file(file_path).map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyIOError, _>(format!("Failed to load calibration file: {}", e))
                })?)
            } else {
                Arc::new(GenericDisplayCharacteristics::default())
            };

        let surface_color_type = if let Some(color_type_str) = surface_color_type {
            color_type_str
                .parse()
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e))?
        } else {
            ColorType::default()
        };

        Ok(Self {
            surface_color_depth: surface_color_type,
            display_characteristics: display_characteristics,
        })
    }
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        Self {
            pedantic: true,
            debug: false,
            internal_color_type: ColorType::default(),
            internal_colors_are_linear: false,
        }
    }
}
