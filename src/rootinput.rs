use std::{io::{IoSliceMut, Read, IoSlice}, mem::size_of, ptr::null_mut, ffi::CString, fs::File, os::unix::{prelude::{FromRawFd, AsRawFd}, net::{UnixDatagram, SocketAncillary, AncillaryData}}};
use libc::{SOCK_SEQPACKET, fork, pid_t, CMSG_SPACE, c_void, CMSG_FIRSTHDR, SOL_SOCKET, SCM_RIGHTS, CMSG_LEN, CMSG_DATA, O_NONBLOCK, O_RDONLY, O_CLOEXEC, O_NOCTTY};
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
    pub fn send(&self, sock: &UnixDatagram) -> usize {
        dbg!("Got Message To Send", &self.path, self.fd);

        /* Normal Data */
        let data_buf: Vec<u8> = self.into();
        let data_slice = IoSlice::new(&data_buf);

        /* Fd Data */
        /*
            This needs to be in the control data
            for the os to understand to share the file descriptor
        */
        let mut cdata_buf = [0u8; 48]; // ? This should be enough
        let mut cdata_ancilliary = SocketAncillary::new(&mut cdata_buf[..]);
        if self.fd.is_some() {
            cdata_ancilliary.add_fds(&[self.fd.unwrap()]);
        }

        // Send it
        sock.send_vectored_with_ancillary(&[data_slice], &mut cdata_ancilliary).unwrap() // This should be blocking
    }

    /**
     * Warning: This is supposed to be blocking
     */
    pub fn recieve(sock: &UnixDatagram) -> Msg {
        /* Normal data */
        let mut data_buf = [0u8; 24]; // ? This Should be enough
        let data_slice = IoSliceMut::new(&mut data_buf);

        /* Fd Data */
        let mut cdata_buf = [0u8; 48]; // ? This should be enough
        let mut cdata_ancilliary = SocketAncillary::new(&mut cdata_buf);

        /* Recieve The Message */
        let (data_size, _) = sock.recv_vectored_with_ancillary(&mut [data_slice], &mut cdata_ancilliary).unwrap();

        /* "Parse" the data */
        // Msg
        let mut msg = Msg::from(&data_buf[..data_size].to_vec());

        // Fd
        if !cdata_ancilliary.is_empty() {
            let data = cdata_ancilliary.messages().next().unwrap().unwrap();
            if let AncillaryData::ScmRights(mut scm_rights) = data {
                let fd = scm_rights.next().unwrap();
                msg.fd = Some(fd);
            }
        }

        return msg;
    }
}

pub struct RootInput {
    //root_sock: UnixDatagram,
    pub user_sock: UnixDatagram,
    root_pid: pid_t
}

impl RootInput {
    pub fn start(devpath: &str) -> RootInput {
        if !is_root() {
            panic!("Not running as root");
        }

        //Creates Socks
        let (user_sock, root_sock) = UnixDatagram::pair().unwrap();

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
        drop_root();

        return Self {user_sock, root_pid: child};
    }

    pub fn open(user_sock: i32, path: &str) -> Result<i32, i32> {
        let user_sock = unsafe { UnixDatagram::from_raw_fd(user_sock) };
        let msg = Msg { msg_type: MsgType::Open, fd: None, path: path.to_string() };
        msg.send(&user_sock);

        // ? Do we need to retry
        let new_msg = Msg::recieve(&user_sock);

        //TODO: Error handling
        if new_msg.fd.is_some() {
            Ok(new_msg.fd.unwrap() as i32)
        } else {
            Err(2)
        }
    }

    /**
     * This runs as root
     */
    fn run(sock: UnixDatagram, devpath: &str) {
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
                    let path_c = CString::new(msg.path).unwrap();
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