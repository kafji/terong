package transport

import (
	"crypto/tls"
	"crypto/x509"
	"fmt"
	"net"
)

func CreateTLSListener(serverCert, serverKey, clientCert []byte) func(net.Listener) net.Listener {
	keyPair, err := tls.X509KeyPair(serverCert, serverKey)
	if err != nil {
		err := fmt.Errorf("failed to parse key pair: %w", err)
		panic(err)
	}

	pool := x509.NewCertPool()
	pool.AppendCertsFromPEM(clientCert)

	cfg := &tls.Config{
		Certificates: []tls.Certificate{keyPair},
		ClientAuth:   tls.RequireAndVerifyClientCert,
		ClientCAs:    pool,
	}

	return func(l net.Listener) net.Listener {
		return tls.NewListener(l, cfg)
	}
}

func CreateTLSDialer(clientCert, clientKey, serverCert []byte) func(*net.Dialer) *tls.Dialer {
	keyPair, err := tls.X509KeyPair(clientCert, clientKey)
	if err != nil {
		err := fmt.Errorf("failed to parse key pair: %w", err)
		panic(err)
	}

	pool := x509.NewCertPool()
	pool.AppendCertsFromPEM(serverCert)

	cfg := &tls.Config{
		Certificates:       []tls.Certificate{keyPair},
		RootCAs:            pool,
		InsecureSkipVerify: true,
		VerifyConnection: func(cs tls.ConnectionState) error {
			opts := x509.VerifyOptions{
				Roots: pool,
			}
			_, err := cs.PeerCertificates[0].Verify(opts)
			if err != nil {
				slog.Debug("failed to verify peer cert", "error", err)
				return err
			}
			return nil
		},
	}

	return func(d *net.Dialer) *tls.Dialer {
		return &tls.Dialer{NetDialer: d, Config: cfg}
	}
}
