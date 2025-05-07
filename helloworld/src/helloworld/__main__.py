import numpy as np


from psydk import run_experiment
from psydk.visual.geometry import deg, circle, path
from psydk.visual.stimuli import PatternStimulus, TextStimulus, ImageStimulus
from psydk.visual.color import linrgb
from psydk.utils import now

def distance(p1, p2):
    """Calculate the distance between two points."""
    return np.sqrt((p1[0] - p2[0]) ** 2 + (p1[1] - p2[1]) ** 2)



def my_experiment(ctx, subject, session, run, overwrite=False, enable_feedback=False) -> None:
    """Run the experiment.
    Parameters
    ----------
    ctx : psydk.ExperimentContext
        The experiment context.

    Returns
    -------
    None
    """

    # get direcortory of the current file
    from pathlib import Path
    res_directory = (Path(__file__).parent / "assets").resolve()

    # register mali font
    ctx.load_font_directory(str(res_directory / "fonts/mali"))

    with ctx.create_default_window(fullscreen=True, monitor=0) as window:
        n_trials = 10

        start_text = TextStimulus(
            "Tap anywhere to start", font_family="Mali", font_weight="medium", font_size=100, fill_color=linrgb(0, 0, 0)
        )

        end_text = TextStimulus(
            "Well done!", font_family="Mali", font_weight="medium", font_size=100, fill_color=linrgb(0, 0, 0)
        )


        key_receiver = window.create_event_receiver()

        while not any([e.kind == "touch_start" or e.kind == "mouse_button_press"   for e in key_receiver.poll().events()]):
            frame = window.get_frame()
            frame.add(start_text)
            window.present(frame)

        target_distance = 700
        n_locations = 4

        # generate n evenly spaced angles
        angles = np.linspace(0, 2 * np.pi, n_locations, endpoint=False)
        locations = [(target_distance * np.cos(angle), target_distance * np.sin(angle)) for angle in angles]

        for trial in range(n_trials):
            # generate a random position for the target
            # target_x = np.random.uniform(-window.get_size()[0] / 2, window.get_size()[0] / 2)
            # target_y = np.random.uniform(-window.get_size()[1] / 2, window.get_size()[1] / 2)

            # obtain a random position
            target_x, target_y = locations[trial % n_locations]
            angle = angles[trial % n_locations]


            # create a circle in the center of the screen
            circle_stim = PatternStimulus(
                circle(150),
                x=0,
                y=0,
                pattern="uniform",
                pattern_size=deg(0.5),
                pattern_rotation=0,
                fill_color=linrgb(0.1, 1.0, 0.1),
                stroke_width=5,
            )

            mouse_stim = ImageStimulus(
                str(res_directory / "imgs/mice/__white_idle_000.png"),
                x=0,
                y=0,
                width=140,
                height=240,
            )

            # create the target circle
            target_stim = PatternStimulus(
                circle(50),
                x=target_x,
                y=target_y,
                pattern="uniform",
                pattern_size=deg(0.5),
                pattern_rotation=0,
                fill_color=linrgb(0.1, 0.1, 0.1),
                stroke_width=25,
            )

            cheese_stim = ImageStimulus(
                str(res_directory / "imgs/cheese/cheese_02.png"),
                x=target_x,
                y=target_y,
                width=150,
                height=150,
            )

            path_stim = PatternStimulus(
                path([]),
                x=0,
                y=0,
                pattern="uniform",
                pattern_size=deg(0.5),
                pattern_rotation=0,
                stroke_color=linrgb(0.5, 0.5, 0.5),
                stroke_width=10,
            )

            draw_state = {
                "points": [],
                "started": False,
                "started_time": None,
                "active": False,
                "finished": False,
                "correct": None,
            }

            def mouse_down_handler(event):
                if distance(event.position, (0, 0)) < 100:
                    if draw_state["started"] and not draw_state["active"]:
                        path_stim["stroke_color"] = linrgb(0.5, 0.5, 0.5)
                        draw_state["points"].clear()
                    else:
                        draw_state["started"] = True
                    draw_state["started_time"] = now()

            def mouse_up_handler(event):
                if draw_state["started"] and draw_state["active"]:
                    # check if we are in the target circle
                    # if yes, make the path green
                    if distance(event.position, (target_x, target_y)) < 200:
                        path_stim["stroke_color"] = linrgb(0, 1, 0)
                        draw_state["correct"] = True
                    else:
                        path_stim["stroke_color"] = linrgb(1, 0, 0)
                        draw_state["correct"] = False

                    draw_state["active"] = False
                    draw_state["finished"] = True

                elif draw_state["started"]:
                    draw_state["started"] = False

            def mouse_move_handler(event):
                if draw_state["started"] and draw_state["active"]:
                    draw_state["points"].append(event.position)

            h1 = window.add_event_handler("mouse_button_press", mouse_down_handler)
            h2 = window.add_event_handler("mouse_button_release", mouse_up_handler)
            h3 = window.add_event_handler("cursor_moved", mouse_move_handler)

            # touch event handlers
            h4 = window.add_event_handler("touch_start", mouse_down_handler)
            h5 = window.add_event_handler("touch_end", mouse_up_handler)
            h6 = window.add_event_handler("touch_move", mouse_move_handler)

            while not draw_state["active"]:
                frame = window.get_frame()
                # check if 1 s has elapsed since started_time
                if draw_state["started"] and draw_state["started_time"] is not None and draw_state["started_time"].elapsed() > 1:
                    draw_state["active"] = True

                if draw_state["started"]:
                    frame.add(circle_stim)

                frame.add(mouse_stim)
                window.present(frame)

            while True:
                # draw the path
                path_stim["shape"] = path(draw_state["points"])

                frame = window.get_frame()


                frame.add(mouse_stim)
                # frame.add(target_stim)
                frame.add(cheese_stim)
                frame.add(path_stim)

                window.present(frame)

                if draw_state["finished"]:
                    break

            # remove event handlers
            window.remove_event_handler(h1)
            window.remove_event_handler(h2)
            window.remove_event_handler(h3)
            window.remove_event_handler(h4)
            window.remove_event_handler(h5)
            window.remove_event_handler(h6)

            # rotate the mouse stim to the target
            mouse_stim["rotation"] = np.rad2deg(angle) + 90

            # show for 1 seconds
            path_stim["shape"] = path(draw_state["points"])
            path_stim.animate("stroke_width", 0, 0.5)

            if draw_state["correct"]:
                mouse_stim.animate("x", target_x, 0.5)
                mouse_stim.animate("y", target_y, 0.5)

            frame = window.get_frame()
            frame.add(target_stim)
            frame.add(cheese_stim)
            frame.add(path_stim)
            frame.add(mouse_stim)
            window.present(frame, repeat_time=0.8)


        frame = window.get_frame()
        frame.add(end_text)
        window.present(frame, repeat_time=1.0)


if __name__ == "__main__":
    run_experiment(my_experiment, subject="01", session="01", run="01", overwrite=True)
