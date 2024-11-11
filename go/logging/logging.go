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
	return &logger{namespace: namespace}
}

type logger struct {
	namespace string
}

func (l *logger) args(args ...any) []any {
	args2 := append([]any{}, "ns", l.namespace)
	args2 = append(args2, args...)
	return args2
}

func (l *logger) Debug(msg string, args ...any) {
	slog.Debug(msg, l.args(args...)...)
}

func (l *logger) Info(msg string, args ...any) {
	slog.Info(msg, l.args(args...)...)
}

func (l *logger) Warn(msg string, args ...any) {
	slog.Warn(msg, l.args(args...)...)
}

func (l *logger) Error(msg string, args ...any) {
	slog.Error(msg, l.args(args...)...)
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
