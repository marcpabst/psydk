use std::path::Path;
use std::str::FromStr;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use skia_safe::resources::ResourceProvider;
use skia_safe::skottie::Animation;
use skia_safe::skottie::{Builder, BuilderFlags};
use strum_macros::EnumString;

#[derive(Debug, Clone, Copy, EnumString)]
#[strum(ascii_case_insensitive)]
pub enum PlaybackMode {
    Loop,
    Once,
    PingPong,
    Manual,
}

#[derive(Clone)]
pub struct LottieAnimation {
    animation: Animation,
    playback_mode: PlaybackMode,
    start_time: std::time::Instant,
    speed: f32,
    is_playing: bool,
}

impl LottieAnimation {
    pub fn from_file<P: AsRef<Path>>(path: P, playback_mode: PlaybackMode, speed: f32) -> Self {
        let animation = Builder::new()
            .make_from_file(path.as_ref())
            .expect("Failed to load Lottie animation");

        Self {
            animation,
            playback_mode,
            start_time: std::time::Instant::now(),
            speed,
            is_playing: false,
        }
    }

    pub fn animation(&self) -> &Animation {
        &self.animation
    }

    pub fn play(&mut self) {
        self.is_playing = true;
        self.start_time = std::time::Instant::now();
    }

    pub fn pause(&mut self) {
        self.is_playing = false;
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    pub fn update(&mut self) {
        if self.is_playing {
            let elapsed = self.start_time.elapsed().as_secs_f32() * self.speed;
            let duration = self.animation.duration();

            match self.playback_mode {
                PlaybackMode::Loop => {
                    // seek to normalized time 0-1 based on elapsed time and duration
                    self.animation.seek((elapsed / duration) % 1.0);
                }
                PlaybackMode::Once => {
                    if elapsed < duration {
                        self.animation.seek(elapsed / duration);
                    } else {
                        self.animation.seek(1.0);
                        self.is_playing = false;
                    }
                }
                PlaybackMode::PingPong => {
                    let t = (elapsed / duration) % 2.0;
                    if t < 1.0 {
                        self.animation.seek(t);
                    } else {
                        self.animation.seek(2.0 - t);
                    }
                }
                PlaybackMode::Manual => {
                    // Do nothing, user must manually seek
                }
            }
        }
    }
}
