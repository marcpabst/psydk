use nalgebra::{Matrix3, Vector3};

/// Convert from CIE 1976 L*u*v* to CIE 1931 XYZ
pub fn luv_to_xyz(luv: &[f32; 3], white_point: &[f32; 3]) -> [f32; 3] {
    let (l, u, v) = (luv[0], luv[1], luv[2]);

    if l == 0.0 {
        return [0.0, 0.0, 0.0];
    }

    let y = if l > 8.0 {
        let y = ((l + 16.0) / 116.0).powi(3);
        if y > 0.008856 {
            y
        } else {
            (l / 903.3)
        }
    } else {
        l / 903.3
    };

    let ref_u = (4.0 * white_point[0]) / (white_point[0] + 15.0 * white_point[1] + 3.0 * white_point[2]);
    let ref_v = (9.0 * white_point[1]) / (white_point[0] + 15.0 * white_point[1] + 3.0 * white_point[2]);

    let a = (1.0 / 3.0) * ((52.0 * l) / (u + 13.0 * l * ref_u) - 1.0);
    let b = -5.0 * y;
    let c = -1.0 / 3.0;
    let d = y * ((39.0 * l) / (v + 13.0 * l * ref_v) - 5.0);

    let x = (d - b) / (a - c);
    let z = x * a + b;

    [x, y, z]
}

/// Convert from CIE 1931 XYZ to CIE 1976 L*u*v*
pub fn XYZ_to_Luv(xyz: &[f32; 3], white_point: &[f32; 3]) -> [f32; 3] {
    let (x, y, z) = (xyz[0], xyz[1], xyz[2]);

    let denom = x + 15.0 * y + 3.0 * z;
    let (u_prime, v_prime) = if denom != 0.0 {
        (4.0 * x / denom, 9.0 * y / denom)
    } else {
        (0.0, 0.0)
    };

    let y_ratio = y / white_point[1];
    let l = if y_ratio > 0.008856 {
        116.0 * y_ratio.cbrt() - 16.0
    } else {
        903.3 * y_ratio
    };

    if l == 0.0 {
        return [0.0, 0.0, 0.0];
    }

    let ref_denom = white_point[0] + 15.0 * white_point[1] + 3.0 * white_point[2];
    let (ref_u, ref_v) = if ref_denom != 0.0 {
        (4.0 * white_point[0] / ref_denom, 9.0 * white_point[1] / ref_denom)
    } else {
        (0.0, 0.0)
    };

    let u = 13.0 * l * (u_prime - ref_u);
    let v = 13.0 * l * (v_prime - ref_v);

    [l, u, v]
}

/// Convert from xyY to CIE 1931 XYZ
pub fn xyY_to_XYZ(xyY: &[f32; 3]) -> [f32; 3] {
    let (x, y, Y) = (xyY[0], xyY[1], xyY[2]);

    if y == 0.0 {
        return [0.0, 0.0, 0.0];
    }

    let X = (x * Y) / y;
    let Z = ((1.0 - x - y) * Y) / y;

    [X, Y, Z]
}

/// Convert from CIE 1931 XYZ to xyY
pub fn XYZ_to_xyY(xyz: &[f32; 3]) -> [f32; 3] {
    let (X, Y, Z) = (xyz[0], xyz[1], xyz[2]);
    let sum = X + Y + Z;

    if sum == 0.0 {
        return [0.0, 0.0, 0.0];
    }

    let x = X / sum;
    let y = Y / sum;

    [x, y, Y]
}

pub fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let l = (max + min) / 2.0;

    if max == min {
        return (0.0, 0.0, l); // achromatic
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;

    (h, s, l)
}

pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s == 0.0 {
        return (l, l, l); // achromatic
    }

    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;

    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);

    (r, g, b)
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 1.0 / 2.0 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

pub fn lab_to_xyz(lab: &[f32; 3], white_point: &[f32; 3]) -> [f32; 3] {
    let (l, a, b) = (lab[0], lab[1], lab[2]);

    let y = (l + 16.0) / 116.0;
    let x = a / 500.0 + y;
    let z = y - b / 200.0;

    let y3 = y.powi(3);
    let x3 = x.powi(3);
    let z3 = z.powi(3);

    let y = if y3 > 0.008856 { y3 } else { (y - 16.0 / 116.0) / 7.787 };
    let x = if x3 > 0.008856 { x3 } else { (x - 16.0 / 116.0) / 7.787 };
    let z = if z3 > 0.008856 { z3 } else { (z - 16.0 / 116.0) / 7.787 };

    [x * white_point[0], y * white_point[1], z * white_point[2]]
}
