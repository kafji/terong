package logging

import (
	"fmt"
	"log/slog"
)

type Logger interface {
	Debug(msg string, args ...any)
	Info(msg string, args ...any)
	Warn(msg string, args ...any)
	Error(msg string, args ...any)
}

func NewLogger(namespace string) Logger {
	return slog.With("ns", namespace)
}

func SetLogLevel(level string) {
	switch level {
	case "debug":
		slog.SetLogLoggerLevel(slog.LevelDebug)
	case "warn":
		slog.SetLogLoggerLevel(slog.LevelWarn)
	case "error":
		slog.SetLogLoggerLevel(slog.LevelError)
	case "info":
		slog.SetLogLoggerLevel(slog.LevelInfo)
	default:
		panic(fmt.Errorf("unexpected log level, was `%s`", level))
	}
}
