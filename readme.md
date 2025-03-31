# psydk

### High-performance, low-latency, cross-platform experiment framework for the cognitive sciences.

> [!WARNING]
> This project is still in early development, and not everything is working yet. Feel free to try it out and provide feedback, but be aware that things may change rapidly and that there may be bugs. Issues and pull requests are welcome!

![PyPI - Version](https://img.shields.io/pypi/v/psydk?style=flat-square&logo=python&logoColor=%23FFFFFF&label=PyPi&labelColor=%23292929&color=%23016DAD) ![PyPI - Version](https://img.shields.io/pypi/v/psydk-py?style=flat-square&logo=anaconda&logoColor=%23FFFFFF&label=Conda&labelColor=%23292929&color=%23016DAD) ![Crates.io Version](https://img.shields.io/crates/v/psydk?style=flat-square&logo=rust&label=Crates.io&labelColor=%23292929&color=%23E43716) ![GitHub Release](https://img.shields.io/github/v/release/marcpabst/psydk?include_prereleases&style=flat-square&logo=github&logoColor=white&label=Release&labelColor=%233292929&color=%23e3e3e3) ![GitHub License](https://img.shields.io/github/license/marcpabst/psydk?style=flat-square&label=License%20&labelColor=%23292929&color=brightgreen)

psydk is a framework for neuroscience and psychology experiments. It is designed to be fast, accurate, and cross-platform.

## Features

- **Accurate timing**: psydk uses the best available timing APIs on each platform to ensure that stimuli are presented at the right time and that you can synchronize your experiment with external devices (currently only supported on Windows and Mac OS).
- **High performance**: psydk is pretty fast. It uses the GPU (via the very maturi Skia library) to render vector and raster stimuli.
- **Cross-platform**: psydk runs on Windows, Mac OS, Linux, Android, and iOS.
- **Easy to use**: psydk is designed to be easy to use. You can write your experiment in Python and use the provided tools to run it on any platform.
- **Extensive self-testing**: psydk can make use of the Windows ETW API to measure the latency of the system and the experiment itself, to help you identify potential problems.
- **Open-source**: psydk is open-source and free to use. You can use it for commercial and non-commercial projects.

## Code Structure

Psydk is split into a number of crates:

- `psydk`: The core functionality of psydk and the main entry point for the library. This is used to build the Python bindings using PyO3.
- `psydk-renderer`: The rendering engine for psydk.
- `timed-audio`: A library for playing audio with accurate timing.
- `serial-triggers`: A library for sending triggers over a serial port (optionally with accurate timing).
