package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"time"
)

func main() {
	start := time.Now()

	usage := fmt.Sprintf("usage: %s <input>", os.Args[0])

	var inputPath string
	if len(os.Args) > 1 {
		inputPath = os.Args[1]
	} else {
		fmt.Fprintf(os.Stderr, "%s\n", usage)
		return
	}

	inputFile, err := os.Open(inputPath)
	if err != nil {
		panic(err)
	}

	outputFile, err := os.Create("./terong-obfuscate.out.json")
	if err != nil {
		panic(err)
	}

	chunkChan := make(chan []eventLog[event], 10)

	chunkSize := 100_000

	go func() {
		defer close(chunkChan)
		r := bufio.NewScanner(inputFile)
		chunk := make([]eventLog[event], 0, chunkSize)
		for r.Scan() {
			var log eventLog[event]
			if err := json.Unmarshal(r.Bytes(), &log); err != nil {
				panic(err)
			}
			chunk = append(chunk, log)
			if len(chunk) >= chunkSize {
				chunkChan <- chunk
				chunk = make([]eventLog[event], 0, chunkSize)
			}
		}
		chunkChan <- chunk
	}()

	obfsctr := newObfuscator()

	records := 0
	w := bufio.NewWriter(outputFile)
	for chunk := range chunkChan {
		for _, log := range chunk {
			ok, ev := obfsctr.Obfuscate(log.Event)
			if !ok {
				continue
			}
			log = eventLog[event]{
				Event: ev,
				Stamp: log.Stamp,
			}
			jsoned, err := json.Marshal(&log)
			if err != nil {
				panic(err)
			}
			_, err = w.Write(jsoned)
			if err != nil {
				panic(err)
			}
			_, err = w.Write([]byte("\n"))
			if err != nil {
				panic(err)
			}
			records++
		}
	}
	if err := w.Flush(); err != nil {
		panic(err)
	}

	fmt.Printf("processed %d records in %v\n", records, time.Since(start))
}

type eventLog[T any] struct {
	Event T      `json:"event"`
	Stamp uint64 `json:"stamp"`
}

type event struct {
	MousePosition   *mousePositionEvent `json:"MousePosition,omitempty"`
	MouseMove       *mouseMoveEvent     `json:"MouseMove,omitempty"`
	MouseButtonDown *mouseButtonEvent   `json:"MouseButtonDown,omitempty"`
	MouseButtonUp   *mouseButtonEvent   `json:"MouseButtonUp,omitempty"`
	MouseScroll     *mouseScrollEvent   `json:"MouseScroll,omitempty"`
	KeyUp           *keyEvent           `json:"KeyUp,omitempty"`
	KeyRepeat       *keyEvent           `json:"KeyRepeat,omitempty"`
	KeyDown         *keyEvent           `json:"KeyDown,omitempty"`
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
		Up *struct {
			Clicks uint8 `json:"clicks"`
		} `json:"Up,omitempty"`
		Down *struct {
			Clicks uint8 `json:"clicks"`
		} `json:"Down,omitempty"`
	} `json:"direction"`
}

type keyEvent struct {
	Key string `json:"key"`
}

type obfuscator struct {
	table map[string]string
}

func newObfuscator() *obfuscator {
	// todo(kfj)
	return &obfuscator{}
}

func (*obfuscator) Obfuscate(e event) (bool, event) {
	if e.KeyUp != nil {
		e = event{KeyUp: e.KeyUp}
	}
	if e.KeyRepeat != nil {
		e = event{KeyRepeat: e.KeyRepeat}
	}
	if e.KeyDown != nil {
		e = event{KeyDown: e.KeyDown}
	}
	return true, e
}
