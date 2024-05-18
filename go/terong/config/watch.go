//go:build linux

package config

import (
	"context"
	"fmt"
	"os"
	"sync"
	"unsafe"

	"golang.org/x/sys/unix"
	"kafji.net/terong/logging"
)

var slog = logging.NewLogger("config")

type Watcher struct {
	mu  sync.Mutex
	err error

	changed chan struct{}
}

func (w *Watcher) Changed() <-chan struct{} {
	return w.changed
}

func (w *Watcher) Error() error {
	w.mu.Lock()
	defer w.mu.Unlock()
	return w.err
}

func Watch(ctx context.Context) *Watcher {
	w := &Watcher{changed: make(chan struct{})}

	go func() {
		defer close(w.changed)

		fd, err := unix.InotifyInit()
		if err != nil {
			w.mu.Lock()
			defer w.mu.Unlock()
			w.err = fmt.Errorf("failed to initialize inotify: %v", err)
			return
		}

		f := os.NewFile(uintptr(fd), filePath)
		defer f.Close()

		slog.Info("watching for changes", "path", filePath)
		wd, err := unix.InotifyAddWatch(fd, filePath, unix.IN_CLOSE_WRITE)
		if err != nil {
			w.mu.Lock()
			defer w.mu.Unlock()
			w.err = fmt.Errorf("failed to add config file to watch list: %v", err)
			return
		}
		defer unix.InotifyRmWatch(fd, uint32(wd))

		for {
			select {
			case <-ctx.Done():
				w.mu.Lock()
				defer w.mu.Unlock()
				w.err = fmt.Errorf("cancelled: %v", err)
				return
			default:
			}

			buf := make([]byte, unix.SizeofInotifyEvent+unix.NAME_MAX+1)
			n, err := f.Read(buf)
			if n == 0 && err != nil {
				w.mu.Lock()
				defer w.mu.Unlock()
				w.err = fmt.Errorf("failed to read inotify event: %v", err)
				return
			}
			buf = buf[:n]

			event := (*unix.InotifyEvent)(unsafe.Pointer(&buf[0]))
			if event.Mask != unix.IN_CLOSE_WRITE {
				continue
			}
			slog.Debug("event", "event", event)
			w.changed <- struct{}{}
		}
	}()

	return w
}
