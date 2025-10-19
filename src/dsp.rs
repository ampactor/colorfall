use crate::util;
use nih_plug::prelude::*;

// --- CORE DSP CONSTANTS ---
pub const TILT_MAX_SHIFT_SEMITONES: f32 = 4.0;
pub const MAX_BANDS: usize = 5;
pub const BAND_CENTER_FREQS: [f32; MAX_BANDS] = [100.0, 400.0, 2500.0, 7000.0, 12000.0];
pub const MAX_COMPENSATION_DB: f32 = 6.0; // Max Q-Boost gain
pub const KNEE_MAX_DB: f32 = 8.0; // Max knee width at Amount = 1.0

/// State for a single processing band.
#[derive(Clone)]
pub struct ProcessingBand {
    /// The serial compensation EQ filter for this band's frequency region.
    pub compensation_eq: Biquad,

    // Envelope and GR states
    pub envelope: f32,
    pub applied_gr_smoother: Smoother<f32>,
}

impl Default for ProcessingBand {
    fn default() -> Self {
        Self {
            compensation_eq: Biquad::default(),
            envelope: 0.0,
            applied_gr_smoother: Smoother::new(SmoothingStyle::Exponential(1.0)),
        }
    }
}

impl ProcessingBand {
    /// Resets the state of the processing band.
    pub fn reset(&mut self) {
        self.compensation_eq.reset();
        self.envelope = 0.0;
        self.applied_gr_smoother.reset(1.0);
    }
}
/// Helper to convert decibels to linear gain.
pub fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// Shifts a frequency by a certain number of semitones based on the `tilt` parameter.
pub fn shift_frequency(base_freq: f32, tilt: f32) -> f32 {
    let shift = tilt * TILT_MAX_SHIFT_SEMITONES;
    base_freq * 2.0_f32.powf(shift / 12.0)
}

/// Biquad filter state for one channel.
#[derive(Default, Clone, Copy)]
pub struct BiquadState {
    z1: f32,
    z2: f32,
}

/// Coefficients for a stereo biquad filter, calculated from specifications.
#[derive(Default, Clone, Copy)]
pub struct BiquadCoefficients {
    pub a1: f32,
    pub a2: f32,
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
}

impl BiquadCoefficients {
    /// Calculates coefficients for a 2nd order Linkwitz-Riley low-pass filter.
    pub fn calculate_lr_lowpass(sample_rate: f32, cutoff_freq: f32) -> Self {
        let w0 = 2.0 * std::f32::consts::PI * cutoff_freq / sample_rate;
        let cos_w0 = w0.cos();
        // Q = 1/sqrt(2) for a Linkwitz-Riley crossover
        // The 0.7071... value is 1/sqrt(2)
        let q = std::f32::consts::FRAC_1_SQRT_2;
        let alpha = w0.sin() / (2.0 * q);

        let b0 = (1.0 - cos_w0) / 2.0;
        let b1 = 1.0 - cos_w0;
        let b2 = (1.0 - cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        let d = a0;
        Self {
            b0: b0 / d,
            b1: b1 / d,
            b2: b2 / d,
            a1: a1 / d,
            a2: a2 / d,
        }
    }

    /// Calculates coefficients for a peaking EQ filter based on the Audio EQ Cookbook.
    pub fn calculate_peaking(sample_rate: f32, freq: f32, q: f32, gain_db: f32) -> Self {
        let a = db_to_gain(gain_db);
        let w0 = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        // Alpha calculation for Peaking Filter (simplest form)
        let alpha = sin_w0 / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        let d = a0;
        // The resulting coefficients are normalized by 1/D
        Self {
            b0: b0 / d,
            b1: b1 / d,
            b2: b2 / d,
            a1: a1 / d,
            a2: a2 / d,
        }
    }
}

/// A stereo biquad filter using a transposed direct form 2 structure.
#[derive(Default, Clone, Copy)]
pub struct Biquad {
    coefs: BiquadCoefficients,
    state_l: BiquadState,
    state_r: BiquadState,
}

impl Biquad {
    /// Processes a stereo sample pair through the filter.
    pub fn process(&mut self, sample_l: f32, sample_r: f32) -> (f32, f32) {
        let c = self.coefs;

        // Channel L: Direct Form 2 Transposed
        let out_l = c.b0 * sample_l + self.state_l.z1;
        self.state_l.z1 = c.b1 * sample_l - c.a1 * out_l + self.state_l.z2;
        self.state_l.z2 = c.b2 * sample_l - c.a2 * out_l;

        // Channel R: Direct Form 2 Transposed
        let out_r = c.b0 * sample_r + self.state_r.z1;
        self.state_r.z1 = c.b1 * sample_r - c.a1 * out_r + self.state_r.z2;
        self.state_r.z2 = c.b2 * sample_r - c.a2 * out_r;

        (out_l, out_r)
    }
    /// Updates the filter's coefficients to a new Linkwitz-Riley low-pass specification.
    pub fn update_lr_lowpass(&mut self, sample_rate: f32, cutoff_freq: f32) {
        self.coefs = BiquadCoefficients::calculate_lr_lowpass(sample_rate, cutoff_freq);
    }

    /// Updates the filter's coefficients to a new peaking EQ specification.
    pub fn update_peaking(&mut self, sample_rate: f32, freq: f32, q: f32, gain_db: f32) {
        self.coefs = BiquadCoefficients::calculate_peaking(sample_rate, freq, q, gain_db);
    }

    /// Resets the filter's internal state.
    pub fn reset(&mut self) {
        self.state_l = BiquadState::default();
        self.state_r = BiquadState::default();
    }
}

/// Novel Saturation: Cubic distortion, intensity linked to Amount.
pub fn saturate(sample: f32, amount: f32) -> f32 {
    // Saturation drive increases with amount.
    let drive = amount * 0.9 + 0.1;

    // Cubic approximation (Tanh approximation for a resonant bloom effect)
    let out = drive * sample - (drive.powi(2) / 3.0) * sample.powf(3.0);

    // Soft clipping/smoothing on the output
    (out * (1.0 - amount * 0.3)).clamp(-1.0, 1.0)
}

/// Computes target gain reduction (in linear gain, 0 to 1) for a band.
pub fn calculate_target_gr(band_idx: usize, amount: f32, tilt: f32, envelope: f32) -> f32 {
    // --- 1. Dynamic Parameter Calculation based on Amount and Tilt ---

    // Tilt Bias: Low bands (-ve tilt favors low, +ve tilt de-favors low)
    let tilt_bias = match band_idx {
        // More processing on tilted-towards bands
        0..=1 => 1.0 + (tilt * -0.8), // Bands 1 & 2 (low-mids)
        2 => 1.0,                     // Band 3 (mid)
        3..=4 => 1.0 + (tilt * 0.8),  // Bands 4 & 5 (high-mids/highs)
        _ => 1.0,
    }
    .clamp(0.2, 1.8f32);

    // Band Frequency Factor: Lower frequencies are inherently more powerful, so we apply more compression.
    let freq_factor = match band_idx {
        0 => 1.5, // Lowest band is most aggressive
        1 => 1.2,
        2 => 1.0,
        3 => 0.8,
        4 => 0.5, // Highest band is least aggressive
        _ => 1.0,
    };

    let intensity = amount * tilt_bias * freq_factor;

    // Threshold: Drops rapidly with Amount
    let threshold_db = -10.0 - (25.0 * intensity.powf(2.0))
        - (tilt * -5.0 * ((band_idx as f32 / 4.0) - 0.5));

    // Ratio: Increases non-linearly with Amount
    let ratio = 1.1 + (15.0 * amount.powf(2.5));

    // Knee: Widens with Amount
    let knee_db = KNEE_MAX_DB * amount.powf(1.5);

    // --- 2. Gain Computer (Simplified Soft-Knee) ---
    let input_db = util::gain_to_db(envelope);
    let gr_db = if input_db < threshold_db - (knee_db / 2.0) {
        // Below Knee (No GR)
        0.0
    } else if input_db > threshold_db + (knee_db / 2.0) {
        // Above Knee (Hard Ratio GR)
        (threshold_db - input_db) * (1.0 - (1.0 / ratio))
    } else {
        // Inside Knee (Soft Knee GR)
        let knee_range = knee_db;
        let _x = (input_db - (threshold_db - knee_range / 2.0)) / knee_range;
        -(1.0 - 1.0 / ratio) * (input_db - (threshold_db - knee_range / 2.0)).powi(2)
            / (2.0 * knee_range)
    };

    // GR must be non-positive
    db_to_gain(gr_db.min(0.0))
}

/// Calculates dynamic attack/release times in samples based on Amount and Frequency.
pub fn calculate_dynamic_time_constants(
    sample_rate: f32,
    band_idx: usize,
    amount: f32,
) -> (f32, f32) {
    let base_freq = BAND_CENTER_FREQS[band_idx];

    // Frequency Scaling: Higher frequencies get faster times
    let freq_scale = (base_freq / 2000.0).sqrt().clamp(0.5, 2.0);

    // Amount Scaling: Higher amount means faster dynamics (more aggressive 'snap')
    let amount_scale = 1.0 - (amount * 0.8); // 1.0 at 0% amount, 0.2 at 100% amount

    // Base Attack (ms): 1ms (fast) to 20ms (slow)
    let attack_ms = (20.0 * amount_scale) / freq_scale.powf(0.5);

    // Base Release (ms): 50ms (fast) to 400ms (slow)
    let release_ms = (300.0 * amount_scale) / freq_scale.powf(1.5);

    // Convert ms to samples/sample_rate
    let attack_samples = sample_rate * (attack_ms / 1000.0);
    let release_samples = sample_rate * (release_ms / 1000.0);

    (attack_samples, release_samples)
}