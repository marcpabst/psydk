from psydk.visual.color import rgb
from psydk.visual.geometry import Shape, cm, px
from psydk.visual.stimuli import BaseStimulus
from psydk.visual.renderer import Brush, StrokeStyle

class FixationCrossStimulus(BaseStimulus):
    def __init__(self, ctx, size=cm(2.0), x=px(0), y=px(0), outset=cm(1), stroke_width=px(10), stroke_color=rgb(0,0,0), antialias=True):
        """Stimulus class for a fixation cross.

        Args:
            ctx: Experiment context provided by the psydk framework
            size: Size of the fixation cross arms (default: 0.5 cm)
            x: X position of the fixation cross center (default: 0)
            y: Y position of the fixation cross center (default: 0)
            outset: Distance from the center to the start of the arms (default: 0.25 cm)
            stroke_width: Width of the lines (default: 5 pixels)
            stroke_color: RGBA color for the lines (default: black)
            antialias: Whether to use anti-aliasing when drawing the lines (default: True)
        """
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
