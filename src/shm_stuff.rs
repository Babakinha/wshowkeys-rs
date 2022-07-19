use libc::{EEXIST, EINTR, shm_open, O_RDWR, O_CREAT, O_EXCL, shm_unlink, ftruncate, close};
use wayland_client::protocol::{wl_buffer::WlBuffer, wl_surface::WlSurface, wl_shm::{WlShm, self}};

use crate::Wsk;

#[derive(Default)]
pub struct PoolBuffer {
    pub buffer: Option<WlBuffer>,
    pub surface: Option<WlSurface>,

    pub cairo: Option<cairo::Context>,
    pub pango: Option<pangocairo::pango::Context>,

    pub width: u32,
    pub height: u32,

    //TODO: data
    pub busy: bool
}

/* Shm */


fn rand_string(len: usize) -> String {
    (0..len).map(|_| (0x20u8 + (rand::random::<f32>() * 96.0) as u8) as char).collect()
}


fn create_shm_file() -> i32 {
    let retries = 100;
    while retries > 0 && errno::errno().0 == EEXIST {
        retries -= 1;
        let name = format!("/wl_shm-{}", rand_string(6));


        // CLOEXEC is guaranteed to be set by shm_opn
        let fd = unsafe { shm_open(name.as_ptr() as *const _ as *const i8, O_RDWR | O_CREAT | O_EXCL, 0600) };
        if fd >= 0 {
            unsafe { shm_unlink(name.as_ptr() as *const _ as *const i8); };
            return fd;
        }
    }

    return -1;
}

fn allocate_shm_file(size: i64) -> i32 {
    let fd = create_shm_file();
    if fd < 0 {
        return -1;
    }

    let mut ret = unsafe { ftruncate(fd, size) };
    while ret < 0 && errno::errno().0 == EINTR {
        ret = unsafe { ftruncate(fd, size) };
    }
    if ret < 0 {
        unsafe { close(fd); };
        return -1;
    }

    return fd;
}

/* Buffer */

pub fn create_buffer(wsk: &mut Wsk, buf: &PoolBuffer, width: u32, height: u32, format: wl_shm::Format) {
    let shm = wsk.wl_shm.as_ref().unwrap();
    let stride: u32 = width * 4;
    let size = stride * height;

    let fd = allocate_shm_file(size as i64);
    assert!(fd != -1);

    let pool = shm.create_pool(fd, size as i32, wsk.wl_qhandle.as_ref().unwrap(), ()).unwrap();
    pool.create_buffer(0, width as i32, height as i32, stride as i32, format, wsk.wl_qhandle.as_ref().unwrap(), ());
}

pub fn get_next_buffer(wsk: &mut Wsk, pool: Vec<PoolBuffer>, width: u32, height: u32) -> PoolBuffer {
    let mut buffer: Option<PoolBuffer> = None;

    for p_buffer in pool {
        if p_buffer.busy { continue; }
        buffer = Some(p_buffer);
    }

    if buffer.is_none() {
        return PoolBuffer::default(); // ? Return null
    }

    let mut buffer = buffer.unwrap();
    if buffer.width != width || buffer.height != height {
        drop(buffer);
        return PoolBuffer::default(); // ? Not in original
    }

    if buffer.buffer.is_none() {
        buffer.buffer = create_buffer(wsk, &buffer, width, height, wl_shm::Format::Argb8888); // ? user-defined
    }

    buffer.busy = true;
    return buffer;
}