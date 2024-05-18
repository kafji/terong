package config

import (
	"os"

	"github.com/BurntSushi/toml"
)

const filePath = "./terong.toml"

type Config struct {
	LogLevel string `toml:"log_level"`
	Server   Server `toml:"server"`
	Client   Client `toml:"client"`
}

type Server struct {
	Port uint16 `toml:"port"`
}

type Client struct {
	ServerAddr string `toml:"server_addr"`
}

func ReadConfig() (Config, error) {
	var c Config
	file, err := os.ReadFile(filePath)
	if err != nil {
		return Config{}, err
	}
	err = toml.Unmarshal(file, &c)
	if err != nil {
		return Config{}, err
	}
	return c, nil
}
