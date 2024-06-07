package config

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestReadEmptyConfig(t *testing.T) {
	c, err := readConfigString("")
	assert.NoError(t, err)
	require.Equal(t, Config{}, *c)
}

func TestReadLogLevel(t *testing.T) {
	c, err := readConfigString(`log_level = "info"
`)
	assert.NoError(t, err)
	require.Equal(t, Config{LogLevel: "info"}, *c)
}

func TestReadServerConfig(t *testing.T) {
	c, err := readConfigString(`[server]
port = 59001
tls_cert_path = "./server_cert.pem"
tls_key_path = "./server_key.pem"
client_tls_cert_path = "./client_cert.pem"
`)
	assert.NoError(t, err)
	require.Equal(t, Config{Server: Server{
		Port:              59001,
		TLSCertPath:       "./server_cert.pem",
		TLSKeyPath:        "./server_key.pem",
		ClientTLSCertPath: "./client_cert.pem",
	}}, *c)
}

func TestReadClientConfig(t *testing.T) {
	c, err := readConfigString(`[client]
server_addr = "192.168.0.1:59001"
tls_cert_path = "./client_cert.pem"
tls_key_path = "./client_key.pem"
server_tls_cert_path = "./server_cert.pem"
`)
	assert.NoError(t, err)
	require.Equal(t, Config{Client: Client{
		ServerAddr:        "192.168.0.1:59001",
		TLSCertPath:       "./client_cert.pem",
		TLSKeyPath:        "./client_key.pem",
		ServerTLSCertPath: "./server_cert.pem",
	}}, *c)
}
