# ColorFall: Production Roadmap

This document outlines the remaining tasks to get the ColorFall plugin ready for a production release, as well as potential features for future versions. (we will always use Rust 2024 edition.)

## Project Status

ColorFall is a creative 5-band dynamics processor and tonal shaping tool. The core DSP engine and GUI are fully implemented, documented, and robust. The project is now in the final tuning and testing phase before its v1.0 release.

### Core Features (✅ Implemented)

- **Dynamic Multiband Processing**: 5-band crossover split with `saturate -> compress` signal flow and independent (dual-mono) L/R dynamics.
- **Reactive Serial EQ**: 5-band serial compensation EQ that is fully synchronized with the compressor bands and reacts in real-time to the applied gain reduction.
- **Dynamic Parameters**: All internal DSP parameters (compression, saturation, EQ) are dynamically linked to the `Amount` and `Tilt` knobs.
- **Loudness Compensation**: RMS-based automatic gain matching between the dry and wet signals is implemented.
- **Functional GUI**: A Vizia-based user interface with sliders for all main parameters and a gain reduction meter is fully implemented and styled.
- **Robust Codebase**: Well-organized and modular code structure (`lib.rs`, `dsp.rs`, `editor.rs`).

---

## 🚀 Pre-Release Checklist (v1.0)

These are the essential tasks to complete before the first official release.

### 🎛️ 1. DSP Fine-Tuning & Manual Testing

This is the most critical and creative step. The goal is to manually test and tweak the plugin's sound until it feels just right.

- [ ] **Tweak `dsp.rs` algorithms**:
  - Adjust the curves and ranges in `calculate_target_gr()` to perfect the compression character.
  - Fine-tune the time constants in `calculate_dynamic_time_constants()`.
  - Adjust the saturation algorithm in `saturate()` for the desired harmonic character.
- [ ] **Tweak `lib.rs` dynamic parameters**:
  - Adjust the Q-factor and gain ranges in `update_dynamic_parameters()` for the serial EQs.
  - Experiment with the `BASE_CROSSOVER_FREQS` to find the most musical split points.
- [ ] **DAW Compatibility Testing**:
  - Test the plugin extensively in various DAWs (e.g., REAPER, Ableton Live, Bitwig, FL Studio) on all target operating systems.
  - Check for any visual glitches, crashes, or unexpected audio behavior.

### 📝 2. Finalize Metadata & Documentation

- [ ] **Update Plugin Metadata**: Fill in the placeholder `URL` and `EMAIL` constants in `lib.rs`.
- [ ] **Write User Documentation**: Create a simple `README.md` for end-users explaining what the plugin does and how to use the controls.

### 📦 3. Build & Package for Release

- [ ] **Create Release Builds**: Use the `cargo xtask bundle colorfall --release` command to create optimized plugin bundles.
- [ ] **Package for Distribution**: Create ZIP archives for each operating system (Windows, macOS, Linux) containing the plugin files (`.vst3`) and the user documentation.

---

## 🔮 Future Development (Post-v1.0)

These are ideas for future updates after the initial release.
- [ ] **Upgrade to LUFS Loudness Compensation**:
  - **Priority**: High.
  - **Task**: Replace the current RMS-based loudness compensation with a proper ITU-R BS.1770-compliant LUFS measurement. This will provide more perceptually accurate gain matching.

- [ ] **Implement Oversampling**:
  - **Priority**: High. This is the most important next step for audio quality.
  - **Task**: Integrate a resampling library (like `rubato`) to run the saturation and compression stages at 2x or 4x the host sample rate to reduce aliasing. This will be a significant architectural change.
- [ ] **Enhance GUI**:
  - Add a real-time spectrum analyzer to visualize the tonal changes.
  - Provide visual feedback for the dynamic EQ curves.
- [ ] **Expand DSP Options**:
  - Add a parameter to select between different saturation algorithms (e.g., tape, tube).
  - Implement a preset system for saving and loading settings.