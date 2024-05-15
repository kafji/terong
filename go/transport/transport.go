package transport

import (
	"errors"
	"io"

	"kafji.net/terong/inputevent"
)

const MaxLength = 2 /* sizeof tag */ + 2 /* sizeof length */ + 1020 /* sizeof value */

type Tag uint16

const (
	TagMouseMoveEvent Tag = iota + 1
	TagMouseClickEvent
	TagMouseScrollEvent
	TagKeyPressEvent
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
