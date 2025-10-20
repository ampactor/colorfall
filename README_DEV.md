# ColorFall VST3 Plugin

A creative 5-band "soundgoodizer" and tonal shaping tool.

This document serves as a comprehensive guide for developers and AI assistants to understand, build, and contribute to the ColorFall project.

---

## 1. Project Overview

ColorFall is a multiband dynamics processor designed for creative tonal shaping. It splits the audio into five frequency bands and applies parallel compression and saturation, followed by a serial dynamic EQ. The core concept is that two simple knobs, `Amount` and `Tilt`, dynamically control dozens of underlying DSP parameters to create complex, evolving sounds.

### 1.1. Core Controls

-   **Amount**: The primary macro control. As `Amount` increases, the compression becomes more aggressive, the saturation drive increases, and the compensatory EQ becomes more resonant.
-   **Tilt**: A frequency-biasing control. Negative values focus the processing (compression and EQ) on the lower frequency bands, while positive values focus on the higher bands.
-   **Mix**: A constant-power dry/wet control for blending the processed signal with the original.
-   **Output**: A final output gain stage for level trimming.

### 1.2. Architecture

The plugin's architecture is a hybrid parallel/serial design, executed on a per-sample basis:

1.  **Parallel Dynamics (5-Band):**
    -   The incoming stereo signal is split into 5 frequency bands using a cascade of 4th-order Linkwitz-Riley crossover filters.
    -   Each band is processed independently and in parallel:
        -   An **Envelope Follower** detects the band's level.
        -   A **Gain Computer** calculates the required gain reduction based on the envelope and the dynamic `Amount` and `Tilt` parameters.
        -   A **Saturator** adds harmonic content, with its drive linked to the `Amount` parameter.
    -   The 5 processed bands are summed back together into a single "wet" signal.

2.  **Serial EQ (5-Band):**
    -   The summed wet signal is then passed through a series of 5 cascading peaking EQ filters.
    -   The gain, Q-factor, and center frequency of these EQs are also dynamically controlled by `Amount` and `Tilt`, creating a resonant character that compensates for and accentuates the dynamics processing.

3.  **Loudness Compensation & Mixing:**
    -   The RMS level of the final wet signal is compared to the RMS of the original dry signal from the *previous* processing block.
    -   A smoothed correction factor is applied to the wet signal to maintain a consistent perceived loudness.
    -   The loudness-compensated wet signal is mixed with the dry signal using a constant-power crossfade determined by the `Mix` parameter.
    -   The final `Output` gain is applied.

---

## 2. Codebase Structure

The project is organized into several key files:

-   `src/lib.rs`: The main plugin entry point. It defines the `ColorFall` and `ColorFallParams` structs, handles parameter smoothing and management, and contains the main `process()` loop that orchestrates the DSP.
-   `src/dsp.rs`: Contains all core, stateless DSP algorithms. This includes the `Biquad` filter implementation, crossover logic, envelope detection, gain computation, and the saturation function. Keeping this code separate allows for easier testing and modification of the DSP without affecting the plugin's state management.
-   `src/editor.rs`: Defines the Vizia-based GUI. It creates the UI layout, binds the `ParamSlider` widgets to the parameters in `ColorFallParams`, and handles the real-time display of the gain reduction meter.
-   `src/style.css`: The stylesheet for the Vizia GUI, defining the look and feel of the plugin window, knobs, and labels.
-   `Cargo.toml`: The Rust project manifest, defining dependencies, features, and metadata.
-   `xtask/`: Contains the `cargo xtask` build commands for bundling the plugin for different platforms.

---

## 3. Development & Building

### 3.1. Prerequisites

-   **Rust Toolchain**: Ensure you have the latest stable Rust toolchain installed via `rustup`.
-   **System Dependencies**: For building the GUI, you will need system libraries for `xcb`.
    -   **Debian/Ubuntu**: `sudo apt-get install -y libxcb-xfixes0-dev libx11-xcb-dev libxcb-icccm4-dev libxcb-dri2-0-dev`
    -   **Fedora**: `sudo dnf install -y libxcb-devel xorg-x11-server-devel`
    -   **Arch**: `sudo pacman -Syu --needed libxcb libx11`

### 3.2. Building the Plugin

The project uses `cargo xtask` for streamlined build and bundling operations.

**To build and bundle for release:**

```bash
cargo xtask bundle colorfall --release
```

This command will compile the plugin in release mode and create the appropriate VST3 bundle in the `target/nih_plug_out` directory.

**To run in debug mode (e.g., with a DAW):**

```bash
cargo xtask build colorfall
```

You can then load the debug version of the plugin from `target/debug/` into your DAW.

---

## 4. DSP Concepts & Tuning Guide

The "sound" of ColorFall comes from the interaction of its dynamic components. When tuning, focus on the functions in `dsp.rs` and the `update_dynamic_parameters` function in `lib.rs`.

-   **`calculate_target_gr()` (`dsp.rs`):** This is the brain of the compressor. The `threshold_db`, `ratio`, and `knee_db` are all calculated based on `intensity`. Modifying these formulas will change the fundamental character of the compression. For example, making the `ratio` increase more slowly will result in a softer sound.

-   **`saturate()` (`dsp.rs`):** This function implements a cubic waveshaper. The `drive` term controls how hard the signal is pushed, and the final `clamp()` and multiplication control the output clipping and overall wetness of the saturation. Experimenting with different polynomial terms (e.g., adding a `sample.powf(5.0)`) can introduce different harmonic flavors.

-   **`update_dynamic_parameters()` (`lib.rs`):** This function controls the "color" part of the plugin—the serial EQs. The `q_base` and `compensation_gain_db` are key variables. Increasing the `q_base` scaling will make the plugin more resonant and "ringy" at high `Amount` settings.

-   **`BASE_CROSSOVER_FREQS` (`lib.rs`):** These constants define the fundamental frequency splits. Adjusting these values will change which parts of the spectrum are processed by which band, significantly altering the overall tonal balance of the effect.
