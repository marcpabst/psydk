# this package makes GStreamer and its plugins available to Python code
# binaries are in ./gstreamer/OS_ARCH/VERSION/libs
from pathlib import Path
import platform

def get_gst_path():
    base_path = Path(__file__).parent / "gstreamer"
    system = platform.system().lower()
    arch = platform.machine().lower()
    gst_version = "1.24.11"
    gst_path = base_path / f"{system}_{arch}" / gst_version / "libs"
    # check that path exists
    if not gst_path.exists():
        raise FileNotFoundError(f"GStreamer path not found: {gst_path}. This is a bug in psydk-gst.")
    return gst_path
