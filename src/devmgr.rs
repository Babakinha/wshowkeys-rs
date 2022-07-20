use std::{ptr::{null_mut, null}, mem::size_of, process::exit};

use errno::set_errno;

use libc::{getuid, socketpair, AF_UNIX, SOCK_SEQPACKET, c_int, fork, pid_t, close, size_t, iovec, c_void, msghdr, CMSG_SPACE, ssize_t, recvmsg, MSG_CMSG_CLOEXEC, CMSG_FIRSTHDR, memcpy, strstr, O_RDONLY, O_CLOEXEC, O_NOCTTY, O_NONBLOCK, open, SOL_SOCKET, SCM_RIGHTS, CMSG_LEN, CMSG_DATA, sendmsg, EINTR, waitpid, ENOTSOCK};
use crate::root_utils::{drop_root, can_root};
/**
 * I Think this all is  a way to create a thread and send msgs in between
 */
pub fn devmgr_start(devpath: &str) -> (i32, pid_t) {
    unsafe {
        if getuid() != 0 {
            eprintln!("Not running as root (probably not gonna work)");
            //TODO: Panic
        }

        // ? Create sock
        let mut sock: [i32; 2] = [0; 2];
        let sock_ptr = sock.as_mut_ptr() as *mut c_int;
        let res = socketpair(AF_UNIX, SOCK_SEQPACKET, 0, sock_ptr);
        if res < 0 {
            eprintln!("devmgr: socketpair: {}", errno::errno());
            //TODO: Panic
        }

        // ? Create fork
        let child: pid_t = fork();
        if child < 0 {
            eprintln!("devmgr: fork: {}", errno::errno());
            close(sock[0]);
            close(sock[1]);
            //TODO: Panic
        }else if child == 0 {
            close(sock[0]);
            devmgr_run(sock[1], devpath) /* Does not return */
        }
        close(sock[1]);

        //TODO: drop to user-specified uid (drop_root_to)
        //TODO: user-specified dont drop root
        drop_root();

        println!("Server: {}, Client: {}", sock[1], sock[0]);
        return (sock[0], child);

    }
}

pub fn devmgr_run(sockfd: i32, devpath: &str) {
    let mut msg: Msg = Msg { msg_type: MsgType::MsgOpen, path: "".to_string() };
    let mut fdin: i32 = -1;
    let mut running: bool = true;

    println!("Server: sock {}, recieving all messages", sockfd);
    while running && recv_msg(sockfd, &mut fdin, &mut msg as *mut _ as *mut c_void, size_of::<Msg>()) > 0 {
        match msg.msg_type {
            MsgType::MsgOpen => {
                set_errno(errno::Errno(0));
                //TODO: Make it rust
                // ? Does this work
                if unsafe {strstr(msg.path.as_ptr() as *const i8, devpath.as_ptr() as *const i8)} != msg.path.as_mut_ptr() as *mut i8 {
                    /* Hecker detected! */
                    exit(1);
                }
                let fd = unsafe {open(msg.path.as_ptr() as *const i8, O_RDONLY|O_CLOEXEC|O_NOCTTY|O_NONBLOCK)};
                let mut ret = errno::errno().0;
                send_msg(sockfd, if ret != 0 { -1 } else { fd }, &mut ret as *mut _ as *mut c_void, size_of::<i32>());
                if fd >= 0 {
                    unsafe { close(fd) };
                }
                break;
            },

            MsgType::MsgEnd => {
                running = false;
                send_msg(sockfd, -1, null_mut(), 0);
                break;
            }
        };
    }

    exit(0);
}

pub fn devmgr_open(sockfd: i32, path: String) -> Result<i32, i32> {
    let mut msg: Msg = Msg { msg_type: MsgType::MsgOpen, path: path };

    dbg!(errno::errno());
    send_msg(sockfd, -1, &mut msg as *mut _ as *mut c_void, size_of::<Msg>());
    dbg!(errno::errno());


    let mut fd: i32 = 0;
    let mut err: i32 = 0;
    let mut ret: ssize_t = 0;
    let mut retry: i32 = 0;

    //while ret == 0 && {retry += 1; retry} < 3 {
    ret = recv_msg(sockfd, &mut fd, &mut err as *mut _ as *mut c_void, size_of::<i32>());
    if sockfd == 3 {
    }

    if err != 0 { Err(-err) } else { Ok(fd) }
}

pub fn devmgr_finish(sock: i32, pid: pid_t) {
    let mut msg: Msg = Msg { msg_type: MsgType::MsgEnd, path: "".to_string() };

    send_msg(sock, -1, &mut msg as *mut _ as *mut c_void, size_of::<Msg>());
    recv_msg(sock, null_mut(), null_mut(), 0);

    unsafe {
        waitpid(pid, null_mut(), 0);
        close(sock);
    };
}

/* Msg stuff */
enum MsgType {
    MsgOpen,
    MsgEnd
}

struct Msg {
    msg_type: MsgType,
    path: String
}

fn recv_msg(sock: i32, fd_out: *mut i32, buf: *mut c_void, buf_len: size_t) -> ssize_t {
    let mut control: Vec<char> = Vec::with_capacity(unsafe {CMSG_SPACE(size_of::<i32>() as u32)} as usize);

    let mut iovec: iovec = iovec { iov_base: buf, iov_len: buf_len };
    let mut msghdr: msghdr = msghdr {
        msg_name: null_mut(),
        msg_namelen: 0,
        msg_iov: &mut iovec,
        msg_iovlen: 1,
        msg_control: null_mut(),
        msg_controllen: 0,
        msg_flags: 0
    };

    if fd_out != null_mut() {
        msghdr.msg_control = control.as_mut_ptr() as *mut c_void;
        msghdr.msg_controllen = control.capacity();
    }

    //println!("Recieving Message:\n  sock: {}", sock);
    //println!("Current Message: {:#?}", msghdr);
    let mut ret: ssize_t = unsafe { recvmsg(sock, &mut msghdr, MSG_CMSG_CLOEXEC) };
    while ret < 0 && errno::errno().0 == EINTR {
        ret = unsafe { recvmsg(sock, &mut msghdr, MSG_CMSG_CLOEXEC) };
    }
    println!("Recieved Message:\n   ret: {}\n   sock: {}\n  errno:{}", ret, sock, errno::errno());
    //println!("Updated Message: {:#?}", msghdr);

    if fd_out != null_mut() {
        let cmsg = unsafe {CMSG_FIRSTHDR(&msghdr)};
        if cmsg != null_mut() {
            unsafe { memcpy(
                fd_out as *mut _ as *mut c_void,
                CMSG_DATA(cmsg) as *const _ as *const c_void,
                size_of::<i32>()
            );};
        }else {
            unsafe { *fd_out = -1 };
        }
    }

    return ret;
}

pub fn send_msg(sock: i32, fd: i32, buf: *mut c_void, buf_len: size_t) {
    let mut control: Vec<char> = Vec::with_capacity(unsafe {CMSG_SPACE(size_of::<i32>() as u32)} as usize);

    let mut iovec: iovec = iovec { iov_base: buf, iov_len: buf_len };
    let mut msghdr: msghdr = msghdr {
        msg_name: null_mut(),
        msg_namelen: 0,
        msg_iov: &mut iovec,
        msg_iovlen: 1,
        msg_control: null_mut(),
        msg_controllen: 0,
        msg_flags: 0
    };

    if fd >= 0 {
        msghdr.msg_control = control.as_mut_ptr() as *mut c_void;
        msghdr.msg_controllen = control.capacity();

        let cmsg = unsafe {CMSG_FIRSTHDR(&msghdr)};

        unsafe {
            (*cmsg).cmsg_level = SOL_SOCKET;
            (*cmsg).cmsg_type = SCM_RIGHTS;
            (*cmsg).cmsg_len = CMSG_LEN(size_of::<i32>() as u32) as usize;

            memcpy(CMSG_DATA(cmsg) as *mut c_void, &fd as *const _ as *const c_void, size_of::<i32>());
        };
    }

    let mut ret: ssize_t = unsafe { sendmsg(sock, &mut msghdr, 0) }; // ENOTSOCK
    // ! 100% cpu
    while ret < 0 && errno::errno().0 == EINTR {
        dbg!("SENDING A LOT");
        ret = unsafe { sendmsg(sock, &mut msghdr, 0) };
    }
}