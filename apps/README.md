# Pysdk App

This directory contains applications to be used with the pysdk library. Currently, this only includes the 'psydk-launcher' application, which serves as a launcher for pysdk-based projects on iOS devices.

## psydk-launcher

The `psydk-launcher` application is designed to facilitate the launching of pysdk-based applications on iOS devices. When first launched, the app creates a folder that can be accessed via the Files app and via Finder on a connected Mac. The app will search for folders in this directory that contain a `pyproject.toml` file with a `[tool.psydk]` section, indicating that they are pysdk projects.

The `[tool.psydk]` table in the `pyproject.toml` file should include the following keys:
- `data_directory`: Specifies the directory where the app's data is stored.
- `options`: A dictionary of options for the pysdk project. These can either be directly defined as key-value pairs (e.g., `run_with_sound = true`) or by using a schema-like structure:
  - `label`: A human-readable label for the option.
  - `type`: The type of the option (one of `bool`, `int`, `float`, `str`, `enum`).
  - `default`: The default value for the option.
  - `options`: (only for `enum` type) A list of possible values.
- `tasks`: Either a list of task names or a dictionary of tables. A `task` table can include:
  - `options`: A dictionary of options that can override the global options for this specific task.

Example `pyproject.toml` configuration:

```toml
[project]
name = "MyPysdkApp"
version = "0.1.0"
description = "An example pysdk application."

[tool.psydk]
data_directory = "data"
options = { run_with_sound = true, difficulty = { label = "Difficulty", type = "enum", default = "normal", options = ["easy", "normal", "hard"] } }
tasks = ["practice", "game"]
# or
# tasks = { practice = { options = { difficulty = "easy" } }, game = {} }
```
