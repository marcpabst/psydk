# import the contents of the Rust library into the Python extension
# set gstreamer plugin environment variable to site-packages/psydk/.dylibs/
import platform

from .psydk import *

# optional: include the documentation from the Rust module
from .psydk import (
    __all__,
    __doc__,  # noqa: F401
    __version__,  # noqa: F401
)

if platform.system() == "Darwin":
    import os

    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), ".dylibs")
    os.environ["GST_PLUGIN_PATH"] = path + ":" + os.environ.get("GST_PLUGIN_PATH", "")


# decorator that will turn an ordinary function myfun(...) into a psydk experiment
def experiment(config):
    def decorator(function):
        def wrapper(*args, **kwargs):
            return run_experiment(function, config, *args, **kwargs)

        return wrapper

    return decorator
