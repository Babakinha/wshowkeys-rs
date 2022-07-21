pub mod devmgr;
pub mod rootinput;
pub mod root_utils;
pub mod cairo_utils;
pub mod pango_utils;
pub mod wsk_keypress;

pub mod libinput_stuff;
pub mod wayland_stuff;
pub mod pango_stuff;
pub mod shm_stuff;
pub mod render_stuff;

use std::{os::unix::prelude::AsRawFd, mem::size_of};

use input::{
    Libinput,
    ffi::{libinput_set_user_data},
};
use libinput_stuff::LibinputImpl;
use rootinput::RootInput;
use shm_stuff::PoolBuffer;
use wayland_client::{
    protocol::{
        wl_display::WlDisplay, wl_compositor::WlCompositor, wl_shm::WlShm, wl_seat::WlSeat, wl_keyboard::WlKeyboard, wl_surface::WlSurface, wl_output::{Subpixel, WlOutput}, wl_buffer::WlBuffer,
    },
    Connection, QueueHandle,
};

use devmgr::{devmgr_start, devmgr_finish};
use libc::{pid_t, c_void, pollfd, POLLIN, poll, nfds_t};
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1::ZxdgOutputManagerV1;
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1::{ZwlrLayerShellV1, self}, zwlr_layer_surface_v1::{ZwlrLayerSurfaceV1, Anchor}};
use wsk_keypress::WskKeypress;
use xkbcommon::xkb;

use crate::libinput_stuff::handle_libinput_event;

/* Our stuff */
#[derive(Default)]
pub struct Wsk {

    /* RootInput, input, xkb */
    root_input: Option<RootInput>,

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
    wl_qhandle: Option<QueueHandle<Wsk>>,
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

    buffers: Vec<PoolBuffer>,
    temp_buffer: Option<*mut PoolBuffer>,
    current_buffer: Option<PoolBuffer>,

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
    wsk.root_input = Some(RootInput::start("/dev/input"));

    /* Normal user code :) */
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
    wsk.input = Some(Libinput::new_from_path(LibinputImpl { user_fd: wsk.root_input.as_ref().unwrap().user_sock.as_raw_fd() } ));

    /* Xkb */
    wsk.xkb_context = Some(xkb::Context::new(xkb::CONTEXT_NO_FLAGS));

    /* Wayland :O */
    //Getting stuff
    wsk.wl_connection = Some(Connection::connect_to_env().unwrap());
    wsk.wl_display = Some(wsk.wl_connection.as_ref().unwrap().display());


    //Setting stuff
    let mut wl_event_queue = wsk.wl_connection.as_ref().unwrap().new_event_queue();
    wsk.wl_qhandle = Some(wl_event_queue.handle());

    wsk.wl_display.as_mut().unwrap().get_registry(wsk.wl_qhandle.as_ref().unwrap(), ()).unwrap();

    wl_event_queue.roundtrip(&mut wsk).unwrap();

    //Check everything
    if
    wsk.wl_compositor.is_none() ||
    wsk.wl_shm.is_none() ||
    wsk.wl_seat.is_none() ||
    wsk.wl_layer_shell.is_none()
    {
        eprintln!("Error: Required Wayland interface not present");
        exit(wsk);
        return;
    }

    //Getting Keyboard (important for libinput)
    wsk.wl_seat.as_ref().unwrap().get_keyboard(wsk.wl_qhandle.as_ref().unwrap(), ()).unwrap();
    wl_event_queue.roundtrip(&mut wsk).unwrap();

    //Getting surface
    wsk.wl_surface = Some(wsk.wl_compositor.as_ref().unwrap().create_surface(wsk.wl_qhandle.as_ref().unwrap(), ()).unwrap());

    //Getting layer shell
    let layer_surface = wsk.wl_layer_shell.as_mut().unwrap().get_layer_surface(
        wsk.wl_surface.as_ref().unwrap(),
        None,
        zwlr_layer_shell_v1::Layer::Top,
        "showkeys".to_string(),
        wsk.wl_qhandle.as_ref().unwrap(),
        ()
    ).unwrap();

    // ? Are this setting right?
    layer_surface.set_size(1, 1);
    layer_surface.set_anchor(anchor);
    layer_surface.set_margin(margin[0], margin[1], margin[2], margin[3]);
    layer_surface.set_exclusive_zone(-1);

    wsk.wl_layer_surface = Some(layer_surface);
    wsk.wl_surface.as_ref().unwrap().commit();

    //Polls
    let mut input = wsk.input.as_mut().unwrap().clone(); // ? There is no problem to clone right (i think its still a pointer)
    let mut pollfds: [pollfd; 2] = [
        pollfd { fd: input.as_raw_fd(), events: POLLIN, revents: 0 },
        pollfd { fd: wl_event_queue.prepare_read().unwrap().connection_fd(), events: POLLIN, revents: 0 }
    ];

    // The end is never the end is never the end is never the end is never the...
    while wsk.run {
        //TODO: Flush display?
        wl_event_queue.flush().unwrap();

        /* Poll */
        let mut timeout = -1;
        if !wsk.keys.is_empty() {
            timeout = 100;
        }

        if unsafe { poll(pollfds.as_mut_ptr(), (size_of::<pollfd>() * pollfds.len()) as nfds_t, timeout) } < 0 {
            eprintln!("poll: {}", errno::errno());
            break;
        }


        /* Dispatch */
        if (pollfds[0].revents & POLLIN) != 0 {
            input.dispatch().unwrap();
            for event in &mut input {
                println!("Event!");
                handle_libinput_event(&mut wsk, &event);
            }
        }

        if (pollfds[1].revents & POLLIN) != 0 {
            //wl_event_queue.blocking_dispatch(&mut wsk).unwrap();
        }
    }

    // Dont forget!
    exit(wsk);
}

pub fn exit(wsk: Wsk) {
    drop(wsk.input.unwrap());
    drop(wsk.root_input.unwrap());
}
