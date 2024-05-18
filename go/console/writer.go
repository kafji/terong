package console

import (
	"fmt"
	"io"
	"os"
)

type writer struct {
	wc chan []byte
}

func (w *writer) Write(p []byte) (int, error) {
	if len(p) == 0 {
		return 0, nil
	}

	if w.wc == nil {
		w.wc = make(chan []byte, 1<<16)
		go func() {
			for {
				b := <-w.wc
				for m := 0; ; {
					n, err := os.Stdout.Write(b[m:])
					if n == 0 {
						panic(fmt.Errorf("failed to write to stdout: %v", err))
					}
					m += n
					if m == len(b) {
						break
					}
				}
			}
		}()
	}

	b := make([]byte, len(p))
	copy(b, p)
	select {
	case w.wc <- b:
	default:
	}

	return len(p), nil
}

var Writer io.Writer = &writer{}
