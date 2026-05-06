from psydk.visual.geometry import Shape
from psydk.visual.stimuli import BaseStimulus
from psydk.visual.renderer import VectorGraphic

from .mixins import ClickableMixin

class SVGStimulus(ClickableMixin, BaseStimulus):
    """Stimulus class for displaying Lottie animations.

    Parameters
    ----------
    ctx : object
        Experiment context provided by the psydk framework.
    svg_path : str
        File path to the Lottie JSON animation file.
    x : Size
        The x-coordinate of the top-left corner of the area where the SVG should be displayed.
    y : Size
        The y-coordinate of the top-left corner of the area where the SVG should be displayed.
    width : Size
        The width of the area where the SVG should be displayed.
    height : Size
        The height of the area where the SVG should be displayed.
    """
    def __init__(self, ctx, svg, x, y, width, height):
        ClickableMixin.__init__(self)
        # check if svg is a file path or an SVG string
        if isinstance(svg, str) and svg.strip().startswith("<"):
            self.svg = VectorGraphic.from_svg_str(svg)
        else:
            self.svg = VectorGraphic.from_svg_path(svg)

        self.x = x
        self.y = y
        self.width = width
        self.height = height

    def draw(self, scene, window_state):
        scene.draw_vector_graphic(window_state, self.svg, x=self.x, y=self.y, width=self.width, height=self.height)

    def contains_point(self, point, window_state):
        return False
