package logging

import (
	"fmt"
	"log/slog"
)

var Filter func(namespace string) bool = func(namespace string) bool { return true }

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

func (l *logger) filterMap(msg string, args []any) (string, []any, bool) {
	if !Filter(l.namespace) {
		return "", nil, false
	}
	return fmt.Sprintf("%s: %s", l.namespace, msg), args, true
}

func (l *logger) Debug(msg string, args ...any) {
	msg, args, ok := l.filterMap(msg, args)
	if !ok {
		return
	}
	slog.Debug(msg, args...)
}

func (l *logger) Info(msg string, args ...any) {
	msg, args, ok := l.filterMap(msg, args)
	if !ok {
		return
	}
	slog.Info(msg, args...)
}

func (l *logger) Warn(msg string, args ...any) {
	msg, args, ok := l.filterMap(msg, args)
	if !ok {
		return
	}
	slog.Warn(msg, args...)
}

func (l *logger) Error(msg string, args ...any) {
	msg, args, ok := l.filterMap(msg, args)
	if !ok {
		return
	}
	slog.Error(msg, args...)
}
