use gst::glib;
use gst::prelude::*;
mod imp;

glib::wrapper! {
    pub(crate) struct SimpleAggPad(ObjectSubclass<imp::SimpleAggPad>) @extends gst_base::AggregatorPad, gst::Pad, gst::Object;
}

glib::wrapper! {
    pub(crate) struct SimpleAgg(ObjectSubclass<imp::SimpleAgg>) @extends gst_base::Aggregator, gst::Element, gst::Object;
}

unsafe impl Send for SimpleAgg {}
unsafe impl Sync for SimpleAgg {}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "simple_agg",
        gst::Rank::NONE,
        SimpleAgg::static_type(),
    )
}
