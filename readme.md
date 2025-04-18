# psydk

### High-performance, low-latency, cross-platform experiment framework for the cognitive sciences.

> [!WARNING]
> This project is still a bit experimental, and not everything is completely working yet. Feel free to try it out and provide feedback, but be aware that things may change rapidly and that there may be bugs. **If you're interested in using psydk for your research, please feel free to reach out!**

[![PyPI - Version](https://img.shields.io/pypi/v/psydk?style=flat-square&logo=python&logoColor=%23FFFFFF&label=PyPi&labelColor=%23292929&color=%23016DAD)](https://pypi.org/project/psydk/)  ![GitHub License](https://img.shields.io/github/license/marcpabst/psydk?style=flat-square&label=License%20&labelColor=%23292929&color=brightgreen)

psydk is a framework for neuroscience and psychology experiments. It is designed to be fast, accurate, and cross-platform.

## Features

- **Accurate timing**: psydk uses the best available timing APIs on each platform to ensure that stimuli are presented at the right time and that you can synchronize your experiment with external devices (currently only supported on Windows and Mac OS).
- **High performance**: psydk is pretty fast. It uses the GPU (via the very maturi Skia library) to render vector and raster stimuli.
- **Cross-platform**: psydk runs on Windows, Mac OS, Linux, Android, and iOS.
- **Easy to use**: psydk is designed to be easy to use. You can write your experiment in Python and use the provided tools to run it on any platform.
- **Extensive self-testing**: psydk can make use of the Windows ETW API to measure the latency of the system and the experiment itself, to help you identify potential problems.
- **Open-source**: psydk is open-source and free to use. You can use it for commercial and non-commercial projects.

## Running on iOS
Together with [Briefcase](https://docs.beeware.org/en/latest/tutorial/tutorial-5/iOS.html), psydk can be used to design Python-based experiments for iOS. To do this, iOS builds are available from PyPi. A demo project will be available soon - feel free to open an issue if you're interested in this feature!

## Code Structure

Psydk is split into a number of crates:

- `psydk`: The core functionality of psydk. This is used to build the Python bindings using PyO3.
- `psydk-renderer`: The rendering engine for psydk.
- `timed-audio`: A library for playing audio with accurate timing.
- `serial-triggers`: A library for sending triggers over a serial port (optionally with accurate timing).
