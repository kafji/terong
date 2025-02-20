package transport

import (
	"context"
	"crypto/rand"
	"crypto/rsa"
	"crypto/tls"
	"crypto/x509"
	"encoding/pem"
	"fmt"
	"io"
	"net"
	"net/netip"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
)

func TestWithValidClientCert(t *testing.T) {
	serverCert, serverKey := genCertKeyPair()
	clientCert, clientKey := genCertKeyPair()

	portChan := make(chan int, 1)
	resultChan := make(chan any, 1)
	go func() {
		read, err := runServer(t.Context(), portChan, serverCert, serverKey, clientCert)
		if err != nil {
			resultChan <- err
		} else {
			resultChan <- read
		}
	}()

	port := <-portChan
	err := runClient(t.Context(), port, clientCert, clientKey, serverCert)
	if err != nil {
		panic(err)
	}

	result := <-resultChan
	assert.Equal(t, []byte("hello"), result)
}

func TestWithInvalidClientCert(t *testing.T) {
	serverCert, serverKey := genCertKeyPair()
	clientCert, _ := genCertKeyPair()

	portChan := make(chan int, 1)
	resultChan := make(chan any, 1)
	go func() {
		read, err := runServer(t.Context(), portChan, serverCert, serverKey, clientCert)
		if err != nil {
			resultChan <- err
		} else {
			resultChan <- read
		}
	}()

	port := <-portChan
	clientCert, clientKey := genCertKeyPair()
	err := runClient(t.Context(), port, clientCert, clientKey, serverCert)
	if err != nil {
		panic(err)
	}

	result := <-resultChan
	if err, ok := result.(error); ok {
		tlsErr := &tls.CertificateVerificationError{}
		if !assert.ErrorAs(t, err, &tlsErr) {
			return
		}
	} else {
		t.Error("expecting error result")
	}
}

func TestWithInvalidServerCert(t *testing.T) {
	serverCert, _ := genCertKeyPair()
	clientCert, clientKey := genCertKeyPair()

	portChan := make(chan int, 1)
	resultChan := make(chan any, 1)
	go func() {
		serverCert, serverKey := genCertKeyPair()
		read, err := runServer(t.Context(), portChan, serverCert, serverKey, clientCert)
		if err != nil {
			resultChan <- err
		} else {
			resultChan <- read
		}
	}()

	port := <-portChan
	err := runClient(t.Context(), port, clientCert, clientKey, serverCert)
	tlsErr := x509.UnknownAuthorityError{}
	assert.ErrorAs(t, err, &tlsErr)
}

func genCertKeyPair() ([]byte, []byte) {
	key, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		panic(err)
	}
	template := x509.Certificate{
		NotBefore: time.Date(2025, 1, 1, 0, 0, 0, 0, time.UTC),
		NotAfter:  time.Date(2027, 1, 1, 0, 0, 0, 0, time.UTC),
	}
	cert, err := x509.CreateCertificate(rand.Reader, &template, &template, &key.PublicKey, key)
	if err != nil {
		panic(err)
	}
	certPEM := pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: cert})
	keyDER, err := x509.MarshalPKCS8PrivateKey(key)
	if err != nil {
		panic(err)
	}
	keyPEM := pem.EncodeToMemory(&pem.Block{Type: "PRIVATE KEY", Bytes: keyDER})
	return certPEM, keyPEM
}

func runServer(ctx context.Context, port chan<- int, serverCert, serverKey, clientCert []byte) ([]byte, error) {
	l, err := (&net.ListenConfig{}).Listen(ctx, "tcp4", "127.0.0.1:0")
	if err != nil {
		panic(err)
	}
	addr, err := netip.ParseAddrPort(l.Addr().String())
	if err != nil {
		panic(err)
	}
	port <- int(addr.Port())
	l = CreateTLSListener(serverCert, serverKey, clientCert)(l)
	conn, err := l.Accept()
	if err != nil {
		err := fmt.Errorf("server: failed to accept connection: %w", err)
		return nil, err
	}
	buf := make([]byte, 5)
	n, err := io.ReadFull(conn, buf)
	if err != nil {
		err := fmt.Errorf("server: failed to read: %w", err)
		return nil, err
	}
	buf = buf[:n]
	return buf, nil
}

func runClient(ctx context.Context, port int, clientCert, clientKey, serverCert []byte) error {
	tls := CreateTLSDialer(clientCert, clientKey, serverCert)
	dialer := tls(&net.Dialer{Timeout: 5 * time.Second})
	conn, err := dialer.DialContext(ctx, "tcp4", fmt.Sprintf("127.0.0.1:%d", port))
	if err != nil {
		err := fmt.Errorf("client: failed to dial server: %w", err)
		return err
	}
	_, err = conn.Write([]byte("hello"))
	if err != nil {
		err := fmt.Errorf("client: failed to write hello: %w", err)
		return err
	}
	return nil
}
