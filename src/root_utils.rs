use std::env;

use libc::{getuid, setuid, getgid, setgid, uid_t};



pub fn is_root() -> bool {unsafe {getuid() == 0}}
pub fn can_root() -> bool {unsafe {setuid(0) == -1}}

pub fn drop_root() {
    if !is_root() {
        return;
    }

    let get_good = unsafe{ getgid() };
    if get_good != 0 {
        unsafe{setgid(get_good);};
        unsafe{setuid(getuid());};
        if can_root() {
            //TODO:
            //We failed. should we panic?
            //Or wait to try sudo?
        }
    }

    match get_sudo_gid() {
        Ok(gid) => {unsafe {setgid(gid);}}
        Err(_) => {}
    };

    match get_sudo_uid() {
        Ok(uid) => {unsafe {setuid(uid);return;}},
        Err(_) => {},
    };

    //TODO:
    //We failed. should we panic?
    return;
}

pub fn drop_root_to(uid: uid_t) {
    if !is_root() {
        return;
    }

    unsafe {setuid(uid);};
    return;
}


pub fn is_sudo_root() -> bool {
    if !is_root() {
        return false;
    }

    let sudo_user = get_sudo_uid();
    match sudo_user {
        Ok(_) => true,
        Err(_) => false
    }

}

pub fn get_sudo_uid() -> Result<uid_t, ()> {
    let sudo_uid = env::var("SUDO_UID");
    match sudo_uid {
        Ok(uid) => Ok(uid.parse::<uid_t>().unwrap()),
        Err(_) => Err(())
    }
}

pub fn get_sudo_gid() -> Result<uid_t, ()> {
    let sudo_uid = env::var("SUDO_GID");
    match sudo_uid {
        Ok(uid) => Ok(uid.parse::<uid_t>().unwrap()),
        Err(_) => Err(())
    }
}