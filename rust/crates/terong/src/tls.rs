use rustls::{RootCertStore, server::WebPkiClientVerifier};
use rustls_pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use std::sync::Arc;
use tokio_rustls::{TlsAcceptor, TlsConnector};

pub fn create_tls_acceptor(server_cert: &[u8], server_key: &[u8], root_cert: &[u8]) -> TlsAcceptor {
    let mut root_store = RootCertStore::empty();
    root_store
        .add(CertificateDer::from_pem_slice(root_cert).unwrap())
        .unwrap();
    let config = Arc::new(
        rustls::ServerConfig::builder()
            .with_client_cert_verifier(WebPkiClientVerifier::builder(Arc::new(root_store)).build().unwrap())
            .with_single_cert(
                vec![
                    CertificateDer::from_pem_slice(server_cert).unwrap(),
                    CertificateDer::from_pem_slice(root_cert).unwrap(),
                ],
                PrivateKeyDer::from_pem_slice(server_key).unwrap(),
            )
            .unwrap(),
    );
    TlsAcceptor::from(config)
}

pub fn create_tls_connector(client_cert: &[u8], client_key: &[u8], root_cert: &[u8]) -> TlsConnector {
    let mut root_store = RootCertStore::empty();
    root_store
        .add(CertificateDer::from_pem_slice(root_cert).unwrap())
        .unwrap();
    let config = Arc::new(
        rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_client_auth_cert(
                vec![
                    CertificateDer::from_pem_slice(client_cert).unwrap(),
                    CertificateDer::from_pem_slice(root_cert).unwrap(),
                ],
                PrivateKeyDer::from_pem_slice(client_key).unwrap(),
            )
            .unwrap(),
    );
    TlsConnector::from(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{DistinguishedName, DnType, KeyPair, SanType, date_time_ymd};
    use rustls_pki_types::ServerName;
    use std::{error::Error, str::FromStr};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::oneshot,
    };

    #[ignore = "pointless(23/05/2025):
    This test was written with the idea that the TLSs subsystem is taking and validating its peer certificates. But today I learn:
        1. Rustls (its WebPKI subsystem) refuse to accept CA cert as end entity cert.
        2. The _more correct_ way is to have a root cert, sign the peer certs with it, and validates the peer certs using the root cert.
    todo(kfj): update the test to reflect that"]
    #[tokio::test]
    async fn test_with_valid_client_cert() {
        let (server_cert, server_key) = gen_cert_key_pair();
        let (client_cert, client_key) = gen_cert_key_pair();

        let (port_tx, port_rx) = oneshot::channel();

        let server = {
            let server_cert = server_cert.clone();
            let client_cert = client_cert.clone();
            tokio::spawn(async move {
                run_server(
                    port_tx,
                    server_cert.as_bytes(),
                    server_key.as_bytes(),
                    client_cert.as_bytes(),
                )
                .await
            })
        };

        let port = port_rx.await.unwrap();

        run_client(
            port,
            client_cert.as_bytes(),
            client_key.as_bytes(),
            server_cert.as_bytes(),
        )
        .await
        .unwrap();

        assert_eq!(server.await.unwrap().unwrap(), b"hello");
    }

    #[ignore = "pointless(23/05/2025):
    This test was written with the idea that the TLSs subsystem is taking and validating its peer certificates. But today I learn:
        1. Rustls (its WebPKI subsystem) refuse to accept CA cert as end entity cert.
        2. The _more correct_ way is to have a root cert, sign the peer certs with it, and validates the peer certs using the root cert.
    todo(kfj): update the test to reflect that"]
    #[tokio::test]
    async fn test_with_invalid_client_cert() {
        let (server_cert, server_key) = gen_cert_key_pair();
        let (client_cert, _) = gen_cert_key_pair();

        let (port_tx, port_rx) = oneshot::channel();

        let server = {
            let server_cert = server_cert.clone();
            let client_cert = client_cert.clone();
            tokio::spawn(async move {
                run_server(
                    port_tx,
                    server_cert.as_bytes(),
                    server_key.as_bytes(),
                    client_cert.as_bytes(),
                )
                .await
            })
        };

        let port = port_rx.await.unwrap();

        let (client_cert, client_key) = gen_cert_key_pair();
        run_client(
            port,
            client_cert.as_bytes(),
            client_key.as_bytes(),
            server_cert.as_bytes(),
        )
        .await
        .unwrap();

        let err = server.await.unwrap().unwrap_err();
        assert_eq!(err.to_string(), "invalid peer certificate: BadSignature");
        let err = err
            .downcast::<std::io::Error>()
            .unwrap()
            .into_inner()
            .unwrap()
            .downcast::<rustls::Error>()
            .unwrap();
        assert_eq!(
            *err,
            rustls::Error::InvalidCertificate(rustls::CertificateError::BadSignature)
        );
    }

    #[tokio::test]
    async fn test_with_invalid_server_cert() {
        let (server_cert, _) = gen_cert_key_pair();
        let (client_cert, client_key) = gen_cert_key_pair();

        let (port_tx, port_rx) = oneshot::channel();

        let _server = {
            let (server_cert, server_key) = gen_cert_key_pair();
            let client_cert = client_cert.clone();
            tokio::spawn(async move {
                run_server(
                    port_tx,
                    server_cert.as_bytes(),
                    server_key.as_bytes(),
                    client_cert.as_bytes(),
                )
                .await
            })
        };

        let port = port_rx.await.unwrap();

        let err = run_client(
            port,
            client_cert.as_bytes(),
            client_key.as_bytes(),
            server_cert.as_bytes(),
        )
        .await
        .unwrap_err();
        assert_eq!(err.to_string(), "invalid peer certificate: BadSignature");
        let err = err
            .downcast::<std::io::Error>()
            .unwrap()
            .into_inner()
            .unwrap()
            .downcast::<rustls::Error>()
            .unwrap();
        assert_eq!(
            *err,
            rustls::Error::InvalidCertificate(rustls::CertificateError::BadSignature)
        );
    }

    fn gen_cert_key_pair() -> (String, String) {
        let mut params = rcgen::CertificateParams::default();
        params.not_before = date_time_ymd(2025, 1, 1);
        params.not_after = date_time_ymd(2027, 1, 1);
        params.distinguished_name = DistinguishedName::new();

        params.distinguished_name.push(DnType::CountryName, "ID");
        params.distinguished_name.push(DnType::OrganizationName, "Example");

        params
            .subject_alt_names
            .push(SanType::IpAddress(std::net::IpAddr::from_str("127.0.0.1").unwrap()));

        let key_pair = KeyPair::generate().unwrap();
        let cert = params.self_signed(&key_pair).unwrap();
        let cert = cert.pem();
        let key = key_pair.serialize_pem();
        (cert, key)
    }

    async fn run_server(
        port: oneshot::Sender<u16>,
        server_cert: &[u8],
        server_key: &[u8],
        client_cert: &[u8],
    ) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let server_addr = listener.local_addr()?;
        port.send(server_addr.port()).ok();
        let (stream, _) = listener.accept().await?;
        let acceptor = create_tls_acceptor(server_cert, server_key, client_cert);
        let mut stream = acceptor.accept(stream).await?;
        let buf = &mut [0; 5];
        stream.read_exact(buf).await?;
        Ok(buf.to_vec())
    }

    async fn run_client(
        port: u16,
        client_cert: &[u8],
        client_key: &[u8],
        server_cert: &[u8],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let server_addr = format!("127.0.0.1:{}", port);
        let stream = TcpStream::connect(&server_addr).await?;
        let connector = create_tls_connector(client_cert, client_key, server_cert);
        let mut stream = connector
            .connect(
                ServerName::IpAddress(rustls_pki_types::IpAddr::try_from("127.0.0.1")?),
                stream,
            )
            .await?;
        stream.write_all(b"hello").await?;
        Ok(())
    }
}
