#[cfg(feature = "get_port")]
mod tests {
    use macross::get_port::{Tcp, Udp, reserve_port, reserve_tcp_port, reserve_udp_port};
    use std::net::{TcpListener, UdpSocket};

    #[test]
    fn test_basic_usage_scenario_for_udp() {
        let port = reserve_udp_port();
        UdpSocket::bind(format!("127.0.0.1:{}", port.take())).unwrap();
    }

    #[test]
    fn test_basic_usage_scenario_for_tcp() {
        let port = reserve_tcp_port();
        TcpListener::bind(format!("127.0.0.1:{}", port.take())).unwrap();
    }

    #[test]
    fn test_from_random() {
        let ports = (0..3).map(|_| fastrand::u16(8000..9000));

        let port = reserve_port::<Udp, _>(ports).unwrap();

        UdpSocket::bind(format!("127.0.0.1:{}", port.take())).unwrap();
    }

    #[test]
    fn test_from_array() {
        let ports = [8000, 8080].into_iter();

        let port = reserve_port::<Tcp, _>(ports).unwrap();

        TcpListener::bind(format!("127.0.0.1:{}", port.take())).unwrap();
    }

    #[test]
    fn test_port_asref_u16() {
        let port = reserve_udp_port();
        let _: &dyn AsRef<u16> = &port;
        assert_eq!(*port.as_ref(), port.peek())
    }
}
