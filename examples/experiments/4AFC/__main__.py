import os
import time
import pandas as pd
import logging
from bids_like import BIDSLayout

from psydk import experiment, ExperimentConfig
from psydk.visual.color import rgb
from psydk.visual.geometry import rectangle, circle, cm, deg, px, vh, vw
from psydk.stimuli import ShapeStimulus, TextboxStimulus

# define the experiment using the @experiment decorator, which optionally takes an ExperimentConfig object to specify experiment-wide settings
@experiment(config=ExperimentConfig(internal_color_type="10U"))
def mytask(ctx, subject, session, condition):
    """
    A simple 4AFC task where the participant has to click on one of four circles.
    The target circle is colored differently on each trial.
    """

    # get the directory of the current script to use as a base for resource paths
    res_dir = os.path.dirname(os.path.abspath(__file__)) + "/resources"

    # generate a timestamp
    timestamp = time.strftime("%Y%m%d%H%M%S")

    # create a BIDSLayout object for managing file paths and metadata according to the BIDS standard
    layout = BIDSLayout("./")

    # generate a file path for the behavioral data TSV file using the BIDSLayout, which will create a path like "sub-test/ses-001/beh/sub-test_ses-001_task-default_beh.tsv"
    df_path = str(
        layout.generate_path(
            {
                "subject": subject,
                "session": session,
                "datatype": "beh",
                "task": condition,
                "timestamp": timestamp,
                "suffix": "beh",
                "extension": ".tsv",
            }
        )
    )

    log_path = str(
        layout.generate_path(
            {
                "subject": subject,
                "session": session,
                "datatype": "beh",
                "task": condition,
                "timestamp": timestamp,
                "suffix": "log",
                "extension": ".txt",
            }
        )
    )

    # set up logging to write to the generated log file path and also print to the console
    logging.basicConfig(
        level=logging.INFO,
        format="%(asctime)s - %(levelname)s - %(message)s",
        handlers=[
            logging.FileHandler(log_path),
            logging.StreamHandler()
        ]
    )


    # open a window
    with ctx.create_default_window() as window:

        # define a black background stimulus that covers the entire window (we could also set the frame background color to black instead of using a stimulus, but this is just for demonstration)
        bg = ShapeStimulus(
            ctx,
            rectangle(vw(1), vh(1), x=-vw(0.5), y=-vh(0.5)),
            fill_color=rgb(0, 0, 0)
        )

        # create 4 circular stimuli
        stims = [
            ShapeStimulus(
                ctx,
                circle(vw(0.1), x=-vw(0.2), y=vw(0.0)),
                fill_color=rgb(0.2, 0.2, 0.2)
            ),
            ShapeStimulus(
                ctx,
                circle(vw(0.1), x=vw(0.2), y=vw(0.0)),
                fill_color=rgb(0.2, 0.2, 0.2)
            ),
            ShapeStimulus(
                ctx,
                circle(vw(0.1), x=vw(0.0), y=vw(0.2)),
                fill_color=rgb(0.2, 0.2, 0.2)
            ),
            ShapeStimulus(
                ctx,
                circle(vw(0.1), x=vw(0.0), y=-vw(0.2)),
                fill_color=rgb(0.2, 0.2, 0.2)
            )
        ]

        # create a text stimulus to display trial information
        text_stim = TextboxStimulus(
            ctx,
            "...",
            font_size=vw(0.02),
            x=-vw(0.5) + cm(3),
            y=vh(0.5) - cm(2),
        )

        # ensure directory exists
        os.makedirs(os.path.dirname(df_path), exist_ok=True)

        # create a dummy trial list
        trial_list = [
            {"target_color": rgb(0.8, 0.0, 0.0), "target_index": 0},
            {"target_color": rgb(0.0, 0.8, 0.0), "target_index": 1},
            {"target_color": rgb(0.0, 0.0, 0.8), "target_index": 2},
            {"target_color": rgb(0.8, 0.8, 0.8), "target_index": 3},
        ] * 2


        trial_df_rows = []

        logging.info(f"Starting experiment with {len(trial_list)} trials")

        for trial_i, trial in enumerate(trial_list):
            # 500ms inter-trial interval
            iti_start = time.time()

            while time.time() - iti_start < .5:
                frame = window.get_frame()
                frame.add(bg)
                window.present(frame)

            # set all stimuli to gray
            for i, stim in enumerate(stims):
                stim.fill_color = rgb(0.2, 0.2, 0.2)

            # set target stimulus color
            stims[trial["target_index"]].fill_color = trial["target_color"]

            # set text to indicate trial number
            text_stim.text = f"Trial {trial_i + 1}"

            # wait for user to click one of the stimuli
            while True:
                # check if any stimulus is clicked
                clicked_index = next((i for i, stim in enumerate(stims) if stim.clicked()), None)

                # if a stimulus was clicked, check if it was the correct one and log the result
                if clicked_index is not None:

                    _correct = clicked_index == trial["target_index"]

                    logging.info(f"Trial {trial_i + 1}: clicked index {clicked_index}, target index {trial['target_index']}, correct: {_correct}")

                    trial_df_rows.append({
                        "trial": trial_i + 1,
                        "target_index": trial["target_index"],
                        "clicked_index": clicked_index,
                        "timestamp": time.time(),
                        "correct": _correct,
                    })

                    # save the trial data to a TSV file after each trial
                    trial_df = pd.DataFrame(trial_df_rows)
                    trial_df.to_csv(df_path, sep="\t", index=False)

                    # end inner loop for current trial and move on to the next one
                    break

                # render the next frame
                frame = window.get_frame()
                frame.add(bg)
                for i, stim in enumerate(stims):
                    frame.add(stim)
                frame.add(text_stim)
                window.present(frame)

        logging.info("Experiment completed successfully")


if __name__ == "__main__":

    # define subject, session, and condition identifiers for BIDS-compliant data storage
    # this should be updated to either read from user input, command-line arguments, or environment variables in a real experiment
    subject = "test"
    session = "001"
    condition = "default"

    # run the experiment
    mytask(subject, session, condition)
