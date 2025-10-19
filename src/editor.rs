use nih_plug::prelude::{AtomicF32, Editor};
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use nih_plug_vizia::vizia::prelude::*;

use crate::ColorFallParams;

#[derive(Lens)]
struct Data {
    params: Arc<ColorFallParams>,
    gain_reduction: Arc<AtomicF32>,
}

impl Model for Data {}

pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (500, 350))
}

pub(crate) fn create(
    params: Arc<ColorFallParams>,
    gain_reduction: Arc<AtomicF32>,
    editor_state: Arc<ViziaState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, _| {
        Data {
            params: params.clone(),
            gain_reduction: gain_reduction.clone(),
        }
        .build(cx);

        // Custom styling for the GUI
        cx.add_stylesheet(include_style!("src/style.css"))
            .expect("Failed to load stylesheet");
        
        // Register the custom fonts from the assets module.
        assets::register_noto_sans_light(cx);
        assets::register_noto_sans_bold(cx);

        VStack::new(cx, |cx| {
            // Header
            Label::new(cx, "ColorFall")
                .font_size(30.0)
                .height(Pixels(50.0))
                // We'll use a class to apply the bold font from the stylesheet
                .class("title")
                .child_top(Stretch(1.0))
                .child_bottom(Pixels(0.0));

            // Main controls and meter
            HStack::new(cx, |cx| {
                // Left-side knobs
                VStack::new(cx, |cx| {
                    // Amount Knob
                    VStack::new(cx, |cx| {
                        Label::new(cx, "Amount").bottom(Pixels(2.0));
                        ParamSlider::new(cx, Data::params, |p| &p.amount)
                            .width(Pixels(75.0))
                            .class("amount");
                        Label::new(cx, Data::params.map(|p| p.amount.to_string()))
                            .top(Pixels(2.0))
                            .class("value-label");
                    })
                    .row_between(Pixels(2.0))
                    .height(Auto);

                    // Tilt Knob
                    VStack::new(cx, |cx| {
                        Label::new(cx, "Tilt").bottom(Pixels(2.0));
                        ParamSlider::new(cx, Data::params, |p| &p.tilt)
                            .width(Pixels(75.0))
                            .class("tilt");
                        Label::new(cx, Data::params.map(|p| p.tilt.to_string()))
                            .top(Pixels(2.0))
                            .class("value-label");
                    })
                    .row_between(Pixels(2.0))
                    .height(Auto);
                })
                .row_between(Pixels(15.0))
                .child_left(Stretch(1.0))
                .child_right(Stretch(1.0));

                // Gain Reduction Meter
                VStack::new(cx, |cx| {
                    Label::new(cx, "GR").bottom(Pixels(2.0));
                    PeakMeter::new(
                        cx,
                        Data::gain_reduction.map(|gr| gr.load(Ordering::Relaxed)),
                        Some(Duration::from_millis(600)),
                    )
                    //.gradient() // Gradient is handled by CSS now
                        .width(Pixels(20.0));
                })
                .height(Stretch(1.0))
                .child_left(Stretch(1.0))
                .child_right(Stretch(1.0));

                // Right-side knobs
                VStack::new(cx, |cx| {
                    // Mix Knob
                    VStack::new(cx, |cx| {
                        Label::new(cx, "Mix").bottom(Pixels(2.0));
                        ParamSlider::new(cx, Data::params, |p| &p.mix)
                            .width(Pixels(75.0))
                            .class("mix");
                        Label::new(cx, Data::params.map(|p| p.mix.to_string()))
                            .top(Pixels(2.0))
                            .class("value-label");
                    })
                    .row_between(Pixels(2.0))
                    .height(Auto);

                    // Output Knob
                    VStack::new(cx, |cx| {
                        Label::new(cx, "Output").bottom(Pixels(2.0));
                        ParamSlider::new(cx, Data::params, |p| &p.output)
                            .width(Pixels(75.0))
                            .class("output");
                        Label::new(cx, Data::params.map(|p| p.output.to_string()))
                            .top(Pixels(2.0))
                            .class("value-label");
                    })
                    .row_between(Pixels(2.0))
                    .height(Auto);
                })
                .row_between(Pixels(15.0))
                .child_left(Stretch(1.0))
                .child_right(Stretch(1.0));
            })
            .col_between(Pixels(20.0));
        })
        .row_between(Pixels(10.0))
        .child_left(Stretch(1.0))
        .child_right(Stretch(1.0));
    })
}