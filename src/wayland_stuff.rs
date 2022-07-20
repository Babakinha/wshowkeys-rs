use std::{ptr::null_mut, ffi::CStr};

use libc::{close, mmap, PROT_READ, MAP_SHARED, munmap, MAP_FAILED, c_char};
use wayland_client::{
    Dispatch,
    protocol::{
        wl_registry::{WlRegistry, self},
        wl_compositor::{WlCompositor, self},
        wl_shm::{WlShm, self},
        wl_seat::{WlSeat, self},
        wl_keyboard::{WlKeyboard, self, KeymapFormat}, wl_surface::{WlSurface, self}, wl_output::{WlOutput, self}, wl_shm_pool::{WlShmPool, self}, wl_buffer::{WlBuffer, self}
    },
    Connection,
    QueueHandle,
    WEnum
};
use wayland_protocols::xdg::xdg_output::zv1::client::zxdg_output_manager_v1::{ZxdgOutputManagerV1, self};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1::{ZwlrLayerShellV1, self}, zwlr_layer_surface_v1::{ZwlrLayerSurfaceV1, self}};
use xkbcommon::xkb;

use crate::Wsk;

/* Wayland */

//Registry Events
// Setting stuff up
impl Dispatch<WlRegistry, ()> for Wsk {
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

                    //Make things here since we already have what we need
                    let surface = compositor.create_surface(qhandle, *data).unwrap();

                    wsk.wl_compositor = Some(compositor);
                    wsk.wl_surface = Some(surface);

                },

                "wl_shm" => {
                    let shm: WlShm = proxy.bind(name, version, qhandle, *data).unwrap();
                    wsk.wl_shm = Some(shm);
                },

                "wl_shm_pool" => {

                }

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

                "wl_output" => {
                    dbg!("Output!");
                    //TODO: support multiple outputs
                    let output: WlOutput = proxy.bind(name, version, qhandle, *data).unwrap();
                    wsk.scale = 1.0;
                    wsk.wl_output = Some(output);
                },

                _ => {}
            };
        }
    }
}

//Surface events
// Pretty boi
impl Dispatch<WlSurface, ()> for Wsk {
    fn event(
        wsk: &mut Self,
        _proxy: &WlSurface,
        event: <WlSurface as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let wl_surface::Event::Enter { output } = event {
            // ? Is this right
            wsk.wl_output = Some(output);
        }
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for Wsk {
    fn event(
        wsk: &mut Self,
        proxy: &ZwlrLayerSurfaceV1,
        event: <ZwlrLayerSurfaceV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure { serial, width, height } => {
                wsk.width = width;
                wsk.height = height;
                proxy.ack_configure(serial);
                wsk.set_dirty();
            },

            zwlr_layer_surface_v1::Event::Closed => {
                wsk.run = false;
            },

            _ => {},
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
        match event {
            wl_seat::Event::Capabilities { capabilities: WEnum::Value(capibilities) } => {
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
            },
            wl_seat::Event::Name { name: _ } => {
                // TODO: Support for multiple seats
                if let Err(_) = wsk.input.as_mut().unwrap().udev_assign_seat("seat0") {
                    eprintln!("Failed to assign libinput seat");
                    wsk.run = false;
                };
            }

            _ => {}
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

            let map_shm = unsafe { mmap(null_mut(), size as usize, PROT_READ, MAP_SHARED, fd, 0) };
            if map_shm == MAP_FAILED {
                unsafe { close(fd); };
                eprintln!("Unable to mmap keymap: {}", errno::errno());
                return;
            }

            if format != KeymapFormat::XkbV1 {
                unsafe {
                    munmap(map_shm, size as usize);
                    close(fd);
                };
                return;
            }

            /*
            let xkb_keymap = xkb::Keymap::new_from_fd(
                wsk.xkb_context.as_ref().unwrap(),
                fd,
                size as usize,
                xkb::KEYMAP_FORMAT_TEXT_V1,
                xkb::KEYMAP_COMPILE_NO_FLAGS
            ).unwrap();
            */

            let xkb_keymap = xkb::Keymap::new_from_string(
                wsk.xkb_context.as_ref().unwrap(),
                unsafe { CStr::from_ptr(map_shm as *const _ as * const c_char).to_str().unwrap().to_string() },
                xkb::KEYMAP_FORMAT_TEXT_V1,
                xkb::COMPILE_NO_FLAGS
            ).unwrap();

            unsafe {
                munmap(map_shm, size as usize);
                close(fd);
            };
            let xkb_state = xkb::State::new(&xkb_keymap);

            // ? Does this unref state by drop the old ones?
            wsk.xkb_keymap = Some(xkb_keymap);
            wsk.xkb_state = Some(xkb_state);
        }
    }
}

//Output
// Hey thats cool
impl Dispatch<WlOutput, ()> for Wsk {
    fn event(
        wsk: &mut Self,
        _proxy: &WlOutput,
        event: <WlOutput as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        match event {
            wl_output::Event::Geometry { subpixel, .. } => {
                wsk.wl_subpixel = Some(subpixel.into_result().unwrap());
            },

            wl_output::Event::Scale { factor } => {
                wsk.scale = factor as f64;
            },

            //wl_output::Event::Name { name } => todo!(),
            //wl_output::Event::Description { description } => todo!(),
            _ => {},
        }
    }
}

//Buffers
// Comment loading...
impl Dispatch<WlBuffer, ()> for Wsk {
    fn event(
        wsk: &mut Self,
        _proxy: &WlBuffer,
        event: <WlBuffer as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release {} = event {
            // ? Is there a better way to do this with udata?
            unsafe {
                (*wsk.temp_buffer.unwrap()).busy = false;
            }
        }
    }
}

/* Ignore this code (Boilerplate stuff) */
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

impl Dispatch<WlShmPool, ()> for Wsk {
    fn event(
        _: &mut Self,
        _: &WlShmPool,
        _: wl_shm_pool::Event,
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