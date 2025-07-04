use gst::glib;
use gst::prelude::*;
mod imp;

glib::wrapper! {
    pub(crate) struct DynamicAggPad(ObjectSubclass<imp::DynamicAggPad>) @extends gst_base::AggregatorPad, gst::Pad, gst::Object;
}

glib::wrapper! {
    pub(crate) struct DynamicAgg(ObjectSubclass<imp::DynamicAgg>) @extends gst_base::Aggregator, gst::Element, gst::Object;
}

unsafe impl Send for DynamicAgg {}
unsafe impl Sync for DynamicAgg {}

pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "dynamic_agg",
        gst::Rank::NONE,
        DynamicAgg::static_type(),
    )
}
