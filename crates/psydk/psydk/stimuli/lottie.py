from psydk.visual.color import rgb
from psydk.visual.geometry import px, Shape
from psydk.visual.stimuli import BaseStimulus
from psydk.visual.renderer import LottieAnimation

from .mixins import ClickableMixin

class LottieStimulus(ClickableMixin, BaseStimulus):
    def __init__(self, ctx, lottie_path, bounding_rect, mode="loop", speed=1.0):
        """Stimulus class for drawing basic shapes.

        Args:
            ctx: Experiment context provided by the psydk framework
            shape: A Shape object defining the geometry to draw
            fill_color: RGBA color for filling the shape (default: white)
            stroke_color: RGBA color for the shape outline (default: black)
            stroke_width: Width of the shape outline in pixels (default: 1)
        """
        ClickableMixin.__init__(self)
        self.lottie = LottieAnimation.from_file(lottie_path, playback_mode=mode, speed=speed)
        self.bounding_rect = bounding_rect
        self.lottie.play()

    def draw(self, scene, window_state):
        scene.draw_lottie_animation(window_state, self.lottie, self.bounding_rect)

    def contains_point(self, point, window_state):
        return self.bounding_rect.contains_point(point, window_state)

    def get_position(self):
        return self.bounding_rect.x, self.bounding_rect.y

    def set_position(self, x, y):
        self.bounding_rect = Shape.rectangle(self.bounding_rect.width, self.bounding_rect.height, x=x, y=y)
