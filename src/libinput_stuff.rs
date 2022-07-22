use std::{path::Path, time::Instant};

use input::{LibinputInterface, event::{keyboard::{KeyboardEventTrait, KeyState}}};
use libc::close;
use xkbcommon::xkb;

use crate::{Wsk, rootinput::RootInput, wsk_keypress::WskKeypress};

/* Libinput */
pub struct LibinputImpl{
    pub user_fd: i32,
}

impl LibinputInterface for LibinputImpl {
    fn open_restricted(&mut self, path: &Path, _flags: i32) -> Result<i32, i32> {
        RootInput::open(self.user_fd,path.to_str().unwrap())
    }

    fn close_restricted(&mut self, fd: i32) {
        unsafe {
            close(fd);
        }
    }
}

pub fn handle_libinput_event(wsk: &mut Wsk, event: &input::Event) {
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

            let key_sym = wsk.xkb_state.as_ref().unwrap().key_get_one_sym(key_code);

            if key_state == KeyState::Pressed {
                let key_name = xkb::keysym_get_name(key_sym);
                let key_utf8 =wsk.xkb_state.as_ref().unwrap().key_get_utf8(key_code);

                let keypress = WskKeypress { sym: key_sym, name: key_name, utf8: key_utf8 };
                wsk.keys.push(keypress);
            }
        },
        _ => {}
    }

    wsk.last_keytime = Some(Instant::now());
    wsk.set_dirty();
}