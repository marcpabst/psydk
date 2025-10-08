use crate::visual::colors::{
    display_charactersitics::GenericDisplayCharacteristics, psydk_1::Psydk1DisplayCharacteristics,
    DisplayCharacteristics,
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
    pub internal_color_depth: PixelDepth,
    /// are internal colors in the display's color space
    pub internal_colors_are_in_display_color_space: bool,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelDepth {
    /// 8-bit unsigned integer per channel
    #[default]
    EightBit,
    /// 10-bit unsigned integer per channel
    TenBit,
    /// 16-bit floating point per channel
    SixteenBitFloat,
}

#[pyclass]
#[derive(Clone)]
/// Configuration for the display.
pub struct DisplayConfig {
    /// The surface color depth to use (normally 8, 10, or 12 bit). Psydk will
    /// throw an error if you try to use a color depth that is not
    /// supported. Note that on some systems, chosing a higher bit depth
    /// than supported by the display will result in temporal dithering being
    /// applied by the operating system/graphics driver.
    pub surface_color_depth: PixelDepth,
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

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            surface_color_depth: PixelDepth::EightBit,
            // display_color_space: ColorSpace::Native,
            display_characteristics: Arc::new(GenericDisplayCharacteristics::default()),
        }
    }
}

#[pymethods]
impl ExperimentConfig {
    #[new]
    #[pyo3(signature = (pedantic=true, debug=false, internal_color_depth=None))]
    /// Defines how your experiment will be run.
    ///
    /// # Parameters
    /// - `pedantic`: If true, psydk will be more strict about checking for errors.
    /// - `debug`: If true, psydk will print debug information.
    /// - `internal_color_depth`: The internal color depth to use. Must be one of '8U', '10U', or '16F'. Defaults to '8U'.
    ///    If you use a display with a higher color depth than 8-bit, you should set this to '10U' or '16F'.
    pub fn new(pedantic: bool, debug: bool, internal_color_depth: Option<&str>) -> PyResult<Self> {
        Ok(Self {
            pedantic,
            debug,
            internal_color_depth: match internal_color_depth {
                Some("8U") => PixelDepth::EightBit,
                Some("10U") => PixelDepth::TenBit,
                Some("16F") => PixelDepth::SixteenBitFloat,
                Some(other) => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                        "Invalid internal_color_depth: {}. Must be one of '8U', '10U', '16F'",
                        other
                    )))
                }
                None => PixelDepth::EightBit,
            },
            internal_colors_are_in_display_color_space: true,
        })
    }
}

#[pymethods]
impl DisplayConfig {
    #[new]
    #[pyo3(signature = (
        surface_color_depth=None,
        generic_display=None,
        calibration_file=None
    ))]
    /// Defines the the properties the colour/luminance output of the display.
    /// You can either provide a calibration file or assume a generic display.
    ///
    /// # Parameters
    /// - `surface_color_depth`: The surface color depth to use (normally 8, 16 or 10 bit) for the display surface.
    ///   Psydk will throw an error if you try to use a color depth that is not supported.
    /// - `generic_display`: If you don't provide a calibration file, psydk will use a generic display model. Must be one of:
    ///   - 'srgb': Standard RGB display (default, gamma 2.2)
    ///   - 'linear_srgb': Linear sRGB display (gamma 1.0)
    ///   - 'display_p3': Display P3 display (gamma 2.2)
    /// - `calibration_file`: Path to a display calibration file in JSON format.
    pub fn new(
        surface_color_depth: Option<&str>,
        generic_display: Option<&str>,
        calibration_file: Option<&str>,
    ) -> PyResult<Self> {
        // build display characteristics
        let display_characteristics: Arc<dyn DisplayCharacteristics + Send + Sync> =
            // try to load calibration file if provided
            if let Some(calibration_file) = calibration_file {
                // load from file
                let file = File::open(calibration_file).map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                        "Failed to open display calibration file: {}",
                        e
                    ))
                })?;

                let reader = BufReader::new(file);
                let gc: Psydk1DisplayCharacteristics = serde_json::from_reader(reader).map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                        "Failed to load display calibration file: {}",
                        e
                    ))
                })?;
                Arc::new(gc)
            } else if let Some(generic_display) = generic_display {
                match generic_display {
                    "srgb" => Arc::new(GenericDisplayCharacteristics::new_srgb()),
                    "linear_srgb" => Arc::new(GenericDisplayCharacteristics::new_linear_srgb()),
                    "display_p3" => Arc::new(GenericDisplayCharacteristics::new_display_p3()),
                    other => panic!("Invalid generic_display: {}. Must be one of 'srgb', 'linear_srgb', 'display_p3'", other),
                }
            } else {
                Arc::new(GenericDisplayCharacteristics::default())
            };

        // parse surface color depth
        let surface_color_depth = match surface_color_depth {
            Some("8U") => PixelDepth::EightBit,
            Some("10U") => PixelDepth::TenBit,
            Some("16F") => PixelDepth::SixteenBitFloat,
            Some(other) => {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                    "Invalid surface_color_depth: {}. Must be one of '8U', '10U', '16F'",
                    other
                )))
            }
            None => PixelDepth::EightBit,
        };
        Ok(Self {
            surface_color_depth,
            display_characteristics,
        })
    }
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        Self {
            pedantic: true,
            debug: false,
            internal_color_depth: PixelDepth::EightBit,
            internal_colors_are_in_display_color_space: true,
        }
    }
}
