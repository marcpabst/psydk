use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
    usize,
};

pub use cpal;
pub use ndarray;

use cpal::{
    FromSample, Sample, SizedSample,
    traits::{DeviceTrait, StreamTrait},
};
use ndarray::{Array, Axis};
use rand::SeedableRng;
use rand_distr::Distribution;

#[derive(Debug, Clone)]
pub enum AudioObject {
    Buffer {
        data: Array<f32, ndarray::IxDyn>,
        sample_rate: u32,
    },
    SineWave {
        frequency: f32,
        amplitude: f32,
        duration: Duration,
    },
    WhiteNoise {
        amplitude: f32,
        seed: Option<u64>,
        duration: Duration,
    },
    Silence {
        duration: Duration,
    },
}

impl AudioObject {
    pub fn from_data(data: Array<f32, ndarray::IxDyn>, sample_rate: u32) -> Self {
        Self::Buffer { data, sample_rate }
    }

    pub fn sine_wave(frequency: f32, amplitude: f32, duration: Duration) -> Self {
        Self::SineWave {
            frequency,
            amplitude,
            duration,
        }
    }

    pub fn white_noise(amplitude: f32, seed: Option<u64>, duration: Duration) -> Self {
        Self::WhiteNoise {
            amplitude,
            seed,
            duration,
        }
    }

    pub fn silence(duration: Duration) -> Self {
        Self::Silence { duration }
    }

    pub fn from_samples(samples: Array<f32, ndarray::IxDyn>, sample_rate: u32) -> Self {
        Self::Buffer {
            data: samples,
            sample_rate,
        }
    }

    pub fn duration(&self) -> Duration {
        match self {
            AudioObject::Buffer { data, sample_rate } => {
                let n_samples = data.len_of(Axis(0));
                let duration = n_samples as f32 / *sample_rate as f32;
                Duration::from_secs_f32(duration)
            }
            AudioObject::SineWave { duration, .. } => *duration,
            AudioObject::WhiteNoise { duration, .. } => *duration,
            AudioObject::Silence { duration, .. } => *duration,
        }
    }

    pub fn sample_rate(&self) -> Option<u32> {
        match self {
            AudioObject::Buffer { sample_rate, .. } => Some(*sample_rate),
            AudioObject::SineWave { .. } => None,
            AudioObject::WhiteNoise { .. } => None,
            AudioObject::Silence { .. } => None,
        }
    }

    pub fn into_writer(
        self,
        stream_sample_rate: u32,
        stream_channels: usize,
    ) -> AudioObjectDataWriter {
        let rng = match self {
            AudioObject::WhiteNoise { seed, .. } => {
                if let Some(seed) = seed {
                    Some(rand::rngs::SmallRng::seed_from_u64(seed))
                } else {
                    Some(rand::rngs::SmallRng::from_os_rng())
                }
            }
            _ => None,
        };

        AudioObjectDataWriter {
            audio_object: self,
            current_idx: 0,
            target_sample_rate: stream_sample_rate,
            target_channels: stream_channels,
            rng,
        }
    }
}

#[derive(Debug)]
pub struct AudioObjectDataWriter {
    audio_object: AudioObject,
    current_idx: usize,
    target_sample_rate: u32,
    target_channels: usize,
    rng: Option<rand::rngs::SmallRng>,
}

impl AudioObjectDataWriter {
    pub fn move_by(&mut self, n_samples: usize) {
        self.current_idx += n_samples;
    }

    pub fn write_data<T>(&mut self, output: &mut [T]) -> Result<bool, anyhow::Error>
    where
        T: Sample + FromSample<f32>,
    {
        match &self.audio_object {
            AudioObject::Buffer { data, .. } => {
                // error if the samplig rate does not math the target sample rate
                if let Some(sample_rate) = self.audio_object.sample_rate() {
                    if sample_rate != self.target_sample_rate {
                        return Err(anyhow::anyhow!(
                            "Sample rate of audio object does not match target sample rate"
                        ));
                    }
                }

                // error if the number of channels does not match the target number of channels
                if data.len_of(Axis(1)) != self.target_channels {
                    return Err(anyhow::anyhow!(
                        "Number of channels of audio object does not match target number of channels"
                    ));
                }

                // write the data to the output buffer
                let n_output_frames = output.len() / self.target_channels;
                // how many frames do we need to write?
                let n_frames = n_output_frames.min(data.len_of(Axis(0)) - self.current_idx);

                // copy the data
                for (i, frame) in output
                    .chunks_mut(self.target_channels)
                    .enumerate()
                    .take(n_frames)
                {
                    for (j, sample) in frame.iter_mut().enumerate() {
                        *sample = T::from_sample(data[[self.current_idx + i, j]]);
                    }
                }

                self.current_idx += n_frames;

                // return true if the end of the audio object has been reached
                Ok(n_frames == 0)
            }
            AudioObject::SineWave {
                frequency,
                amplitude,
                duration,
            } => {
                let n_output_frames = output.len() / self.target_channels;
                let sample_rate = self.target_sample_rate as f32;
                let t = self.current_idx as f32 / sample_rate;
                let n_frames = n_output_frames
                    .min(((duration.as_secs_f32() - t) * sample_rate).round() as usize);

                for (i, frame) in output
                    .chunks_mut(self.target_channels)
                    .enumerate()
                    .take(n_frames)
                {
                    let t = t + i as f32 / sample_rate;
                    let value = amplitude * (2.0 * std::f32::consts::PI * frequency * t).sin();
                    for sample in frame.iter_mut() {
                        *sample = T::from_sample(value);
                    }
                }

                self.current_idx += n_frames;

                // return true if the end of the audio object has been reached
                Ok(n_frames == 0)
            }
            AudioObject::WhiteNoise {
                amplitude,
                seed: _,
                duration,
            } => {
                let n_output_frames = output.len() / self.target_channels;
                let sample_rate = self.target_sample_rate as f32;
                let t = self.current_idx as f32 / sample_rate;
                let n_frames = n_output_frames
                    .min(((duration.as_secs_f32() - t) * sample_rate).round() as usize);

                let normal = rand_distr::Normal::new(0.0, 1.0).unwrap();
                let mut rng = self.rng.as_mut().unwrap();

                for (_, frame) in output
                    .chunks_mut(self.target_channels)
                    .enumerate()
                    .take(n_frames)
                {
                    for sample in frame.iter_mut() {
                        let random_f: f32 = normal.sample(&mut rng);
                        *sample = T::from_sample(amplitude * (2.0 * random_f - 1.0));
                    }
                }

                self.current_idx += n_frames;

                // return true if the end of the audio object has been reached
                Ok(n_frames == 0)
            }
            _ => todo!(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum StreamCommand {
    PlayNow(AudioObject, u32),
    PlayAt(AudioObject, Instant, u32),
    GetStatus(std::sync::mpsc::Sender<Status>),
    GetLatency(std::sync::mpsc::Sender<Option<u32>>),
    Stop,
    Close,
}

#[derive(Debug, Clone)]
pub enum Status {
    Playing,
    Stopped,
}

#[derive(Debug)]
pub enum CallbackCommand {
    /// Set the audio object to play with the given delay in samples
    SetAudioObject(AudioObject, u32),
    /// Remove the audio object
    RemoveAudioObject,
    /// Timestamp the current chunk of data
    Timestamp(oneshot::Sender<Instant>),
}

#[derive(Clone)]
pub struct Stream {
    cpal_config: cpal::StreamConfig,
    cpal_device: cpal::Device,
    closed: bool,
    // channels for communication with the stream thread
    command_sender: std::sync::mpsc::Sender<StreamCommand>,
    sample_rate: u32,
}

impl Stream {
    pub fn new(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        sample_format: cpal::SampleFormat,
    ) -> Self {
        match sample_format {
            cpal::SampleFormat::I16 => Stream::new_typed::<i16>(device, config),
            // cpal::SampleFormat::I24 => run::<I24>(&device, &stream_config),
            cpal::SampleFormat::I32 => Stream::new_typed::<i32>(device, config),
            // cpal::SampleFormat::I48 => run::<I48>(&device, &stream_config),
            cpal::SampleFormat::I64 => Stream::new_typed::<i64>(device, config),
            cpal::SampleFormat::U8 => Stream::new_typed::<u8>(device, config),
            cpal::SampleFormat::U16 => Stream::new_typed::<u16>(device, config),
            // cpal::SampleFormat::U24 => run::<U24>(&device, &stream_config),
            cpal::SampleFormat::U32 => Stream::new_typed::<u32>(device, config),
            // cpal::SampleFormat::U48 => run::<U48>(&device, &stream_config),
            cpal::SampleFormat::U64 => Stream::new_typed::<u64>(device, config),
            cpal::SampleFormat::F32 => Stream::new_typed::<f32>(device, config),
            cpal::SampleFormat::F64 => Stream::new_typed::<f64>(device, config),
            sample_format => panic!("Unsupported sample format '{sample_format}'"),
        }
    }

    pub fn new_typed<T>(device: &cpal::Device, config: &cpal::StreamConfig) -> Self
    where
        T: SizedSample + FromSample<f32>,
    {
        let (command_sender, command_receiver) = std::sync::mpsc::channel();

        let _config = config.clone();
        let _device = device.clone();

        // spawn a thread to handle the stream
        std::thread::spawn(move || {
            // create a cpal stream
            let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

            let _channels = _config.channels as usize;

            let mut ao_writer = None;

            // create a channel to communicate with the callback using CallbackCommand
            let (callback_sender, callback_receiver) = std::sync::mpsc::channel();

            let mut _current_sample = 0;

            let stream = _device
                .build_output_stream(
                    &_config,
                    move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                        // check if there is a new command
                        match callback_receiver.try_recv() {
                            Ok(CallbackCommand::SetAudioObject(audio_object, delay)) => {
                                ao_writer = Some(
                                    audio_object.into_writer(_config.sample_rate.0, _channels),
                                );
                                ao_writer.as_mut().unwrap().move_by(delay as usize);
                                _current_sample = 0;
                            }
                            Ok(CallbackCommand::Timestamp(sender)) => {
                                sender.send(Instant::now()).unwrap();
                            }
                            Ok(CallbackCommand::RemoveAudioObject) => {
                                ao_writer = None;
                            }
                            _ => {}
                        }
                        if let Some(_ao_writer) = ao_writer.as_mut() {
                            // write the audio object data
                            let out = _ao_writer.write_data(data).unwrap();
                            if out {
                                ao_writer = None;
                            }
                        } else {
                            for sample in data.iter_mut() {
                                *sample = T::from_sample(0.0);
                            }
                        }
                    },
                    err_fn,
                    None,
                )
                .unwrap();
            stream.play().unwrap();

            let scheudled_aos: Arc<Mutex<Vec<(AudioObject, Instant)>>> =
                Arc::new(Mutex::new(Vec::new()));

            // create another thread who's job is dispatching the audio objects at the right time
            // for this, it will iterate over the scheduled audio objects and check if they should be played
            // if empty, it will sleep for a short time and then check again
            // if none of the audio objects are scheduled to be played, it will sleep until 2ms before the next audio object is scheduled to be played
            let _scheudled_aos_clone = scheudled_aos.clone();
            let _callback_sender = callback_sender.clone();

            std::thread::spawn(move || {
                loop {
                    let mut scheudled_aos = _scheudled_aos_clone.lock().unwrap();
                    if scheudled_aos.is_empty() {
                        // nothing to do, sleep for a short time
                        std::thread::sleep(Duration::from_millis(1));
                    } else {
                        // get the time of the next audio object
                        let next_time = scheudled_aos.iter().map(|(_, t)| *t).min().unwrap();
                        let now = Instant::now();
                        if next_time > now {
                            // sleep until 2ms before the next audio object is scheduled to be played
                            let next_check_time = next_time - Duration::from_millis(2);
                            if next_check_time > now {
                                std::thread::sleep(next_check_time - now);
                            }
                        } else {
                            // get the audio objects that should be played now
                            let now = Instant::now();

                            scheudled_aos.retain(|(ao, t)| {
                                if *t <= now {
                                    _callback_sender
                                        .send(CallbackCommand::SetAudioObject(ao.clone(), 0))
                                        .unwrap();
                                    false
                                } else {
                                    true
                                }
                            });
                        }
                    }
                }
            });

            // now start waiting for commands
            for command in command_receiver {
                match command {
                    StreamCommand::PlayNow(audio_object, _) => {
                        callback_sender
                            .send(CallbackCommand::SetAudioObject(audio_object, 0))
                            .unwrap();
                    }
                    StreamCommand::PlayAt(audio_object, at, _) => {
                        let mut scheudled_aos = scheudled_aos.lock().unwrap();
                        scheudled_aos.push((audio_object, at));
                    }
                    StreamCommand::Stop => {
                        callback_sender
                            .send(CallbackCommand::RemoveAudioObject)
                            .unwrap();
                    }
                    StreamCommand::GetStatus(sender) => {
                        sender.send(Status::Playing).unwrap();
                    }
                    StreamCommand::GetLatency(sender) => {
                        sender.send(stream.latency()).unwrap();
                    }
                    StreamCommand::Close => {
                        callback_sender
                            .send(CallbackCommand::RemoveAudioObject)
                            .unwrap();
                        break;
                    }
                }
            }
        });

        Self {
            cpal_config: config.clone(),
            cpal_device: device.clone(),
            closed: false,
            command_sender,
            sample_rate: config.sample_rate.0,
        }
    }

    pub fn play_now(&self, audio_object: AudioObject) {
        self.command_sender
            .send(StreamCommand::PlayNow(audio_object, 0))
            .unwrap();
    }

    pub fn play_at(&self, audio_object: AudioObject, at: Instant) {
        self.command_sender
            .send(StreamCommand::PlayAt(audio_object, at, 0))
            .unwrap();
    }

    pub fn latency_samples(&self) -> Option<u32> {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.command_sender
            .send(StreamCommand::GetLatency(sender))
            .unwrap();
        receiver.recv().unwrap()
    }

    pub fn latency_duration(&self) -> Option<Duration> {
        self.latency_samples().map(|samples| {
            let sample_rate = self.cpal_config.sample_rate.0 as f32;
            Duration::from_secs_f32(samples as f32 / sample_rate)
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.cpal_config.sample_rate.0
    }
}
