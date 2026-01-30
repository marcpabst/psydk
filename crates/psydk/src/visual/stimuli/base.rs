use crate::visual::colors::Color;
use crate::visual::colors::IntoColor;
use crate::visual::renderer::{
    affine::Affine,
    brushes::{Brush, Extend, ImageSampling},
    colors::RGBA,
    styles::ImageFitMode,
};
use psydk_proc::{FromPyStr, StimulusParams};
use std::sync::Arc;
use strum::EnumString;
use uuid::Uuid;

use super::{
    animations::Animation, helpers, impl_pystimulus_for_wrapper, PyStimulus, Stimulus, StimulusParamValue,
    StimulusParams, StrokeStyle,
};

use crate::{
    context::ExperimentContext,
    visual::{
        geometry::{Shape, Size, Transformation2D},
        window::{Frame, WindowState, WindowStateSnapshot},
    },
};

use pyo3::prelude::*;

// /// allow BaseStimulus to be converted to PyStimulus
// impl FromPyObject<'_, '_> for PyStimulus {
//     fn extract_bound(ob: &Bound<'_, PyAny>) -> PyResult<Self> {
//         let stimulus = obj.extract::<BaseStimulus>()?;
//         Ok(PyStimulus::from(stimulus))
//     }
// }

// impl Into<BaseStimulus> for WrappedStimulus {
// impl Into<WrappedStimulus> for BaseStimulus {
//     fn into(self) -> WrappedStimulus {
//         WrappedStimulus { inner: Box::new(self) }
//     }
// }

#[derive(Debug, Clone)]
#[pyclass(subclass)]
pub struct BaseStimulus {
    pub id: Uuid,
}

impl BaseStimulus {
    pub fn new(ctx: &ExperimentContext) -> Self {
        Self { id: Uuid::new_v4() }
    }
}

impl Stimulus for BaseStimulus {
    fn draw(&mut self, scene: &mut crate::visual::renderer::wrapped::Scene, win_state: &WindowStateSnapshot) {
        // by default, stimuli will do nothing
    }

    fn uuid(&self) -> Uuid {
        todo!()
    }

    fn animations(&mut self) -> Option<&mut Vec<Animation>> {
        None
    }

    fn set_transformation(&mut self, transformation: crate::visual::geometry::Transformation2D) {
        todo!()
    }

    fn transformation(&self) -> crate::visual::geometry::Transformation2D {
        todo!()
    }

    fn get_param(&self, name: &str) -> Option<super::StimulusParamValue> {
        todo!()
    }

    fn set_param(&mut self, name: &str, value: super::StimulusParamValue) {
        todo!()
    }
}

#[pymethods]
impl BaseStimulus {
    #[new]
    fn py_new(ctx: &ExperimentContext) -> Self {
        Self::new(ctx)
    }
}
