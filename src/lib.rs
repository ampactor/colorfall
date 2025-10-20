//! # ColorFall: A Multiband Dynamics Processor
//!
//! A multiband dynamics processor with a colorful character.
//!
//! A 5-band dynamic processor designed as a creative "soundgoodizer" and sound design tool.
//!
//!
//! ## Architecture
//!
//! The plugin employs a hybrid parallel/serial signal flow:
//!
//! 1.  **Parallel Dynamics:** The incoming audio is split into 5 frequency bands using Linkwitz-Riley
//!     crossover filters. Each band is processed in parallel with its own compressor and saturator.
//!     The parameters for these processes (threshold, ratio, attack, release, saturation drive) are
//!     dynamically linked to the main "Amount" and "Tilt" knobs.
//! 2.  **Serial EQ:** After the dynamics stage, the 5 processed bands are summed back together. This
//!     "wet" signal is then passed through a series of 5 cascading peaking EQ filters. The gain and
//!     Q of these filters are also dynamically linked to the "Amount" and "Tilt" knobs, creating
//!     a resonant, shifting character that interacts with itself.
//! 3.  **Loudness Compensation:** The RMS level of the final wet signal is compared to the original
//!     dry signal, and an automatic gain correction is applied to maintain consistent perceived loudness.
use nih_plug_vizia::ViziaState;

#[cfg(feature = "vizia")]
mod editor;

// All of our DSP code is in here
mod dsp;

use dsp::{
    Biquad, MAX_BANDS, MAX_COMPENSATION_DB, ProcessingBand, TILT_MAX_SHIFT_SEMITONES,
    shift_frequency,
};
use nih_plug::prelude::*;
use std::{f32::consts::FRAC_PI_2, sync::atomic::Ordering};
use std::{num::NonZeroU32, sync::Arc};

// --- PLUGIN PARAMETERS ---

/// The parameters for the ColorFall plugin.
#[derive(Params)]
struct ColorFallParams {
    #[cfg(feature = "vizia")]
    #[persist = "editor-state"]
    editor_state: Arc<ViziaState>,
    /// The main control knob. Drives compression, saturation, and EQ gain.
    /// Ranges from 0.0 (subtle) to 1.0 (mangled).
    #[id = "amount"]
    pub amount: FloatParam,

    /// Shifts the frequency focus of the processing.
    /// -1.0 focuses on low frequencies, +1.0 focuses on high frequencies.
    #[id = "tilt"]
    pub tilt: FloatParam,

    /// The dry/wet mix of the plugin.
    #[id = "mix"]
    pub mix: FloatParam,

    /// A final output gain stage.
    #[id = "output"]
    pub output: FloatParam,
}

impl Default for ColorFallParams {
    fn default() -> Self {
        Self {
            amount: FloatParam::new("Amount", 0.1, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Exponential(50.0))
                .with_unit(" %")
                .with_value_to_string(formatters::v2s_f32_percentage(1))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            // Exponential smoothing is generally more musical for gain-related parameters.
            tilt: FloatParam::new(
                "Tilt",
                0.0,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            )
            .with_unit(" Semitones")
            .with_value_to_string(formatters::v2s_f32_rounded(2))
            .with_smoother(SmoothingStyle::Exponential(50.0)),
            mix: FloatParam::new(
                "Mix",
                0.5, // Default Mix: 50% dry/wet
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit("%")
            .with_value_to_string(formatters::v2s_f32_percentage(1))
            // Using a constant power pan smoother for a more perceptually linear dry/wet mix
            .with_smoother(SmoothingStyle::Linear(50.0)),
            output: FloatParam::new(
                "Output",
                0.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 24.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
            .with_smoother(SmoothingStyle::Exponential(50.0)),
            // GUI state
            #[cfg(feature = "vizia")] editor_state: Self::default_editor_state(),
        }
    }
}
impl ColorFallParams {
    fn default_editor_state() -> Arc<ViziaState> {
        ViziaState::new(|| (500, 350))
    }
}

// --- MAIN PLUGIN STRUCT ---

/// The main plugin structure, holding the parameters and the DSP state.
/// This is the heart of the plugin, where audio processing and parameter handling
/// are orchestrated.
struct ColorFall {
    params: Arc<ColorFallParams>,
    sample_rate: f32,

    // Crossover filters to split the signal into bands
    crossovers: [Biquad; MAX_BANDS - 1],

    // The processing chain for each band
    bands: [ProcessingBand; MAX_BANDS],

    /// RMS trackers for the dry and wet signals, used for automatic gain compensation.
    dry_rms_tracker: f32,
    wet_rms_tracker: f32,

    /// A smoother for the automatic gain correction factor to prevent sudden changes.
    loudness_correction_smoother: Smoother<f32>,

    /// A smoother for the gain reduction meter to make it more readable.
    gr_meter_smoother: Smoother<f32>,

    /// The gain reduction value for the GUI meter.
    gain_reduction_meter: Arc<AtomicF32>,
}

impl Default for ColorFall {
    fn default() -> Self {
        Self {
            params: Arc::default(),
            sample_rate: 44100.0,
            crossovers: Default::default(),
            bands: Default::default(),
            dry_rms_tracker: 0.0,
            wet_rms_tracker: 0.0,
            loudness_correction_smoother: Smoother::new(SmoothingStyle::Exponential(200.0)),
            gr_meter_smoother: Smoother::new(SmoothingStyle::Exponential(50.0)),
            gain_reduction_meter: Arc::new(AtomicF32::new(0.0)),
        }
    }
}

// --- DSP LOGIC ---

/// The base crossover frequencies before any tilt is applied.
const BASE_CROSSOVER_FREQS: [f32; MAX_BANDS - 1] = [150.0, 800.0, 4000.0, 9000.0];

impl ColorFall {
    /// Updates all dynamically changing parameters based on the main controls.
    /// This is called once per block to set the "base" for the per-sample smoothers.
    /// It calculates the target crossover frequencies and the target EQ settings for each band
    /// based on the current 'Amount' and 'Tilt' values.
    fn update_dynamic_parameters(&mut self, amount: f32, tilt: f32) {
        for (i, band) in self.bands.iter_mut().enumerate() {
            // --- Tilt Bias Calculation ---
            // This applies a bias to the intensity of processing for each band based on the 'Tilt' control.
            // A negative tilt emphasizes processing on the lower bands, while a positive tilt
            // emphasizes the higher bands. The mid-band (i=2) is unaffected.
            let band_tilt_factor = match i {
                0..=1 => 1.0 + (tilt * -0.6),
                2 => 1.0,
                3..=4 => 1.0 + (tilt * 0.6),
                _ => 1.0,
            }
            .clamp(0.4, 1.6);
            let final_band_intensity = (amount * band_tilt_factor).sqrt(); // Use sqrt for a more responsive curve

            // --- Dynamic EQ Q-Factor and Gain Calculation ---
            // The 'Amount' knob directly influences the base Q-factor of the compensation EQ.
            // Higher 'Amount' values lead to a more resonant, "peaky" sound.
            let q_base = 0.7 + (8.0 * amount); // Q increases more linearly with Amount.

            // The 'Tilt' control also affects the Q-factor. Tilting towards the highs makes the
            // high-frequency EQs sharper (more resonant) and the low-frequency EQs broader, and vice-versa.
            // This adds to the "shifting character" of the plugin.
            let q_tilt_factor = 1.0 + (tilt * (i as f32 - 2.0) * 0.4);
            let q_factor = (q_base * q_tilt_factor).clamp(0.5, 20.0f32);

            // The gain of the compensation EQ is directly proportional to the band's final intensity.
            let compensation_gain_db = MAX_COMPENSATION_DB * final_band_intensity;

            // --- Dynamic Frequency Shifting ---
            // The crossover frequencies are shifted up or down based on the 'Tilt' control.
            let mut shifted_crossover_freqs = [0.0; MAX_BANDS - 1];
            for j in 0..(MAX_BANDS - 1) {
                let shifted_freq = shift_frequency(BASE_CROSSOVER_FREQS[j], tilt);
                shifted_crossover_freqs[j] = shifted_freq;
                self.crossovers[j].update_lr_lowpass(self.sample_rate, shifted_freq);
            }

            // The center frequency for each band's compensation EQ is the geometric mean of its
            // bounding crossover frequencies. This ensures the EQ always targets the center of the
            // processed band, even as the crossovers are shifting.
            let lower_bound = if i == 0 {
                20.0
            } else {
                shifted_crossover_freqs[i - 1]
            };
            let upper_bound = if i == MAX_BANDS - 1 {
                self.sample_rate / 2.0
            } else {
                shifted_crossover_freqs[i]
            };
            let band_center_freq = (lower_bound * upper_bound).sqrt();

            // Update Compensation EQ coefficients
            band.compensation_eq.update_peaking(
                // This now uses the dynamic band_center_freq
                self.sample_rate,
                band_center_freq,
                q_factor,
                compensation_gain_db,
            );
        }
    }

    /// Processes a single stereo sample through the entire multiband chain.
    /// Returns the processed (wet) sample and the total gain reduction in dB for this sample.
    fn process_sample(
        &mut self,
        sample_l: f32,
        sample_r: f32,
        amount: f32,
        tilt: f32,
    ) -> (f32, f32, f32) {
        let mut band_signals_l = [0.0; MAX_BANDS];
        let mut band_signals_r = [0.0; MAX_BANDS];

        let mut last_lp_l = sample_l;
        let mut last_lp_r = sample_r;
        let mut total_gr_db_sample = 0.0;

        // Split into bands using a cascade of Linkwitz-Riley crossover filters.
        for i in (0..(MAX_BANDS - 1)).rev() {
            let (lp_l, lp_r) = self.crossovers[i].process(last_lp_l, last_lp_r);
            band_signals_l[i + 1] = last_lp_l - lp_l; // High-pass is the remainder
            band_signals_r[i + 1] = last_lp_r - lp_r;
            last_lp_l = lp_l;
            last_lp_r = lp_r;
        }
        band_signals_l[0] = last_lp_l; // The final low-pass signal is the lowest band
        band_signals_r[0] = last_lp_r;

        let (mut wet_l, mut wet_r) = (0.0, 0.0);

        // Process each band's dynamics and saturation in parallel.
        for i in 0..MAX_BANDS {
            let (mut band_l, mut band_r) = (band_signals_l[i], band_signals_r[i]);

            // 1. Envelope Follower
            // A simple one-pole low-pass filter on the squared signal (power) acts as an
            // envelope follower. The attack and release times are calculated dynamically.
            let band_power = (band_l * band_l + band_r * band_r) * 0.5;
            let (attack, release) =
                dsp::calculate_dynamic_time_constants(self.sample_rate, i, amount);
            let alpha = if band_power > self.bands[i].envelope {
                1.0 - (-1.0 / attack).exp()
            } else {
                1.0 - (-1.0 / release).exp()
            };
            self.bands[i].envelope = (1.0 - alpha) * self.bands[i].envelope + alpha * band_power;
            let envelope_sqrt = self.bands[i].envelope.sqrt();

            // 2. Calculate Target GR
            // This function computes the desired gain reduction based on the envelope and dynamic parameters.
            let target_gr = dsp::calculate_target_gr(i, amount, tilt, envelope_sqrt);

            // 3. Apply Smoothed GR
            // The target GR is smoothed to prevent clicks and artifacts.
            self.bands[i]
                .applied_gr_smoother
                .set_target(self.sample_rate, target_gr);
            let gr_factor = self.bands[i].applied_gr_smoother.next();
            total_gr_db_sample += util::gain_to_db(gr_factor);

            band_l *= gr_factor;
            band_r *= gr_factor;

            // 4. Saturation
            // A novel cubic saturator adds harmonic content. Its intensity is linked to 'Amount'.
            band_l = dsp::saturate(band_l, amount);
            band_r = dsp::saturate(band_r, amount);

            // 5. Sum the processed bands back together
            wet_l += band_l;
            wet_r += band_r;
        }
        // Denormal Guard: Add a tiny offset to prevent floating point subnormals, which can
        // cause a massive performance hit on some CPUs.
        wet_l += 1.0e-20;
        wet_r += 1.0e-20;

        (wet_l, wet_r, total_gr_db_sample)
    }
}
// --- NIH-PLUG IMPLEMENTATION ---

impl Plugin for ColorFall {
    const NAME: &'static str = "ColorFall";
    const VENDOR: &'static str = "Colorfall";
    const URL: &'static str = "https://example.com"; // TODO: Update this
    const EMAIL: &'static str = "contact@example.com"; // TODO: Update this
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    // We're not using any background tasks or SysEx messages in this plugin.
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        // The sample rate may change on initialization, so we need to update it here
        self.sample_rate = buffer_config.sample_rate;
        // Then, call reset() to ensure all state is initialized correctly for the new sample rate.
        self.reset();
        true
    }

    fn reset(&mut self) {
        // Reset all DSP state, including filters and smoothers.
        for crossover in &mut self.crossovers {
            crossover.reset();
        }
        for band in &mut self.bands {
            band.reset();
        }
        self.loudness_correction_smoother.reset(1.0);
        self.gr_meter_smoother.reset(0.0);
        self.dry_rms_tracker = 0.0;
        self.wet_rms_tracker = 0.0;
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut block_avg_input = 0.0;
        let mut block_avg_output = 0.0;
        let mut total_gr_db = 0.0;

        // --- 1. DYNAMIC PARAMETER UPDATE ---
        // We update the filter coefficients once per block based on the unsmoothed parameter values.
        // This is a compromise for efficiency. While per-sample updates would be more accurate for
        // fast automation, it's computationally expensive. This block-based update is sufficient
        // for most use cases and avoids performance issues.
        self.update_dynamic_parameters(self.params.amount.value(), self.params.tilt.value());

        // --- 2. LOUDNESS CORRECTION CALCULATION ---

        // Calculate a makeup gain factor to match the wet signal's power (from the previous block)
        // to the dry signal's power. This introduces a one-block latency to the loudness
        // compensation, but it's a standard, stable, and efficient approach.
        let required_correction = if self.wet_rms_tracker > 1.0e-6 && self.dry_rms_tracker > 1.0e-6
        {
            (self.dry_rms_tracker / self.wet_rms_tracker).sqrt()
        } else {
            1.0
        };

        // Smooth the correction factor
        // We set the target here, and the smoother will gradually approach it over the block.
        self.loudness_correction_smoother
            .set_target(self.sample_rate, required_correction);

        // --- 3. SAMPLE PROCESSING LOOP ---
        let mut channels = buffer.iter_samples();
        let mut left = channels.next().unwrap();
        let mut right = channels.next().unwrap();
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            // Get smoothed parameter values for this sample
            // This is the core of the sample-accurate automation. Each parameter's smoother
            // provides the next value in its trajectory.
            let amount = self.params.amount.smoothed.next();
            let tilt = self.params.tilt.smoothed.next();
            let mix = self.params.mix.smoothed.next();
            let output_gain = util::db_to_gain(self.params.output.smoothed.next());
            let loudness_correction = self.loudness_correction_smoother.next();

            let mix_phase = mix * FRAC_PI_2;
            // A constant-power crossfade for the dry/wet mix. This is perceptually more
            // linear than a linear crossfade.
            let (dry_gain, wet_gain) = (mix_phase.cos(), mix_phase.sin());

            // We need to process interleaved audio, so we'll zip the two channel iterators
            let (sample_l, sample_r) = (*l, *r); // Dereference to get the values

            let (dry_l, dry_r) = (sample_l, sample_r);

            // --- A. Track Dry Signal Power for Loudness Compensation ---
            block_avg_input += (dry_l * dry_l + dry_r * dry_r) * 0.5;

            // --- B. Process one sample through the chain ---
            // This is the main DSP function, handling multiband splitting, dynamics, and saturation.
            let (mut wet_l, mut wet_r, sample_gr_db) =
                self.process_sample(sample_l, sample_r, amount, tilt);
            total_gr_db += sample_gr_db;

            // --- C. Serial Compensation EQ Stage ---
            // After the parallel band processing, the summed wet signal is passed through
            // the series of dynamic EQs.
            for i in 0..MAX_BANDS {
                (wet_l, wet_r) = self.bands[i].compensation_eq.process(wet_l, wet_r);
            }

            // --- D. Final Loudness Compensation ---
            wet_l *= loudness_correction;
            wet_r *= loudness_correction;

            // --- E. Track Wet Signal Power for Loudness Compensation ---
            let wet_power = (wet_l * wet_l + wet_r * wet_r) * 0.5;
            block_avg_output += wet_power;

            // --- F. Constant Power Dry/Wet Mix and Output Gain ---
            *l = (dry_l * dry_gain) + (wet_l * wet_gain);
            *r = (dry_r * dry_gain) + (wet_r * wet_gain);

            // Apply Master Output Gain
            *l *= output_gain;
            *r *= output_gain;
        }

        // --- 4. Post-Block RMS Update ---
        // After processing the entire block, we update the RMS trackers. These values will be
        // used in the *next* block's loudness correction calculation.
        let block_size = buffer.samples() as f32;
        if block_size > 0.0 {
            let avg_input_power = block_avg_input / block_size;
            let avg_output_power = block_avg_output / block_size;
            self.dry_rms_tracker = avg_input_power;
            self.wet_rms_tracker = avg_output_power;

            // Update the GR meter parameter for the GUI to read.
            let avg_gr_db = total_gr_db / block_size;
            self.gr_meter_smoother
                .set_target(self.sample_rate, avg_gr_db);

            // If the GUI is open, update the shared atomic value for the meter.
            #[cfg(feature = "vizia")]
            if self.params.editor_state.is_open() {
                self.gain_reduction_meter
                    .store(self.gr_meter_smoother.next(), Ordering::Relaxed);
            }
        }

        ProcessStatus::Normal
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        #[cfg(feature = "vizia")]
        editor::create(
            self.params.clone(),
            self.gain_reduction_meter.clone(),
            self.params.editor_state.clone(),
        )
    }
}

impl Vst3Plugin for ColorFall {
    const VST3_CLASS_ID: [u8; 16] = *b"ColorfallShpshft";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Dynamics,
        Vst3SubCategory::Distortion,
    ];
}

nih_export_vst3!(ColorFall);
