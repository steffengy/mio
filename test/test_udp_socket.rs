use mio::{Events, Poll, PollOpt, Ready, Token};
use mio::net::UdpSocket;
use bytes::BufMut;
use std::io::ErrorKind;
use std::str;
use std::time;
use localhost;

const LISTENER: Token = Token(0);
const SENDER: Token = Token(1);

pub struct UdpHandlerSendRecv {
    tx: UdpSocket,
    rx: UdpSocket,
    msg: &'static str,
    buf: &'static [u8],
    rx_buf: Vec<u8>,
    connected: bool,
    shutdown: bool,
}

impl UdpHandlerSendRecv {
    fn new(tx: UdpSocket, rx: UdpSocket, connected: bool, msg : &'static str) -> UdpHandlerSendRecv {
        UdpHandlerSendRecv {
            tx: tx,
            rx: rx,
            msg: msg,
            buf: msg.as_bytes(),
            rx_buf: Vec::with_capacity(1024),
            connected: connected,
            shutdown: false,
        }
    }
}

fn assert_send<T: Send>() {
}

fn assert_sync<T: Sync>() {
}

#[cfg(test)]
fn test_send_recv_udp(tx: UdpSocket, rx: UdpSocket, connected: bool) {
    debug!("Starting TEST_UDP_SOCKETS");
    let mut poll = Poll::new().unwrap();

    assert_send::<UdpSocket>();
    assert_sync::<UdpSocket>();

    // ensure that the sockets are non-blocking
    let mut buf = [0; 128];
    assert_eq!(ErrorKind::WouldBlock, rx.recv_from(&mut buf).unwrap_err().kind());

    info!("Registering SENDER");
    poll.register().register(&tx, SENDER, Ready::writable(), PollOpt::edge()).unwrap();

    info!("Registering LISTENER");
    poll.register().register(&rx, LISTENER, Ready::readable(), PollOpt::edge()).unwrap();

    let mut events = Events::with_capacity(1024);

    info!("Starting event loop to test with...");
    let mut handler = UdpHandlerSendRecv::new(tx, rx, connected, "hello world");

    while !handler.shutdown {
        poll.poll(&mut events, None).unwrap();

        for event in &events {
            if event.readiness().is_readable() {
                match event.token() {
                    LISTENER => {
                        debug!("We are receiving a datagram now...");
                        let cnt = unsafe {
                            if !handler.connected {
                                handler.rx.recv_from(handler.rx_buf.bytes_mut()).unwrap().0
                            } else {
                                handler.rx.recv(handler.rx_buf.bytes_mut()).unwrap()
                            }
                        };

                        unsafe { BufMut::advance_mut(&mut handler.rx_buf, cnt); }
                        assert!(str::from_utf8(handler.rx_buf.as_ref()).unwrap() == handler.msg);
                        handler.shutdown = true;
                    },
                    _ => ()
                }
            }

            if event.readiness().is_writable() {
                match event.token() {
                    SENDER => {
                        let cnt = if !handler.connected {
                            let addr = handler.rx.local_addr().unwrap();
                            handler.tx.send_to(handler.buf, &addr).unwrap()
                        } else {
                            handler.tx.send(handler.buf).unwrap()
                        };

                        handler.buf = &handler.buf[cnt..];
                    },
                    _ => {}
                }
            }
        }
    }
}

#[test]
pub fn test_udp_socket() {
    let addr = localhost();
    let any = localhost();

    let tx = UdpSocket::bind(&any).unwrap();
    let rx = UdpSocket::bind(&addr).unwrap();

    test_send_recv_udp(tx, rx, false);
}

#[test]
pub fn test_udp_socket_send_recv() {
    let addr = localhost();
    let any = localhost();

    let tx = UdpSocket::bind(&any).unwrap();
    let rx = UdpSocket::bind(&addr).unwrap();

    let tx_addr = tx.local_addr().unwrap();
    let rx_addr = rx.local_addr().unwrap();

    assert!(tx.connect(&rx_addr).is_ok());
    assert!(rx.connect(&tx_addr).is_ok());

    test_send_recv_udp(tx, rx, true);
}

#[test]
pub fn test_udp_socket_discard() {
    let addr = localhost();
    let any = localhost();
    let outside = localhost();

    let tx = UdpSocket::bind(&any).unwrap();
    let rx = UdpSocket::bind(&addr).unwrap();
    let udp_outside = UdpSocket::bind(&outside).unwrap();

    let tx_addr = tx.local_addr().unwrap();
    let rx_addr = rx.local_addr().unwrap();

    assert!(tx.connect(&rx_addr).is_ok());
    assert!(udp_outside.connect(&rx_addr).is_ok());
    assert!(rx.connect(&tx_addr).is_ok());

    let mut poll = Poll::new().unwrap();

    let r = udp_outside.send("hello world".as_bytes());
    assert!(r.is_ok() || r.unwrap_err().kind() == ErrorKind::WouldBlock);

    poll.register().register(&rx, LISTENER, Ready::readable(), PollOpt::edge()).unwrap();
    poll.register().register(&tx, SENDER, Ready::writable(), PollOpt::edge()).unwrap();

    let mut events = Events::with_capacity(1024);

    poll.poll(&mut events, Some(time::Duration::from_secs(5))).unwrap();

    for event in &events {
        if event.readiness().is_readable() {
            match event.token() {
                LISTENER => {
                    assert!(false, "Expected to no receive a packet but got something")
                },
                _ => ()
            }
        }
    }
}
