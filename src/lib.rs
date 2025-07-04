use gst::glib;

mod dynamic_agg;
mod rgb2gray;
mod simple_agg;

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    rgb2gray::register(plugin)?;
    dynamic_agg::register(plugin)?;
    simple_agg::register(plugin)?;

    Ok(())
}

gst::plugin_define!(
    rstutorial,                                                 // plugin name
    env!("CARGO_PKG_DESCRIPTION"),                              // short description
    plugin_init,                                                // entry point function
    concat!(env!("CARGO_PKG_VERSION"), "-", env!("COMMIT_ID")), // version number
    "MIT/x11",                                                  // license
    env!("CARGO_PKG_NAME"),                                     // source package name
    env!("CARGO_PKG_NAME"),                                     // binary package name
    env!("CARGO_PKG_REPOSITORY"),                               // origin of the plugin
    env!("BUILD_REL_DATE")                                      // release date version
);
