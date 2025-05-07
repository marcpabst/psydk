struct Params {
    correction: u32, // 0: none, 1: LUT
    texture_width: u32,
    texture_height: u32,
};

@vertex
fn vs_main(@builtin(vertex_index) ix: u32) -> @builtin(position) vec4<f32> {
    // Generate a full screen quad in normalized device coordinates
    var vertex = vec2(-1.0, 1.0);
    switch ix {
        case 1u: {
            vertex = vec2(-1.0, -1.0);
        }
        case 2u, 4u: {
            vertex = vec2(1.0, -1.0);
        }
        case 5u: {
            vertex = vec2(1.0, 1.0);
        }
        default: {}
    }
    return vec4(vertex, 0.0, 1.0);
}

// bind the input texture to the shader
@group(0) @binding(0)
var fine_output: texture_2d<f32>;

// bind the uniform buffer to the shader
@group(0) @binding(1)
var<uniform> params: Params;

// bind the LUT texture
@group(0) @binding(2)
var lut: texture_2d_array<f32>;



@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let rgba_input = textureLoad(fine_output, vec2<i32>(pos.xy), 0);
    let rgb_pm = vec3(rgba_input.rgb * rgba_input.a);

    if params.correction == 0 {
        // No correction, return premultiplied RGB and original alpha
        return vec4(rgb_pm, rgba_input.a);
    }
    else if params.correction == 1 {
        // Convert 0-1 value to texture coordinates
        let r_texel = calcTexelCoord(rgb_pm.r);
        let g_texel = calcTexelCoord(rgb_pm.g);
        let b_texel = calcTexelCoord(rgb_pm.b);

        // Sample each channel from its own array layer
        let corrected_r = textureLoad(lut, r_texel, 0, 0).r; // Layer 0: Red
        let corrected_g = textureLoad(lut, g_texel, 1, 0).r; // Layer 1: Green
        let corrected_b = textureLoad(lut, b_texel, 2, 0).r; // Layer 2: Blue

        return vec4(corrected_r, corrected_g, corrected_b, rgba_input.a);
    }

    // If we reach here, the correction type is not recognized
    // Return the original color
    return vec4(rgb_pm, rgba_input.a);
}

// Calculate 2D texel coordinates for a value between 0-1
fn calcTexelCoord(value: f32) -> vec2<i32> {
    let total_entries = i32(params.texture_width * params.texture_height);
    let index = i32(value * f32(total_entries - 1) + 0.5); // Round to nearest

    // Convert to 2D coordinates
    let x = index % i32(params.texture_width);
    let y = index / i32(params.texture_width);

    return vec2<i32>(x, y);
}
