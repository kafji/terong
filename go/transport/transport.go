package transport

import (
	"errors"
	"fmt"
	"io"
	"time"

	"kafji.net/terong/inputevent"
)

const MaxLength = 2 /* sizeof tag */ + 2 /* sizeof length */ + 1020 /* sizeof value */

const PingTimeout = 10 * time.Second

const PingInterval = 5 * time.Second

type Tag uint16

const (
	TagMouseMoveEvent Tag = iota + 1
	TagMouseClickEvent
	TagMouseScrollEvent
	TagKeyPressEvent
	TagPing
)

func TagFor(v any) (Tag, error) {
	switch v.(type) {

	case inputevent.MouseMove:
		return TagMouseMoveEvent, nil

	case inputevent.MouseClick:
		return TagMouseClickEvent, nil

	case inputevent.MouseScroll:
		return TagMouseScrollEvent, nil

	case inputevent.KeyPress:
		return TagKeyPressEvent, nil
	}

	return 0, errors.New("unexpected type")
}

func WriteTag(w io.Writer, tag Tag) error {
	return writeUint16(w, uint16(tag))
}

func WriteLength(w io.Writer, length uint16) error {
	return writeUint16(w, length)
}

func writeUint16(w io.Writer, v uint16) error {
	_, err := w.Write([]byte{byte(v >> 8), byte(v)})
	return err
}

func ReadTag(r io.Reader) (Tag, error) {
	tag, err := readUint16(r)
	return Tag(tag), err
}

func ReadLength(r io.Reader) (uint16, error) {
	return readUint16(r)
}

func readUint16(r io.Reader) (uint16, error) {
	buf := make([]byte, 2)
	_, err := io.ReadFull(r, buf)
	v := uint16(0)
	v |= uint16(buf[0]) << 8
	v |= uint16(buf[1])
	return v, err
}

type Frame struct {
	Tag    Tag
	Length uint16
	Value  []byte
}

func WriteFrame(w io.Writer, frm Frame) error {
	err := WriteTag(w, frm.Tag)
	if err != nil {
		return fmt.Errorf("failed to write tag: %v", err)
	}

	err = WriteLength(w, frm.Length)
	if err != nil {
		return fmt.Errorf("failed to write length: %v", err)
	}

	_, err = w.Write(frm.Value[:frm.Length])
	if err != nil {
		return fmt.Errorf("failed to write value: %v", err)
	}

	return nil
}

func ReadFrame(r io.Reader) (Frame, error) {
	tag, err := ReadTag(r)
	if err != nil {
		return Frame{}, fmt.Errorf("failed to read tag: %v", err)
	}

	length, err := ReadLength(r)
	if err != nil {
		return Frame{}, fmt.Errorf("failed to read length: %v", err)
	}

	value := make([]byte, length)
	_, err = io.ReadFull(r, value)
	if err != nil {
		return Frame{}, fmt.Errorf("failed to read value: %v", err)
	}

	return Frame{Tag: tag, Length: length, Value: value}, nil
}
