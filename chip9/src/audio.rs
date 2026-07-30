//! The buzzer. Ported from `js/audio.js`.
//!
//! CHIP-9 has exactly one sound: while the sound timer is non-zero, a tone
//! plays. The JavaScript version connected and disconnected a 400 Hz square
//! wave oscillator; this does the same with an SDL audio callback, but ramps
//! the gain over a few milliseconds so that starting and stopping does not
//! click.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{Error, Result};
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use sdl2::AudioSubsystem;

/// Pitch of the buzzer, matching the JavaScript oscillator.
const FREQUENCY_HZ: f32 = 400.0;

/// Peak amplitude. CHIP-9 beeps a lot, so this stays modest.
const AMPLITUDE: f32 = 0.12;

/// How much of the amplitude the gain travels per sample, which works out to a
/// ramp of about four milliseconds at 44.1 kHz.
const GAIN_STEP: f32 = 1.0 / 180.0;

const SAMPLE_RATE: i32 = 44_100;

struct SquareWave {
    /// Position within the current period, in `0.0..1.0`.
    phase: f32,
    phase_step: f32,
    /// Where the gain is heading, shared with the emulator thread.
    enabled: Arc<AtomicBool>,
    /// Where the gain is now, in `0.0..=1.0`.
    gain: f32,
}

impl AudioCallback for SquareWave {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        let target = f32::from(u8::from(self.enabled.load(Ordering::Relaxed)));

        for sample in out.iter_mut() {
            if self.gain < target {
                self.gain = (self.gain + GAIN_STEP).min(target);
            } else if self.gain > target {
                self.gain = (self.gain - GAIN_STEP).max(target);
            }

            *sample = if self.phase < 0.5 {
                AMPLITUDE
            } else {
                -AMPLITUDE
            } * self.gain;
            self.phase = (self.phase + self.phase_step).fract();
        }
    }
}

/// Plays the CHIP-9 tone on demand.
pub struct Beeper {
    device: AudioDevice<SquareWave>,
    enabled: Arc<AtomicBool>,
}

impl Beeper {
    /// Opens the audio device the interpreter beeps through.
    ///
    /// # Errors
    ///
    /// Returns an error when SDL cannot open a playback device.
    pub fn new(audio_subsystem: &AudioSubsystem) -> Result<Self> {
        let desired = AudioSpecDesired {
            freq: Some(SAMPLE_RATE),
            channels: Some(1),
            samples: Some(512),
        };

        let enabled = Arc::new(AtomicBool::new(false));
        let callback_enabled = Arc::clone(&enabled);

        let device = audio_subsystem
            .open_playback(None, &desired, |spec| SquareWave {
                phase: 0.0,
                phase_step: FREQUENCY_HZ / spec.freq as f32,
                enabled: callback_enabled,
                gain: 0.0,
            })
            .map_err(Error::msg)?;

        // The device stays open for the whole run: the gain ramp, not the
        // device, is what starts and stops the tone.
        device.resume();

        Ok(Self { device, enabled })
    }

    /// Starts or stops the tone.
    pub fn set_playing(&self, playing: bool) {
        self.enabled.store(playing, Ordering::Relaxed);
    }

    /// The sample rate the audio device actually opened with.
    #[must_use]
    pub fn sample_rate(&self) -> i32 {
        self.device.spec().freq
    }
}
