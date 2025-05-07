# Psydk on different Platforms

## Overview Table
| Feature | Windows | Linux | macOS | iOS |
| ------- | ------- | ----- | ----- | --- |
| Supported platform version | Windows 10+ | Ubuntu 22.04+ | macOS 14.0+ | iOS 16.0+ |
| Supported architecture | x86 | x86, ARM64 | ARM64 | ARM64 |
| Supported rendering backends | DirectX 12<sup>1</sup>, Vulkan | Vulkan | Metal<sup>1</sup>, Vulkan | Metal |
| Accurate frame timing | ✅ | n/a | ✅ | n/a |
| Accurate audio timing | ✅ | n/a | ✅ | ✅ |
| Video playback | ✅ | ✅ | ✅ | n/a |

<sup>1</sup> default backend, can be changed to Vulkan


## Desktop

On desktop, psydk can simply be installed into any CPython 3.8+ environment using pip or another package manager of your choice.

### Windows

Psydk uses DirectX 12 for rendering and requires Windows 10 or higher. We only testsed on Windows 11, but it should work on Windows 10 as well. We do not activly support Windows 7 or 8. In principle, it should also be possible use Vulkan on Windows, but we have not tested this yet and DirectX 12 is usually faster and more reliable.

### Linux
Psydk uses Vulkan for rendering on Linux. We only test psydk on the latest version of Ubuntu LTS (currenlty 22.04). It should work on other distributions as well, but we do not actively support them.

### macOS
Psydk uses Metal for rendering on macOS. We only test psydk on the latest version of macOS running on Apple silicon (currently 14.0).

## Other platforms

### iOS
Psydk can be used on iOS using the [briefcase](https://briefcase.readthedocs.io/en/latest/) tool. Since pre-compiled wheels are available on PyPi, you can simply install psydk as described in `briefcase` documentation.

### Android
In principle, psydk can be used on Android as well. However, we do not actively support this platform and currently do not provide pre-compiled wheels for Android. If you are interested in using psydk on Android, please contact us and we will try to help you.
