use std::{io::{IoSliceMut, Read}, mem::size_of, ptr::null_mut, ffi::CString, fs::File, os::unix::prelude::FromRawFd};

use libc::{SOCK_SEQPACKET, fork, pid_t, CMSG_SPACE, c_void, CMSG_FIRSTHDR, SOL_SOCKET, SCM_RIGHTS, CMSG_LEN, CMSG_DATA, O_NONBLOCK, O_RDONLY, O_CLOEXEC, O_NOCTTY};
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
    fd: Option<i32>,
    path: String,
}

// Seriliazation and Deseriliazation
impl Into<Vec<u8>> for &Msg {
    fn into(self) -> Vec<u8> {
        let mut msg = vec![self.msg_type as u8];
        msg.append(&mut self.path.as_bytes().to_vec());

        return msg;
    }
}

impl From<&Vec<u8>> for Msg {
    fn from(vec: &Vec<u8>) -> Msg {
        let mut vec = vec.clone(); // TODO: I'm tired
        let msg_type: MsgType = if vec[0] == 0 { MsgType::Open } else { MsgType::End };
        vec.drain(..1);
        let path = String::from_utf8(vec).unwrap();
        Msg { msg_type, fd: None, path}

    }
}


// Send end Recieve
impl Msg {
    /**
     * Warning: This is supposed to be blocking
     */
    //TODO: Make it rusty (and shorter if possible)
    pub fn send(&self, sock: &UnixSocket) -> usize {
        dbg!("Got Message To Send", &self.path, self.fd);
        /* Create Message Header */
		let mut header = unsafe { std::mem::zeroed::<libc::msghdr>() };

        /* Normal Data */
        let vec: Vec<u8> = self.into();
        dbg!(&vec);
        dbg!(vec.capacity());
        dbg!(vec.len());
        let mut io_slice = libc::iovec { iov_base: vec.as_ptr() as *mut libc::c_void, iov_len: vec.capacity() };

        header.msg_iov = &mut io_slice;
        header.msg_iovlen = 1; // 1 beavuse we are only sending one slice

        /* Fd Data */
        /*
            This needs to be in the control data
            for the os to understand to share the file descriptor
        */
        // We Allocat the right size of the data we are going to need
        if self.fd.is_some() {
            /* Making the CMsg */

            /*
            let mut control_data: Vec<u8> = Vec::with_capacity(unsafe { CMSG_SPACE(size_of::<i32>() as u32 /* i32 for fd */) } as usize);
            unsafe { control_data.set_len(24) } ;
            
            dbg!(control_data.capacity());

            header.msg_control = control_data.as_mut_ptr() as *mut c_void; // ? Is this right
            header.msg_controllen = control_data.capacity();

            let cmsg =  unsafe { CMSG_FIRSTHDR(&header) };

            /* Set CMsg fields */
            unsafe {
                (*cmsg).cmsg_level = SOL_SOCKET;
                (*cmsg).cmsg_type = SCM_RIGHTS;
                (*cmsg).cmsg_len = CMSG_LEN(size_of::<i32>() as u32) as usize; // This is the size of header + data(i32 for the fd)

                //CMSG_DATA return a pointer to the data, so we fill it with the fd
                *(CMSG_DATA(cmsg) as *mut i32) = self.fd.unwrap();

                dbg!(*cmsg);
            }
            /* Set Header fields */
            dbg!(&control_data);
            //header.msg_controllen = unsafe { (*cmsg).cmsg_len };
            */
            let mut ancc_buf = [0u8; 48];
            let mut test = SocketAncillary::new(&mut ancc_buf[..]);
            let size = test.len();
            test.add_fds(&[self.fd.unwrap()]);
            header.msg_control = ancc_buf.as_mut_ptr() as *mut c_void;
            header.msg_controllen = size;
            dbg!(&ancc_buf);
            dbg!(&size);


        }

        // Send it
        unsafe {
            let mut ret = libc::sendmsg(sock.as_raw_fd(), &header, 0);
            dbg!(ret);
            dbg!(errno::errno());
            // The end is never the end is never the end... until it is
            while ret <= 0 {
                ret = libc::sendmsg(sock.as_raw_fd(), &header, 0);
            }
            return ret as usize;
        }
    }

    /**
     * Warning: This is supposed to be blocking
     */
    //TODO: Make it rusty (and shorter if possible)
    pub fn recieve(sock: &UnixSocket) -> Msg {
        /* Create Message Header */
		let mut header = unsafe { std::mem::zeroed::<libc::msghdr>() };

        /* Normal data */
        let mut data_buf = Vec::with_capacity(24);//[0u8; 24];
        let mut io_slice = libc::iovec { iov_base: data_buf.as_mut_ptr() as *mut c_void, iov_len: data_buf.capacity() };

        // ? Do we need to do iov_base
        header.msg_iov = &mut io_slice;
        header.msg_iovlen = 1; // Since we are senfing only one iovec

        /* Fd Data */
        let mut control_data: Vec<u8> = Vec::with_capacity(unsafe { CMSG_SPACE(size_of::<i32>() as u32 /* i32 for fd */) } as usize);

        header.msg_control = control_data.as_mut_ptr() as *mut libc::c_void;
        header.msg_controllen = control_data.capacity();

        /* Here it comes, the data is coming */
        unsafe {
            let mut ret = libc::recvmsg(sock.as_raw_fd(), &mut header, 0);
            // The end is never the end is never the end... until it is
            while ret <= 0 {
                ret = libc::recvmsg(sock.as_raw_fd(), &mut header, 0);
            }

            data_buf.set_len(ret as usize);
        };

        /* "Parse" the data */
        let mut msg = Msg::from(&data_buf);

        //dbg!(&data_buf);
        //TODO: Check if we didnt sen a fd
        if control_data.len() >= 1 {
            let chdr = unsafe { CMSG_FIRSTHDR(&header) };

            // Check if we got trolled
            if unsafe { (*chdr).cmsg_type } != SCM_RIGHTS {
                panic!("Error getting message"); // TODO: Better message
            }

            let fd = unsafe { *(CMSG_DATA(chdr) as *mut i32) };
            msg.fd = Some(fd);
        }


        return msg;
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
            dbg!("Started Recieving");
            let msg = Msg::recieve(&sock);
            dbg!("Recieved");

            match msg.msg_type {
                MsgType::Open => {
                    dbg!(&msg.path);
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
                        let msg = Msg { msg_type: MsgType::Open, fd: Some(fd), path: "Test".to_string() };
                        dbg!("Sending msg");
                        unsafe { libc::sleep(1) };
                        msg.send(&sock);
                        dbg!("Message sent");
                    } else {
                        // ? Send close
                        let msg = Msg { msg_type: MsgType::Open, fd: None, path: "Test".to_string() };
                        msg.send(&sock);
                    }

                    //unsafe { libc::sleep(10) };
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