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
var lut: texture_1d<f32>;
@group(0) @binding(3)
var lut_sampler: sampler;

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let rgba_input = textureLoad(fine_output, vec2<i32>(pos.xy), 0);
    let rgb_pm = vec3(rgba_input.rgb * rgba_input.a);

    if params.correction == 0 {
        // No correction, return premultiplied RGB and original alpha
        return vec4(rgb_pm, rgba_input.a);
    }
    else if params.correction == 1 {

        // Sample each channel from the LUT texture
        // We assume the LUT is a 1D RGBA texture where the R, G, B channels are stored in the first row
        // and the alpha channel is not used for correction
        // we use the provided sampler to sample the LUT texture
        let corrected_r = textureSampleLevel(lut, lut_sampler, rgb_pm.r, 0.0).r;
        let corrected_g = textureSampleLevel(lut, lut_sampler, rgb_pm.g, 0.0).g;
        let corrected_b = textureSampleLevel(lut, lut_sampler, rgb_pm.b, 0.0).b;

        return vec4(corrected_r, corrected_g, corrected_b, rgba_input.a);
    }

    // If we reach here, the correction type is not recognized
    // Return the original color
    return vec4(rgb_pm, rgba_input.a);
}
