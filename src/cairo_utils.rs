use wayland_client::protocol::wl_output::Subpixel;

pub trait ToSubpixelOrder {
    fn to_cairo_subpixel_order(&self) -> cairo::SubpixelOrder;
}

impl ToSubpixelOrder for Subpixel {
    fn to_cairo_subpixel_order(&self) -> cairo::SubpixelOrder {
        match self {
            Subpixel::HorizontalRgb => cairo::SubpixelOrder::Rgb,
            Subpixel::HorizontalBgr => cairo::SubpixelOrder::Bgr,
            Subpixel::VerticalRgb => cairo::SubpixelOrder::Vrgb,
            Subpixel::VerticalBgr => cairo::SubpixelOrder::Vrgb,
            _ => cairo::SubpixelOrder::Default,
        }
    }
}

pub trait SetSourceU32 {
    fn set_source_u32(&self, source: u32);
}

impl SetSourceU32 for cairo::Context {
    fn set_source_u32(&self, source: u32){
        self.set_source_rgba(
            (source >> (3*8) & 0xFF) as f64 / 255.0,
            (source >> (2*8) & 0xFF) as f64 / 255.0,
            (source >> (1*8) & 0xFF) as f64 / 255.0,
            (source >> (0*8) & 0xFF) as f64 / 255.0
        );
    }
}