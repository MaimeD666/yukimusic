use rustfft::{num_complex::Complex, Fft, FftPlanner};
use std::sync::Arc;

pub const FRAMES: usize = 4096;
pub const CHANNELS: usize = 2;
pub const HOP: usize = 800;
const BANDS: usize = 48;
const RATE: f32 = 48000.;

pub struct Spectrum {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    left: Vec<Complex<f32>>,
    right: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
}

impl Spectrum {
    pub fn new() -> Self {
        let fft = FftPlanner::<f32>::new().plan_fft_forward(FRAMES);
        let scratch = vec![Complex::default(); fft.get_inplace_scratch_len()];
        Self {
            fft,
            window: (0..FRAMES)
                .map(|i| {
                    0.5 - 0.5 * (2. * std::f32::consts::PI * i as f32 / (FRAMES - 1) as f32).cos()
                })
                .collect(),
            left: vec![Complex::default(); FRAMES],
            right: vec![Complex::default(); FRAMES],
            scratch,
        }
    }

    pub fn analyze(&mut self, stereo: &[f32]) -> [f32; BANDS] {
        if stereo.len() != FRAMES * CHANNELS {
            return [0.; BANDS];
        }
        for (i, pair) in stereo.chunks_exact(CHANNELS).enumerate() {
            self.left[i] = Complex::new(pair[0] * self.window[i], 0.);
            self.right[i] = Complex::new(pair[1] * self.window[i], 0.);
        }
        self.fft
            .process_with_scratch(&mut self.left, &mut self.scratch);
        self.fft
            .process_with_scratch(&mut self.right, &mut self.scratch);
        let mut bands = [0.; BANDS];
        for (i, value) in bands.iter_mut().enumerate() {
            // Fractional bin overlap avoids rounding several bass bands to the
            // exact same FFT bin. Keep the 35 Hz–16 kHz scale shared with bass mode.
            let lower = 35. * (16000f32 / 35.).powf(i as f32 / BANDS as f32) * FRAMES as f32 / RATE;
            let upper =
                35. * (16000f32 / 35.).powf((i + 1) as f32 / BANDS as f32) * FRAMES as f32 / RATE;
            let first = (lower - 0.5).ceil().max(1.) as usize;
            let last = (upper + 0.5).floor().min((FRAMES / 2 - 1) as f32) as usize;
            let mut power = 0.;
            let mut weight = 0.;
            for bin in first..=last {
                let overlap = (upper.min(bin as f32 + 0.5) - lower.max(bin as f32 - 0.5)).max(0.);
                // Average channel energy, not samples: antiphase stereo must
                // remain visible, and a right-only instrument must not vanish.
                power += (self.left[bin].norm_sqr() + self.right[bin].norm_sqr()) * 0.5 * overlap;
                weight += overlap;
            }
            let power = power / weight.max(f32::EPSILON) / (FRAMES * FRAMES) as f32;
            let db = 10. * power.max(1e-10).log10();
            *value = ((db + 70.) / 60.).clamp(0., 1.);
        }
        bands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f32, left: f32, right: f32) -> Vec<f32> {
        (0..FRAMES)
            .flat_map(|i| {
                let sample = (2. * std::f32::consts::PI * hz * i as f32 / RATE).sin();
                [sample * left, sample * right]
            })
            .collect()
    }

    #[test]
    fn peaks_follow_the_sound_frequency() {
        let mut analyzer = Spectrum::new();
        for hz in [55., 125., 440., 1000., 4000., 10000.] {
            let bands = analyzer.analyze(&tone(hz, 0.7, 0.7));
            let peak = bands
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .unwrap()
                .0;
            let expected =
                ((hz / 35.).ln() / (16000f32 / 35.).ln() * BANDS as f32).floor() as usize;
            assert!(
                peak.abs_diff(expected) <= 1,
                "{hz} Hz: peak {peak}, expected {expected}"
            );
            assert!(bands[peak] > 0.4);
            assert!(bands[(peak + 24) % BANDS] < 0.05);
        }
    }

    #[test]
    fn both_channels_and_antiphase_remain_visible() {
        let mut analyzer = Spectrum::new();
        let left = analyzer.analyze(&tone(440., 0.7, 0.));
        let right = analyzer.analyze(&tone(440., 0., 0.7));
        let mono = analyzer.analyze(&tone(440., 0.7, 0.7));
        let antiphase = analyzer.analyze(&tone(440., 0.7, -0.7));
        for i in 0..BANDS {
            assert!((left[i] - right[i]).abs() < 1e-6);
            assert!((mono[i] - antiphase[i]).abs() < 1e-6);
        }
        assert!(right.iter().copied().fold(0f32, f32::max) > 0.6);
    }

    #[test]
    fn dynamics_and_silence_are_not_normalized_away() {
        let mut analyzer = Spectrum::new();
        let loud = analyzer.analyze(&tone(440., 0.7, 0.7));
        let quiet = analyzer.analyze(&tone(440., 0.07, 0.07));
        let peak = loud
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        assert!(loud[peak] - quiet[peak] > 0.3);
        assert_eq!(analyzer.analyze(&vec![0.; FRAMES * CHANNELS]), [0.; BANDS]);
    }
}
