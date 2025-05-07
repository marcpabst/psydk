
## Colours

Accurate colour representation is crucial for many visual experiments. This section contains information about how psydk handles colours, including the colour spaces used, how to create and manipulate colours, and how to measure and calibrate your display's gamma curve.

````{note}
If you're just looking for a quick overview of the colour-related functions, you can skip to the [colour-related functions](#colour-related-functions) section.
````

In psydk, colours are generally represented as a tuple of three or four floats, each ranging from 0.0 to 1.0 (the first three values represent the red, green, and blue components of the colour, while the optional fourth value represents the alpha (transparency) component). These values are **generally expected to be linear RGB values**.

Psydk is mostly agnostic to the actual colour space used (wrt. the three red, green, and blue primaries). However, it is **not** agnostic to the luminance encoding. As mentoend above,  raw RGB tuples are always expected to be **linearly encoded**. In practice, this can sometimes be confusing. **Thereore, it is highly recommended to use the provided helper functions to create colours**. The {func}`~psydk.visual.color.rgb` function specifies a colour in the sRGB colour space (with the the sRGB encoding funcion applied), while the  {func}`~psydk.visual.color.linrgb` function specifies a colour in the linear sRGB colour space.

```{mermaid}
flowchart LR
  A[User-defined colour] -- de-/encode --> B[Internal colour space]
  B -- de-/encode --> C[Output colour space]
  C --> D[Display device]
```


By default, psydk uses a linear RGB colour space for all calculations, including blending (this is what we call the **internal colour space**). Normally, the **output colour space** is the display device's colour space (assuming your operating system or driver does not perform any colour management). Most of the time, your screen's colour space will approximate sRGB (IEC 61966-2-1). In this case, the internal colour space is linear sRGB, and psydk will re-encode the colour values to (encoded) sRGB before outputting them. Psydk will also try to correctly tag the output (on supported platforms), so that the OS will not perform any additional colour management (unless you want it to). This is the default behaviour and is what you should use in most cases.

Unfortunately, most screens are not well gamma-calibrated. Therefore, psydk provides the option to use a display-specific gamma curve for the re-encoding step. This also allows skipping the re-encoding altogether when working with a screen that expects linear RGB values.

### Linear RGB (aka gamma correction)
The term "gamma correction" is somewhat misleading. It typically refers to the process of converting between linear RGB values and gamma-encoded RGB values. In reality, there is nothing inherently wrong that needs "correcting", both your computer and your display generally handle colours as intended, usually following the IEC 61966-2-1 standard (commonly known as sRGB).

However, there is an important exception: your display may not perfectly adhere to the standard. This means that even if you encode a colour correctly in sRGB, it might not appear exactly as intended on your screen. In such cases, you can measure your display’s actual gamma curve and use a custom encoding function based on these measurements, rather than relying solely on the standard sRGB encoding. This ensures colours are displayed as accurately as possible on your specific device.

#### Creating Lookup Tables

A lookup table (LUT) is a mapping between linear colour values (that are used for blending operations) and the gamma-encoded values (what your display expects). Because psydk uses floating point values for colours, the LUT is 1D array of a high number of samples where each index corresponds to a specific linear RGB value between 0 and 1. The entries in the LUT are the gamma-encoded values that should be used for each linear RGB value.

In practice, the LUT is stored as a 2D texture with 256x256 samples, which allows for high precision and avoids lossy round-tripping.

There are **three main ways to create a lookup table**, listed here in order of increasing accuracy:

- **Fit a gamma function:** If you have only a modest number of samples from your display, you can model the gamma response using a traditional gamma function.

- **Polynomial interpolation:** With a larger set of samples, you can use polynomial interpolation to construct the lookup table. This approach can capture unusual or non-standard gamma curves, but it requires more samples to achieve good accuracy.

- **Full sampling:** If you sample every luminance value (for example, 256 samples per channel on an 8-bit display), you can use these raw measurements directly to build the lookup table. This method provides the highest accuracy as it does not rely on any assumptions about the gamma function or interpolation.

Psydk provides the {func}`~psydk.visual.color.create_gamma_lut` function to create a lookup table from a set of measurements. The function takes a list of luminance values and their corresponding gamma-encoded values, and (by default) will automatically choose the best method for creating the LUT based on the number of samples provided.

#### Measuring your display's gamma curve

To measure your display’s gamma curve, you will need a calibrated colorimeter, photometer, or spectrometer. If possible, disable any dynamic brightness or contrast settings on your display, as these can interfere with the measurements. Also, turn off any colour-related settings, since they may dynamically adjust the mixture of red, green, and blue channels.

The measurement process itself is straightforward: display a series of red, green, and blue patches on your screen, each with a specific luminance value. Use the colorimeter to measure the actual luminance of each patch and record the results. Once you have collected these measurements, you can use them to create a lookup table for the gamma curve (see the section above).


### Colour-related functions

```{eval-rst}
.. automodule:: psydk.visual.color
  :members:
  :undoc-members:
```
