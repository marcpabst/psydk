from psydk.utils import now
from psydk.visual.color import linrgb, luv, xyz
from psydk.visual.geometry import cm, deg, px, rectangle, vh, vw, circle
from psydk.visual.stimuli import (
    PatternStimulus,
    TextStimulus,
    SVGStimulus,
)
from psydk import run_experiment, DisplayConfig, ExperimentConfig

from typing import Callable, List, Tuple

import time
import sys
import os
import numpy as np

# Define optotype configurations
SLOAN_LETTERS = {
    "letters": ["C", "D", "H", "K", "N", "O", "R", "S", "V", "Z"],
    "path": "letters/sloan/{x}.svg",
    "logmar2deg": 0.1,
}

LANDHOLT_C_LETTERS = {
    "letters": ["C0", "C90", "C180", "C270"],
    "path": "letters/landholt/{x}.svg",
    "logmar2deg": 0.1,
}

# Select the current optotype set to use
LETTERS = SLOAN_LETTERS
N_LETTERS = 5  # Number of letters to display
SIZE_LETTER_LOG = 1.0  # Initial letter size in logMAR


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
        bg = PatternStimulus(
            circle(vw(2)),  # Circle with radius of 2% viewport width
            x=-vw(0.5),     # Positioned left of center
            y=-vh(0.5),     # Positioned below center
            pattern="uniform",
            fill_color=linrgb(1, 1, 1)  # White color
        )

        # Set working directory to module directory for loading SVG files
        os.chdir(os.path.dirname(os.path.abspath(__file__)))

        # Create text stimulus to display current letter size
        logmar_text = TextStimulus(
            f"size: {SIZE_LETTER_LOG:.2f}",
            x=0.,                           # Centered horizontally
            y=vh(0.5)-cm(2),               # Positioned near top
            font_size=cm(0.5),             # Font size of 0.5cm
            fill_color=linrgb(0, 0, 0),    # Black text
        )

        # Randomly select letters to display
        letter_letters = np.random.choice(
            LETTERS["letters"],
            size=N_LETTERS,
            replace=False
        )

        # Calculate initial letter size in degrees of visual angle
        log_letter_size = SIZE_LETTER_LOG
        letter_size = (10 ** log_letter_size) * LETTERS["logmar2deg"]

        # Create SVG stimuli for each letter
        letters = [
            SVGStimulus(
                LETTERS["path"].format(x=l),  # Path to the SVG file
                # Calculate x position to center the row of letters
                x=deg((i - (len(letter_letters) - 1) / 2) * (letter_size + letter_size * 2)),
                y=deg(0),                     # Centered vertically
                height=deg(letter_size),      # Set height
                width=deg(letter_size)        # Set width
            )
            for i, l in enumerate(letter_letters)
        ]

        # Initialize face tracker if running on iOS
        if sys.platform == "ios":
            from psydk.sensors import FaceTracker
            ft = FaceTracker()

        # Main experiment loop
        while True:
            # Check if background stimulus was clicked
            if bg.clicked():
                print("Screen clicked, updating letters")
                # Decrease letter size logarithmically
                log_letter_size -= 0.1
                letter_size = (10 ** log_letter_size) * LETTERS["logmar2deg"]

                # Reset size if it gets too small
                if log_letter_size < -0.3:
                    log_letter_size = SIZE_LETTER_LOG
                    letter_size = (10 ** log_letter_size) * LETTERS["logmar2deg"]

                # Update each letter's properties
                for i, ll in enumerate(letters):
                    ll["height"] = deg(letter_size)
                    ll["width"] = deg(letter_size)
                    # Recalculate position based on new size
                    ll["x"] = deg((i - (len(letter_letters) - 1) / 2) * (letter_size + letter_size * 2))

                # Update text with new size value
                logmar_text["text"] = f"size: {log_letter_size:.2f}"

            # Create and render frame
            frame = window.get_frame()
            frame.add(bg)

            # Get face distance if on iOS, otherwise use a default value
            if sys.platform == "ios":
                dist = ft.get_last_face_distance()
            else:
                dist = 4.0  # Default viewing distance in meters

            # Add stimuli to frame if a valid distance is available
            if dist is not None:
                for ll in letters:
                    frame.add(ll)
                frame.add(logmar_text)

                # Update window viewing distance (convert to mm)
                window.viewing_distance = dist * 1000

            # Present the current frame
            window.present(frame)


if __name__ == "__main__":
    # Configure experiment with 16-bit floating point color depth
    exp_config = ExperimentConfig(internal_color_depth="16F")
    # Run the experiment with the specified configuration
    run_experiment(run, config=exp_config)
