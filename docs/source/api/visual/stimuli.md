# Creating and Displaying Visual Stimuli

The visual module contains classes for creating visual stimuli.

## Basic Building Blocks

Psydk supports a variety of visual stimuli, including simple patterns, images, and {py:class}`~psydk.visual.stimuli.TextStimulus`. To create a visual stimulus, you typically create an instance of a stimulus class and then add it to a {class}`~psydk.visual.Frame` object, which can be obtained from a {class}`~psydk.visual.Window` object using the {meth}`~psydk.visual.Window.get_frame` method:

```python
stimulus = ShapeStimulus(...)

while True:
    frame = window.get_frame()
    frame.add_stimulus(stimulus)
    frame.present()
```

```{eval-rst}
.. automodule:: psydk.visual
  :members:
  :undoc-members:
```

## Geometry

The `geometry` module provides classes for creating and manipulating geometric shapes and specifying properties in physical units.

Whenever you need to specify a physical dimension, such as the size of a stimulus or the position of a point, you can either pass

1. a numeric value, which will be interpreted in pixels,
2. a string with a unit suffix (e.g., `"1.5cm"`, `"2in"`, `"3mm"`),
3. a {class}`~psydk.visual.geometry.Size` object (or a tuple of these), or
4. an expression that combines multiple {class}`~psydk.visual.geometry.Size` objects using arithmetic operations.

To make working with physical units easier, the `geometry` module provides a set of convenience functions for specifying common units:

| Function | Unit | Note |
| --- | --- | --- |
| {func}`~psydk.visual.geometry.px` | Pixels | |
| {func}`~psydk.visual.geometry.cm` | Centimeters | depends on the screen's pixel density (DPI) |
| {func}`~psydk.visual.geometry.in` | Inches | depends on the screen's pixel density (DPI) |
| {func}`~psydk.visual.geometry.mm` | Millimeters | | depends on the screen's pixel density (DPI) |
| {func}`~psydk.visual.geometry.pt` | Points (1/72 of an inch) | depends on the screen's pixel density (DPI) |
| {func}`~psydk.visual.geometry.deg` | Degrees of visual angle | depends on the screen's pixel density (DPI) and the viewing distance |


<!--({func}`~psydk.visual.geometry.cm`, {func}`~psydk.visual.geometry.in`, {func}`~psydk.visual.geometry.mm`, {func}`~psydk.visual.geometry.px`, and {func}`~psydk.visual.geometry.pt`). These functions all return a {class}`~psydk.visual.geometry.Size` object.-->

```{eval-rst}
.. automodule:: psydk.visual.geometry
  :members:
  :undoc-members:
```

## Stimuli

Stimuli are the basic building blocks of visual experiments. They are the objects that are displayed on the screen to the participant.

```{eval-rst}
.. automodule:: psydk.stimuli
  :members:
  :undoc-members:
```
