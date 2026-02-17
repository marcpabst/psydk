from psydk import experiment
from psydk.visual.color import rgb
from psydk.visual.geometry import cm, vh, vw, px, Shape
from psydk.stimuli import ShapeStimulus, FixationCrossStimulus, LottieStimulus, ButtonStimulus, TextboxStimulus
from psydk import WindowConfig, ExperimentConfig

import numpy as np

@experiment(ExperimentConfig(internal_color_type="10U"))
def run(ctx, *args, **kwargs):

    ctx.load_system_fonts()

    # win_conf = WindowConfig(calibration_file="debug_calibration.json")
    win_conf = WindowConfig(surface_color_type="10U")

    # Create the main experiment window
    with ctx.create_default_window(config=win_conf) as window:

        stim_balloon = LottieStimulus(ctx, "balloon.json", mode="loop", speed=1.2, bounding_rect=Shape.rectangle(cm(10), cm(10), x=-cm(5), y=-cm(5)))

        stim_button = ButtonStimulus(ctx, "Click me Please!", x=px(100), y=px(100), fill_color=rgb(0, 1, 0))

        stim_textbox = TextboxStimulus(ctx, "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.",
            x=px(-500), y=px(-500), width=cm(15), height=cm(5),
            fill_color=rgb(1, 1, 0),
            font_size=cm(1), font_family="Times New Roman",
            stroke_color=rgb(0, 0, 1),
            stroke_width=px(2)
        )

        stim_bg = ShapeStimulus(
            ctx,
            Shape.rectangle(vw(1), vh(1), x=-vw(0.5), y=-vh(0.5)),  # Full-screen rectangle
            fill_color=rgb(1, 1, 1),  # White color
            stroke_color=rgb(1, 1, 1, 1),
            stroke_width=0
        )

        stim_circle = ShapeStimulus(
            ctx,
            Shape.circle(vw(0.2)),  # Circle with radius of 2% viewport width
            fill_color=rgb(1, 0, 0, 0.5),  # White color
            stroke_color=rgb(0, 0, 0, 1),
            stroke_width=cm(5)
        )

        stim_cross = FixationCrossStimulus(ctx)

        for i in range(100000):
            frame = window.get_frame()
            stim_circle.fill_color = rgb(np.sin(i/10)*0.5+0.5, 0, 0, 1)

            frame.add(stim_bg)
            # frame.add(stim_circle)

            frame.add(stim_balloon)
            frame.add(stim_cross)
            frame.add(stim_button)
            frame.add(stim_textbox)

            # Present the current frame
            window.present(frame)


if __name__ == "__main__":



    run()
