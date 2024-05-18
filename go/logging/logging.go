package logging

import (
	"fmt"
	"log/slog"
)

type Logger struct {
	namespace string
}

func New(namespace string) *Logger {
	return &Logger{namespace: namespace}
}

func (l *Logger) transform(msg string, args []any) (string, []any) {
	return fmt.Sprintf("%s: %s", l.namespace, msg), args
}

func (l *Logger) Debug(msg string, args ...any) {
	msg, args = l.transform(msg, args)
	slog.Debug(msg, args...)
}

func (l *Logger) Info(msg string, args ...any) {
	msg, args = l.transform(msg, args)
	slog.Info(msg, args...)
}

func (l *Logger) Warn(msg string, args ...any) {
	msg, args = l.transform(msg, args)
	slog.Warn(msg, args...)
}

func (l *Logger) Error(msg string, args ...any) {
	msg, args = l.transform(msg, args)
	slog.Error(msg, args...)
}
