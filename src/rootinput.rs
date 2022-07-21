use std::{io::{IoSlice, IoSliceMut, self, Read}, mem::size_of, ptr::null_mut, ffi::CString, fs::File, os::unix::prelude::FromRawFd};

use libc::{SOCK_SEQPACKET, fork, pid_t, CMSG_SPACE, c_void, CMSG_FIRSTHDR, SOL_SOCKET, SCM_RIGHTS, CMSG_LEN, memcpy, CMSG_DATA, EINTR, O_NONBLOCK, O_RDONLY, O_CLOEXEC, O_NOCTTY};
use posix_socket::{UnixSocket, ancillary::SocketAncillary};

use crate::root_utils::{is_root, drop_root};

/* Msg */

#[derive(PartialEq, Clone, Copy)]
enum MsgType {
    Open,
    End
}

struct Msg {
    msg_type: MsgType,
    fd: Option<u8>,
    path: String,
}

// Seriliazation and Deseriliazation
impl Into<Vec<u8>> for &Msg {
    fn into(self) -> Vec<u8> {
        let mut msg = vec![self.msg_type as u8];
        if self.fd.is_some() { msg.push(self.fd.unwrap() + 1) } else { msg.push(0) }
        msg.append(&mut self.path.as_bytes().to_vec());

        return msg;
    }
}

impl From<&Vec<u8>> for Msg {
    fn from(vec: &Vec<u8>) -> Msg {
        let mut vec = vec.clone(); // TODO: I'm tired
        let msg_type: MsgType = if vec[0] == 0 { MsgType::Open } else { MsgType::End };
        let fd = if vec[1] == 0 { None } else { Some(vec[1] - 1) };
        vec.drain(..2);
        let path = String::from_utf8(vec).unwrap();
        Msg { msg_type, fd, path}

    }
}


// Send end Recieve
impl Msg {
    /**
     * Warning: This is supposed to be blocking
     */
    pub fn send(&self, sock: &UnixSocket) -> usize {

        let vec: Vec<u8> = self.into();

        // The end is never the end is never the end... until it is
        let mut ret = sock.send(&vec, 0);
        while ret.is_err() || ret.as_ref().unwrap_or(&0) <= &0 {
            ret = sock.send(&vec, 0);
        }

        return ret.unwrap();
    }

    /**
     * Warning: This is supposed to be blocking
     */
    pub fn recieve(sock: &UnixSocket) -> Msg {
        let mut buf = [0u8; 24];
        let mut ret = sock.recv(&mut buf, 0);

        // The end is never the end is never the end... until it is
        while ret.is_err() || ret.as_ref().unwrap_or(&0) <= &0 {
            ret = sock.recv(&mut buf, 0);
        }

        return Msg::from(&Vec::from(&buf[..ret.unwrap()]));

    }
}


pub struct RootInput {
    //root_sock: UnixSocket,
    pub user_sock: UnixSocket,
    root_pid: pid_t
}

impl RootInput {
    pub fn start(devpath: &str) -> RootInput {
        if !is_root() {
            //panic!("Not running as root");
        }

        //Creates Socks
        let (user_sock, root_sock) = UnixSocket::pair(SOCK_SEQPACKET, 0).unwrap();

        //Create Forks
        let child = unsafe { fork() };

        if child < 0 {
            // We failed to fork
            drop(root_sock);
            drop(user_sock);
            panic!("Unable to create fork");
        } else if child == 0 {
            // We are the fork
            drop(user_sock);
            Self::run(root_sock, devpath);
            unsafe { libc::exit(1); };
        }
        //We are not the fork
        drop(root_sock);

        //TODO: drop to user-specified uid
        //TODO: dont drop root if user-specified
        //drop_root();

        return Self {user_sock, root_pid: child};
    }

    pub fn open(user_sock: i32, path: &str) -> Result<i32, i32> {
        let user_sock = unsafe { UnixSocket::from_raw_fd(user_sock) };
        let msg = Msg { msg_type: MsgType::Open, fd: None, path: path.to_string() };
        msg.send(&user_sock);

        // ? Do we need to retry
        dbg!( unsafe { libc::getuid() });
        let new_msg = Msg::recieve(&user_sock);
        //TODO: Error handling
        dbg!(path, new_msg.fd);
        let mut test = unsafe { File::from_raw_fd(new_msg.fd.unwrap() as i32) };
        let mut tstring = String::new();
        test.read_to_string(&mut tstring).unwrap();
        dbg!(tstring);

        if new_msg.fd.is_some() {


            Ok(new_msg.fd.unwrap() as i32)
        } else {
            Err(2)
        }
    }

    /**
     * This runs as root
     */
    fn run(sock: UnixSocket, devpath: &str) {
        let mut running = true;
        while running {
            // This is blocking
            let msg = Msg::recieve(&sock);

            match msg.msg_type {
                MsgType::Open => {
                    if !msg.path.contains(devpath) {
                        /* Hecker detected */
                        dbg!("Hecker");
                        return; // I think this exits out the function, and then exit(1) is called
                    }

                    //TODO: Rusty way (OpenOptions?)
                    errno::set_errno(errno::Errno(0));
                    dbg!( unsafe { libc::getuid() });
                    let path_c = CString::new("/home/babakinha/test.txt").unwrap();
                    let fd = unsafe { libc::open(path_c.as_ptr(), O_RDONLY|O_CLOEXEC|O_NOCTTY|O_NONBLOCK) };
                    dbg!(errno::errno());
                    if errno::errno().0 == 0 {
                        let msg = Msg { msg_type: MsgType::Open, fd: Some(fd as u8), path: "".to_string() };
                        msg.send(&sock);
                    } else {
                        // ? Send close
                        let msg = Msg { msg_type: MsgType::Open, fd: None, path: "".to_string() };
                        msg.send(&sock);
                    }

                    unsafe { libc::sleep(10) };
                    //let mut test = unsafe { File::from_raw_fd(fd) };
                    //let mut tstring = String::new();
                    //test.read_to_string(&mut tstring).unwrap();
                    //dbg!(tstring);

                    // ? Is this right
                    if fd >= 0 {
                        dbg!("Closed");
                        unsafe { libc::close(fd) }; // ? Why does this work (if this isnt closed user cant read the file)
                    }
                    break;
                },
                MsgType::End => {
                    running = false;
                    let msg = Msg { msg_type: MsgType::End, fd: None, path: "".to_string() };
                    msg.send(&sock);
                    break;
                }
            };

        }
    }
}

// Drop RootInput
impl Drop for RootInput {
    // ? Is this safe
    fn drop(&mut self) {
        let msg = Msg { msg_type: MsgType::End, fd: None, path: "".to_string() };
        msg.send(&self.user_sock);
        Msg::recieve(&self.user_sock);

        unsafe {
            libc::waitpid(self.root_pid, null_mut(), 0);
            libc::close(self.user_sock.as_raw_fd());
        }
    }
}