use std::{
    io::ErrorKind,
    net::{SocketAddr, TcpListener, UdpSocket},
};
use thiserror::Error;

mod private {
    use super::*;

    pub trait Sealed {}
    impl Sealed for Tcp {}
    impl Sealed for Udp {}
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to find usable port after {0} attempts")]
    Exhausted(usize),
}

#[derive(Debug)]
pub struct Tcp(TcpListener);

#[derive(Debug)]
pub struct Udp(UdpSocket);

#[derive(Debug)]
pub struct ReservedPort<T: Reserve> {
    number: u16,
    _res: T,
}

impl<T: Reserve> ReservedPort<T> {
    /// Returns the port number and release its reservation.
    #[inline]
    pub fn take(self) -> u16 {
        self.number
    }

    /// Returns the port number without releasing its reservation.
    #[inline]
    pub fn peek(&self) -> u16 {
        self.number
    }
}

impl<T: Reserve> AsRef<u16> for ReservedPort<T> {
    fn as_ref(&self) -> &u16 {
        &self.number
    }
}

pub trait Reserve: private::Sealed + Sized {
    /// Attempt to reserve port.
    ///
    /// Returns `None` on reserve, `Some` otherwise.
    fn reserve(port: u16) -> Option<ReservedPort<Self>>;
}

impl Reserve for Tcp {
    fn reserve(port: u16) -> Option<ReservedPort<Self>> {
        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        match TcpListener::bind(addr) {
            Ok(res) => ReservedPort {
                number: res.local_addr().unwrap().port(),
                _res: Tcp(res),
            }
            .into(),
            Err(x) if x.kind() == ErrorKind::AddrInUse => None,
            Err(x) => panic!("{}", x),
        }
    }
}

impl Reserve for Udp {
    fn reserve(port: u16) -> Option<ReservedPort<Self>> {
        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        match UdpSocket::bind(addr) {
            Ok(res) => ReservedPort {
                number: res.local_addr().unwrap().port(),
                _res: Udp(res),
            }
            .into(),
            Err(x) if x.kind() == ErrorKind::AddrInUse => None,
            Err(x) => panic!("{}", x),
        }
    }
}

pub trait ProducePort {
    fn get_port(&mut self) -> u16;
    fn length(&self) -> usize;
}

impl<T> ProducePort for T
where
    T: ExactSizeIterator<Item = u16>,
{
    fn get_port(&mut self) -> u16 {
        self.next().unwrap()
    }

    fn length(&self) -> usize {
        self.len()
    }
}

impl ProducePort for Singleton {
    fn get_port(&mut self) -> u16 {
        self.0
    }

    fn length(&self) -> usize {
        1
    }
}

pub fn reserve_port<T, P>(mut port_producer: P) -> Result<ReservedPort<T>, Error>
where
    T: Reserve,
    P: ProducePort,
{
    let ports_count = port_producer.length();
    let mut attempts = 0;
    let port = loop {
        if attempts >= ports_count {
            return Err(Error::Exhausted(attempts));
        }
        let port = port_producer.get_port();
        match T::reserve(port) {
            Some(x) => break x,
            None => (),
        }
        attempts += 1;
    };
    Ok(port)
}

struct Singleton(u16);

/// Reserves random UDP port from OS.
#[inline]
pub fn reserve_udp_port() -> ReservedPort<Udp> {
    reserve_port::<Udp, _>(Singleton(0)).unwrap()
}

/// Reserves random TCP port from OS.
#[inline]
pub fn reserve_tcp_port() -> ReservedPort<Tcp> {
    reserve_port::<Tcp, _>(Singleton(0)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_for_udp() {
        let port_1 = reserve_udp_port();
        let port_2 = reserve_udp_port().take();

        let port = reserve_port::<Udp, _>([port_1.peek(), port_2].into_iter()).unwrap();

        assert_eq!(port.peek(), port_2)
    }

    #[test]
    fn test_retry_for_tcp() {
        let port_1 = reserve_tcp_port();
        let port_2 = reserve_tcp_port().take();

        let port = reserve_port::<Tcp, _>([port_1.peek(), port_2].into_iter()).unwrap();

        assert_eq!(port.peek(), port_2)
    }

    #[test]
    fn test_maximum_retries_for_udp() {
        let port = reserve_udp_port();

        let error = reserve_port::<Udp, _>(Singleton(port.peek())).unwrap_err();

        assert_eq!(
            error.to_string(),
            "failed to find usable port after 1 attempts"
        );
    }

    #[test]
    fn test_maximum_retries_for_tcp() {
        let port = reserve_tcp_port();

        let error = reserve_port::<Tcp, _>(Singleton(port.peek())).unwrap_err();

        assert_eq!(
            error.to_string(),
            "failed to find usable port after 1 attempts"
        );
    }
}
