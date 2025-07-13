pub mod win;

pub fn estimate_refresh_rate(timestamps: &[f64]) -> Option<(f64, f64)> {
    // Constants from the original code
    const TIGHTGROUPMS: f64 = 1.5; // ms: inter-frame times expected within this grouping
    const MSCHANGE: f64 = 10.0; // ms: interval change more than this is considered a Hz change
    const LOWESTVALIDHZ: f64 = 35.0; // Lowest valid Hz (filters inactive tab timings)
    const MINSAMPLES: usize = 10; // Minimum samples before starting estimation

    if timestamps.len() < 5 {
        return None;
    }

    // Store the original last timestamp for later correction
    let last_input_timestamp = *timestamps.last().unwrap();

    // Storage for validated frame times
    let mut valid_times_raw = Vec::new();
    let mut valid_times_smooth = Vec::new();

    // Process timestamps with sliding window
    for i in 4..timestamps.len() {
        // Get last 5 timestamps
        let (l0, l1, l2, l3, l4) = (
            timestamps[i - 4],
            timestamps[i - 3],
            timestamps[i - 2],
            timestamps[i - 1],
            timestamps[i],
        );

        // Calculate inter-frame intervals
        let intervals = [l4 - l3, l3 - l2, l2 - l1, l1 - l0];

        // Check if intervals form a tight group
        let min_interval = intervals.iter().copied().fold(f64::INFINITY, f64::min);
        let max_interval = intervals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let grouping = max_interval - min_interval;

        if grouping < TIGHTGROUPMS {
            // Average interval over 4 frames
            let avg_ms = (l4 - l0) / 4.0;

            // Filter out inactive tab timings
            if avg_ms < 1000.0 / LOWESTVALIDHZ {
                valid_times_raw.push(l2);
                valid_times_smooth.push((l0 + l1 + l2 + l3 + l4) / 5.0);
            }
        }
    }

    // Need minimum samples for reliable estimation
    if valid_times_raw.len() < MINSAMPLES {
        return None;
    }

    // Compute initial interval estimate
    if valid_times_smooth.len() < 2 {
        return None;
    }

    // Initial estimate from average of intervals
    let intervals: Vec<f64> = valid_times_smooth.windows(2).map(|w| w[1] - w[0]).collect();

    if intervals.is_empty() {
        return None;
    }

    let ms_estimate = intervals.iter().sum::<f64>() / intervals.len() as f64;

    // Build X values (frame counts) and track missed frames
    let mut x_values = vec![0.0];
    let mut total_expected_frames = 0.0;

    for i in 1..valid_times_smooth.len() {
        let interval = valid_times_smooth[i] - valid_times_smooth[i - 1];
        let frames_elapsed = (interval / ms_estimate).round();
        total_expected_frames += frames_elapsed;
        x_values.push(x_values.last().unwrap() + frames_elapsed);
    }

    // Compute least squares regression
    let n = valid_times_raw.len() as f64;
    let sx: f64 = x_values.iter().sum();
    let sy: f64 = valid_times_raw.iter().map(|t| t - valid_times_raw[0]).sum();
    let sxx: f64 = x_values.iter().map(|x| x * x).sum();
    let sxy: f64 = x_values
        .iter()
        .zip(valid_times_raw.iter())
        .map(|(x, t)| x * (t - valid_times_raw[0]))
        .sum();

    // Calculate slope (interval in ms)
    let denominator = n * sxx - sx * sx;
    if denominator.abs() < 1e-10 {
        return None;
    }

    let m = (n * sxy - sx * sy) / denominator;

    // Validate the fit
    let b = (sxx * sy - sx * sxy) / denominator;
    let timebase = valid_times_raw[0] + b;
    let half_ms = m / 2.0;

    // Check drift to validate the estimate
    let mut max_drift: f64 = 0.0;
    let mut min_drift: f64 = 0.0;

    for t in &valid_times_raw[1..] {
        // Calculate offset from expected vsync time
        let offset = ((t - timebase + half_ms) % m) - half_ms;
        min_drift = min_drift.min(offset);
        max_drift = max_drift.max(offset);
    }

    // If drift is too large, estimation failed
    if max_drift - min_drift >= half_ms {
        eprintln!("Drift too large, estimation failed.");
        None
    } else {
        let refresh_rate = 1000.0 / m;

        // Calculate jitter-corrected timestamp for the last input frame
        let frames_from_timebase = ((last_input_timestamp - timebase) / m).round();
        let corrected_last_timestamp = timebase + frames_from_timebase * m;

        Some((refresh_rate, corrected_last_timestamp))
    }
}

/// Given slices `xs` and `ys` of equal length describing a sawtooth pattern,
/// returns the x-values where each rising segment’s linear fit crosses y = 0.
pub fn find_zero_crossings(xs: &[f64], ys: &[f64]) -> Vec<f64> {
    assert_eq!(xs.len(), ys.len(), "xs and ys must have the same length");
    let n = xs.len();
    let mut zeros = Vec::new();
    let mut start = 0;

    // Helper to process one segment [start..end)
    fn process_segment(xs: &[f64], ys: &[f64], start: usize, end: usize, zeros: &mut Vec<f64>) {
        let len = end - start;
        if len < 2 {
            return;
        }

        // Compute sums for least‐squares
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xx = 0.0;
        let mut sum_xy = 0.0;
        for i in start..end {
            let x = xs[i];
            let y = ys[i];
            sum_x += x;
            sum_y += y;
            sum_xx += x * x;
            sum_xy += x * y;
        }
        let n = len as f64;
        let denom = n * sum_xx - sum_x * sum_x;
        if denom.abs() < 1e-12 {
            return;
        } // avoid division by zero

        let m = (n * sum_xy - sum_x * sum_y) / denom;
        let b = (sum_y - m * sum_x) / n;
        if m.abs() < 1e-12 {
            return;
        } // skip near‐horizontal fits

        // Solve 0 = m*x + b  =>  x = -b/m
        zeros.push(-b / m);
    }

    // Iterate, splitting on any decrease in y
    for i in 1..n {
        if ys[i] < ys[i - 1] {
            process_segment(xs, ys, start, i, &mut zeros);
            start = i;
        }
    }
    // Process final segment
    process_segment(xs, ys, start, n, &mut zeros);

    zeros
}
