use std::sync::{Arc, RwLock};

use rodio::Source;

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

pub struct EqSource<S: Source<Item = f32>> {
    inner: S,
    settings: Arc<RwLock<EqSettings>>,
    states: Vec<Vec<BiquadState>>,
    coeffs: Vec<BiquadCoeffs>,
    cached_gains: [f32; BAND_COUNT],
    enabled: bool,
    channels: u16,
    sample_rate: u32,
    channel_idx: u16,
}

impl<S: Source<Item = f32>> EqSource<S> {
    pub fn new(inner: S, settings: Arc<RwLock<EqSettings>>) -> Self {
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

impl<S: Source<Item = f32>> Iterator for EqSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;
        let ch = self.channel_idx as usize;
        self.channel_idx = (self.channel_idx + 1) % self.channels;

        if ch == 0 {
            self.refresh();
        }

        if !self.enabled {
            return Some(sample);
        }

        let mut v = sample as f64;
        for (i, state) in self.states[ch].iter_mut().enumerate() {
            v = state.process(&self.coeffs[i], v);
        }
        Some(v.clamp(-1.0, 1.0) as f32)
    }
}

impl<S: Source<Item = f32>> Source for EqSource<S> {
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
        self.inner.try_seek(pos)
    }
}
