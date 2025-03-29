use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait};
use ndarray::IxDyn;
use timed_audio::ndarray::ArrayD;
use timed_audio::{AudioObject, Stream};

fn main() -> anyhow::Result<()> {
    let host = cpal::default_host();

    let device = host
        .default_output_device()
        .expect("no output device available");

    let config = device.default_output_config().unwrap();
    let sample_format = config.sample_format();

    run(&device, &config.into(), sample_format)
}

pub fn run(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
) -> anyhow::Result<()> {
    let stream = Stream::new(device, config, sample_format);

    // create a 2s sine wave at 440 Hz
    let mut data = ArrayD::zeros(IxDyn(&[44100, 2]));
    for (i, sample) in data.iter_mut().enumerate() {
        *sample =
            (2.0 * std::f32::consts::PI * 440.0 * i as f32 / stream.sample_rate() as f32).sin();
    }

    let now = Instant::now();

    let ao = AudioObject::from_samples(data, stream.sample_rate());

    stream.play_at(ao, now + std::time::Duration::from_millis(50));

    std::thread::sleep(std::time::Duration::from_millis(500000));

    println!("Closing stream");

    Ok(())
}
