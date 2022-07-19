pub mod devmgr;
pub mod root_utils;
pub mod cairo_utils;
pub mod pango_utils;
pub mod wsk_keypress;

pub mod libinput_stuff;
pub mod wayland_stuff;
pub mod pango_stuff;
pub mod render_stuff;


use input::{
    Libinput,
    ffi::{libinput_set_user_data},
};
use libinput_stuff::LibinputImpl;
use wayland_client::{
    protocol::{
        wl_display::WlDisplay, wl_compositor::WlCompositor, wl_shm::WlShm, wl_seat::WlSeat, wl_keyboard::WlKeyboard, wl_surface::WlSurface, wl_output::{Subpixel, WlOutput},
    },
    Connection,
};

use devmgr::{devmgr_start, devmgr_finish};
use libc::{pid_t, c_void};
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1::ZxdgOutputManagerV1;
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1::{ZwlrLayerShellV1, self}, zwlr_layer_surface_v1::{ZwlrLayerSurfaceV1, Anchor}};
use wsk_keypress::WskKeypress;
use xkbcommon::xkb;

use crate::libinput_stuff::handle_libinput_event;

/* Our stuff */
#[derive(Default)]
pub struct Wsk {

    /* Devmgr, input, xkb */
    devmgr: i32,
    devmgr_pid: pid_t,

    input: Option<Libinput>,


    xkb_context: Option<xkb::Context>,
    xkb_keymap: Option<xkb::Keymap>,
    xkb_state: Option<xkb::State>,

    /* Config */
    foreground: u32,
    background: u32,
    specialfg: u32,

    font: String,
    timeout: u32, // ? he uses i32 but i dont think we can have negative timeout

    /* Wayland stuff */
    wl_connection: Option<Connection>,
    wl_display: Option<WlDisplay>,
    wl_compositor: Option<WlCompositor>,
    wl_shm: Option<WlShm>,

    wl_seat: Option<WlSeat>,
    wl_keyboard: Option<WlKeyboard>,

    wl_output_mgr: Option<ZxdgOutputManagerV1>,
    wl_layer_shell: Option<ZwlrLayerShellV1>,

    wl_surface: Option<WlSurface>,
    wl_layer_surface: Option<ZwlrLayerSurfaceV1>,

    wl_output: Option<WlOutput>,
    wl_subpixel: Option<Subpixel>,

    width: u32,
    height: u32,
    scale: f64,

    /* Keys */
    keys: Vec<WskKeypress>,
    //last_keytime: timespec,

    /* Misc */
    run: bool,

    dirty: bool,
    frame_scheduled: bool,

}

impl Wsk {
    pub fn set_dirty(&mut self) {
        if self.frame_scheduled {
            self.dirty = true;
        }else if self.wl_surface.is_some() {
            render_stuff::render_frame(self);
        }
    }
}

fn main() {
    /* Running as root :O */
    let mut wsk = Wsk::default();

    let (devmgr, devmgr_pid) = devmgr_start("/dev/input/");

    /* Normal user code :) */
    wsk.devmgr = devmgr;
    wsk.devmgr_pid = devmgr_pid;

    let _ret: i32 = 0;

    /* Default Settings */
    let anchor: Anchor = Anchor::empty();
    let margin: [i32; 4] = [32, 32, 32, 32]; // Top, Right, Bottom, Left
    wsk.background = 0x000000CC;
    wsk.specialfg = 0xAAAAAAFF;
    wsk.foreground = 0xFFFFFFFF;
    wsk.font = "monospace 24".to_string();
    wsk.timeout = 1;

    wsk.run = true;

    //TODO: Parse options

    /* Libinput */
    wsk.input = Some(Libinput::new_with_udev(LibinputImpl));
    unsafe {
        libinput_set_user_data(
            &mut wsk.input.as_ref().unwrap() as *mut _ as *mut input::ffi::libinput,
            &mut wsk.devmgr as *mut _ as *mut c_void
        );
    };

    /* Xkb */
    wsk.xkb_context = Some(xkb::Context::new(xkb::CONTEXT_NO_FLAGS));

    /* Wayland :O */
    //Getting stuff
    wsk.wl_connection = Some(Connection::connect_to_env().unwrap());
    wsk.wl_display = Some(wsk.wl_connection.as_ref().unwrap().display());


    //Setting stuff
    let mut wl_event_queue = wsk.wl_connection.as_ref().unwrap().new_event_queue();
    let wl_qhandle = wl_event_queue.handle();

    wsk.wl_display.as_mut().unwrap().get_registry(&wl_qhandle, ()).unwrap();

    wl_event_queue.roundtrip(&mut wsk).unwrap();

    //Getting layer shell
    let layer_surface = wsk.wl_layer_shell.as_mut().unwrap().get_layer_surface(
        wsk.wl_surface.as_ref().unwrap(),
        None,
        zwlr_layer_shell_v1::Layer::Top,
        "showkeys".to_string(),
        &wl_qhandle,
        ()
    ).unwrap();

    // ? Are this setting right?
    layer_surface.set_size(1, 1);
    layer_surface.set_anchor(anchor);
    layer_surface.set_margin(margin[0], margin[1], margin[2], margin[3]);
    layer_surface.set_exclusive_zone(-1);

    wsk.wl_layer_surface = Some(layer_surface);
    wsk.wl_surface.as_ref().unwrap().commit();

    // The end is never the end is never the end is never the end is never the...
    let mut input = wsk.input.as_mut().unwrap().clone(); // ? There is no problem to clone right (i think its still a pointer)
    // Temp Main loop
    while wsk.run {
        wl_event_queue.dispatch_pending(&mut wsk).unwrap();

        //Handle libinput events
        input.dispatch().unwrap();
        for event in &mut input {
            println!("Event!");
            handle_libinput_event(&mut wsk, &event);
        }
    }

    // Dont forget!
    drop(wsk.input.unwrap());
    devmgr_finish(wsk.devmgr, wsk.devmgr_pid);
}
