# Desinging Stimuli

The visual module contains classes for creating visual stimuli.

## Basic Building Blocks

Psydk supports a variety of visual stimuli, including simple patterns, images, and {py:class}`~psydk.visual.stimuli.TextStimulus`. To create a visual stimulus, you typically create an instance of a stimulus class and then add it to a {class}`~psydk.visual.Frame` object, which can be obtained from a {class}`~psydk.visual.Window` object using the {meth}`~psydk.visual.Window.get_frame` method.

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

To make working with physical units easier, the `geometry` module provides a set of convenience functions for specifying common units ({func}`~psydk.visual.geometry.cm`, {func}`~psydk.visual.geometry.in`, {func}`~psydk.visual.geometry.mm`, {func}`~psydk.visual.geometry.px`, and {func}`~psydk.visual.geometry.pt`). These functions all return a {class}`~psydk.visual.geometry.Size` object.

```{eval-rst}
.. automodule:: psydk.visual.geometry
  :members:
  :undoc-members:
```

## Stimuli

Stimuli are the basic building blocks of visual experiments. They are the objects that are displayed on the screen to the participant.

### PatternStimulus

A pattern stimulus is a versatile class for creating visual stimuli composed of various shapes. It allows customization of both outlines and fill patterns, including options such as solid fills, stripes, and checkerboards.

```{eval-rst}
.. autoclass:: psydk.visual.stimuli.PatternStimulus
  :members:
  :undoc-members:
```

### TextStimulus

A text stimulus enables you to display text on the screen. It allows you to customize the font, style, size, color, and position of the text.

To use a custom font, you either need to load the system font using the {meth}`~psydk.psydk.ExperimentContext.load_system_fonts` method or load a font file using the {meth}`~psydk.psydk.ExperimentContext.load_font_file` method. The loaded fonts are then available for use in the text stimulus.

```{eval-rst}
.. autoclass:: psydk.visual.stimuli.TextStimulus
  :members:
  :undoc-members:
```

### ImageStimulus

```{eval-rst}
.. autoclass:: psydk.visual.stimuli.ImageStimulus
  :members:
  :undoc-members:
```
