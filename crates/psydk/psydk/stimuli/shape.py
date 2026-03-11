from psydk.visual.color import rgb
from psydk.visual.geometry import px
from psydk.visual.stimuli import BaseStimulus
from psydk.visual.renderer import Brush, StrokeStyle

from .mixins import ClickableMixin

class ShapeStimulus(ClickableMixin, BaseStimulus):
    """Stimulus class for drawing basic shapes.

    Parameters
    ----------
    ctx : object
        Experiment context provided by the psydk framework.
    shape : Shape
        A Shape object defining the geometry to draw.
    fill_color : rgba, optional
        RGBA color for filling the shape (default: white).
    stroke_color : rgba, optional
        RGBA color for the shape outline (default: black).
    stroke_width : unit, optional
        Width of the shape outline in pixels (default: 0).
    """
    def __init__(self, ctx, shape, fill_color=rgb(1, 1, 1), stroke_color=rgb(0, 0, 0, 1), stroke_width=px(0)):

        ClickableMixin.__init__(self)
        self.shape = shape
        self.fill_color = fill_color
        self.stroke_color = stroke_color
        self.stroke_width = stroke_width

    def draw(self, scene, window_state):
        fill_brush = Brush.solid(self.fill_color, window_state)
        stroke_brush = Brush.solid(self.stroke_color, window_state)
        stroke_options = StrokeStyle(self.stroke_width, window_state)
        scene.draw_shape_filled(window_state, self.shape, fill_brush)
        scene.draw_shape_stroked(window_state, self.shape, stroke_brush, stroke_options)

    def contains_point(self, point, window_state):
        return self.shape.contains_point(point, window_state)
