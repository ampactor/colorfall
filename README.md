# ColorFall

A creative 5-band "soundgoodizer" and tonal shaping tool for VST3 compatible hosts.

## What is it?

ColorFall is a multiband dynamics processor designed for creative tonal shaping. It splits your audio into five frequency bands and applies parallel compression and saturation, followed by a serial dynamic EQ. The core concept is that two simple knobs, `Amount` and `Tilt`, dynamically control dozens of underlying DSP parameters to create complex, evolving sounds.

It's designed to be a simple tool for adding character, from subtle warmth and cohesion to aggressive, colorful distortion.

## Controls

-   **Amount**: The primary macro control. As `Amount` increases, the compression becomes more aggressive, the saturation drive increases, and the compensatory EQ becomes more resonant.
-   **Tilt**: A frequency-biasing control. Negative values focus the processing (compression and EQ) on the lower frequency bands, while positive values focus on the higher bands.
-   **Mix**: A constant-power dry/wet control for blending the processed signal with the original.
-   **Output**: A final output gain stage for level trimming.
-   **GR Meter**: Shows the amount of gain reduction being applied across all bands.

## Installation

1.  Download the latest release for your operating system from the releases page.
2.  Unzip the archive.
3.  Copy the `ColorFall.vst3` file to your VST3 plugins folder:
    -   **Windows**: `C:\Program Files\Common Files\VST3`
    -   **macOS**: `/Library/Audio/Plug-Ins/VST3`
    -   **Linux**: `~/.vst3/`
4.  Rescan for plugins in your DAW.

