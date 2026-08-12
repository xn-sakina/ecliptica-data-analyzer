use std::{sync::atomic::Ordering, thread, time::Duration};

use crossbeam_channel::{Receiver, RecvTimeoutError};
use rodio::{OutputStream, Sink, Source, source::SineWave};

use crate::{
    config::AlertSoundStyle,
    runtime::{EventLevel, SharedState},
};

/// Keep the slider useful across quiet speakers without allowing the generated
/// tones to approach clipping at its maximum value.
const ALERT_BASE_GAIN: f32 = 0.28;

#[derive(Debug, Clone, Copy)]
pub enum SoundCommand {
    Locked(f32, AlertSoundStyle),
    Unlocked(f32, AlertSoundStyle),
    PreviewLocked(f32, AlertSoundStyle),
    PreviewUnlocked(f32, AlertSoundStyle),
}

pub fn spawn(shared: SharedState, receiver: Receiver<SoundCommand>) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("ecliptica-audio".to_owned())
        .spawn(move || run(shared, receiver))
        .expect("failed to start audio worker")
}

fn run(shared: SharedState, receiver: Receiver<SoundCommand>) {
    let audio = OutputStream::try_default();
    let (_stream, handle) = match audio {
        Ok(value) => value,
        Err(error) => {
            shared.event(
                EventLevel::Error,
                format!(
                    "{}: {error}",
                    shared.text(crate::i18n::text::AUDIO_INIT_FAILED)
                ),
            );
            return;
        }
    };
    while !shared.shutdown.load(Ordering::Relaxed) {
        match receiver.recv_timeout(Duration::from_millis(200)) {
            Ok(command) => {
                let sink = match Sink::try_new(&handle) {
                    Ok(sink) => sink,
                    Err(error) => {
                        shared.event(
                            EventLevel::Error,
                            format!(
                                "{}: {error}",
                                shared.text(crate::i18n::text::AUDIO_PLAYBACK_FAILED)
                            ),
                        );
                        continue;
                    }
                };
                let volume = match command {
                    SoundCommand::Locked(volume, _)
                    | SoundCommand::Unlocked(volume, _)
                    | SoundCommand::PreviewLocked(volume, _)
                    | SoundCommand::PreviewUnlocked(volume, _) => volume,
                };
                // Sine tones remain portable and dependency-free. Envelopes
                // soften their edges while the base gain keeps 1.0 audible.
                let gain = volume.clamp(0.0, 1.0) * ALERT_BASE_GAIN;
                match command {
                    SoundCommand::Locked(_, style) | SoundCommand::PreviewLocked(_, style) => {
                        append_locked(&sink, style, gain);
                    }
                    SoundCommand::Unlocked(_, style) | SoundCommand::PreviewUnlocked(_, style) => {
                        append_unlocked(&sink, style, gain);
                    }
                }
                sink.detach();
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn append_locked(sink: &Sink, style: AlertSoundStyle, gain: f32) {
    match style {
        AlertSoundStyle::Soft => {
            // A gentle rising two-note cue retained from the original design.
            sink.append(tone(349.23, 170, 45, 65, gain * 0.78));
            sink.append(tone(523.25, 270, 55, 110, gain * 0.9));
        }
        AlertSoundStyle::Crisp => {
            // Three short ascending chimes cut through speech without lingering.
            sink.append(chime(523.25, 90, gain * 0.72));
            sink.append(chime(659.25, 90, gain * 0.8));
            sink.append(chime(783.99, 150, gain * 0.9));
        }
        AlertSoundStyle::Prominent => {
            // Alternating, harmonic-rich alarm pulses are intentionally obvious.
            for frequency in [880.0, 659.25, 880.0, 659.25] {
                sink.append(alarm_pulse(frequency, 145, gain));
            }
        }
    }
}

fn append_unlocked(sink: &Sink, style: AlertSoundStyle, gain: f32) {
    match style {
        AlertSoundStyle::Soft => {
            // A sustained low release chord retained from the original design.
            let release_chord = SineWave::new(196.0)
                .amplify(gain * 0.82)
                .mix(SineWave::new(98.0).amplify(gain * 0.24))
                .take_duration(Duration::from_millis(720))
                .fade_in(Duration::from_millis(120))
                .fade_out(Duration::from_millis(340));
            sink.append(release_chord);
        }
        AlertSoundStyle::Crisp => {
            // A descending pair gives the opposite direction to the lock cue.
            sink.append(chime(659.25, 120, gain * 0.8));
            sink.append(chime(392.0, 240, gain * 0.88));
        }
        AlertSoundStyle::Prominent => {
            // Two emphatic descending pulses remain distinct from the lock alarm.
            sink.append(alarm_pulse(783.99, 190, gain));
            sink.append(alarm_pulse(493.88, 300, gain * 0.92));
        }
    }
}

fn tone(
    frequency: f32,
    duration_ms: u64,
    fade_in_ms: u64,
    fade_out_ms: u64,
    gain: f32,
) -> impl Source<Item = f32> + Send {
    SineWave::new(frequency)
        .take_duration(Duration::from_millis(duration_ms))
        .fade_in(Duration::from_millis(fade_in_ms))
        .fade_out(Duration::from_millis(fade_out_ms))
        .amplify(gain)
}

fn chime(frequency: f32, duration_ms: u64, gain: f32) -> impl Source<Item = f32> + Send {
    SineWave::new(frequency)
        .amplify(gain * 0.72)
        .mix(SineWave::new(frequency * 2.0).amplify(gain * 0.18))
        .take_duration(Duration::from_millis(duration_ms))
        .fade_in(Duration::from_millis(8))
        .fade_out(Duration::from_millis(duration_ms / 2))
}

fn alarm_pulse(frequency: f32, duration_ms: u64, gain: f32) -> impl Source<Item = f32> + Send {
    SineWave::new(frequency)
        .amplify(gain * 0.72)
        .mix(SineWave::new(frequency * 1.5).amplify(gain * 0.26))
        .take_duration(Duration::from_millis(duration_ms))
        .fade_in(Duration::from_millis(12))
        .fade_out(Duration::from_millis(45))
}
