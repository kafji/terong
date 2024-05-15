package terong

import "flag"

type Args struct {
	ConfigFile string
}

func ParseArgs() Args {
	var configFile = flag.String("config-file", ".", "set file path for config file")
	flag.Parse()
	a := Args{ConfigFile: *configFile}
	return a
}

type Config struct {
	Port uint16
}

func ReadConfig(filePath string) (Config, error) {
	c := Config{}
	if c.Port == 0 {
		c.Port = 59001
	}
	return c, nil
}
