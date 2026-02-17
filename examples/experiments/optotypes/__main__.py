from psydk.utils import now
from psydk.visual.color import rgb
from psydk.visual.geometry import cm, deg, px, rectangle, vh, vw, circle
from psydk.visual.stimuli import (
    PatternStimulus,
    TextStimulus,
    SVGStimulus,
)
from psydk import experiment, WindowConfig, ExperimentConfig

from typing import Callable, List, Tuple

import time
import sys
import os
import numpy as np
import pandas as pd

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

def get_gaze_on_screen(face_transform, camera_transform, eye_transform):
    # 1. TRANSPOSE FIXED: Check if the translation column is empty.
    # ARKit matrices are Column-Major. If loaded flat into numpy, they behave as Row-Major.
    # We transpose them to ensure they align with standard math (Translation in the last column).

    # Simple check: If the bottom-right element is 1 and the translation column (last column) is 0,0,0,
    # it is highly likely the matrix needs transposing.
    if np.allclose(face_transform[:3, 3], 0) and not np.allclose(face_transform[3, :3], 0):
        face_transform = face_transform.T
        camera_transform = camera_transform.T
        eye_transform = eye_transform.T

    # 2. Compute Eye in World Space
    eye_world = face_transform @ eye_transform

    # 3. Compute Eye in Camera Space
    # We invert the camera transform to go from World -> Camera Space
    camera_inv = np.linalg.inv(camera_transform)
    eye_camera = camera_inv @ eye_world

    # 4. Extract Ray Origin and Direction
    # Origin is the translation vector (Column 3)
    ray_origin = eye_camera[:3, 3]

    # Gaze Direction is the Z-axis (Column 2)
    # ARKit Face Coordinate System: +Z points OUT of the eye/face.
    gaze_direction = eye_camera[:3, 2]

    # Normalize
    gaze_direction = gaze_direction / np.linalg.norm(gaze_direction)

    # 5. Intersect with Screen Plane (Z = 0)
    # The camera is at Z=0 looking down -Z. The screen is essentially the XY plane at Z=0.
    # We calculate 't' (distance) where the ray hits Z=0.
    # Ray: P = Origin + t * Direction
    # 0 = Origin_z + t * Direction_z  ->  t = -Origin_z / Direction_z

    if abs(gaze_direction[2]) < 1e-6:
        return None # Parallel to screen

    t = -ray_origin[2] / gaze_direction[2]

    # If t is negative, the intersection is behind the eye (looking away from screen)
    if t < 0:
        return None

    intersection_point = ray_origin + t * gaze_direction

    # Return X, Y in meters relative to Camera Lens
    return intersection_point[0], intersection_point[1]

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
        bg = PatternStimulus(
            ctx,
            circle(vw(2)),  # Circle with radius of 2% viewport width
            x=-vw(0.5),     # Positioned left of center
            y=-vh(0.5),     # Positioned below center
            pattern="uniform",
            fill_color=rgb(1, 1, 1)  # White color
        )

        # Set working directory to module directory for loading SVG files
        os.chdir(os.path.dirname(os.path.abspath(__file__)))

        # Create text stimulus to display current letter size
        logmar_text = TextStimulus(
            f"size: {SIZE_LETTER_LOG:.2f}",
            x=0.,                           # Centered horizontally
            y=vh(0.5)-cm(2),               # Positioned near top
            font_size=cm(0.5),             # Font size of 0.5cm
            fill_color=rgb(0, 0, 0),    # Black text
            context=ctx,
        )

        debug_text = TextStimulus(
            f"debug",
            x=vw(-0.5),                    # Left side
            y=vh(0.5)-cm(2),               # Positioned near top
            font_size=cm(0.5),             # Font size of 0.5cm
            fill_color=rgb(0, 0, 0),    # Black text
            context=ctx,
        )

        debug_circle = PatternStimulus(
            ctx,
            circle(cm(0.5)),  # Circle with radius of 0.5 cm
            x=vw(-0.5),       # Positioned left side
            y=vh(0.5)-cm(5),  # Positioned below debug text
            pattern="uniform",
            fill_color=rgb(1, 0, 0)  # Red color
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
                width=deg(letter_size),        # Set width
                context=ctx,
            )
            for i, l in enumerate(letter_letters)
        ]

        # Initialize face tracker if running on iOS
        if sys.platform == "ios":
            from psydk.sensors import FaceTracker
            ft = FaceTracker()

        et_rows = []

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


            # work out framerate of face tracking

            for face_frame in ft.drain():
                timestamp = face_frame.timestamp()


            face_frame = ft.last_frame()

            dist = 1.0  # default distance in meters

            if face_frame is not None and len(face_frame.faces()) > 0:
                tracking_result = face_frame.faces()[0]
                left_eye_transform = tracking_result.left_eye_transform()
                right_eye_transform = tracking_result.right_eye_transform()
                camera_transform = tracking_result.camera_transform()
                face_transform = tracking_result.face_transform()

                # assume iPad Pro M4 13" dimensions
                # physical dimensions in meters: 0.2149 x 0.1626, in pixels: 2732 x 2048
                x_left, y_left = get_gaze_on_screen(
                    face_transform,
                    camera_transform,
                    left_eye_transform
                )
                x_right, y_right = get_gaze_on_screen(
                    face_transform,
                    camera_transform,
                    right_eye_transform
                )
                x = (x_left + x_right) / 2.0
                y = (y_left + y_right) / 2.0

                # convert from meters to relative coordinates on the iPad screen (0 to 1)
                x = (x / 0.2149)
                y = (y / 0.1626)

                # scale to pixels
                x = x * 2732
                y = y * 2048

                debug_circle["x"] = x * 5
                debug_circle["y"] = -y * 5

                dist = tracking_result.mean_eye_distance()

            # Add stimuli to frame if a valid distance is available
            if dist is not None:
                for ll in letters:
                    frame.add(ll)
                frame.add(logmar_text)
                frame.add(debug_text)
                frame.add(debug_circle)

                # Update window viewing distance (convert to mm)
                window.viewing_distance = dist * 1000

            # Present the current frame
            window.present(frame)


if __name__ == "__main__":
    run()
