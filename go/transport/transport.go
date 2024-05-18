package transport

import (
	"errors"
	"fmt"
	"io"
	"net"
	"sync"
	"time"

	"kafji.net/terong/inputevent"
	"kafji.net/terong/logging"
)

var slog = logging.New("transport")

const MaxLength = 2 /* sizeof tag */ + 2 /* sizeof length */ + 1020 /* sizeof value */

const PingTimeout = 10 * time.Second

const PingInterval = 5 * time.Second

var ErrMaxLengthExceeded = errors.New("frame length is larger than maximum value length")

type Tag uint16

const (
	tagEventMinorant Tag = iota + 1
	TagEventMouseMove
	TagEventMouseClick
	TagEventMouseScroll
	TagEventKeyPress
	tagEventMajorant

	TagPing
)

var TagEvents = sync.OnceValue(func() []Tag {
	tags := make([]Tag, 0)
	for i := tagEventMinorant; i < tagEventMajorant; i++ {
		tags = append(tags, i)
	}
	return tags
})

func TagFor(v any) (Tag, error) {
	switch v.(type) {

	case inputevent.MouseMove:
		return TagEventMouseMove, nil

	case inputevent.MouseClick:
		return TagEventMouseClick, nil

	case inputevent.MouseScroll:
		return TagEventMouseScroll, nil

	case inputevent.KeyPress:
		return TagEventKeyPress, nil
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

	if length > MaxLength {
		err = ErrMaxLengthExceeded
	}

	return Frame{Tag: tag, Length: length, Value: value}, err
}

type Session struct {
	conn net.Conn

	mu           sync.Mutex
	closed       bool
	err          error
	pingDeadline time.Time

	inbox chan Frame
}

func EmptySession() *Session {
	return &Session{closed: true}
}

func NewSession(conn net.Conn) *Session {
	s := &Session{conn: conn, inbox: make(chan Frame)}
	s.start()
	return s
}

func (s *Session) start() {
	go func() {
		for {
			frm, err := s.ReadFrame()
			if err != nil {
				s.mu.Lock()
				defer s.mu.Unlock()
				s.err = err
				close(s.inbox)
				return
			}
			s.inbox <- frm
		}
	}()
}

func (s *Session) Inbox() <-chan Frame {
	return s.inbox
}

func (s *Session) Closed() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.closed
}

func (s *Session) Error() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.err
}

func (s *Session) ResetPingDeadline() {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.pingDeadline = time.Now().Add(PingTimeout)
}

func (s *Session) PingDeadline() <-chan struct{} {
	s.mu.Lock()
	defer s.mu.Unlock()
	ch := make(chan struct{})
	go func() {
		time.After(time.Until(s.pingDeadline))
		ch <- struct{}{}
	}()
	return ch
}

func (s *Session) WriteFrame(frm Frame) error {
	t := time.Now().Add(100 * time.Millisecond)
	err := s.conn.SetWriteDeadline(t)
	if err != err {
		return fmt.Errorf("failed to set write deadline: %v", err)
	}
	return WriteFrame(s.conn, frm)
}

func (s *Session) WritePing() error {
	frm := Frame{Tag: TagPing, Length: 0}
	return s.WriteFrame(frm)
}

func (s *Session) ReadFrame() (Frame, error) {
	return ReadFrame(s.conn)
}

func (s *Session) Close() {
	if s.closed {
		return
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed {
		return
	}
	s.closed = true
	err := s.conn.Close()
	if err != nil {
		slog.Warn("failed to close connection",
			"error", err,
			"local_addr", s.conn.LocalAddr(),
			"remote_addr", s.conn.RemoteAddr(),
		)
	}
}
