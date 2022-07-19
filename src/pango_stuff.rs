use pangocairo::pango;

use crate::pango_utils::AttrScale;

pub fn get_pango_layout(cairo: &cairo::Context, font: &str, scale: f64, text: &str) -> pango::Layout {
    let layout = pangocairo::create_layout(cairo).unwrap();
    let attrs = pango::AttrList::new();

    layout.set_text(text);
    attrs.insert(pango::Attribute::new_scale(scale));

    let font_description = pango::FontDescription::from_string(font);
    layout.set_font_description(Some(&font_description));
    layout.set_single_paragraph_mode(true);
    layout.set_attributes(Some(&attrs));

    drop(attrs); // ? pang_attr_list_unref
    drop(font_description); // ? pango_font_decription_unref
    return layout;

}
/**
 * Use format!() for the text
 * Returns width, height, baseline
 */
pub fn get_text_size(cairo: &cairo::Context, font: &str, scale: f64, text: &str) -> (i32, i32, i32) {
    let layout = get_pango_layout(cairo, font, scale, text);
    pangocairo::update_layout(cairo, &layout);

    let (width, height) = layout.pixel_size();
    let baseline = layout.baseline() / pango::SCALE;

    drop(layout); // ? g_object_unref

    return (width, height, baseline)
}


/**
 * Use format!() for the text
*/
pub fn pango_print(cairo: &cairo::Context, font: &str, scale: f64, text: &str) {
    let layout = get_pango_layout(cairo, font, scale, text);
    let font_options = cairo.font_options().unwrap();
    pangocairo::context_set_font_options(&layout.context().unwrap(), Some(&font_options));
    drop(font_options);

    pangocairo::update_layout(cairo, &layout);
    pangocairo::show_layout(cairo, &layout);
    drop(layout);
}
