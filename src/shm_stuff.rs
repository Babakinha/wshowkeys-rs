use std::{ptr::null_mut, ffi::CString};

use cairo::ImageSurface;
use libc::{EEXIST, EINTR, shm_open, O_RDWR, O_CREAT, O_EXCL, shm_unlink, ftruncate, close, PROT_READ, PROT_WRITE, MAP_SHARED, c_void};
use wayland_client::protocol::{wl_buffer::WlBuffer, wl_shm};

use crate::Wsk;

#[derive(Default)]
pub struct PoolBuffer {
    pub buffer: Option<WlBuffer>,
    pub surface: Option<ImageSurface>,

    pub cairo: Option<cairo::Context>,
    pub pango: Option<pangocairo::pango::Context>,

    pub width: u32,
    pub height: u32,

    pub data: Option<*mut c_void>,
    pub busy: bool
}

/* Shm */


fn rand_string(len: usize) -> String {
    (0..len).map(|_| (0x20u8 + (rand::random::<f32>() * 96.0) as u8) as char).collect()
}


fn create_shm_file() -> i32 {
    let mut retries = 100;
    while retries > 0 && errno::errno().0 == EEXIST {
        retries -= 1;
        let name = format!("/wl_shm-{}", rand_string(6));
        let name_cstr = CString::new(name.as_str()).unwrap();

        // CLOEXEC is guaranteed to be set by shm_opn
        let fd = unsafe { shm_open(name_cstr.as_ptr() as *const i8, O_RDWR | O_CREAT | O_EXCL, 0600) };
        if fd >= 0 {
            unsafe { shm_unlink(name_cstr.as_ptr() as *const i8); };
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

// ? Is returning and using the buf right?
pub fn create_buffer<'a>(wsk: &mut Wsk, buf: &mut PoolBuffer, width: u32, height: u32, format: wl_shm::Format) {
    //let mut buf = PoolBuffer::default();
    let shm = wsk.wl_shm.as_ref().unwrap();
    let stride: u32 = width * 4;
    let size = stride * height;

    let fd = allocate_shm_file(size as i64);
    assert!(fd != -1);

    // ? Is there a Rusty way to do this
    let data = unsafe { libc::mmap(null_mut(), size as usize, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0) };

    let pool = shm.create_pool(fd, size as i32, wsk.wl_qhandle.as_ref().unwrap(), ()).unwrap();
    buf.buffer = Some(pool.create_buffer(
        0, width as i32, height as i32, stride as i32, format, wsk.wl_qhandle.as_ref().unwrap(), ()
    ).unwrap());
    pool.destroy(); // ? Is this the same as drop(pool)
    unsafe { close(fd); };

    buf.width = width;
    buf.height = height;
    buf.data = Some(data);
    buf.surface = Some( unsafe { cairo::ImageSurface::create_for_data_unsafe(
        data as *mut _ as *mut u8, cairo::Format::ARgb32, width as i32, height as i32, stride as i32
    ).unwrap() });
    buf.cairo = Some(cairo::Context::new(buf.surface.as_ref().unwrap()).unwrap());
    buf.pango = Some(pangocairo::create_context(buf.cairo.as_ref().unwrap()).unwrap());

    //return buf;

}

//TODO: Rusty pls
pub fn get_next_buffer(wsk: &mut Wsk, width: u32, height: u32) -> PoolBuffer {
    let mut buffer: Option<PoolBuffer> = None;

    // FIXME: This is important
    for p_buffer in &wsk.buffers {
        if p_buffer.busy { continue; }
        //buffer = Some(p_buffer);
    }

    if buffer.is_none() {
        return PoolBuffer::default(); // ? Return null
    }

    let mut buffer = buffer.unwrap();
    if buffer.width != width || buffer.height != height {
        if buffer.buffer.is_some() {
            buffer.buffer.as_mut().unwrap().destroy();
        }
    }

    if buffer.buffer.is_none() {
        create_buffer(wsk, &mut buffer, width, height, wl_shm::Format::Argb8888);
    }

    buffer.busy = true;
    return buffer;
}