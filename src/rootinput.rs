use std::{io::IoSlice, mem::size_of};

use libc::{SOCK_SEQPACKET, fork, pid_t, CMSG_SPACE};
use posix_socket::UnixSocket;

use crate::root_utils::{is_root, drop_root};

/* Msg */

#[derive(PartialEq)]
enum MsgType {
    Open,
    Close
}

struct Msg {
    msg_type: MsgType,
    path: String
}

// Seriliazation and Deseriliazation
impl Into<Vec<u8>> for Msg {
    fn into(self) -> Vec<u8> {
        let mut msg = vec![self.msg_type as u8];
        msg.append(&mut self.path.as_bytes().to_vec());

        return msg;
    }
}

impl From<Vec<u8>> for Msg {
    fn from(vec: Vec<u8>) -> Msg {
        let mut vec = vec.clone(); // TODO: I'm tired
        let msg_type: MsgType = if vec[0] == 0 { MsgType::Open } else { MsgType::Close };
        vec.drain(..1);
        let path = String::from_utf8(vec).unwrap();
        dbg!( MsgType::Open == msg_type, &path);
        Msg { msg_type, path}

    }
}


// Send end Recieve
impl Msg {
    pub fn send(self, sock: UnixSocket, fd: i32) -> usize {

        if fd >= 0 {
            let control: Vec<u8> = Vec::with_capacity(unsafe {CMSG_SPACE(size_of::<i32>() as u32)} as usize);

            let vec: Vec<u8> = self.into();
            sock.send_msg(&[IoSlice::new(&vec)], Some(&control), 0).unwrap()
        } else {
            let vec: Vec<u8> = self.into();
            sock.send_msg(&[IoSlice::new(&vec)], None, 0).unwrap()
        }

    }
}



pub struct RootInput {}

impl RootInput {
    pub fn start() -> (UnixSocket, pid_t) {
        let root_input = Self {};
        if !is_root() {
            panic!("Not running as root");
        }

        //Creates Socks
        let (root_sock, user_sock) = UnixSocket::pair(SOCK_SEQPACKET, 0).unwrap();

        //Create Forks
        let child = unsafe { fork() };

        if child < 0 {
            // Error handling
            drop(root_sock);
            drop(user_sock);
            panic!("Unable to create fork");
        } else if child == 0 {
            // We are the fork
            drop(user_sock);
            RootInput::run(root_sock);
            unsafe { libc::exit(1); };
        }
        //We are not the fork
        drop(root_sock);

        //TODO: drop to user-specified uid (drop_root_to)
        //TODO: user-specified dont drop roo
        drop_root();

        return (user_sock, child);
    }

    fn run(sock: UnixSocket) {
        let mut buf = [0u8; 24]; // ? This should be enough
        let len = sock.recv(&mut buf, 0);
    }
}