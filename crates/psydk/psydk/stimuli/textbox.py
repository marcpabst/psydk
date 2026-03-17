from psydk.visual.color import rgb
from psydk.visual.geometry import px, cm, Shape
from psydk.visual.stimuli import BaseStimulus
from psydk.visual.renderer import Brush, StrokeStyle, Text


class TextboxStimulus(BaseStimulus):
    """Stimulus class for drawing a textbox with text inside.

    Parameters
    ----------
    ctx : object
        Experiment context provided by the psydk framework.
    text : str
        The text content to display inside the textbox.
    x : unit, optional
        X position of the top-left corner of the textbox (default: ``px(0)``).
    y : unit, optional
        Y position of the top-left corner of the textbox (default: ``px(0)``).
    width : unit or None, optional
        Width of the textbox. If None, it will be determined by the text
        content (default: None).
    height : unit or None, optional
        Height of the textbox. If None, it will be determined by the text
        content (default: None).
    bg_fill_color : rgba, optional
        RGBA color for filling the background of the textbox
        (default: ``rgb(1, 1, 1, 0)``, transparent).
    stroke_color : rgba, optional
        RGBA color for the textbox outline
        (default: ``rgb(0, 0, 0, 0)``, transparent).
    fill_color : rgba, optional
        RGBA color for filling the text (default: ``rgb(1, 1, 1)``, white).
    stroke_width : unit, optional
        Width of the textbox outline (default: ``px(5)``).
    font_size : unit, optional
        Font size for the text (default: ``cm(0.5)``).
    font_family : str, optional
        Font family for the text (default: ``"Arial"``).
    """


    def __init__(self, ctx, text, x=px(0), y=px(0), width=None, height=None,
        bg_fill_color=rgb(1, 1, 1, 0),
        stroke_color=rgb(0, 0, 0, 0),
        fill_color=rgb(1, 1, 1),
        stroke_width=px(5),
        font_size=cm(0.5),
        font_family="Arial"
    ):
        self.fill_color = fill_color
        self.bg_fill_color = bg_fill_color
        self.stroke_color = stroke_color
        self.stroke_width = stroke_width
        self.x = x
        self.y = y
        self.width = width
        self.height = height

        self.font_size = font_size
        self.font_family = font_family

        self._rawtext = text
        self._text = Text(text, font_family=self.font_family, font_size=self.font_size)

    @property
    def text(self):
        return self._rawtext

    @text.setter
    def text(self, value):
        self._rawtext = value
        self._text = Text(value, font_family=self.font_family, font_size=self.font_size)

    def draw(self, scene, window_state):
        fill_brush = Brush.solid(self.bg_fill_color, window_state)
        text_brush = Brush.solid(self.fill_color, window_state)  # Text color matches stroke color
        stroke_brush = Brush.solid(self.stroke_color, window_state)
        stroke_options = StrokeStyle(self.stroke_width, window_state)

        scene.build_text(window_state, self._text, text_brush)
        box_w, box_h = self._text.measure()

        if self.width is not None:
            box_w = self.width
            self._text.layout_width = self.width  # Set the text layout width to match the box width
        if self.height is not None:
            box_h = self.height

        box_shape = Shape.rectangle(box_w, box_h, x=self.x, y=self.y)

        scene.draw_shape_filled(window_state, box_shape, fill_brush)
        scene.draw_shape_stroked(window_state, box_shape, stroke_brush, stroke_options)


        scene.draw_text(window_state, self._text, x=self.x, y=self.y)
