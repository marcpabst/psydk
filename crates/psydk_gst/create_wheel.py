# this creates a wheel file for the psydk_gst package
# there is no actual binary code in this package, just the GStreamer binaries in a subfolder (well, that is binary code, but not compiled by us)
# however, gstreamer is quite big to we only want to contain the relevant files for the current platform
# # we delcare our wheel compatible with all Python versions 3.x
import os
import platform
import shutil
from pathlib import Path
import tempfile
import setuptools


BUILD_FOR = ["macosx_10_13_arm64", "win32"]

def copy(src, dst):
    if os.path.islink(src):
        linkto = os.readlink(src)
        os.symlink(linkto, dst)
    else:
        shutil.copy(src,dst)

def create_wheel():
    base_path = Path(__file__).parent
    gst_base_path = base_path / "gstreamer"
    dist_path = base_path / "dist"
    dist_path.mkdir(exist_ok=True)

    for tag in BUILD_FOR:
        print(f"Creating wheel for {tag} on {platform.system().lower()}_{platform.machine().lower()}")

        gst_version = "1.24.11"
        gst_path = gst_base_path / f"{tag}" / gst_version / "libs"

        if not gst_path.exists():
            raise FileNotFoundError(f"GStreamer path not found: {gst_path}. This is a bug in psydk-gst.")


        # create a temporary directory to assemble the wheel contents
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            package_path = temp_path / "psydk_gst"
            package_path.mkdir()

            # copy __init__.py
            shutil.copy(base_path / "__init__.py", package_path / "__init__.py")

            # copy gstreamer binaries
            dest_gst_path = package_path / "gstreamer" / tag / gst_version / "libs"
            dest_gst_path.mkdir(parents=True)
            for item in gst_path.iterdir():
                dest_item_path = dest_gst_path / item.name
                if item.is_dir():
                    shutil.copytree(item, dest_item_path, symlinks=True)
                else:
                    copy(item, dest_item_path)

            # change working directory to temp_path
            os.chdir(temp_path)

            # create the wheel
            setuptools.setup(
                name="psydk_gst",
                version="0.1.0",
                packages=["psydk_gst"],
                package_data={"psydk_gst": [f"gstreamer/{tag}/{gst_version}/libs/**"]},
                include_package_data=True,
                dist_dir=str(dist_path),
                script_args=["bdist_wheel", "--universal", "--dist-dir", str(dist_path)],
                options={
                    "bdist_wheel": {
                        "plat_name": tag,

                    }
                },
            )





if __name__ == "__main__":
    create_wheel()
