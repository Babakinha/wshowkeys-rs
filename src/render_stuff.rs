use std::{fs::File, os::unix::prelude::AsRawFd, io::BufWriter, ptr::null_mut};

use crate::{Wsk, cairo_utils::ToSubpixelOrder, cairo_utils::SetSourceU32, pango_stuff::{get_text_size, pango_print}};
use wayland_client::{protocol::wl_shm, QueueHandle};

/* Rendering stuff (with cairo) */
pub fn render_frame(wsk: &mut Wsk, tmp: File) {
    dbg!("Frame");
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

    let scale = if wsk.scale == 0.0 { 1.0 } else { wsk.scale };
    let mut width: u32 = 0;
    let mut height: u32 = 0;
    (width, height) = render_to_cairo(wsk, &cairo, scale, width, height);

    if height / scale as u32 != wsk.height ||
        width / scale as u32 != wsk.width  ||
        wsk.width == 0
    {
        //Reconfigure surface
        if width == 0 || height == 0 {
            wsk.wl_surface.as_ref().unwrap().attach(None, 0, 0);
        } else {
            wsk.wl_layer_surface.as_ref().unwrap().set_size(width / scale as u32, height / scale as u32);
        }

        //TODO: This could be a infinite loop if the compositor set us a diferrent height than we want to
        wsk.wl_surface.as_ref().unwrap().commit();
    } else if height > 0 {
        //Replay recording into shm and send it off
        //wsk.current_buffer = Some(get_next_buffer(wsk, wsk.width, wsk.height));
        if wsk.wl_buffer.is_none() {
            drop(recorder);
            drop(cairo);
            return;
        }

        //let shm = ;
        // ? Is this right, do we need to do this every frame

        shm.save().unwrap();
        shm.set_operator(cairo::Operator::Clear);
        shm.paint().unwrap();
        shm.restore().unwrap();

        shm.set_source_surface(&recorder, 0.0, 0.0).unwrap();
        shm.paint().unwrap();

        let wl_surface = wsk.wl_surface.as_ref().unwrap();
        wl_surface.set_buffer_scale(scale as i32);
        wl_surface.damage_buffer(0, 0, wsk.width as i32, wsk.height as i32);
        wl_surface.commit();
    }
}

fn render_to_cairo(wsk: &mut Wsk, cairo: &cairo::Context, scale: f64, width: u32, height: u32) -> (u32, u32) {
    let mut width = width;
    let mut height = height;

    cairo.set_operator(cairo::Operator::Source);
    cairo.set_source_u32(wsk.background);
    cairo.paint().unwrap();

    println!("Keys: {:?}", &wsk.keys);

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

        cairo.move_to(width as f64, 0.0);

        let w: i32;
        let h: i32;
        if special {
            let text = format!("{}+", name);
            (w, h, _) = get_text_size(cairo, &wsk.font, scale, &text);
            pango_print(cairo, &wsk.font, scale, &text);
        } else {
            let text = format!("{}", name);
            (w, h, _) = get_text_size(cairo, &wsk.font, scale, &text);
            pango_print(cairo, &wsk.font, scale, &text);
        }

        width += w as u32;
        height = height.max(h as u32);

    }

    return (width, height);
    //return (5000, 5000);

}

/* Buffer stuff (For Shm) */
pub fn create_buffer_stuff(wsk: &mut Wsk, tmp: &File, width: u32, height: u32, format: wl_shm::Format) {
    // ? * 4 for argb
    let pool = wsk.wl_shm.as_ref().unwrap().create_pool(tmp.as_raw_fd(),(width * height * 4) as i32, wsk.wl_qhandle.as_ref().unwrap(), ()).unwrap();
    let buffer = pool.create_buffer(
        0,
        width as i32,
        height as i32,
        (width * 4) as i32,
        format,
        wsk.wl_qhandle.as_ref().unwrap(),
        ()
    ).unwrap();
    wsk.wl_buffer = Some(buffer.clone());
}

pub fn create_cairo_stuff(wsk: &mut Wsk, tmp: &mut File , width: u32, height: u32)
    -> ( cairo::ImageSurface, cairo::Context, pangocairo::pango::Context ) {
    // ? Should we make it so we can change the format
    //TODO: Make it rusty
    let surface = unsafe {
        let data_map = libc::mmap(
            null_mut(),
            (width * height * 4) as usize,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_STACK,
            tmp.as_raw_fd(),
            0
        );

        cairo::ImageSurface::create_for_data_unsafe(
            data_map as *mut u8,
            cairo::Format::ARgb32,
            width as i32,
            height as i32,
            (width * 4) as i32

        ).unwrap()
    };

    let pango = cairo::Context::new(&surface).unwrap();
    let cairo = pangocairo::create_context(&pango).unwrap();

    return ( surface, pango, cairo );
}
