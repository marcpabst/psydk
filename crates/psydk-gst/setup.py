# setup.py
import os
import platform
import shutil
from pathlib import Path
import tempfile
import setuptools
import tomllib

# BUILD_FOR = ["macosx_11_0_arm64", "win32"]

base_path = Path(__file__).parent
dist_path = base_path / "dist"
dist_path.mkdir(exist_ok=True)

# 2. Read the version from pyproject.toml
with open("pyproject.toml", "rb") as f:
    pyproject_data = tomllib.load(f)
    # Navigate the TOML structure
    PKG_VERSION = pyproject_data["project"]["version"]
    # remove the build number if present
    # e.g. "1.2.3-4" -> "1.2.3"
    GST_VERSION = PKG_VERSION.split("-")[0]


# get the tag from the environment
TAG = os.environ.get('WHEEL_TAG')
if not TAG:
    raise RuntimeError("You must set the WHEEL_TAG environment variable to build.")

print(f"Creating wheel for {TAG} on {platform.system().lower()}_{platform.machine().lower()}")

# version number in pyproject.toml matches the folder name here
gst_version = GST_VERSION
gst_base_path = Path("gstreamer")
gst_path = gst_base_path / gst_version / f"{TAG}"

#
full_path =  (Path(__file__).parent / "src" / "psydk_gst" / gst_base_path)
if not full_path.exists():
    # print current working directory
    print(f"Current working directory: {os.getcwd()}")
    raise FileNotFoundError(f"GStreamer path not found: {full_path}. This likely means that the GStreamer binaries for {TAG} are not available.")
else:
    print(f"Found GStreamer binaries at {gst_path}")


if "win" in TAG:
    package_data_paths = [
        f"{gst_path}/bin/**.dll",
        f"{gst_path}/lib/**.dll",
        f"{gst_path}/share/**",
    ]
elif "macosx" in TAG:
    package_data_paths = [
        f"{gst_path}/lib/**.dylib",
        f"{gst_path}/lib/**.so",
        f"{gst_path}/share/**",
        f"{gst_path}/libexec/**",
    ]

print("Package data paths:")
for p in package_data_paths:
    print(f" - {p}")


# create the wheel
setuptools.setup(
    package_data={"psydk_gst": package_data_paths},
    include_package_data=False,
    script_args=["bdist_wheel", "--dist-dir", str(dist_path)],
    options={
        "bdist_wheel": {
            "plat_name": TAG,
        }
    },
)

# clear the build folder
shutil.rmtree(base_path / "build", ignore_errors=True)
