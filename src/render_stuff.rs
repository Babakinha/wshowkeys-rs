use crate::{Wsk, cairo_utils::ToSubpixelOrder, cairo_utils::SetSourceU32};

/* Rendering stuff (with cairo) */

pub fn render_frame(wsk: &mut Wsk) {
    let recorder = cairo::RecordingSurface::create(cairo::Content::ColorAlpha, None).unwrap();
    let cairo = cairo::Context::new(&recorder).unwrap();
    // ? Should we make this user-definied
    cairo.set_antialias(cairo::Antialias::Best);

    let mut font_options = cairo::FontOptions::new().unwrap();
    font_options.set_hint_style(cairo::HintStyle::Full);
    font_options.set_antialias(cairo::Antialias::Subpixel);

    if wsk.wl_subpixel.is_some() {
        font_options.set_subpixel_order(wsk.wl_subpixel.as_ref().unwrap().to_cairo_subpixel_order());
    }
    cairo.set_font_options(&font_options);
    drop(font_options);
    cairo.save().unwrap();
    cairo.set_operator(cairo::Operator::Clear);
    cairo.paint().unwrap();
    cairo.restore().unwrap();

    let scale = if wsk.scale == 0 { 1 } else { wsk.scale };
    let width: u32 = 0;
    let height: u32 = 0;
    render_to_cairo(wsk, &cairo, scale, width, height)
}

fn render_to_cairo(wsk: &mut Wsk, cairo: &cairo::Context, scale: i32, width: u32, height: u32) {
    cairo.set_operator(cairo::Operator::Source);
    cairo.set_source_u32(wsk.background);
    cairo.paint().unwrap();

    // ? I dont know how, or if i should use iterators
    for key in &wsk.keys {
        let mut special = false;
        let mut name = &key.utf8;

        //Shoult we use Option<String>?
        if name == "" {
            special = true;
            cairo.set_source_u32(wsk.specialfg);
            name = &key.name;
        } else {
            cairo.set_source_u32(wsk.foreground);
        }

        cairo.move_to(width.into(), 0.0);

        if special {
            //TODO: Pango
        }

    }

    wsk.keys = vec![];

}