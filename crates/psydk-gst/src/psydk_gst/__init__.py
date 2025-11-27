# this package makes GStreamer and its plugins available to Python code
# binaries are in ./gstreamer/version/tag/
from pathlib import Path
import platform

def get_gst_path():
    base_path = Path(__file__).parent / "gstreamer"

    # check that there is exactly one version folder
    versions = [d for d in base_path.iterdir() if d.is_dir()]
    if len(versions) != 1:
        raise RuntimeError(f"Expected exactly one GStreamer version folder in {base_path}, found: {[v.name for v in versions]}")
    version_folder = versions[0]

    # check that there is exactly one tag folder
    tags = [d for d in version_folder.iterdir() if d.is_dir()]
    if len(tags) != 1:
        raise RuntimeError(f"Expected exactly one GStreamer tag folder for {platform.system().lower()}_{platform.machine().lower()} in {version_folder}, found: {[t.name for t in tags]}")
    tag_folder = tags[0]

    gst_path = tag_folder

    # check that path exists
    if not gst_path.exists():
        raise FileNotFoundError(f"GStreamer path not found: {gst_path}. This is a bug in psydk-gst.")
    return gst_path
