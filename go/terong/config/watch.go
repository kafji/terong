package config

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"unsafe"

	"golang.org/x/sys/unix"
)

func Watch(ctx context.Context) <-chan any {
	msgs := make(chan any)

	go func() {
		fd, err := unix.InotifyInit()
		if err != nil {
			msgs <- fmt.Errorf("failed to initialize inotify: %v", err)
			return
		}

		f := os.NewFile(uintptr(fd), filePath)
		defer f.Close()

		slog.Info("watching for changes", "path", filePath)
		wd, err := unix.InotifyAddWatch(fd, filePath, unix.IN_CLOSE_WRITE)
		if err != nil {
			msgs <- fmt.Errorf("failed to add to watch list: %v", err)
			return
		}
		defer unix.InotifyRmWatch(fd, uint32(wd))

	loop:
		for {
			select {
			case <-ctx.Done():
				break loop
			default:
			}

			buf := make([]byte, unix.SizeofInotifyEvent+unix.NAME_MAX+1)
			n, err := f.Read(buf)
			if n == 0 && err != nil {
				msgs <- fmt.Errorf("failed to read inotify event: %v", err)
			}
			buf = buf[:n]

			event := (*unix.InotifyEvent)(unsafe.Pointer(&buf[0]))

			if event.Mask != unix.IN_CLOSE_WRITE {
				continue
			}

			msgs <- struct{}{}
		}
	}()

	return msgs
}
