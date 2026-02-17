from psydk.visual.color import rgb
from psydk.visual.geometry import px, cm, Shape
from psydk.visual.stimuli import BaseStimulus
from psydk.visual.renderer import Brush, StrokeStyle, Text


class TextboxStimulus(BaseStimulus):
    def __init__(self, ctx, text, x=px(0), y=px(0), width=None, height=None,
        fill_color=rgb(1, 1, 1), stroke_color=rgb(0, 0, 0, 1), stroke_width=px(5), font_size=cm(0.5), font_family="Arial"):
        """Stimulus class for drawing basic shapes.

        Args:
            ctx: Experiment context provided by the psydk framework
            shape: A Shape object defining the geometry to draw
            fill_color: RGBA color for filling the shape (default: white)
            stroke_color: RGBA color for the shape outline (default: black)
            stroke_width: Width of the shape outline in pixels (default: 1)
        """
        self.fill_color = fill_color
        self.stroke_color = stroke_color
        self.stroke_width = stroke_width
        self.x = x
        self.y = y
        self.width = width
        self.height = height

        self.text = Text(text, font_family=font_family, font_size=font_size)

    def draw(self, scene, window_state):
        fill_brush = Brush.solid(self.fill_color, window_state)
        text_brush = Brush.solid(self.stroke_color, window_state)  # Text color matches stroke color
        stroke_brush = Brush.solid(self.stroke_color, window_state)
        stroke_options = StrokeStyle(self.stroke_width, window_state)

        scene.build_text(window_state, self.text, text_brush)
        box_w, box_h = self.text.measure()

        if self.width is not None:
            box_w = self.width
            self.text.layout_width = self.width  # Set the text layout width to match the box width
        if self.height is not None:
            box_h = self.height

        box_shape = Shape.rectangle(box_w, box_h, x=self.x, y=self.y)

        scene.draw_shape_filled(window_state, box_shape, fill_brush)
        scene.draw_shape_stroked(window_state, box_shape, stroke_brush, stroke_options)


        scene.draw_text(window_state, self.text, x=self.x, y=self.y)
