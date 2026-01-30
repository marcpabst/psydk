use ndarray::{Array, IxDyn};
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};

pub struct AudioData {
    pub samples: Array<f32, IxDyn>,
    pub sample_rate: u32,
    pub channels: usize,
}

pub fn load_audio_file<P: AsRef<Path>>(
    file_path: P,
    track_index: Option<usize>,
) -> Result<AudioData, Box<dyn std::error::Error>> {
    // Open the audio file
    let file = File::open(file_path.as_ref())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    // Create a probe hint using the file extension
    let mut hint = Hint::new();
    if let Some(extension) = file_path.as_ref().extension() {
        if let Some(extension_str) = extension.to_str() {
            hint.with_extension(extension_str);
        }
    }

    // Use the default options for metadata and format readers
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    // Probe the media source
    let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;

    // Get the instantiated format reader
    let mut format = probed.format;

    // Get all audio tracks
    let audio_tracks: Vec<_> = format
        .tracks()
        .iter()
        .enumerate()
        .filter(|(_, t)| t.codec_params.codec != CODEC_TYPE_NULL)
        .collect();

    if audio_tracks.is_empty() {
        return Err("No supported audio tracks found".into());
    }

    // Select the track based on the provided index, or use the first one
    let selected_track_index = track_index.unwrap_or(0);
    if selected_track_index >= audio_tracks.len() {
        return Err(format!(
            "Track index {} out of range. Available tracks: {}",
            selected_track_index,
            audio_tracks.len()
        )
        .into());
    }

    let (_, track) = audio_tracks[selected_track_index];

    // Get track information
    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(1);

    log::debug!(
        "Loading track {} with {} channels at {} Hz",
        selected_track_index,
        channels,
        sample_rate
    );

    // Use the default options for the decoder
    let dec_opts: DecoderOptions = Default::default();

    // Create a decoder for the track
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;

    // Store samples for each channel
    let mut channel_samples: Vec<Vec<f32>> = vec![Vec::new(); channels];

    // The decode loop
    loop {
        // Get the next packet from the media format
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::ResetRequired) => {
                unimplemented!("Track reset not implemented");
            }
            Err(Error::IoError(err)) => {
                if err.kind() == std::io::ErrorKind::UnexpectedEof {
                    break;
                } else {
                    return Err(Box::new(err));
                }
            }
            Err(err) => {
                return Err(Box::new(err));
            }
        };

        // Consume any new metadata
        while !format.metadata().is_latest() {
            format.metadata().pop();
        }

        // Skip packets that don't belong to the selected track
        if packet.track_id() != track_id {
            continue;
        }

        // Decode the packet into audio samples
        let decoded = decoder.decode(&packet)?;

        // Convert samples to f32 and store by channel
        convert_and_store_samples(decoded, &mut channel_samples)?;
    }

    // Create ndarray with shape [n_samples, n_channels]
    let n_samples = channel_samples[0].len();
    let mut samples_array = Array::zeros(IxDyn(&[n_samples, channels]));

    for (ch_idx, channel_data) in channel_samples.iter().enumerate() {
        for (sample_idx, &sample) in channel_data.iter().enumerate() {
            samples_array[[sample_idx, ch_idx]] = sample;
        }
    }

    Ok(AudioData {
        samples: samples_array,
        sample_rate,
        channels,
    })
}

fn convert_and_store_samples(
    audio_buf: AudioBufferRef,
    channel_samples: &mut [Vec<f32>],
) -> Result<(), Box<dyn std::error::Error>> {
    match audio_buf {
        AudioBufferRef::F32(buf) => {
            for ch in 0..buf.spec().channels.count() {
                if ch < channel_samples.len() {
                    channel_samples[ch].extend_from_slice(buf.chan(ch));
                }
            }
        }
        AudioBufferRef::U8(buf) => {
            for ch in 0..buf.spec().channels.count() {
                if ch < channel_samples.len() {
                    channel_samples[ch].extend(buf.chan(ch).iter().map(|&s| (s as f32 - 128.0) / 128.0));
                }
            }
        }
        AudioBufferRef::U16(buf) => {
            for ch in 0..buf.spec().channels.count() {
                if ch < channel_samples.len() {
                    channel_samples[ch].extend(buf.chan(ch).iter().map(|&s| (s as f32 - 32768.0) / 32768.0));
                }
            }
        }
        AudioBufferRef::U24(buf) => {
            for ch in 0..buf.spec().channels.count() {
                if ch < channel_samples.len() {
                    channel_samples[ch].extend(buf.chan(ch).iter().map(|&s| s.0 as f32 / 8388608.0));
                }
            }
        }
        AudioBufferRef::U32(buf) => {
            for ch in 0..buf.spec().channels.count() {
                if ch < channel_samples.len() {
                    channel_samples[ch].extend(buf.chan(ch).iter().map(|&s| (s as f32 - 2147483648.0) / 2147483648.0));
                }
            }
        }
        AudioBufferRef::S8(buf) => {
            for ch in 0..buf.spec().channels.count() {
                if ch < channel_samples.len() {
                    channel_samples[ch].extend(buf.chan(ch).iter().map(|&s| s as f32 / 128.0));
                }
            }
        }
        AudioBufferRef::S16(buf) => {
            for ch in 0..buf.spec().channels.count() {
                if ch < channel_samples.len() {
                    channel_samples[ch].extend(buf.chan(ch).iter().map(|&s| s as f32 / 32768.0));
                }
            }
        }
        AudioBufferRef::S24(buf) => {
            for ch in 0..buf.spec().channels.count() {
                if ch < channel_samples.len() {
                    channel_samples[ch].extend(buf.chan(ch).iter().map(|&s| s.0 as f32 / 8388608.0));
                }
            }
        }
        AudioBufferRef::S32(buf) => {
            for ch in 0..buf.spec().channels.count() {
                if ch < channel_samples.len() {
                    channel_samples[ch].extend(buf.chan(ch).iter().map(|&s| s as f32 / 2147483648.0));
                }
            }
        }
        AudioBufferRef::F64(buf) => {
            for ch in 0..buf.spec().channels.count() {
                if ch < channel_samples.len() {
                    channel_samples[ch].extend(buf.chan(ch).iter().map(|&s| s as f32));
                }
            }
        }
    }
    Ok(())
}

// Helper function to list available tracks
fn list_audio_tracks<P: AsRef<Path>>(file_path: P) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let file = File::open(file_path.as_ref())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(extension) = file_path.as_ref().extension() {
        if let Some(extension_str) = extension.to_str() {
            hint.with_extension(extension_str);
        }
    }

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();
    let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;
    let format = probed.format;

    let tracks: Vec<String> = format
        .tracks()
        .iter()
        .enumerate()
        .filter(|(_, t)| t.codec_params.codec != CODEC_TYPE_NULL)
        .map(|(i, t)| {
            format!(
                "Track {}: {} channels, {} Hz, {:?}",
                i,
                t.codec_params.channels.map(|c| c.count()).unwrap_or(1),
                t.codec_params.sample_rate.unwrap_or(0),
                t.codec_params.codec
            )
        })
        .collect();

    Ok(tracks)
}

pub fn resample_audio(audio_data: AudioData, target_sample_rate: u32) -> Result<AudioData, Box<dyn std::error::Error>> {
    // If the sample rates are the same, return the original data
    if audio_data.sample_rate == target_sample_rate {
        return Ok(audio_data);
    }

    let input_sample_rate = audio_data.sample_rate;
    let channels = audio_data.channels;

    log::debug!("Resampling from {} Hz to {} Hz", input_sample_rate, target_sample_rate);

    // Convert ndarray to Vec<Vec<f32>> format (channels x samples)
    let input_samples = audio_data.samples;
    let n_samples = input_samples.shape()[0];

    let mut channel_data: Vec<Vec<f32>> = vec![Vec::with_capacity(n_samples); channels];
    for sample_idx in 0..n_samples {
        for ch_idx in 0..channels {
            channel_data[ch_idx].push(input_samples[[sample_idx, ch_idx]]);
        }
    }

    // Calculate resampling parameters
    let resample_ratio = target_sample_rate as f64 / input_sample_rate as f64;
    let output_length = ((n_samples as f64 * resample_ratio).round() as usize).max(1);

    // Create resampler with high-quality settings
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let mut resampler = SincFixedIn::<f32>::new(
        resample_ratio,
        2.0, // max_resample_ratio_relative (allows for some flexibility)
        params,
        n_samples,
        channels,
    )?;

    // Resample the audio
    let resampled_data = resampler.process(&channel_data, None)?;

    // Convert back to ndarray format [n_samples, n_channels]
    let resampled_n_samples = resampled_data[0].len();
    let mut resampled_array = Array::zeros(IxDyn(&[resampled_n_samples, channels]));

    for ch_idx in 0..channels {
        for (sample_idx, &sample) in resampled_data[ch_idx].iter().enumerate() {
            resampled_array[[sample_idx, ch_idx]] = sample;
        }
    }

    Ok(AudioData {
        samples: resampled_array,
        sample_rate: target_sample_rate,
        channels,
    })
}
