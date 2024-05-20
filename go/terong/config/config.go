package config

import (
	"os"

	"github.com/BurntSushi/toml"
	"kafji.net/terong/logging"
)

var slog = logging.NewLogger("config")

const filePath = "./terong.toml"

type Config struct {
	LogLevel string `toml:"log_level"`
	Server   Server `toml:"server"`
	Client   Client `toml:"client"`
}

type Server struct {
	Port              uint16 `toml:"port"`
	TLSCertPath       string `toml:"tls_cert_path"`
	TLSKeyPath        string `toml:"tls_key_path"`
	ClientTLSCertPath string `toml:"client_tls_cert_path"`
}

type Client struct {
	ServerAddr        string `toml:"server_addr"`
	TLSCertPath       string `toml:"tls_cert_path"`
	TLSKeyPath        string `toml:"tls_key_path"`
	ServerTLSCertPath string `toml:"server_tls_cert_path"`
}

func ReadConfig() (*Config, error) {
	var c Config
	file, err := os.ReadFile(filePath)
	if err != nil {
		return nil, err
	}
	err = toml.Unmarshal(file, &c)
	if err != nil {
		return nil, err
	}
	return &c, nil
}
