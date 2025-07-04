use std::sync::Mutex;

use gst::glib::object::Cast;
use gst::glib::types::StaticType;
use gst::glib::{self};
use gst::prelude::{ElementExtManual, GstObjectExt, PadExt};
use gst::subclass::prelude::*;
use gst::{Caps, Event, FlowError};
use gst_base::AggregatorPad;
use gst_base::prelude::{AggregatorExt, AggregatorExtManual, AggregatorPadExt};
use gst_base::subclass::prelude::{AggregatorImpl, AggregatorImplExt, AggregatorPadImpl};
use once_cell::sync::Lazy;

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "simpleagg",
        gst::DebugColorFlags::empty(),
        Some("GL simpleagg video filter"),
    )
});

#[derive(Default)]
pub(crate) struct SimpleAggPad {}

impl SimpleAggPad {}

#[glib::object_subclass]
impl ObjectSubclass for SimpleAggPad {
    const NAME: &'static str = "SimpleAggPad";
    type Type = super::SimpleAggPad;
    type ParentType = gst_base::AggregatorPad;
}

impl ObjectImpl for SimpleAggPad {}

impl GstObjectImpl for SimpleAggPad {}

impl PadImpl for SimpleAggPad {}

impl AggregatorPadImpl for SimpleAggPad {}

#[derive(Default)]
pub(crate) struct SimpleAggState {
    video_info: Option<gst_video::VideoInfo>,
}

#[derive(Default)]
pub(crate) struct SimpleAgg {
    state: Mutex<SimpleAggState>,
}

#[glib::object_subclass]
impl ObjectSubclass for SimpleAgg {
    const NAME: &'static str = "SimpleAgg";
    type Type = super::SimpleAgg;
    type ParentType = gst_base::Aggregator;

    fn with_class(_klass: &Self::Class) -> Self {
        Self {
            state: Mutex::new(SimpleAggState::default()),
        }
    }
}

impl ObjectImpl for SimpleAgg {}

impl GstObjectImpl for SimpleAgg {}

impl ElementImpl for SimpleAgg {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                "Simple aggregator",
                "Aggregator",
                "Simple video aggregator",
                "Genius Sports",
            )
        });

        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let caps = Caps::builder("video/x-raw")
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

            vec![
                gst::PadTemplate::new(
                    "src",
                    gst::PadDirection::Src,
                    gst::PadPresence::Always,
                    &caps,
                )
                .unwrap(),
                gst::PadTemplate::with_gtype(
                    "sink",
                    gst::PadDirection::Sink,
                    gst::PadPresence::Request,
                    &caps,
                    super::SimpleAggPad::static_type(),
                )
                .unwrap(),
            ]
        });

        PAD_TEMPLATES.as_ref()
    }
}

impl AggregatorImpl for SimpleAgg {
    fn sink_event(&self, aggregator_pad: &AggregatorPad, event: Event) -> bool {
        if let gst::EventView::Caps(caps) = event.view() {
            if let Ok(video_info) = gst_video::VideoInfo::from_caps(caps.caps()) {
                if self.set_video_info(&video_info).is_err() {
                    gst::error!(CAT, imp: self, "Error initializing muxer/demuxer");
                }
                self.fixate_src_caps(caps.caps().to_owned());
            }
        }
        self.parent_sink_event(aggregator_pad, event)
    }

    fn update_src_caps(&self, caps: &Caps) -> Result<Caps, FlowError> {
        gst::warning!(CAT, imp: self, "Update src caps: {caps}");
        let state = self.state.lock().unwrap();
        if let Some(ref video_info) = state.video_info {
            gst::warning!(
                CAT,
                imp: self,
                "Update src caps, video_info {video_info:?}"
            );

            let video_caps = video_info.to_caps().map_err(|_| FlowError::NotNegotiated)?;
            self.obj().set_src_caps(&video_caps);

            gst::warning!(CAT, imp: self, "Update src caps, video_caps {video_caps}");
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
        if let Some(src_caps) = self.obj().src_pad().current_caps() {
            gst::warning!(CAT, imp: self, "Output caps on src pad: {:?}", src_caps);
        } else {
            gst::warning!(CAT, imp: self, "No current caps on src pad");
        }

        for sink in self
            .obj()
            .sink_pads()
            .into_iter()
            .map(|pad| pad.downcast::<super::SimpleAggPad>().unwrap())
        {
            if let Some(sink_caps) = sink.current_caps() {
                gst::warning!(CAT, imp: self, "Caps on sink pad {}: {:?}", sink.name(), sink_caps);
            } else {
                gst::warning!(CAT, imp: self, "No current caps on sink pad {}", sink.name());
            }

            if let Some(b) = sink.pop_buffer() {
                gst::info!(CAT, imp: self, "buffer popped");

                if b.pts().is_none() {
                    gst::error!(CAT, imp: self, "input buffers must have PTS, got None");
                    return Err(gst::FlowError::Error);
                }

                return self.finish_buffer(b);
            }
        }

        Ok(gst::FlowSuccess::Ok)
    }
}

impl SimpleAgg {
    fn set_video_info(&self, video_info: &gst_video::VideoInfo) -> Result<(), ()> {
        gst::info!(CAT, imp: self, "Setting video info: {:?}", video_info);
        let mut state = self.state.lock().unwrap();
        state.video_info = Some(video_info.clone());
        Ok(())
    }
}
