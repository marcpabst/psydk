use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait};
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

    // print the latency of the stream
    println!("Stream latency: {:?}", stream.latency_duration());
    let now = Instant::now();

    let ao = AudioObject::white_noise(0.5, None, std::time::Duration::from_millis(10000));

    stream.play_at(ao, now + std::time::Duration::from_millis(50));

    let ao = AudioObject::sine_wave(440.0, 0.5, std::time::Duration::from_millis(500));

    std::thread::sleep(std::time::Duration::from_millis(1000));

    stream.play_now(ao);

    std::thread::sleep(std::time::Duration::from_millis(500000));

    println!("Closing stream");

    Ok(())
}
