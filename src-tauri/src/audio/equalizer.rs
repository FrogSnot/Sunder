use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, RwLock};
use rodio::{Sample, Source};

pub const BAND_COUNT: usize = 10;
const BAND_FREQUENCIES: [f32; BAND_COUNT] = [
    32.0, 64.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

const Q_FACTOR: f32 = 1.414;

#[derive(Clone)]
pub struct EqSettings {
    pub enabled: bool,
    pub gains: [f32; BAND_COUNT],
}

impl Default for EqSettings {
    fn default() -> Self {
        Self { enabled: false, gains: [0.0; BAND_COUNT] }
    }
}

pub trait EqSample: Sample + Send + Sync + 'static {
    fn to_f32_sample(self) -> f32;
}

impl EqSample for i16 {
    #[inline]
    fn to_f32_sample(self) -> f32 {
        self as f32 / 32768.0
    }
}

impl EqSample for f32 {
    #[inline]
    fn to_f32_sample(self) -> f32 {
        self
    }
}

impl EqSample for u16 {
    #[inline]
    fn to_f32_sample(self) -> f32 {
        (self as f32 - 32768.0) / 32768.0
    }
}

struct BiquadCoeffs {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

struct BiquadState {
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl BiquadState {
    fn new() -> Self {
        Self { x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0 }
    }

    fn process(&mut self, c: &BiquadCoeffs, x: f64) -> f64 {
        let y = c.b0 * x + c.b1 * self.x1 + c.b2 * self.x2
            - c.a1 * self.y1 - c.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

fn peaking_eq(freq: f32, gain_db: f32, q: f32, sr: f32) -> BiquadCoeffs {
    let a = 10.0_f64.powf(gain_db as f64 / 40.0);
    let w0 = 2.0 * std::f64::consts::PI * freq as f64 / sr as f64;
    let sin_w0 = w0.sin();
    let cos_w0 = w0.cos();
    let alpha = sin_w0 / (2.0 * q as f64);

    let a0 = 1.0 + alpha / a;
    BiquadCoeffs {
        b0: (1.0 + alpha * a) / a0,
        b1: (-2.0 * cos_w0) / a0,
        b2: (1.0 - alpha * a) / a0,
        a1: (-2.0 * cos_w0) / a0,
        a2: (1.0 - alpha / a) / a0,
    }
}

pub struct EqSource<S: Source>
where
    S::Item: EqSample,
{
    inner: S,
    settings: Arc<RwLock<EqSettings>>,
    states: Vec<Vec<BiquadState>>,
    coeffs: Vec<BiquadCoeffs>,
    cached_gains: [f32; BAND_COUNT],
    enabled: bool,
    channels: u16,
    sample_rate: u32,
    channel_idx: u16,
    fade_samples: usize,
    silence_samples: usize,
    fade_counter: usize,
    pending_seek: Arc<AtomicI64>,
    fade_out_counter: usize,
    fade_out_samples: usize,
    needs_refresh: Arc<AtomicBool>,
}

impl<S: Source> EqSource<S>
where
    S::Item: EqSample,
{
    pub fn new(
        inner: S,
        settings: Arc<RwLock<EqSettings>>,
        pending_seek: Arc<AtomicI64>,
    ) -> Self {
        let channels = inner.channels();
        let sample_rate = inner.sample_rate();

        let states = (0..channels)
            .map(|_| (0..BAND_COUNT).map(|_| BiquadState::new()).collect())
            .collect();

        let coeffs = BAND_FREQUENCIES
            .iter()
            .map(|&f| peaking_eq(f, 0.0, Q_FACTOR, sample_rate as f32))
            .collect();

        let enabled = settings.read().unwrap().enabled;

        Self {
            inner,
            settings,
            states,
            coeffs,
            cached_gains: [0.0; BAND_COUNT],
            enabled,
            channels,
            sample_rate,
            channel_idx: 0,
            fade_samples: (sample_rate as f32 * 0.10) as usize, // 100ms fade
            silence_samples: (sample_rate as f32 * 0.05) as usize, // 50ms absolute silence
            fade_counter: 0,
            pending_seek,
            fade_out_counter: 0,
            fade_out_samples: (sample_rate as f32 * 0.02) as usize, // 20ms fade-out
            needs_refresh: Arc::new(AtomicBool::new(false)),
        }
    }

    fn refresh(&mut self) {
        let s = self.settings.read().unwrap();
        self.enabled = s.enabled;
        if s.gains != self.cached_gains {
            self.cached_gains = s.gains;
            self.coeffs = BAND_FREQUENCIES
                .iter()
                .zip(self.cached_gains.iter())
                .map(|(&f, &g)| peaking_eq(f, g, Q_FACTOR, self.sample_rate as f32))
                .collect();
        }
    }
}

impl<S: Source> Iterator for EqSource<S>
where
    S::Item: EqSample,
{
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let ch = self.channel_idx as usize;

        // Handle pending seek (Fade-out -> Seek -> Reset -> Fade-in)
        if ch == 0 {
            let ms = self.pending_seek.load(Ordering::Acquire);
            if ms >= 0 {
                if self.fade_out_counter < self.fade_out_samples {
                    // Still fading out
                    self.fade_out_counter += 1;
                } else {
                    // Fade out finished, perform actual seek
                    let pos = std::time::Duration::from_millis(ms as u64);
                    let _ = self.try_seek(pos);
                    // Reset Atomic to -1 after handling
                    self.pending_seek.store(-1, Ordering::Release);
                    self.fade_out_counter = 0;
                }
            } else {
                self.fade_out_counter = 0;
            }
        }

        let sample_raw = self.inner.next()?;
        let sample = sample_raw.to_f32_sample();
        self.channel_idx = (self.channel_idx + 1) % self.channels;

        if ch == 0 && self.needs_refresh.load(Ordering::Relaxed) {
            self.refresh();
            self.needs_refresh.store(false, Ordering::Relaxed);
        }

        let mut out = if self.enabled {
            let mut v = sample as f64;
            for (i, state) in self.states[ch].iter_mut().enumerate() {
                v = state.process(&self.coeffs[i], v);
            }
            v.clamp(-1.0, 1.0) as f32
        } else {
            sample
        };

        // Apply fade-out gating if we are in a pending seek
        if self.fade_out_counter > 0 {
            let t = 1.0 - (self.fade_out_counter as f32 / self.fade_out_samples as f32);
            out *= t * t; // Quadratic fade out
        }

        // Silence + Fade-in logic (always active to prevent pops)
        if self.fade_counter < self.silence_samples {
            out = 0.0;
            if ch == (self.channels - 1) as usize {
                self.fade_counter += 1;
            }
        } else if self.fade_counter < (self.silence_samples + self.fade_samples) {
            let t = (self.fade_counter - self.silence_samples) as f32 / self.fade_samples as f32;
            let gain = t * t * t;
            out *= gain;
            if ch == (self.channels - 1) as usize {
                self.fade_counter += 1;
            }
        }

        Some(out)
    }
}

impl<S: Source> Source for EqSource<S>
where
    S::Item: EqSample,
{
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: std::time::Duration) -> Result<(), rodio::source::SeekError> {
        self.inner.try_seek(pos)?;
        for ch_states in &mut self.states {
            for state in ch_states {
                *state = BiquadState::new();
            }
        }
        self.fade_counter = 0;
        Ok(())
    }
}
