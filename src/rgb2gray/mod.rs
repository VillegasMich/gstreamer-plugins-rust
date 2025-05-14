use gst::{glib, prelude::StaticType};

mod imp;

glib::wrapper! {
    pub struct Rgb2Gray(ObjectSubclass<imp::Rgb2Gray>) @extends gst_base::BaseTransform, gst::Element, gst::Object, gst_video::VideoFilter;
}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "rsrgb2gray",
        gst::Rank::NONE,
        Rgb2Gray::static_type(),
    )
}
