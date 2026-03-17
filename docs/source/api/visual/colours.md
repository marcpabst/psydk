
## Colors

In psydk, colors are generally represented as structs of four floating point values, typically in the range of 0 to 1. The most common format is RGBA (red, green, blue, alpha), where the first three values represent the intensity of the red, green, and blue channels, and the fourth value represen
ts the alpha (opacity) channel. However, psydk also supports other color formats, such as XYZ (+alpha) and other color representations. 

Colors also have an associated color space and other metadata (we call these data structures "tagged colors"), which specifies how the numeric values should be interpreted. For example, for RGB(A) colors, the color space defines the primaries (red, green, and blue) and the encoding function (function that maps the numeric values to actual light intensities, often known as the "gamma curve"). You also may chose to use you display's native color space (either gamma-encoded or linear). Psydk provides a number of shorthands for creating tagged colors in various color spaces:

| Function | Color Space | Encoding |
| --- | --- | --- |
| {func}`~psydk.visual.color.rgb` | Display native RGB | Non-linear (matches the display's gamma curve) |
| {func}`~psydk.visual.color.linrgb` | Display native RGB | Linear |
| {func}`~psydk.visual.color.srgb` | sRGB (IEC 61966-2-1) | Piecewise gamma (IEC 61966-2-1) |
| {func}`~psydk.visual.color.linsrgb` | sRGB (IEC 61966-2-1) | Linear |
| {func}`~psydk.visual.color.xyz` | CIE1931 XYZ | — |
| {func}`~psydk.visual.color.luv` | CIE1976 Luv | — |

### Example

```python
from psydk.visual.color import rgb, srgb, linrgb

# Define the brightest possible red in the display's native color space
red = rgb(1.0, 0.0, 0.0)

# Define the brightest possible red in the sRGB color space
red_srgb = srgb(1.0, 0.0, 0.0)
```

### Higher bit depth rendering

psydk uses Skia's `graphite` render which represnts colours internally as 32-bit signed floating point values. However, psydk will need to render your stimuli onto a buffer (texture) that is then sent to the GPU for display. Psydk uses a two-stage approach, whin involves first rendering the stimuli onto a high-precision off-screen buffer (which can be in a high bit depth format, such as 10-bit unsigned integer 32-bit floating point), and then converting this buffer to the appropriate format for display (e.g., 8-bit unsigned integer) before sending it to the display.

The color format of these buffers can be configured using the appropriate configuation parameters when creating a {class}`~psydk.visual.Experiment` (for the off-screen buffer) and a {class}`~psydk.visual.Window` (for the display buffer). By default, psydk will use a high-precision format for the off-screen buffer and an 8-bit format for the display buffer, but you can change this if your display supports higher bit depths and you want to take advantage of that. Chosing the appropriate color formats for your experiment can take some consideration, as it can affect the accuracy of color representation and the performance of your experiment. For example, using a high-precision format for the off-screen buffer can allow for more accurate color representation, but it may also increase memory usage and reduce performance.

```{note}
On some platforms, the operating system might introduce another layer of color management, which can affect how colors are displayed. For example, on iOS, all display content is always color-managed and needs to be tagged with the appropriate color space or be treated as sRGB. Note that this in itself does not prevent accurate color representation, as this merely changes what is regarded as the display's "native" color space. However, if you want to display colors outside of sRGB (and your display supports it), you will need to make sure that the operating system treats your content as a wide-gamut color space.
```

### API

```{eval-rst}
.. automodule:: psydk.visual.color
  :members:
  :undoc-members:
```
