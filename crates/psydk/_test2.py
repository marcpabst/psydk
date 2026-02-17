import json

NUM_ENTRIES = 16384

# Linear inverse LUT: index 0 -> 1.0, index 16383 -> 0.0
lut = [1.0 - i / (NUM_ENTRIES - 1) for i in range(NUM_ENTRIES)]

calibration = {
    "name": "Dummy Display",
    "transform": [
        1.0, 0.0, 0.0,
        0.0, 1.0, 0.0,
        0.0, 0.0, 1.0,
    ],
    "eotf": [
        {"type": "lut", "data": lut},  # R
        {"type": "lut", "data": lut},  # G
        {"type": "lut", "data": lut},  # B
    ],
    "white_point": [
      0.3127,
      0.329
    ]
  }

with open("debug_calibration.json", "w") as f:
    json.dump(calibration, f, indent=2)

print(f"Written {NUM_ENTRIES}-entry inverse LUT calibration file.")
print(f"  lut[0]     = {lut[0]}")
print(f"  lut[8191]  = {lut[8191]:.10f}")
print(f"  lut[16383] = {lut[-1]}")
