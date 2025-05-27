package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"iter"
	"os"
	"sync"
)

func main() {
	var path string
	if len(os.Args) > 1 {
		path = os.Args[1]
	} else {
		fmt.Fprintf(os.Stderr, "usage: %s ...\n", os.Args[0])
		return
	}

	f, err := os.Open(path)
	if err != nil {
		panic(err)
	}

	chunkChan := make(chan []eventLog[event], 1)

	bufPool := new(sync.Pool)
	bufPool.New = func() any {
		return make([]eventLog[event], 0)
	}

	go func() {
		buf := *bufPool.Get().(*[]eventLog[event])
		for log := range eventLogs[event](f) {
			buf = append(buf, log)
			if len(buf) >= 100_000 {
				chunkChan <- buf
				buf = *bufPool.Get().(*[]eventLog[event])
			}
		}
		chunkChan <- buf
		buf = bufPool.Get().([]eventLog[event])
	}()

	for logs := range chunkChan {
		_ = logs
		buf := logs[0:0:0]
		bufPool.Put(&buf)
	}
}

type eventLog[T any] struct {
	Event T      `json:"event"`
	Stamp uint64 `json:"stamp"`
}

type event struct {
	MousePosition   mousePositionEvent `json:"MousePosition"`
	MouseMove       mouseMoveEvent     `json:"MouseMove"`
	MouseButtonDown mouseButtonEvent   `json:"MouseButtonDown"`
	MouseButtonUp   mouseButtonEvent   `json:"MouseButtonUp"`
	MouseScroll     mouseScrollEvent   `json:"MouseScroll"`
	KeyUp           keyEvent           `json:"KeyUp"`
	KeyRepeat       keyEvent           `json:"KeyRepeat"`
	KeyDown         keyEvent           `json:"KeyDown"`
}

type mousePositionEvent struct {
	X int16 `json:"x"`
	Y int16 `json:"y"`
}

type mouseMoveEvent struct {
	DX int16 `json:"dx"`
	DY int16 `json:"dy"`
}

type mouseButtonEvent struct {
	Button string `json:"button"`
}

type mouseScrollEvent struct {
	Direction struct {
		Up struct {
			Clicks uint8 `json:"clicks"`
		} `json:"Up"`
		Down struct {
			Clicks uint8 `json:"clicks"`
		} `json:"Down"`
	} `json:"direction"`
}

type keyEvent struct {
	Key string `json:"key"`
}

func eventLogs[T any](r io.Reader) iter.Seq[eventLog[T]] {
	buf := bufio.NewScanner(r)
	return iter.Seq[eventLog[T]](func(yield func(v eventLog[T]) bool) {
		for buf.Scan() {
			var v eventLog[T]
			err := json.Unmarshal(buf.Bytes(), &v)
			if err != nil {
				panic(err)
			}
			if !yield(v) {
				break
			}
		}
	})
}
