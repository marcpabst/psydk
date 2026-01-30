use pyo3::prelude::{Borrowed, PyModule};
use std::time::Instant;

use super::{Stimulus, StimulusParamValue};
use crate::visual::colors::Color;
use crate::visual::colors::IntoColor;
use crate::visual::{
    geometry::Size,
    window::{Window, WindowState, WindowStateSnapshot},
};
use pyo3::{types::PyAnyMethods, Bound, FromPyObject, PyAny, PyResult};

#[derive(FromPyObject, Debug, Clone)]
pub enum Repeat {
    /// Play the animation the specified number of times.
    Loop(u32),
    /// Ping-pong the animation the specified number of times.
    PingPong(u32),
}

#[derive(Debug, Clone, Copy)]
pub enum TransitionFunction {
    /// No transition function.
    None,
    /// A linear transition function.
    Linear(f32, f32),
    /// A cubic bezier transition function.
    CubicBezier(f32, f32, f32, f32),
}

// implement FromPyObject for TransitionFunction
impl FromPyObject<'_, '_> for TransitionFunction {
    type Error = pyo3::PyErr;
    fn extract(ob: Borrowed<'_, '_, PyAny>) -> Result<Self, Self::Error> {
        // try to extract a string from the object and then convert it to a TransitionFunction
        if let Ok(name) = ob.extract::<&str>() {
            Ok(TransitionFunction::from_str(name))
        } else {
            // if the object is not a string, try to extract a tuple of f32s
            let tuple = ob.extract::<(f32, f32, f32, f32)>()?;
            Ok(TransitionFunction::CubicBezier(tuple.0, tuple.1, tuple.2, tuple.3))
        }
    }
}

impl TransitionFunction {
    pub fn linear() -> Self {
        Self::Linear(0.0, 1.0)
    }

    pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self::CubicBezier(x1, y1, x2, y2)
    }

    pub fn ease_in() -> Self {
        Self::CubicBezier(0.42, 0.0, 1.0, 1.0)
    }

    pub fn ease_out() -> Self {
        Self::CubicBezier(0.0, 0.0, 0.58, 1.0)
    }

    pub fn ease_in_out() -> Self {
        Self::CubicBezier(0.42, 0.0, 0.58, 1.0)
    }

    pub fn from_str(name: &str) -> Self {
        match name {
            "linear" => Self::linear(),
            "ease-in" => Self::ease_in(),
            "ease-out" => Self::ease_out(),
            "ease-in-out" => Self::ease_in_out(),
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Animation {
    /// The name of the attribute that should be animated.
    paramter: String,
    /// The value that the attribute should be animated from.
    from: StimulusParamValue,
    /// The value that the attribute should be animated to.
    to: StimulusParamValue,
    /// The duration of the animation in seconds.
    duration: f32,
    /// The time at which the animation should start from when it is created.
    start_time: Instant,
    /// Repeat the animation according to the specified repeat mode.
    repeat: Repeat,
    /// The easing function that should be used for the animation.
    easing: TransitionFunction,
}

impl Animation {
    pub fn new(
        parameter: &str,
        from: StimulusParamValue,
        to: StimulusParamValue,
        duration: f32,
        start_time: Instant,
        repeat: Repeat,
        easing: TransitionFunction,
    ) -> Self {
        Self {
            paramter: parameter.to_string(),
            from,
            to,
            duration,
            start_time,
            repeat,
            easing,
        }
    }

    /// Returns the name of the attribute that should be animated.
    pub fn parameter(&self) -> &str {
        &self.paramter
    }

    /// Returns the current value of the animated parameter at the specified time (f32).
    pub fn value_f32(from: f32, to: f32, elapsed: f32, duration: f32, easing: TransitionFunction) -> f32 {
        let t = elapsed / duration;
        let t = match easing {
            TransitionFunction::None => t,
            TransitionFunction::Linear(a, b) => a + (b - a) * t,
            TransitionFunction::CubicBezier(p1, p2, p3, p4) => {
                let t2 = t * t;
                let t3 = t2 * t;
                let c = 3.0 * (p1 - p2);
                let b = 3.0 * (p3 - p1) - c;
                let a = 1.0 - c - b;
                a * t3 + b * t2 + c * t
            }
        };

        from + (to - from) * t
    }

    /// Returns the current value of the animated parameter at the specified time.
    pub fn value(&self, time: Instant, window_state: &WindowStateSnapshot) -> StimulusParamValue {
        if self.finished(time) {
            return self.to.clone();
        }

        // let elapsed = time.duration_since(self.start_time).as_secs_f32();
        let elapsed = match self.repeat {
            Repeat::Loop(n) => {
                let elapsed = time.duration_since(self.start_time).as_secs_f32();
                elapsed % self.duration
            }
            Repeat::PingPong(n) => {
                let elapsed = time.duration_since(self.start_time).as_secs_f32();
                let elapsed = elapsed % (self.duration * 2.0);
                if elapsed > self.duration {
                    self.duration - (elapsed - self.duration)
                } else {
                    elapsed
                }
            }
        };

        let duration = self.duration;
        let easing = self.easing.clone();
        let from = self.from.clone();
        let to = self.to.clone();

        let window_size = window_state.size;
        let screen_props = window_state.physical_screen;

        match (from, to) {
            (StimulusParamValue::f32(f), StimulusParamValue::f32(t)) => {
                StimulusParamValue::f32(Self::value_f32(f, t, elapsed, duration, easing))
            }
            (StimulusParamValue::Size(f), StimulusParamValue::Size(t)) => {
                let f = f.eval(window_size, screen_props) as f32;
                let t = t.eval(window_size, screen_props) as f32;
                let value = Self::value_f32(f, t, elapsed, duration, easing);
                StimulusParamValue::Size(Size::Pixels(value as f32))
            }
            // for now just animate in linear RGB space
            // (StimulusParamValue::Color(f), StimulusParamValue::Color(t)) => {
            //     let value_r = Self::value_f32(f.r as f32, t.r as f32, elapsed, duration, easing);
            //     let value_g = Self::value_f32(f.g as f32, t.g as f32, elapsed, duration, easing);
            //     let value_b = Self::value_f32(f.b as f32, t.b as f32, elapsed, duration, easing);
            //     let value_a = Self::value_f32(f.a as f32, t.a as f32, elapsed, duration, easing);
            //     StimulusParamValue::Color(crate::visual::colors::Color::new_srgba(
            //         value_r as f32,
            //         value_g as f32,
            //         value_b as f32,
            //         value_a as f32,
            //     ))
            // }
            _ => self.to.clone(),
        }
    }

    /// Returns whether the animation has finished.
    pub fn finished(&self, time: Instant) -> bool {
        match self.repeat {
            Repeat::Loop(n) => {
                let elapsed = time.duration_since(self.start_time).as_secs_f32();
                elapsed > self.duration * n as f32
            }
            Repeat::PingPong(n) => {
                let elapsed = time.duration_since(self.start_time).as_secs_f32();
                elapsed > self.duration * n as f32 * 2.0
            }
        }
    }
}
