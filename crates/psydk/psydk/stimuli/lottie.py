from psydk.visual.color import rgb
from psydk.visual.geometry import px
from psydk.visual.stimuli import BaseStimulus
from psydk.visual.renderer import LottieAnimation


class LottieStimulus(BaseStimulus):
    def __init__(self, ctx, lottie_path, bounding_rect=None, mode="loop", speed=1.0):
        """Stimulus class for drawing basic shapes.

        Args:
            ctx: Experiment context provided by the psydk framework
            shape: A Shape object defining the geometry to draw
            fill_color: RGBA color for filling the shape (default: white)
            stroke_color: RGBA color for the shape outline (default: black)
            stroke_width: Width of the shape outline in pixels (default: 1)
        """
        self.lottie = LottieAnimation.from_file(lottie_path, playback_mode=mode, speed=speed)
        self.bounding_rect = bounding_rect
        self.lottie.play()

    def draw(self, scene, window_state):
        scene.draw_lottie_animation(window_state, self.lottie, self.bounding_rect)
