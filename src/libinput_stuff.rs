use std::path::Path;

use input::{LibinputInterface, event::{keyboard::{KeyboardEventTrait, KeyState}}};
use libc::close;
use xkbcommon::xkb;

use crate::{devmgr::devmgr_open, Wsk};

/* Libinput */
pub struct LibinputImpl;

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

pub fn handle_libinput_event(wsk: &mut Wsk, event: &input::Event) {
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