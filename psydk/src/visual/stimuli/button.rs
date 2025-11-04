use crate::visual::colors::Color;
use crate::visual::colors::IntoColor;

use psydk_proc::{FromPyStr, StimulusParams};
use renderer::{
    affine::Affine,
    brushes::{Brush, Extend, ImageSampling},
    colors::RGBA,
    renderer::SharedRendererState,
    styles::ImageFitMode,
    DynamicBitmap, DynamicScene,
};
use std::sync::Arc;
use strum::EnumString;
use uuid::Uuid;

unsafe impl Send for ButtonStimulus {}

use super::{
    animations::Animation, helpers, impl_pystimulus_for_wrapper, PyStimulus, Stimulus, StimulusParamValue,
    StimulusParams, StrokeStyle,
};
use crate::{
    context::ExperimentContext,
    visual::{
        geometry::{Anchor, Shape, Size, Transformation2D},
        stimuli::text::{FontWeight, TextAlignment, TextStimulus},
        window::{Frame, WindowState},
    },
};

#[derive(StimulusParams, Clone, Debug)]
pub struct ButtonParams {
    pub x: Size,
    pub y: Size,
    pub width: Size,
    pub height: Size,
    pub text: String,
    pub text_size: Size,
    pub text_color: Color,
    pub text_color_hover: Color,
    pub text_color_pressed: Color,
    pub text_color_disabled: Color,
    pub fill_color: Color,
    pub fill_color_hover: Color,
    pub fill_color_pressed: Color,
    pub fill_color_disabled: Color,
    pub stroke_style: StrokeStyle,
    pub stroke_color: Color,
    pub stroke_width: Size,
    pub alpha: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Normal,
    Hovered,
    Pressed,
    Disabled,
}

#[derive(Debug)]
pub struct ButtonStimulus {
    id: uuid::Uuid,
    params: ButtonParams,
    state: ButtonState,

    flag_clicked: bool,
    text_stimulus: TextStimulus,

    transform: Transformation2D,
    animations: Vec<Animation>,
    visible: bool,
}

impl ButtonStimulus {
    pub fn new(
        x: Size,
        y: Size,
        width: Size,
        height: Size,
        text: String,
        text_size: Size,
        text_color: Color,
        text_color_hover: Option<Color>,
        text_color_pressed: Option<Color>,
        text_color_disabled: Option<Color>,
        fill_color: Color,
        fill_color_hover: Option<Color>,
        fill_color_pressed: Option<Color>,
        fill_color_disabled: Option<Color>,
        stroke_style: StrokeStyle,
        stroke_color: Color,
        stroke_width: Size,
        alpha: Option<f64>,
        transform: Transformation2D,
        context: &ExperimentContext,
    ) -> Self {
        // Create a text stimulus for the button text
        let text_stimulus = TextStimulus::new(
            x.clone(),
            y.clone(),
            &text,
            TextAlignment::Center,
            Anchor::Center,
            text_size.clone(),
            "Noto Sans",
            FontWeight::Regular,
            text_color.clone(),
            1.0,
            Transformation2D::Identity(),
            context,
        );

        let mut stim = Self {
            id: Uuid::new_v4(),
            params: ButtonParams {
                x,
                y,
                width,
                height,
                text,
                text_size,
                text_color,
                text_color_hover: text_color_hover.unwrap_or(text_color.lighten(0.2)),
                text_color_pressed: text_color_pressed.unwrap_or(text_color.darken(0.2)),
                text_color_disabled: text_color_disabled.unwrap_or(Color::default()),
                fill_color,
                fill_color_hover: fill_color_hover.unwrap_or(fill_color.lighten(0.2)),
                fill_color_pressed: fill_color_pressed.unwrap_or(fill_color.darken(0.2)),
                fill_color_disabled: fill_color_disabled.unwrap_or(Color::default()),
                stroke_style,
                stroke_color,
                stroke_width,
                alpha,
            },
            text_stimulus,
            state: ButtonState::Normal,
            flag_clicked: false,
            transform,
            animations: Vec::new(),
            visible: true,
        };

        stim
    }
}

#[derive(Debug, Clone)]
#[pyclass(name = "ButtonStimulus", extends=PyStimulus)]
/// A stimulus that displays a shape.
///
/// Parameters
/// ----------
/// shape : Shape
///     The shape to display.
/// x : Size, optional
///     The x-coordinate of the center of the shape.
/// y : Size, optional
///     The y-coordinate of the center of the shape.
/// fill_color : Union[Color, (float, float, float), (float, float, float, float), str], optional
///    The fill color of the shape.
/// stroke_style : StrokeStyle, optional
///    The stroke style of the shape.
/// stroke_color : Union[Color, (float, float, float), (float, float, float, float), str], optional
///   The stroke color of the shape.
/// stroke_width : Union[Size, float], optional
///  The stroke width of the shape.
/// alpha : float, optional
///  The alpha channel of the shape.
/// transform : Transformation2D, optional
/// The transformation of the shape.
pub struct PyButtonStimulus();

#[pymethods]
impl PyButtonStimulus {
    #[new]
    #[pyo3(signature = (
        x = IntoSize(Size::Pixels(0.0)),
        y = IntoSize(Size::Pixels(0.0)),
        width = IntoSize(Size::Pixels(100.0)),
        height = IntoSize(Size::Pixels(50.0)),
        text = String::new(),
        text_size = IntoSize(Size::Pixels(12.0)),
        text_color = IntoColor::default(),
        text_color_hover = None,
        text_color_pressed = None,
        text_color_disabled = None,
        fill_color = IntoColor::default(),
        fill_color_hover = None,
        fill_color_pressed = None,
        fill_color_disabled = None,
        stroke_style = StrokeStyle::default(),
        stroke_color = IntoColor(Color::default()),
        stroke_width = IntoSize(Size::Pixels(0.0)),
        alpha = None,
        transform = Transformation2D::Identity(),
        context = None,
    ))]
    /// A stimulus that displays a shape.
    ///
    /// Parameters
    /// ----------
    /// shape : Shape
    ///     The shape to display.
    /// x : Size, optional
    ///     The x-coordinate of the center of the shape.
    /// y : Size, optional
    ///     The y-coordinate of the center of the shape.
    /// fill_color : Union[Color, (float, float, float), (float, float, float, float), str], optional
    ///    The fill color of the shape.
    /// stroke_style : StrokeStyle, optional
    ///    The stroke style of the shape.
    /// stroke_color : Union[Color, (float, float, float), (float, float, float, float), str], optional
    ///   The stroke color of the shape.
    /// stroke_width : Union[Size, float], optional
    ///    The stroke width of the shape.
    /// alpha : float, optional
    ///    The alpha channel of the shape.
    /// transform : Transformation2D, optional
    ///    The transformation of the shape.
    fn __new__(
        py: Python,
        x: IntoSize,
        y: IntoSize,
        width: IntoSize,
        height: IntoSize,
        text: String,
        text_size: IntoSize,
        text_color: IntoColor,
        text_color_hover: Option<IntoColor>,
        text_color_pressed: Option<IntoColor>,
        text_color_disabled: Option<IntoColor>,
        fill_color: IntoColor,
        fill_color_hover: Option<IntoColor>,
        fill_color_pressed: Option<IntoColor>,
        fill_color_disabled: Option<IntoColor>,
        stroke_style: StrokeStyle,
        stroke_color: IntoColor,
        stroke_width: IntoSize,
        alpha: Option<f64>,
        transform: Transformation2D,
        context: Option<ExperimentContext>,
    ) -> (Self, PyStimulus) {
        let context = helpers::get_experiment_context(context, py).unwrap();
        (
            Self(),
            PyStimulus::new(ButtonStimulus::new(
                x.into(),
                y.into(),
                width.into(),
                height.into(),
                text,
                text_size.into(),
                text_color.into(),
                text_color_hover.map(|c| c.into()),
                text_color_pressed.map(|c| c.into()),
                text_color_disabled.map(|c| c.into()),
                fill_color.into(),
                fill_color_hover.map(|c| c.into()),
                fill_color_pressed.map(|c| c.into()),
                fill_color_disabled.map(|c| c.into()),
                stroke_style,
                stroke_color.into(),
                stroke_width.into(),
                alpha,
                transform,
                &context,
            )),
        )
    }

    /// Returns True if the button was clicked, otherwise False. Resets the clicked state to False.
    fn clicked(slf: PyRef<'_, Self>) -> bool {
        let mut stim = slf.as_ref().0.lock();
        if let Some(button) = stim.downcast_mut::<ButtonStimulus>() {
            button.clicked()
        } else {
            unreachable!("PyButtonStimulus should always be a ButtonStimulus")
        }
    }
}

impl_pystimulus_for_wrapper!(PyButtonStimulus, ButtonStimulus);

impl Stimulus for ButtonStimulus {
    fn uuid(&self) -> Uuid {
        self.id
    }

    fn animations(&mut self) -> &mut Vec<Animation> {
        &mut self.animations
    }

    fn add_animation(&mut self, animation: Animation) {
        self.animations.push(animation);
    }

    fn draw(&mut self, scene: &mut DynamicScene, window_state: &WindowState) {
        if !self.visible {
            return;
        }

        let windows_size = window_state.size;
        let screen_props = window_state.physical_screen;
        let dc = &*window_state.display_characteristics;

        let renderer_factory = &window_state.shared_renderer_state;

        let x_origin = self.params.x.eval(windows_size, screen_props) as f64;
        let y_origin = self.params.y.eval(windows_size, screen_props) as f64;

        let fill_color = match self.state {
            ButtonState::Normal => self.params.fill_color.to_display_rgba(dc),
            ButtonState::Hovered => self.params.fill_color_hover.to_display_rgba(dc),
            ButtonState::Pressed => self.params.fill_color_pressed.to_display_rgba(dc),
            ButtonState::Disabled => self.params.fill_color_disabled.to_display_rgba(dc),
        };

        let fill_brush = Brush::Solid(fill_color.into());

        let stroke_color = self.params.stroke_color.to_display_rgba(dc);

        let stroke_brush = renderer::brushes::Brush::Solid(stroke_color.into());

        let stroke_width = self.params.stroke_width.eval(windows_size, screen_props) as f64;

        let stroke_options = renderer::styles::StrokeStyle::new(stroke_width);

        // draw the text
        let text_color = match self.state {
            ButtonState::Normal => self.params.text_color.to_display_rgba(dc),
            ButtonState::Hovered => self.params.text_color_hover.to_display_rgba(dc),
            ButtonState::Pressed => self.params.text_color_pressed.to_display_rgba(dc),
            ButtonState::Disabled => self.params.text_color_disabled.to_display_rgba(dc),
        };

        let width = self.params.width.eval(windows_size, screen_props) as f64;
        let height = self.params.height.eval(windows_size, screen_props) as f64;

        // move x and y so that the rectangle is drawn at the correct position
        let x = self.params.x.eval(windows_size, screen_props) as f64 - width / 2.0;
        let y = self.params.y.eval(windows_size, screen_props) as f64 - height / 2.0;

        // // move by x_origin and y_origin
        // let x = x + x_origin;
        // let y = y + y_origin;

        let shape = renderer::shapes::Shape::rectangle((x, y), width, height);

        scene.draw_shape_fill(shape.clone(), fill_brush.clone(), None, None);

        scene.draw_shape_stroke(shape, stroke_brush, stroke_options, None, None);

        // draw the text stimulus
        self.text_stimulus.draw(scene, window_state);
    }
    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_transformation(&mut self, transformation: crate::visual::geometry::Transformation2D) {
        self.transform = transformation;
    }

    fn transformation(&self) -> crate::visual::geometry::Transformation2D {
        self.transform.clone()
    }

    fn get_param(&self, name: &str) -> Option<StimulusParamValue> {
        self.params.get_param(name)
    }

    fn set_param(&mut self, name: &str, value: StimulusParamValue) {
        self.params.set_param(name, value)
    }

    fn dispatch_event(&mut self, event: &crate::input::Event, window_state: &WindowState) -> bool {
        // if this is a mouse move event, we check if the mouse is over the button

        match event {
            crate::input::Event::CursorMoved { position, .. } | crate::input::Event::TouchMove { position, .. } => {
                self.handle_mouse_move(position, window_state)
            }
            crate::input::Event::MouseButtonPress { position, .. }
            | crate::input::Event::TouchStart { position, .. } => self.handle_mouse_down(position, window_state),
            crate::input::Event::MouseButtonRelease { position, .. }
            | crate::input::Event::TouchEnd { position, .. }
            | crate::input::Event::TouchCancel { position, .. } => self.handle_mouse_up(position, window_state),
            _ => false,
        }
    }
}

impl ButtonStimulus {
    fn contains_point(&self, position: &(f32, f32), window_state: &WindowState) -> bool {
        let windows_size = window_state.size;
        let screen_props = window_state.physical_screen;

        let pos_x = (Size::Pixels(position.0) - self.params.x.clone() + self.params.width.clone() / 2.0);
        let pos_y = (Size::Pixels(position.1) - self.params.y.clone() + self.params.height.clone() / 2.0);

        // // check if the position is within the button's rectangle
        let x = pos_x.eval(windows_size, screen_props) as f32;
        let y = pos_y.eval(windows_size, screen_props) as f32;
        let width = self.params.width.eval(windows_size, screen_props) as f32;
        let height = self.params.height.eval(windows_size, screen_props) as f32;

        x >= 0.0 && y >= 0.0 && x <= width && y <= height
    }

    fn handle_mouse_move(&mut self, position: &(f32, f32), window_state: &WindowState) -> bool {
        if self.state == ButtonState::Pressed && self.contains_point(position, window_state) {
            // if the button is pressed and the cursor is over the button, we keep the state as pressed
            true // we handle the event
        } else if self.state == ButtonState::Pressed && !self.contains_point(position, window_state) {
            // if the button is pressed and the cursor is not over the button, we set the state to normal
            self.state = ButtonState::Normal;
            false // we do not handle the event
        } else if self.contains_point(position, window_state) {
            // if the cursor is over the button, we set the state to hovered
            self.state = ButtonState::Hovered;
            true // we handle the event
        } else {
            // if the cursor is not over the button, we set the state to normal
            self.state = ButtonState::Normal;
            false // we do not handle the event
        }
    }
    fn handle_mouse_down(&mut self, position: &(f32, f32), window_state: &WindowState) -> bool {
        if self.contains_point(position, window_state) {
            // set the button state to pressed
            self.state = ButtonState::Pressed;
            true // if the cursor is over the button, we handle the event
        } else {
            // set the button state to normal
            self.state = ButtonState::Normal;
            false // if the cursor is not over the button, we do not handle the event
        }
    }
    fn handle_mouse_up(&mut self, position: &(f32, f32), window_state: &WindowState) -> bool {
        if self.state == ButtonState::Pressed && self.contains_point(position, window_state) {
            // if the button was pressed and the cursor is still over the button
            self.state = ButtonState::Hovered; // reset the state to normal after the click
            self.flag_clicked = true; // set the flag to indicate that the button was clicked
            true
        } else if self.state == ButtonState::Pressed && !self.contains_point(position, window_state) {
            // if the button was pressed and the cursor is not over the button
            self.state = ButtonState::Normal;
            false // we do not handle the event
        } else {
            false
        }
    }

    fn clicked(&mut self) -> bool {
        if self.flag_clicked {
            self.flag_clicked = false; // reset the flag
            true // return true if the button was clicked
        } else {
            false // return false if the button was not clicked
        }
    }
}
