pub mod devmgr;
pub mod root_utils;
pub mod wayland_utils;

use input::{Libinput, LibinputInterface, ffi::{libinput_set_user_data}};
use wayland_client::{protocol::{wl_display::WlDisplay, wl_registry::WlRegistry}, Connection, EventQueue, QueueHandle};
use wayland_utils::EmptyAppData;
use xkbcommon::xkb::{self, ffi::XKB_CONTEXT_NO_FLAGS};
use std::path::Path;
use devmgr::{devmgr_start, devmgr_finish, devmgr_open};
use libc::{pid_t, c_void, close};

/* Our stuff */
#[derive(Default)]
pub struct Wsk {
    /* Devmgr, input, xkb */
    devmgr: i32,
    devmgr_pid: pid_t,

    input: Option<Libinput>,
    xkb_context: Option<xkb::Context>,

    /* Config */
    foreground: u32,
    background: u32,
    specialfg: u32,

    font: String,
    timeout: u32, // ? he uses i32 but i dont think we can have negative timeout

    /* Wayland stuff */
    wl_connection: Option<Connection>,
    wl_display: Option<WlDisplay>,
    wl_event_queue: Option<EventQueue<EmptyAppData>>,
    wl_queue_handle: Option<QueueHandle<EmptyAppData>>,
    wl_registry: Option<WlRegistry>,
}

/* Libinput */
struct LibinputImpl;

impl LibinputInterface for LibinputImpl {
    fn open_restricted(&mut self, path: &Path, _flags: i32) -> Result<i32, i32> {
        let fd: *mut i32 = self as *mut _ as *mut i32;
        unsafe {
            devmgr_open(*fd, path.to_str().unwrap().to_string())
        }
    }

    fn close_restricted(&mut self, fd: i32) {
        unsafe {
            close(fd);
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

    let ret: i32 = 0;

    //Default Settings
    let anchor: u32 = 0;
    let margin: i32 = 32;
    wsk.background = 0x000000CC;
    wsk.specialfg = 0xAAAAAAFF;
    wsk.foreground = 0xFFFFFFFF;
    wsk.font = "monospace 24".to_string();
    wsk.timeout = 1;

    //TODO: Parse options

    //libinput
    wsk.input = Some(Libinput::new_with_udev(LibinputImpl));
    unsafe {
        libinput_set_user_data(
            &mut wsk.input.as_ref().unwrap() as *mut _ as *mut input::ffi::libinput,
            &mut wsk.devmgr as *mut _ as *mut c_void
        );
    };

    //xkb
    wsk.xkb_context = Some(xkb::Context::new(XKB_CONTEXT_NO_FLAGS));

    //Wayland :O
    // Getting stuff
    wsk.wl_connection = Some(Connection::connect_to_env().unwrap());
    wsk.wl_display = Some(wsk.wl_connection.as_ref().unwrap().display());

    wsk.wl_event_queue = Some(wsk.wl_connection.as_ref().unwrap().new_event_queue());
    wsk.wl_queue_handle = Some(wsk.wl_event_queue.as_ref().unwrap().handle());

    wsk.wl_registry = Some(wsk.wl_display.as_ref().unwrap().get_registry(&wsk.wl_queue_handle.as_ref().unwrap(), ()).unwrap());

    // Doing stuff

    //wsk.wl_re

    wsk.wl_event_queue.as_mut().unwrap().roundtrip(&mut EmptyAppData).unwrap();


    // Dont forget!
    drop(wsk.input.unwrap());
    devmgr_finish(wsk.devmgr, wsk.devmgr_pid);
}
