from psydk.utils import now
from psydk.visual.color import rgb
from psydk.visual.geometry import cm, deg, px, rectangle, vh, vw, circle
from psydk.visual.stimuli import (
    PatternStimulus,
    TextStimulus,
    BaseStimulus,
    SVGStimulus,
)
from psydk.visual.renderer import (
    Shape,
)
from psydk import experiment, WindowConfig, ExperimentConfig


class FixationCrossStimulus(BaseStimulus):
    def __init__(self, ctx):
        self.shape = Shape.new_rectangle(0,0, 100.0, 100.0)
        pass

    def draw(self, scene, window_state):
        scene.set_bg_color((1.0, 0.0, 0.0, 1.0))
        print(self.shape)

        pass

@experiment(None)
def run(ctx, *args, **kwargs):
    """
    Main experiment function that displays optotypes and handles user interaction.

    Args:
        ctx: Experiment context provided by the psydk framework
        *args: Additional positional arguments
        **kwargs: Additional keyword arguments
    """

    # Create the main experiment window
    with ctx.create_default_window() as window:
        # Create background stimulus - a white circle in the bottom-left corner
        stim_circle = PatternStimulus(
            ctx,
            circle(vw(0.2)),  # Circle with radius of 2% viewport width
            x=vw(0),     # Positioned left of center
            y=vh(0),     # Positioned below center
            pattern="uniform",
            stroke_width = px(10),
            fill_color=rgb(1, 0, 0)  # White color
        )

        stim_cross = FixationCrossStimulus(ctx)

        for i in range(100000):
            print(f"Frame {i} at time {now()}")
            frame = window.get_frame()
            # Draw background stimulus
            stim_circle["fill_color"] = rgb((i % 255) / 255, 0, 0)
            frame.add(stim_cross)
            frame.add(stim_circle)

            # Present the current frame
            window.present(frame)


if __name__ == "__main__":
    run()
