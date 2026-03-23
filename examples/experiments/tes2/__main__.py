from psydk import experiment
from psydk.visual.color import rgb
from psydk.visual.geometry import cm, vh, vw, px, Shape
from psydk.stimuli import ShapeStimulus, FixationCrossStimulus, LottieStimulus, ButtonStimulus, TextboxStimulus, DraggableStimulus, SVGStimulus
from psydk import WindowConfig, ExperimentConfig

import numpy as np

@experiment(ExperimentConfig(internal_color_type="10U"))
def my_experiment(ctx, *args, **kwargs):

    # Create the main experiment window
    with ctx.create_default_window(config=WindowConfig(surface_color_type="10U")) as window:

        def escape_handler(event):
            if event.key == "Escape":
                exit()

        window.add_event_handler("key_press", escape_handler)

        er = window.create_event_receiver()

        # stim_balloon = DraggableStimulus(ctx, LottieStimulus(ctx, "balloon.json", mode="loop", speed=1.2, bounding_rect=Shape.rectangle(cm(10), cm(10), x=-cm(5), y=-cm(5))))

        stim_button = ButtonStimulus(ctx, "Click me Please!", x=px(100), y=px(100), fill_color=rgb(0, 1, 0))
        stim_button.add_click_handler(lambda _: print("Button clicked!"))

        stim_textbox = TextboxStimulus(ctx, "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.",
            x=px(-500), y=px(-500), width=cm(15),
            fill_color=rgb(1, 1, 0),
            font_size=px(40), font_family="Times New Roman",
            stroke_color=rgb(0, 0, 1),
            stroke_width=px(2),
        )

        # svg_stim = SVGStimulus(ctx, "C0.svg", px(0), px(0), width=cm(10), height=cm(10))

        stim_bg = ShapeStimulus(
            ctx,
            Shape.rectangle(vw(1), vh(1), x=-vw(0.5), y=-vh(0.5)),  # Full-screen rectangle
            fill_color=rgb(1, 1, 1),  # White color
            stroke_color=rgb(1, 1, 1, 1),
            stroke_width=0
        )

        stim_circle = ShapeStimulus(
            ctx,
            Shape.circle(vw(0.3), x=vw(-0.4), y=vh(0.25)),  # Circle with radius of 20% viewport width, positioned in bottom-right corner
            fill_color=rgb(1, 1, 1),  # White color
            stroke_color=rgb(0, 0, 0),
            stroke_width=cm(0)
        )

        stim_circle.add_click_handler(lambda _: print("Circle clicked!"))


        stim_cross = FixationCrossStimulus(ctx)

        while True:

            # for e in er.poll().events():
            #     if e.kind == "key_press":
            #         print(f"Key pressed: {e.key}")

            # Obtain a new frame to draw on
            frame = window.get_frame()

            # Add stimuli to the frame in the desired drawing order
            frame.add(stim_bg)
            # frame.add(stim_balloon)
            frame.add(stim_cross)
            frame.add(stim_button)
            frame.add(stim_textbox)
            frame.add(stim_circle)
            # frame.add(svg_stim)

            # Present the current frame
            window.present(frame)


if __name__ == "__main__":
    print("Starting experiment...")
    my_experiment()
