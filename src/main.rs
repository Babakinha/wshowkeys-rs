pub mod devmgr;
pub mod root_utils;

use input::{
    Libinput,
    LibinputInterface,
    ffi::{libinput_set_user_data},
    event::{keyboard::{KeyboardEventTrait, KeyState}}
};
use wayland_client::{
    protocol::{
        wl_display::WlDisplay,
        wl_registry::{self, WlRegistry},
        wl_compositor::{self, WlCompositor},
        wl_shm::{self, WlShm},
        wl_seat::{self, WlSeat}, wl_keyboard::{WlKeyboard, self, KeymapFormat}
    },
    Connection,
    QueueHandle,
    Dispatch, WEnum
};

use wayland_protocols::{xdg::xdg_output::zv1::client::zxdg_output_manager_v1::{ZxdgOutputManagerV1, self}};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{ZwlrLayerShellV1, self};
use xkbcommon::xkb::{self, ffi::{XKB_CONTEXT_NO_FLAGS, XKB_KEYMAP_FORMAT_TEXT_V1, XKB_KEYMAP_COMPILE_NO_FLAGS}};
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
    wl_output_mgr: Option<ZxdgOutputManagerV1>,
    wl_layer_shell: Option<ZwlrLayerShellV1>,
    wl_keyboard: Option<WlKeyboard>,

    /* Misc */
    run: bool
}

/* Wayland */

//Registry Events
// Setting stuff up
impl Dispatch<wl_registry::WlRegistry, ()> for Wsk {
    fn event(
        wsk: &mut Self,
        proxy: &WlRegistry,
        event: <WlRegistry as wayland_client::Proxy>::Event,
        data: &(),
        _conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match &interface[..] {
                "wl_compositor" => {
                    let compositor: WlCompositor = proxy.bind(name, version, qhandle, *data).unwrap();
                    wsk.wl_compositor = Some(compositor);
                },

                "wl_shm" => {
                    let shm: WlShm = proxy.bind(name, version, qhandle, *data).unwrap();
                    wsk.wl_shm = Some(shm);
                },

                "wl_seat" => {
                    let seat: WlSeat = proxy.bind(name, version, qhandle, *data).unwrap();
                    wsk.wl_seat = Some(seat);
                },

                // Unstable D:
                "zxdg_output_manager_v1" => {
                    let output_mgr: ZxdgOutputManagerV1 = proxy.bind(name, version, qhandle, *data).unwrap();
                    wsk.wl_output_mgr = Some(output_mgr);

                }

                "zwlr_layer_shell_v1" => {
                    let layer_shell: ZwlrLayerShellV1 = proxy.bind(name, version, qhandle, *data).unwrap();
                    wsk.wl_layer_shell = Some(layer_shell);
                }

                //"wl_output" => {
                //    //TODO: This
                //},

                _ => {}
            };
        }
    }
}

//Seat events
// Getting the keyboard
impl Dispatch<WlSeat, ()> for Wsk {
    fn event(
        wsk: &mut Self,
        proxy: &WlSeat,
        event: <WlSeat as wayland_client::Proxy>::Event,
        data: &(),
        _conn: &Connection,
        qhandle: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities: WEnum::Value(capibilities) } = event {
            if wsk.wl_keyboard.is_some() {
                //TODO: Support for multiple seats
                return;
            }

            if capibilities.contains(wl_seat::Capability::Keyboard) {
                wsk.wl_keyboard = Some(proxy.get_keyboard(qhandle, *data).unwrap());
            } else {
                eprintln!("wl_seat does not support keyboard");
                wsk.run = false;
            }
        } else if let wl_seat::Event::Name { name } = event {
            // TODO: Support for multiple seats
            match wsk.input.as_mut().unwrap().udev_assign_seat(name.as_str()) {
                Ok(_) => {},
                Err(_) => {
                    eprintln!("Failed to assign libinput seat");
                    wsk.run = false;
                },
            };
        }
    }
}

//Keyboard Events
// Guess what
impl Dispatch<WlKeyboard, ()> for Wsk {
    fn event(
        wsk: &mut Self,
        _proxy: &WlKeyboard,
        event: <WlKeyboard as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let wl_keyboard::Event::Keymap { format: WEnum::Value(format), fd, size } = event {
            if format != KeymapFormat::XkbV1 {
                unsafe { close(fd); };
                return;
            }

            let xkb_keymap = xkb::Keymap::new_from_fd(
                wsk.xkb_context.as_ref().unwrap(),
                fd,
                size as usize,
                XKB_KEYMAP_FORMAT_TEXT_V1,
                XKB_KEYMAP_COMPILE_NO_FLAGS
            ).unwrap();
            unsafe { close(fd); };

            let xkb_state = xkb::State::new(&xkb_keymap);
            // ? unref state
            wsk.xkb_keymap = Some(xkb_keymap);
            wsk.xkb_state = Some(xkb_state);
        }
    }
}

// Ignore this code (Boilerplate stuff)
impl Dispatch<WlCompositor, ()> for Wsk {
    fn event(
        _: &mut Self,
        _: &WlCompositor,
        _: wl_compositor::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Nothing we want here
    }
}

impl Dispatch<WlShm, ()> for Wsk {
    fn event(
        _: &mut Self,
        _: &WlShm,
        _: wl_shm::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Nothing we want here
    }
}


impl Dispatch<ZxdgOutputManagerV1, ()> for Wsk {
    fn event(
        _: &mut Self,
        _: &ZxdgOutputManagerV1,
        _: zxdg_output_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Nothing we want here
    }
}

impl Dispatch<ZwlrLayerShellV1, ()> for Wsk {
    fn event(
        _: &mut Self,
        _: &ZwlrLayerShellV1,
        _: zwlr_layer_shell_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Nothing we want here
    }
}
//Ok you can stop ignoring now

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

fn handle_libinput_event(wsk: &mut Wsk, event: &input::Event) {
    println!("{:?}", event);
    if wsk.xkb_state.is_none() {
        return;
    }

    match event {
        input::Event::Keyboard(keyboard_event) => {
            let key_state = keyboard_event.key_state();
            let key_code = keyboard_event.key();

            wsk.xkb_state.as_mut().unwrap().update_key(
                key_code,
                if key_state == KeyState::Released { xkb::KeyDirection::Up} else { xkb::KeyDirection::Down }
            );

            let _key_sym = wsk.xkb_state.as_ref().unwrap().key_get_one_sym(key_code);

            if key_state == KeyState::Pressed {
                //TODO: Key pressed
                println!("TODO: Key Pressed")
            }
        },
        _ => {}
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

    //Default Settings
    let _anchor: u32 = 0;
    let _margin: i32 = 32;
    wsk.background = 0x000000CC;
    wsk.specialfg = 0xAAAAAAFF;
    wsk.foreground = 0xFFFFFFFF;
    wsk.font = "monospace 24".to_string();
    wsk.timeout = 1;

    wsk.run = true;

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


    // Doing stuff
    let mut wl_event_queue = wsk.wl_connection.as_ref().unwrap().new_event_queue();
    let wl_qhandle = wl_event_queue.handle();

    wsk.wl_display.as_mut().unwrap().get_registry(&wl_qhandle, ()).unwrap();

    wl_event_queue.roundtrip(&mut wsk).unwrap();
    let mut input = wsk.input.as_mut().unwrap().clone(); // ? There is no problem to clone right (i think its still a pointer)
    // Temp Main loop
    while wsk.run {
        wl_event_queue.blocking_dispatch(&mut wsk).unwrap();

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
