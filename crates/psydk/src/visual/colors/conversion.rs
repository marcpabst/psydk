use super::display::DisplayCharacteristics;
use super::{Color, RGBColorSpace};
use nalgebra::{Matrix3, Vector3, Vector4};
/// Converts an sRGB color to CIE XYZ color space.
///
/// # Arguments
/// * `srgb` - A Vector4 containing sRGB values in the range [0.0, 1.0]
///
/// # Returns
/// A Vector4 containing XYZ values (typically in range [0.0, 1.0] for standard colors)
pub fn srgba_to_xyz(srgb: impl Into<Vector4<f32>>) -> Vector4<f32> {
    let srgb_vec = srgb.into();

    // Step 1: Apply inverse gamma correction (sRGB to linear RGB)
    let linear_rgb = srgb_vec.xyz().map(|component| {
        if component <= 0.04045 {
            component / 12.92
        } else {
            ((component + 0.055) / 1.055).powf(2.4)
        }
    });

    // Step 2: Apply the sRGB to XYZ transformation matrix
    // Using D65 illuminant (standard for sRGB)
    let transform_matrix = Matrix3::new(
        0.4124564, 0.3575761, 0.1804375, 0.2126729, 0.7151522, 0.0721750, 0.0193339, 0.1191920, 0.9503041,
    );

    let xyz = transform_matrix * linear_rgb;
    Vector4::new(xyz.x, xyz.y, xyz.z, srgb_vec.w)
}

/// Converts linear sRGB color to CIE XYZ color space.
/// # Arguments
/// * `srgb_linear` - A Vector4 containing linear sRGB values in the range [0.0, 1.0]
/// # Returns
/// A Vector4 containing XYZ values + alpha
pub fn srgba_linear_to_xyz(srgb_linear: impl Into<Vector4<f32>>) -> Vector4<f32> {
    let srgb_vec = srgb_linear.into();

    // Step 1: Apply the sRGB to XYZ transformation matrix
    // Using D65 illuminant (standard for sRGB)
    let transform_matrix = Matrix3::new(
        0.4124564, 0.3575761, 0.1804375, 0.2126729, 0.7151522, 0.0721750, 0.0193339, 0.1191920, 0.9503041,
    );

    let xyz = transform_matrix * srgb_vec.xyz();
    Vector4::new(xyz.x, xyz.y, xyz.z, srgb_vec.w)
}

/// Converts CIE L*u*v* color to CIE XYZ color space.
fn luva_to_xyza(luv: impl Into<Vector4<f32>>, white_point: impl Into<Vector3<f32>>) -> Vector4<f32> {
    let luv = luv.into();
    let white_point = white_point.into();

    let l = luv.x;
    let u = luv.y;
    let v = luv.z;
    let a = luv.w;

    // Handle the special case where L = 0 (black)
    if l <= 0.0 {
        return Vector4::new(0.0, 0.0, 0.0, a);
    }

    // Calculate Y from L
    let y = if l > 8.0 {
        white_point.y * ((l + 16.0) / 116.0).powi(3)
    } else {
        white_point.y * l * (3.0f32 / 29.0f32).powi(3)
    };

    // Calculate reference u' and v' from white point
    let denom_ref = white_point.x + 15.0 * white_point.y + 3.0 * white_point.z;
    let u_ref = (4.0 * white_point.x) / denom_ref;
    let v_ref = (9.0 * white_point.y) / denom_ref;

    // Calculate u' and v' from u and v
    let u_prime = u / (13.0 * l) + u_ref;
    let v_prime = v / (13.0 * l) + v_ref;

    // Calculate X and Z from u', v', and Y
    let x = y * (9.0 * u_prime) / (4.0 * v_prime);
    let z = y * (12.0 - 3.0 * u_prime - 20.0 * v_prime) / (4.0 * v_prime);

    Vector4::new(x, y, z, a)
}

/// Converts CIE L*a*b* color to CIE XYZ color space.
fn laba_to_xyza(laba: impl Into<Vector4<f32>>, white_point: impl Into<Vector3<f32>>) -> Vector4<f32> {
    // Get white point
    let white_point = white_point.into();
    let xn = white_point.x;
    let yn = white_point.y;
    let zn = white_point.z;

    let lab = laba.into();

    let alpha = lab.w;

    let l = lab.x;
    let a = lab.y;
    let b = lab.z;

    // Calculate f(Y/Yn) from L*
    let fy = (l + 16.0) / 116.0;

    // Calculate f(X/Xn) and f(Z/Zn)
    let fx = a / 500.0 + fy;
    let fz = fy - b / 200.0;

    // Define the threshold and conversion constants
    const DELTA: f32 = 6.0 / 29.0;
    const DELTA_CUBED: f32 = DELTA * DELTA * DELTA; // (6/29)³
    const FACTOR: f32 = 3.0 * DELTA * DELTA; // 3 * (6/29)²

    // Convert f values to XYZ using inverse transformation
    let x = xn
        * if fx > DELTA {
            fx.powi(3)
        } else {
            (fx - 16.0 / 116.0) * FACTOR
        };

    let y = yn
        * if l > 8.0 {
            fy.powi(3)
        } else {
            l * (DELTA / 2.0).powi(3) // Equivalent to L * (3/29)³
        };

    let z = zn
        * if fz > DELTA {
            fz.powi(3)
        } else {
            (fz - 16.0 / 116.0) * FACTOR
        };

    Vector4::new(x, y, z, alpha)
}

pub fn color_to_internal_device_rgba(
    color: Color,
    dc: &dyn DisplayCharacteristics,
    linear_blending: bool,
) -> Vector4<f32> {
    //
    let linear_device_rgb = color_to_linear_device_rgba(color, dc);

    if !linear_blending {
        // Apply EOTF
        return dc.apply_eotf(&linear_device_rgb);
    } else {
        // We will apply EOTF later during blending, so return linear device RGB for now
        return linear_device_rgb;
    }
}

/// Convert a Color to device space RGBA with EOTFs applied.
///
/// This function:
/// 1. Converts the color to linear device RGB (if needed)
/// 2. Applies the appropriate EOTF (Electro-Optical Transfer Function)
/// 3. Returns a Vector4 with RGBA components in device space
pub fn color_to_linear_device_rgba(color: Color, dc: &dyn DisplayCharacteristics) -> Vector4<f32> {
    match color {
        Color::RGBA(rgba) => {
            let rgba = match rgba.space {
                // Already in device space with EOTF applied, so we need to apply the inverse EOTF to get linear device RGB
                RGBColorSpace::Device => dc
                    .apply_inverse_eotf(&Vector4::new(rgba.r, rgba.g, rgba.b, rgba.a))
                    .expect("Failed to apply inverse EOTF"),

                // Linear device space - no transformation needed
                RGBColorSpace::DeviceLinear => Vector4::new(rgba.r, rgba.g, rgba.b, rgba.a),

                // sRGB with encoding - convert to linear, transform to device, apply EOTF
                RGBColorSpace::SRGB => {
                    // Transform from linear sRGB to linear device RGB
                    let xyza = srgba_to_xyz(Vector4::new(rgba.r, rgba.g, rgba.b, rgba.a));
                    dc.xyza_to_device_rgba(&xyza)
                }

                // Linear sRGB - transform to device
                RGBColorSpace::SRGBLinear => {
                    // Transform from linear sRGB to linear device RGB
                    let xyza = srgba_linear_to_xyz(Vector4::new(rgba.r, rgba.g, rgba.b, rgba.a));
                    dc.xyza_to_linear_device_rgba(&xyza)
                }
            };
            rgba
        }

        // For other color spaces, convert to XYZ, then to device RGB
        Color::XYZA(xyza) => {
            let xyza = Vector4::new(xyza.x, xyza.y, xyza.z, xyza.a);
            dc.xyza_to_device_rgba(&xyza)
        }

        Color::LuvA(luva) => {
            // Convert Luv to XYZ first
            let white_point = luva.white_point;
            let luva = Vector4::new(luva.l, luva.u, luva.v, luva.a);
            let xyza = luva_to_xyza(luva, white_point);
            dc.xyza_to_linear_device_rgba(&xyza)
        }

        Color::LabA(laba) => {
            // Convert Lab to XYZ first
            let xyza = laba_to_xyza(Vector4::new(laba.l, laba.a, laba.b, laba.a), laba.white_point);
            dc.xyza_to_linear_device_rgba(&xyza)
        }
    }
}
