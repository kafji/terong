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

var slog = logging.NewLogger("transport")

const ValueMaxLength = 1024 - 2 /* tag */ - 2 /* length */
// ValueMaxLength can fit in uint16.
const _ uint16 = ValueMaxLength

const PingTimeout = PingInterval + 1*time.Second
const PingInterval = 5 * time.Second
const ConnectTimeout = 5 * time.Second
const ReconnectDelay = 5 * time.Second
const WriteTimeout = 100 * time.Millisecond

var ErrMaxLengthExceeded = errors.New("length is larger than the maximum length")
var ErrPingTimedOut = errors.New("ping timed out")

type Tag uint16

const (
	TagMouseMove Tag = iota + 1
	TagMouseClick
	TagMouseScroll
	TagKeyPress

	TagPing

	TagClose
)

func TagFor(v any) (Tag, error) {
	switch v.(type) {
	case inputevent.MouseMove:
		return TagMouseMove, nil
	case inputevent.MouseClick:
		return TagMouseClick, nil
	case inputevent.MouseScroll:
		return TagMouseScroll, nil
	case inputevent.KeyPress:
		return TagKeyPress, nil
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

	if length > ValueMaxLength {
		err = ErrMaxLengthExceeded
	}

	return Frame{Tag: tag, Length: length, Value: value}, err
}

type Session struct {
	conn net.Conn

	mu     sync.Mutex
	closed bool

	sendPingDeadline chan struct{}
	recvPingDeadline chan struct{}

	inbox    chan Frame
	inboxErr error
}

func EmptySession() *Session {
	return &Session{closed: true}
}

func NewSession(conn net.Conn) *Session {
	s := &Session{conn: conn, inbox: make(chan Frame)}

	go func() {
		defer close(s.inbox)
		for {
			frm, err := s.ReadFrame()
			if err != nil {
				s.inboxErr = err
				return
			}
			s.inbox <- frm
		}
	}()

	return s
}

func (s *Session) Inbox() <-chan Frame {
	return s.inbox
}

func (s *Session) InboxErr() error {
	return s.inboxErr
}

func (s *Session) ResetSendPingDeadline() {
	ch := make(chan struct{}, 1)
	go func() {
		time.After(time.Until(time.Now().Add(PingInterval)))
		ch <- struct{}{}
	}()
	s.sendPingDeadline = ch
}

func (s *Session) SendPingDeadline() <-chan struct{} {
	return s.sendPingDeadline
}

func (s *Session) ResetRecvPingDeadline() {
	ch := make(chan struct{}, 1)
	go func() {
		time.After(time.Until(time.Now().Add(PingTimeout)))
		ch <- struct{}{}
	}()
	s.recvPingDeadline = ch
}

func (s *Session) RecvPingDeadline() <-chan struct{} {
	return s.recvPingDeadline
}

func (s *Session) WriteFrame(frm Frame) error {
	t := time.Now().Add(WriteTimeout)
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

func (s *Session) SendPing() error {
	if err := s.WritePing(); err != nil {
		return err
	}
	s.ResetSendPingDeadline()
	return nil
}

func (s *Session) writeCloseFrame(reason string) error {
	value := []byte(reason)
	length := len(value)
	if length > ValueMaxLength {
		length = ValueMaxLength
		slog.Warn("reason is longer than maximum value length")
	}
	frm := Frame{Tag: TagClose, Length: uint16(length), Value: value[:length]}
	return s.WriteFrame(frm)
}

func (s *Session) Close(reason string) {
	if s.closed {
		return
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.closed {
		return
	}
	s.closed = true

	slog.Debug("sending close frame")
	err := s.writeCloseFrame(reason)
	if err != nil {
		slog.Warn("failed to write close frame", "error", err)
	}

	err = s.conn.Close()
	if err != nil {
		slog.Warn(
			"failed to close connection",
			"error", err,
			"local_addr", s.conn.LocalAddr(),
			"remote_addr", s.conn.RemoteAddr(),
		)
	}
}

func (s *Session) Closed() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.closed
}
