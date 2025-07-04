use gst::Buffer;
use gst::Caps;
use gst::Event;
use gst::FlowError;
use gst::glib;
use gst::prelude::*;
use gst::subclass::prelude::*;
use gst_base::AggregatorPad;
use gst_base::subclass::prelude::*;
use gst_video::prelude::*;
use once_cell::sync::Lazy;
use std::collections::VecDeque;
use std::sync::Mutex;

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "dynamicagg",
        gst::DebugColorFlags::empty(),
        Some("Dynamic Aggregator Element"),
    )
});

#[derive(Default)]
pub(crate) struct DynamicAggPad {}

#[glib::object_subclass]
impl ObjectSubclass for DynamicAggPad {
    const NAME: &'static str = "DynamicAggPad";
    type Type = super::DynamicAggPad;
    type ParentType = gst_base::AggregatorPad;
}

impl ObjectImpl for DynamicAggPad {}

impl GstObjectImpl for DynamicAggPad {}

impl PadImpl for DynamicAggPad {}

impl AggregatorPadImpl for DynamicAggPad {}

#[derive(Default)]
pub(crate) struct DynamicAggState {
    video_info: Option<gst_video::VideoInfo>,
    text_caps: Option<gst::Caps>,
    text_accumulator: VecDeque<Buffer>,
    text_formats: Vec<String>,
}

#[derive(Default)]
pub(crate) struct DynamicAgg {
    state: Mutex<DynamicAggState>,
}

#[glib::object_subclass]
impl ObjectSubclass for DynamicAgg {
    const NAME: &'static str = "DynamicAgg";
    type Type = super::DynamicAgg;
    type ParentType = gst_base::Aggregator;

    fn with_class(_klass: &Self::Class) -> Self {
        Self {
            state: Mutex::new(DynamicAggState::default()),
        }
    }
}

impl ObjectImpl for DynamicAgg {}

impl GstObjectImpl for DynamicAgg {}

impl ElementImpl for DynamicAgg {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                "Dynamic Aggregator",
                "Aggregator",
                "Aggregates video and text inputs",
                "Genius Sports",
            )
        });
        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let video_caps = gst::Caps::builder("video/x-raw")
                .field(
                    "format",
                    gst::List::new(["I420", "RGB", "RGBA", "BGR", "BGRA"]),
                )
                .field("width", gst::IntRange::new(0, i32::MAX))
                .field("height", gst::IntRange::new(0, i32::MAX))
                .field(
                    "framerate",
                    gst::FractionRange::new(
                        gst::Fraction::new(0, 1),
                        gst::Fraction::new(i32::MAX, 1),
                    ),
                )
                .build();

            let text_caps = gst::Caps::builder("text/x-raw")
                .field("format", gst::List::new(["utf8"]))
                .build();

            let video_sink_pad_template = gst::PadTemplate::with_gtype(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &video_caps,
                super::DynamicAggPad::static_type(),
            )
            .unwrap();

            let text_sink_pad_template = gst::PadTemplate::with_gtype(
                "comm_%u",
                gst::PadDirection::Sink,
                gst::PadPresence::Request,
                &text_caps,
                super::DynamicAggPad::static_type(),
            )
            .unwrap();

            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &video_caps,
            )
            .unwrap();

            vec![
                video_sink_pad_template,
                text_sink_pad_template,
                src_pad_template,
            ]
        });
        PAD_TEMPLATES.as_ref()
    }
}

impl AggregatorImpl for DynamicAgg {
    fn create_new_pad(
        &self,
        templ: &gst::PadTemplate,
        name: Option<&str>,
        caps: Option<&gst::Caps>,
    ) -> Option<AggregatorPad> {
        let pad = self.parent_create_new_pad(templ, name, caps);
        if let Some(ref pad) = pad {
            gst::debug!(CAT, "Created pad: {}", pad.name());
        }

        pad
    }

    fn sink_event(&self, aggregator_pad: &AggregatorPad, event: Event) -> bool {
        if let gst::EventView::Caps(caps) = event.view() {
            let pad_name = aggregator_pad.name();
            gst::info!(CAT, imp: self, "Caps event on pad {}: {:?}", pad_name, caps.caps());

            let caps_name = caps.caps().structure(0).unwrap().name().as_str();

            if caps_name == "video/x-raw" {
                if let Ok(video_info) = gst_video::VideoInfo::from_caps(caps.caps()) {
                    let mut state = self.state.lock().unwrap();
                    state.video_info = Some(video_info);
                    self.fixate_src_caps(caps.caps().to_owned());
                }
            } else if caps_name == "text/x-raw" {
                gst::info!(CAT, imp: self, "\nReceived text/x-raw caps on pad {}: {:?}", pad_name, caps.caps());
                let mut state = self.state.lock().unwrap();
                state.text_caps = Some(caps.caps().to_owned());
                gst::info!(CAT, imp: self, "Stored text caps: {:?}", caps);
                if let Some(structure) = caps.caps().structure(0) {
                    if let Ok(format) = structure.get::<&str>("format") {
                        gst::debug!(CAT, imp: self, "\nText format: {}", format);
                        state.text_formats.push(format.to_string());
                    }
                }
            }
        }
        self.parent_sink_event(aggregator_pad, event)
    }

    fn update_src_caps(&self, caps: &Caps) -> Result<Caps, FlowError> {
        gst::debug!(CAT, imp: self, "Update src caps: {caps}");
        let state = self.state.lock().unwrap();
        if let Some(ref video_info) = state.video_info {
            gst::debug!(
                CAT,
                imp: self,
                "Update src caps, video_info {video_info:?}"
            );

            let video_caps = video_info.to_caps().map_err(|_| FlowError::NotNegotiated)?;
            self.obj().set_src_caps(&video_caps);

            gst::debug!(CAT, imp: self, "Update src caps, video_caps {video_caps}");
            Ok(video_caps)
        } else {
            gst::error!(
                CAT,
                imp: self,
                "No video info available to update src caps"
            );
            Err(FlowError::NotNegotiated)
        }
    }

    fn aggregate(&self, _timeout: bool) -> Result<gst::FlowSuccess, gst::FlowError> {
        let mut texts: Vec<(String, String)> = Vec::new();
        let mut video_buffer: Option<Buffer> = None;

        if let Some(src_caps) = self.obj().src_pad().current_caps() {
            gst::debug!(CAT, imp: self, "Output caps on src pad: {:?}", src_caps);
        } else {
            gst::debug!(CAT, imp: self, "No current caps on src pad");
        }

        for sink in self
            .obj()
            .sink_pads()
            .into_iter()
            .map(|p| p.dynamic_cast::<AggregatorPad>().unwrap())
        {
            let pad_name = sink.name();
            gst::debug!(CAT, imp: self, "Pad name: {}", pad_name);

            if sink.is_eos() {
                if pad_name == "sink" {
                    gst::info!(CAT, imp: self, "Received EOS on video sink pad");
                }
                continue;
            }

            if pad_name == "sink" {
                if let Some(b) = sink.pop_buffer() {
                    gst::info!(CAT, imp: self, "Video buffer received");
                    video_buffer = Some(b);
                }
            }
            if pad_name.starts_with("comm_") {
                while let Some(b) = sink.pop_buffer() {
                    gst::info!(CAT, imp: self, "Text buffer received on pad {}", pad_name);
                    self.process_text_buffer(b, pad_name.as_str(), &mut texts)?;
                }
            } else {
                gst::warning!(CAT, imp: self, "Unknown pad type: {}", pad_name);
            }
        }

        if let Some(video_buffer) = video_buffer {
            gst::info!(CAT, imp: self, "\nFinishing video buffer");
            return self.finish_buffer(video_buffer);
        } else {
            gst::warning!(CAT, imp: self, "No video buffer received");
            self.push_gap_event();
        }
        Ok(gst::FlowSuccess::Ok)
    }
}

impl DynamicAgg {
    fn process_text_buffer(
        &self,
        buffer: Buffer,
        pad_name: &str,
        texts: &mut Vec<(String, String)>,
    ) -> Result<(), FlowError> {
        let mut state = self.state.lock().unwrap();
        state.text_accumulator.push_back(buffer.clone());
        let map = buffer.map_readable().map_err(|_| gst::FlowError::Error)?;
        let text = String::from_utf8(map.to_vec()).map_err(|_| gst::FlowError::Error)?;
        state.text_formats.push(pad_name.to_string());
        texts.push((pad_name.to_string(), text.to_string()));
        gst::debug!(CAT, imp: self, "\nProcessed text from {}: {}", pad_name, text);

        Ok(())
    }

    fn push_gap_event(&self) {
        if let Some(src_pad) = self.obj().static_pad("src") {
            let gap_event = gst::event::Gap::builder(gst::ClockTime::from_nseconds(0)).build();
            let _ = src_pad.push_event(gap_event);
        }
    }
}
