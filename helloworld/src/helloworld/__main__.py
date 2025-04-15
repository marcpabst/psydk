import numpy as np


from psydk import run_experiment
from psydk.visual.geometry import deg, circle, path
from psydk.visual.stimuli import PatternStimulus
from psydk.visual.color import linrgb


def distance(p1, p2):
    """Calculate the distance between two points."""
    return np.sqrt((p1[0] - p2[0]) ** 2 + (p1[1] - p2[1]) ** 2)


def my_experiment(
    ctx, subject, session, run, overwrite=False, enable_feedback=False
) -> None:
    """Run the experiment.
    Parameters
    ----------
    ctx : psydk.ExperimentContext
        The experiment context.

    Returns
    -------
    None
    """

    with ctx.create_default_window(fullscreen=True, monitor=2) as window:
        # create event receiver
        event_receiver = window.create_event_receiver()

        n_trials = 100

        for trial in range(n_trials):
            # generate a random position for the target
            target_x = np.random.uniform(
                -window.get_size()[0] / 2, window.get_size()[0] / 2
            )
            target_y = np.random.uniform(
                -window.get_size()[1] / 2, window.get_size()[1] / 2
            )

            points = []

            # create a circle in the center of the screen
            circle_stim = PatternStimulus(
                circle(100),
                x=0,
                y=0,
                pattern="uniform",
                pattern_size=deg(0.5),
                pattern_rotation=0,
                fill_color=linrgb(0.1, 0.1, 0.1),
                stroke_width=25,
            )

            # create the target circle
            target_stim = PatternStimulus(
                circle(100),
                x=target_x,
                y=target_y,
                pattern="uniform",
                pattern_size=deg(0.5),
                pattern_rotation=0,
                fill_color=linrgb(0.1, 0.1, 0.1),
                stroke_width=25,
            )

            path_stim = PatternStimulus(
                path([]),
                x=0,
                y=0,
                pattern="uniform",
                pattern_size=deg(0.5),
                pattern_rotation=0,
                stroke_color=linrgb(0.5, 0.5, 0.5),
                stroke_width=25,
            )

            draw_state = {
                "points": [],
                "active": False,
                "finished": False,
            }

            def mouse_down_handler(event):
                if not draw_state["active"]:
                    # check if we are in the circle stimulus
                    if distance(event.position, (0, 0)) < 100:
                        path_stim["stroke_color"] = linrgb(0.5, 0.5, 0.5)
                        draw_state["points"] = points.clear()
                        draw_state["active"] = True

            def mouse_up_handler(event):
                if draw_state["active"]:
                    # check if we are in the target circle
                    # if yes, make the path green
                    if distance(event.position, (target_x, target_y)) < 100:
                        path_stim["stroke_color"] = linrgb(0, 1, 0)
                    else:
                        path_stim["stroke_color"] = linrgb(1, 0, 0)

                    draw_state["active"] = False
                    draw_state["finished"] = True

            def mouse_move_handler(event):
                if draw_state["active"]:
                    points.append(event.position)

            h1 = window.add_event_handler("mouse_button_press", mouse_down_handler)
            h2 = window.add_event_handler("mouse_button_release", mouse_up_handler)
            h3 = window.add_event_handler("cursor_moved", mouse_move_handler)

            while not draw_state["finished"]:
                # draw the path
                path_stim["shape"] = path(points)

                frame = window.get_frame()

                frame.add(circle_stim)
                frame.add(target_stim)
                frame.add(path_stim)

                window.present(frame)

            # remove event handlers
            window.remove_event_handler(h1)
            window.remove_event_handler(h2)
            window.remove_event_handler(h3)

            # show for 1 seconds
            frame = window.get_frame()
            frame.add(circle_stim)
            frame.add(target_stim)
            frame.add(path_stim)
            window.present(frame, repeat_time=0.5)


if __name__ == "__main__":
    run_experiment(my_experiment, subject="01", session="01", run="01", overwrite=True)
