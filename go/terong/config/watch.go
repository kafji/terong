package config

import (
	"context"
	"fmt"

	"github.com/fsnotify/fsnotify"
)

type Watcher struct {
	cfgs chan *Config
	err  error
}

func (w *Watcher) Configs() <-chan *Config {
	return w.cfgs
}

func (w *Watcher) Err() error {
	return w.err
}

func Watch(ctx context.Context) *Watcher {
	w := &Watcher{cfgs: make(chan *Config)}

	go func() {
		defer close(w.cfgs)

		watcher, err := createWatcher()
		if err != nil {
			w.err = fmt.Errorf("failed to create file watcher: %v", err)
			return
		}
		defer watcher.Close()

		for {
			select {
			case <-ctx.Done():
				err := ctx.Err()
				slog.Debug("context error", "error", err)
				w.err = err
				return

			case event, ok := <-watcher.Events:
				if !ok {
					slog.Debug("watcher events closed")
					select {
					case err := <-watcher.Errors:
						w.err = err
					default:
					}
					return
				}
				slog.Debug("watcher event", "event", event)
				if !event.Op.Has(fsnotify.Write) || event.Name != "terong.toml" {
					continue
				}
				slog.Debug("reading config")
				cfg, err := ReadConfig()
				if err != nil {
					slog.Warn("failed to read config", "error", err)
					continue
				}
				slog.Debug("sending config")
				w.cfgs <- cfg
			}
		}
	}()

	return w
}

func createWatcher() (*fsnotify.Watcher, error) {
	watcher, err := fsnotify.NewWatcher()
	if err != nil {
		return nil, err
	}
	err = watcher.Add(".\\terong.toml")
	if err != nil {
		return nil, err
	}

	return watcher, nil
}
