from psydk.visual.color import rgb
from psydk.visual.geometry import Shape, cm, px
from psydk.visual.stimuli import BaseStimulus
from psydk.visual.renderer import Brush, StrokeStyle

class FixationCrossStimulus(BaseStimulus):
    """Stimulus class for drawing a fixation cross.

    Parameters
    ----------
    ctx : object
        Experiment context provided by the psydk framework.
    size : unit, optional
        Length of each arm of the cross (default: ``cm(2.0)``).
    x : unit, optional
        X position of the center of the cross (default: ``px(0)``).
    y : unit, optional
        Y position of the center of the cross (default: ``px(0)``).
    outset : unit, optional
        Distance from the center to the start of each arm (default: ``cm(1)``).
    stroke_width : unit, optional
        Width of the cross arms (default: ``px(10)``).
    stroke_color : rgba, optional
        RGBA color for the cross arms (default: ``rgb(0, 0, 0)``).
    antialias : bool, optional
        Whether to apply anti-aliasing when drawing the cross (default: True).
    """
    def __init__(self, ctx, size=cm(2.0), x=px(0), y=px(0), outset=cm(1), stroke_width=px(10), stroke_color=rgb(0,0,0), antialias=True):
        self.size = size
        self.x, self.y = x, y
        self.outset = outset
        self.stroke_color = stroke_color
        self.stroke_width = stroke_width
        self.antialias = antialias

    def draw(self, scene, window_state):
        stroke_options = StrokeStyle(self.stroke_width, window_state)
        stroke_brush  = Brush.solid(self.stroke_color, window_state)
        scene.draw_shape_stroked(window_state, Shape.line(self.x - self.outset, self.y, self.x - self.outset - self.size, self.y), stroke_brush, stroke_options, anti_alias = self.antialias)
        scene.draw_shape_stroked(window_state, Shape.line(self.x + self.outset, self.y, self.x + self.outset + self.size, self.y), stroke_brush, stroke_options, anti_alias = self.antialias)
        scene.draw_shape_stroked(window_state, Shape.line(self.x, self.y - self.outset, self.x, self.y - self.outset - self.size), stroke_brush, stroke_options, anti_alias = self.antialias)
        scene.draw_shape_stroked(window_state, Shape.line(self.x, self.y + self.outset, self.x, self.y + self.outset + self.size), stroke_brush, stroke_options, anti_alias = self.antialias)
