# clear ../target/wheels
# rm -rf ../target/wheels

# build the python wheel
# export OSX_SDKROOT=$(xcrun --sdk macosx --show-sdk-path)

export IOS_SDKROOT=$(xcrun --sdk iphoneos --show-sdk-path)
export PYO3_CROSS_LIB_DIR="$PYTHONDIR"
export PYO3_CROSS_PYTHON_VERSION="$PYTHON_VERSION"
env SDKROOT="$IOS_SDKROOT" \
PYO3_CROSS_LIB_DIR="$PYTHONDIR" \
CARGO_TARGET_AARCH64_APPLE_IOS_RUSTFLAGS="-C link-arg=-isysroot -C link-arg=$IOS_SDKROOT \
	-C link-arg=-arch -C link-arg=arm64 -C link-arg=-miphoneos-version-min=14.0 -C link-arg=-L \
	-C link-arg=$PYTHONDIR \
	-C link-arg=-undefined \
	-C link-arg=dynamic_lookup" \
	maturin build --target aarch64-apple-ios --release


# SDKROOT=$(xcrun --sdk iphoneos --show-sdk-path) \
# PYO3_CROSS_PYTHON_VERSION="$PYTHON_VERSION" \
# PYO3_CROSS_LIB_DIR="$PYTHONDIR" \
# CARGO_TARGET_AARCH64_APPLE_IOS_RUSTFLAGS="-C link-arg=-isysroot -C link-arg=$IOS_SDKROOT \
# -C link-arg=-arch -C link-arg=arm64 -C link-arg=-miphoneos-version-min=14.0 -C link-arg=-L \
# -C link-arg=$PYTHONDIR \
# -C link-arg=-undefined \
# -C link-arg=dynamic_lookup" \
# maturin build --target aarch64-apple-ios --release


rc=$?

# check if the build was successful
if [ $rc -ne 0 ]; then
    echo "Build failed with exit code $rc"
    exit $rc
fi
