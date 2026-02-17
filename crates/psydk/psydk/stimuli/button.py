from psydk.visual.color import rgb
from psydk.visual.geometry import px, cm, Shape
from psydk.visual.stimuli import BaseStimulus
from psydk.visual.renderer import Brush, StrokeStyle, Text


class ButtonStimulus(BaseStimulus):
    def __init__(self, ctx, text, x=px(0), y=px(0), width=cm(5), height=cm(2),
        fill_color=rgb(1, 1, 1), stroke_color=rgb(0, 0, 0, 1), stroke_width=px(5)):
        """Stimulus class for drawing basic shapes.

        Args:
            ctx: Experiment context provided by the psydk framework
            shape: A Shape object defining the geometry to draw
            fill_color: RGBA color for filling the shape (default: white)
            stroke_color: RGBA color for the shape outline (default: black)
            stroke_width: Width of the shape outline in pixels (default: 1)
        """
        self.button_shape = Shape.rectangle(width, height, x=x, y=y)
        self.fill_color = fill_color
        self.stroke_color = stroke_color
        self.stroke_width = stroke_width
        self.x = x
        self.y = y

        self.text = Text(text, font_family="Arial", font_size=cm(5))

    def draw(self, scene, window_state):
        fill_brush = Brush.solid(self.fill_color, window_state)
        text_brush = Brush.solid(self.stroke_color, window_state)  # Text color matches stroke color
        stroke_brush = Brush.solid(self.stroke_color, window_state)
        stroke_options = StrokeStyle(self.stroke_width, window_state)

        scene.build_text(window_state, self.text, text_brush)
        text_w, text_h = self.text.measure()
        self.button_shape = Shape.rectangle(text_w, text_h, x=self.x, y=self.y)

        scene.draw_shape_filled(window_state, self.button_shape, fill_brush)
        scene.draw_shape_stroked(window_state, self.button_shape, stroke_brush, stroke_options)


        scene.draw_text(window_state, self.text, x=self.x, y=self.y)
