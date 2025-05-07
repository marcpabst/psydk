# Getting Started

## Installation

You can easily install `psydk` from PyPI using pip or another package manager of your choice. Psydk bundles all the dependencies required to run the experiments. The only requirement is CPython 3.8 or higher.




`````{tabs}
````{group-tab} PyPi
Psydk is available on PyPI and has no other dependencies. You can install it using pip:

```bash
pip install psydk
```
````

````{group-tab} Git
Psydk is a valid PEP 508 package and can be installed from source using `pip install`, provided you have  up-to-date versions of Rust and cargo installed.

```bash
git clone https://github.com/marcpabst/psydk
cd psydk/psydk
pip install .
```
````


````{group-tab} Pixi
If you're using `pixi`, you can install psydk using the following command:

```bash
pixi add psydk --pypi
```
````

````{group-tab} iOS
iOS wheels are available from PyPi and reccemedned way of running experiments on iOS is briefcase. You can find more information on how to use psydk in the iOS documentation.
````
`````

## Your first experiment

### Setting up your development environment

This really comes down to you personal preferences. All you really need is a Python environment with `psydk` installed. No other dependencies required!

However, we strongly recommend using a virtual environment to avoid conflicts with other packages. In the following, we will use `pixi` to manage Python, psydk, and other dependencies. You can find more information on how to use `pixi` in the [pixi documentation](https://pixi.sh/latest/#installation). You can also use `conda` or `venv` to create a virtual environment, but you will miss out on some of the features that `pixi` provides.

`````{tabs}
````{group-tab} pixi
Create the pixi project and change into the project directory:
```bash
pixi create my_experiment
cd my_experiment
```
Then, install the dependencies (Python, psydk, and pandas):
```bash
pixi add python
pixi add psydk --pypi
pixi add pandas
```
````
````{group-tab} pip
If necessary, activate your virtual environment and install the dependencies:
```bash
pip install psydk pandas
```
````
`````

Next, you can create a new Python file in the project directory. You can name it anything you like, but we will use `my_experiment.py` in the following examples.

### Defining an experiment

Let's come up with a simple experiment to get you started. We will create a simple experiment that presents a series of shapes and records the time it takes for the participant to respond by pressing a key. The experiment will be run in a loop, and the results will be saved to a CSV file.

The easiest way to get started is to define an experiment function and annotate it with the `@experiment` decorator. The function can take an arbitrary number of arguments, but the first argument must be a {class}`~psydk.ExperimentContext` object.

Using tht decorator, you can run this function like any other Python function. The decorator will take care of setting up the experiment context and running the experiment:

```python
from psydk import experiment

@experiment
def my_experiment(ctx):
    print("Hello, world!")

if __name__ == "__main__":
    my_experiment()
```

This will run the experiment and print "Hello, world!" to the console. You can also pass arguments to the experiment function:

```python
from psydk import experiment

@experiment
def my_experiment(ctx, arg1, arg2):
    print(f"Hello, {arg1} and {arg2}!")
```

## Creating a window and showing a stimulus

The first thing you need to do is create a window. The easiest way to do this is to use the {meth}`~psydk.ExperimentContext.create_default_window` method. This will create a window with on the requested monitor and fullscreen mode.

Since a window is useless without actually displaying something, we also create a simple stimulis. For now, lets create a 100 pixel wide and 100 pixel high red square. We can do this using the {class}`~psydk.visual.stimuli.PatternStimulus` class:

```{note}
Colours are simply tuples of three or four floats in the range [0, 1]. The first three values are the red, green, and blue components of the colour, and the optional fourth value is the alpha (transparency) component. While it is perfectly possible to just pass tuples to function that expect a colour value, we recommend using the {func}`~psydk.visual.color.rgb` and {func}`~psydk.visual.color.linrgb` functions to define colours.
```


```python
from psydk import experiment
from psydk.visual import stimuli, rectangle, rgb

@experiment
def my_experiment(ctx):
    # Create a window
    window = ctx.create_default_window()

    # Create a red square
    square = stimuli.PatternStimulus(
        rectangle(width=100, height=100),
        fill_color=rgb(1, 0, 0),  # RGB color
    )

    # Show the square for 100 frames
    for _ in range(100):
        # Get the a frame from the window
        frame = window.get_frame()
        # Add the square to the frame
        frame.add(square)
        # Present the frame to the window
        window.present(frame)

if __name__ == "__main__":
    my_experiment()
```
