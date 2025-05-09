use std::{borrow::Cow, sync::Arc};

use psydk_proc::StimulusParams;
use pyo3::{exceptions::PyValueError, ffi::c_str, prelude::*};
use renderer::{
    affine::Affine,
    brushes::{Brush, Gradient},
    colors::RGBA,
};
use uuid::Uuid;

use super::{
    animations::Animation, impl_pystimulus_for_wrapper, pattern::FillPattern, LinRgba, PyStimulus, Stimulus,
    StimulusParamValue, StimulusParams, StrokeStyle,
};
use crate::{
    context::{ExperimentContext, PyRendererFactory},
    visual::{geometry::Size, window::Window},
};

pub(crate) fn create_fill_brush_uniform<'a>(fill_color: &LinRgba) -> Brush<'a> {
    Brush::Solid((*fill_color).into())
}

pub(crate) fn create_fill_brush_pattern<'a>(
    foreground_color: &LinRgba,
    pattern: &FillPattern,
    pattern_origin: (f32, f32),
) -> Brush<'a> {
    match pattern {
        FillPattern::Uniform => Brush::Solid((*foreground_color).into()),
        FillPattern::Stripes => todo!(),
        FillPattern::Sinosoidal => todo!(),
        FillPattern::Checkerboard => todo!(),
    }
}

pub(crate) fn create_fill_brush<'a>(
    fill_color: &Option<LinRgba>,
    stroke_style: &Option<StrokeStyle>,
    stroke_color: &Option<LinRgba>,
    stroke_width: &Option<Size>,
    gradient: &Option<Gradient>,
    // image: Option<Image>,
) -> Brush<'a> {
    // gradient takes precedence over fill_color
    if let Some(gradient) = gradient {
        Brush::Gradient(gradient.clone())
    } else if let Some(fill_color) = fill_color {
        create_fill_brush_uniform(fill_color)
    } else {
        create_fill_brush_uniform(&LinRgba::new(0.0, 0.0, 0.0, 0.0))
    }
}

pub(crate) fn create_fill_brush2<'a>(
    pattern: &Option<FillPattern>,
    fill_origin: Option<(f32, f32)>,
    fill_color: &Option<LinRgba>,
    stroke_style: &Option<StrokeStyle>,
    stroke_color: &Option<LinRgba>,
    stroke_width: &Option<Size>,
    gradient: &Option<Gradient>,
) -> Result<Brush<'a>, crate::errors::PsydkError> {
    let fill_origin = fill_origin.unwrap_or((0.0, 0.0));
    if let Some(pattern) = pattern {
        let default_color = LinRgba::default();
        let fill_color = fill_color.as_ref().unwrap_or(&default_color);
        Ok(create_fill_brush_pattern(fill_color, pattern, fill_origin))
    } else if let Some(gradient) = gradient {
        Ok(Brush::Gradient(gradient.clone()))
    } else if let Some(fill_color) = fill_color {
        Ok(create_fill_brush_uniform(fill_color))
    } else {
        Ok(create_fill_brush_uniform(&LinRgba::new(0.0, 0.0, 0.0, 0.0)))
    }
}

// pub(crate) fn get_renderer_factory(py: Python) -> PyResult<PyRendererFactory> {
//     // first, try to get __renderer_factory from the __globals__
//     let renderer_factory = py
//         .eval(c_str!("__renderer_factory"), None, None)
//         .expect("No renderer factory found in function scope. Are you calling this function from a stimulus callback?");

//     // covert to Rust type
//     // let renderer_factory = PyRendererFactory::extract_bound(renderer_factory).unwrap();
//     let renderer_factory: PyRendererFactory = renderer_factory.extract().unwrap();
//     Ok(renderer_factory)
// }

pub(crate) fn get_experiment_context(em: Option<ExperimentContext>, py: Python) -> PyResult<ExperimentContext> {
    // if we already have an experiment context, return it
    if let Some(em) = em {
        return Ok(em);
    }

    // first, try to get _experiment_context from the __globals__
    let ec = py.eval(c_str!("_experiment_context"), None, None).map_err(|_| {
        PyValueError::new_err("No experiment context found in function scope. Try passing it explicitly.")
    })?;

    // covert to Rust type
    let ec: ExperimentContext = ec.extract().unwrap();
    Ok(ec)
}

pub fn is_valid_font_file(path: &str) -> bool {
    let path = std::path::Path::new(path);
    path.exists() && path.is_file() && path.extension().map_or(false, |ext| ext == "ttf" || ext == "otf")
}
