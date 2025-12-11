use std::fs::File;
use std::io::BufReader;

use ndarray::{arr1, s, Array1, Array2, Axis};
use serde::Deserialize;

use crate::visual::colors::display_characteristics::EOTF;
use crate::visual::colors::DisplayCharacteristics;

#[derive(Deserialize)]
struct LutRange {
    r: [f32; 2],
    g: [f32; 2],
    b: [f32; 2],
}

#[derive(Deserialize)]
struct Lut {
    r: Vec<f32>,
    g: Vec<f32>,
    b: Vec<f32>,
}

#[derive(Deserialize)]
struct ResidualParams {
    res_poly_coefficients: Vec<Vec<f32>>,
    res_poly_intercept: f32,
    res_poly_powers: Vec<Vec<f32>>,
    res_poly_mean: Vec<f32>,
    res_poly_scale: Vec<f32>,
    res_poly_domain: String,
}

#[derive(Deserialize)]
/// This transform consists of an affine transform
/// followed by a polynomial residual correction.
/// The last step is a per-channel EOTF LUT.
pub struct Psydk1DisplayCharacteristics {
    // Name
    name: String,
    // Affine tranform
    /// 3x3 matrix (linear)
    M: Vec<Vec<f32>>,
    /// 3x1 vector (intercept)
    b: Vec<f32>,
    #[serde(flatten)]
    /// Polynomial residual
    residual: ResidualParams,
    // EOTF LUT
    /// Range for each input channel
    eotf_inv_lut_range: LutRange,
    /// LUTs for each channel
    eotf_inv_lut: Lut,
}

impl Psydk1DisplayCharacteristics {
    pub fn new_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let profile: Psydk1DisplayCharacteristics = serde_json::from_reader(reader)?;
        Ok(profile)
    }
    fn poly_features(z: &Array2<f32>, powers: &Array2<f32>) -> Array2<f32> {
        let n_samples = z.nrows();
        let n_poly_features = powers.nrows();
        let n_inputs = z.ncols();

        let mut result = Array2::<f32>::zeros((n_samples, n_poly_features));

        for (sample_idx, sample) in z.outer_iter().enumerate() {
            for feature_idx in 0..n_poly_features {
                let mut prod = 1.0;
                for input_idx in 0..n_inputs {
                    let base = sample[input_idx];
                    let exponent = powers[[feature_idx, input_idx]];
                    prod *= base.powf(exponent);
                }
                result[[sample_idx, feature_idx]] = prod;
            }
        }

        result
    }

    fn standardize(f: Array2<f32>, mean: &Array1<f32>, scale: &Array1<f32>) -> Array2<f32> {
        let mut standardized = f;
        standardized -= &mean.view().insert_axis(Axis(0));
        standardized /= &scale.view().insert_axis(Axis(0));
        standardized
    }

    fn poly_residual(z: &Array2<f32>, params: &ResidualParams) -> Array2<f32> {
        let coeffs = Array2::from_shape_vec(
            (
                params.res_poly_coefficients.len(),
                params.res_poly_coefficients[0].len(),
            ),
            params.res_poly_coefficients.iter().flatten().cloned().collect(),
        )
        .expect("invalid coefficient shape");

        let intercept = arr1(&[params.res_poly_intercept]);
        let powers = Array2::from_shape_vec(
            (params.res_poly_powers.len(), params.res_poly_powers[0].len()),
            params.res_poly_powers.iter().flatten().cloned().collect(),
        )
        .expect("invalid power shape");

        let mean = Array1::from(params.res_poly_mean.clone());
        let scale = Array1::from(params.res_poly_scale.clone());

        let f = Self::poly_features(z, &powers);
        let f_std = Self::standardize(f, &mean, &scale);
        let mut result = f_std.dot(&coeffs.t());
        result += &intercept.view().insert_axis(Axis(0));
        result
    }

    fn affine_residual_predict(x: &Array2<f32>, params: &Psydk1DisplayCharacteristics) -> Array2<f32> {
        let m = Array2::from_shape_vec(
            (params.M.len(), params.M[0].len()),
            params.M.iter().flatten().cloned().collect(),
        )
        .expect("invalid M shape");

        let b = Array1::from(params.b.clone());

        let mut base = x.dot(&m.t());
        base += &b.view().insert_axis(Axis(0));

        let z = if params.residual.res_poly_domain == "source" {
            x.clone()
        } else {
            base.clone()
        };

        let resid = Self::poly_residual(&z, &params.residual);
        base + resid
    }

    fn linear_interp(x: f32, x_min: f32, x_max: f32, lut: &[f32]) -> f32 {
        let n = lut.len();
        if x <= x_min {
            return lut[0];
        }
        if x >= x_max {
            return lut[n - 1];
        }

        let t = (x - x_min) / (x_max - x_min);
        let pos = t * (n as f32 - 1.0);
        let idx = pos.floor() as usize;
        let frac = pos - idx as f32;

        let y0 = lut[idx];
        let y1 = lut[idx + 1];
        y0 + frac * (y1 - y0)
    }

    fn xyz_to_rgb(&self, xyz_scaled: &Array2<f32>) -> Array2<f32> {
        let rgb_lin = Self::affine_residual_predict(&xyz_scaled, self);

        let mut rgb = rgb_lin.clone();

        for (idx, channel) in ["r", "g", "b"].iter().enumerate() {
            let (range, lut) = match *channel {
                "r" => (self.eotf_inv_lut_range.r, &self.eotf_inv_lut.r),
                "g" => (self.eotf_inv_lut_range.g, &self.eotf_inv_lut.g),
                "b" => (self.eotf_inv_lut_range.b, &self.eotf_inv_lut.b),
                _ => unreachable!(),
            };

            for sample_idx in 0..rgb.nrows() {
                let value = rgb_lin[[sample_idx, idx]];
                rgb[[sample_idx, idx]] = Self::linear_interp(value, range[0], range[1], lut);
            }
        }

        rgb
    }
}

// fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let file = File::open("/Users/marc/repos/argyllclient/src/argyllclient/data/ipad_calibration_params.json")?;

//     let reader = BufReader::new(file);
//     let profile: PsydkProfile1 = serde_json::from_reader(reader)?;
//     println!("Params loaded");

//     // Example usage
//     let xyz = Array2::from_shape_vec((1, 3), vec![0.3_f32, 0.4_f32, 0.5_f32])?;
//     let rgb = profile.xyz_to_rgb(&xyz);

//     println!("{rgb:?}");

//     // benchmarking
//     let n_times = 10000;
//     // create random input
//     let random_xyz = Array2::from_shape_vec((n_times, 3), (0..n_times * 3).map(|_| rand::random::<f32>()).collect())?;
//     let t0 = std::time::Instant::now();
//     for i in 0..n_times {
//         let _ = profile.xyz_to_rgb(&random_xyz.slice(s![i..i + 1, ..]).to_owned());
//     }
//     let elapsed = t0.elapsed();
//     println!(
//         "Elapsed for {n_times} runs: {:?}. Average: {:?}",
//         elapsed,
//         elapsed / n_times as u32
//     );

//     Ok(())
// }
//
//

impl DisplayCharacteristics for Psydk1DisplayCharacteristics {
    fn name(&self) -> &str {
        &self.name
    }

    fn xyz_to_rgb(&self, xyz: &nalgebra::Vector3<f32>) -> nalgebra::Vector3<f32> {
        let xyz_arr = Array2::from_shape_vec((1, 3), vec![xyz.x, xyz.y, xyz.z]).expect("invalid shape");
        let rgb_arr = self.xyz_to_rgb(&xyz_arr);
        nalgebra::Vector3::new(rgb_arr[[0, 0]], rgb_arr[[0, 1]], rgb_arr[[0, 2]])
    }

    fn xyz_to_linear_rgb(&self, xyz: &nalgebra::Vector3<f32>) -> nalgebra::Vector3<f32> {
        // only affine + residual, no LUT
        let xyz_arr = Array2::from_shape_vec((1, 3), vec![xyz.x, xyz.y, xyz.z]).expect("invalid shape");
        let rgb_arr = Self::affine_residual_predict(&xyz_arr, self);
        nalgebra::Vector3::new(rgb_arr[[0, 0]], rgb_arr[[0, 1]], rgb_arr[[0, 2]])
    }

    fn linear_rgb_to_rgb(&self, linear_rgb: &nalgebra::Vector3<f32>) -> nalgebra::Vector3<f32> {
        todo!()
    }

    fn supports_inverse(&self) -> bool {
        false
    }

    fn rgb_to_xyz(&self, rgb: &nalgebra::Vector3<f32>) -> Option<nalgebra::Vector3<f32>> {
        None
    }

    fn eotf(&self) -> Option<[EOTF; 3]> {
        None
    }

    fn white_point(&self) -> (f32, f32) {
        (0.3127, 0.3290) // D65
    }

    fn white_point_luminance(&self) -> Option<f32> {
        None
    }

    fn spectral_primaries(&self) -> Option<[(f32, f32); 3]> {
        None
    }
}
