from psydk import run_experiment
from psydk.visual.stimuli import TextStimulus
from psydk.visual.color import linrgb
from psydk.visual.geometry import cm, deg, px, rectangle, vh, vw




def main_function():
    def my_experiment(ctx):

        # create a window
        print("Creating window...")
        window = ctx.create_default_window(fullscreen=True, monitor=0)
        print("Window created")

        bg_stim = text = TextStimulus(
            "Success!",
            font_weight="medium",
            font_size=100,
            fill_color=linrgb(1, 0.1, 0.1),
            context=ctx,
        )

        print("Stimulus created")

        for i in range(500):
            bg_stim["font_size"] = 100 + i % 100
            frame = window.get_frame()
            frame.add(bg_stim)
            window.present(frame)


    print("This is the main function of tmod.py")
    run_experiment(my_experiment)
    import time
    time.sleep(2000)  # Just to keep the script running for a while
