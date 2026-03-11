from psydk.visual.color import rgb
from psydk.visual.geometry import px, Shape
from psydk.visual.stimuli import BaseStimulus
from psydk.visual.renderer import LottieAnimation

from .mixins import ClickableMixin

class LottieStimulus(ClickableMixin, BaseStimulus):
    """Stimulus class for displaying Lottie animations.

    Parameters
    ----------
    ctx : object
        Experiment context provided by the psydk framework.
    lottie_path : str
        File path to the Lottie JSON animation file.
    bounding_rect : Shape
        A Shape.rectangle defining the area where the animation should be displayed.
    mode : str, optional
        Playback mode for the animation. Can be "loop", "play_once", or "ping_pong" (default: "loop").
    speed : float, optional
        Playback speed multiplier for the animation (default: 1.0).
    """
    def __init__(self, ctx, lottie_path, bounding_rect, mode="loop", speed=1.0):

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
