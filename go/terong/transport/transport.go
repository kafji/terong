package transport

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"io"
	"math/rand"
	"net"
	"sync"
	"time"

	"kafji.net/terong/inputevent"
	"kafji.net/terong/logging"
)

var slog = logging.NewLogger("terong/transport")

const (
	ValueMaxLength = 1024 - 2 /* tag */ - 2 /* length */
	// ValueMaxLength can fit in uint16.
	_ uint16 = ValueMaxLength
)

const (
	PingTimeout    = 10 * time.Second
	ConnectTimeout = 5 * time.Second
	ReconnectDelay = 5 * time.Second
	WriteTimeout   = 100 * time.Millisecond
)

var (
	ErrMaxLengthExceeded = errors.New("length is larger than the maximum length")
	ErrPingTimedOut      = errors.New("ping timed out")
)

type Tag uint16

const (
	TagMouseMove Tag = iota + 1
	TagMouseClick
	TagMouseScroll
	TagKeyPress

	TagPing
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

func writeTag(w io.Writer, tag Tag) error {
	return writeUint16(w, uint16(tag))
}

func writeLength(w io.Writer, length uint16) error {
	return writeUint16(w, length)
}

func writeUint16(w io.Writer, v uint16) error {
	_, err := w.Write([]byte{byte(v >> 8), byte(v)})
	return err
}

func readTag(r io.Reader) (Tag, error) {
	tag, err := readUint16(r)
	return Tag(tag), err
}

func readLength(r io.Reader) (uint16, error) {
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

func writeFrame(w io.Writer, frm Frame) error {
	err := writeTag(w, frm.Tag)
	if err != nil {
		return fmt.Errorf("failed to write tag: %w", err)
	}

	err = writeLength(w, frm.Length)
	if err != nil {
		return fmt.Errorf("failed to write length: %w", err)
	}

	_, err = w.Write(frm.Value[:frm.Length])
	if err != nil {
		return fmt.Errorf("failed to write value: %w", err)
	}

	return nil
}

func readFrame(r io.Reader) (Frame, error) {
	tag, err := readTag(r)
	if err != nil {
		return Frame{}, fmt.Errorf("failed to read tag: %w", err)
	}

	length, err := readLength(r)
	if err != nil {
		return Frame{}, fmt.Errorf("failed to read length: %w", err)
	}

	value := make([]byte, length)
	_, err = io.ReadFull(r, value)
	if err != nil {
		return Frame{}, fmt.Errorf("failed to read value: %w", err)
	}

	if length > ValueMaxLength {
		err = ErrMaxLengthExceeded
	}

	return Frame{Tag: tag, Length: length, Value: value}, err
}

type Session struct {
	conn net.Conn
	w    *bufio.Writer
	r    *bufio.Reader

	mu     sync.Mutex
	closed bool

	sendPingDeadline chan struct{}
	recvPingDeadline chan struct{}

	inbox       chan Frame
	inboxErr    error
	cancelInbox context.CancelFunc
}

func EmptySession() *Session {
	return &Session{closed: true}
}

func NewSession(ctx context.Context, conn net.Conn) *Session {
	inbox := make(chan Frame)
	inboxCtx, cancelInbox := context.WithCancel(ctx)
	s := &Session{
		conn:        conn,
		w:           bufio.NewWriter(conn),
		r:           bufio.NewReader(conn),
		inbox:       inbox,
		cancelInbox: cancelInbox,
	}
	s.SetSendPingDeadline()
	s.SetRecvPingDeadline()

	go func() {
		defer close(s.inbox)
		err := func() error {
			for {
				frm, err := s.ReadFrame()
				if err != nil {
					return err
				}
				select {
				case <-inboxCtx.Done():
					return inboxCtx.Err()
				case s.inbox <- frm:
				}
			}
		}()
		s.inboxErr = err
	}()

	return s
}

func (s *Session) Inbox() <-chan Frame {
	return s.inbox
}

func (s *Session) InboxErr() error {
	return s.inboxErr
}

func (s *Session) SetSendPingDeadline() {
	ch := make(chan struct{}, 1)
	go func() {
		d := PingTimeout/2 + time.Duration(rand.Intn(int(PingTimeout/time.Second/2)))
		time.Sleep(d)
		ch <- struct{}{}
	}()
	s.sendPingDeadline = ch
}

func (s *Session) SendPingDeadline() <-chan struct{} {
	return s.sendPingDeadline
}

func (s *Session) SetRecvPingDeadline() {
	ch := make(chan struct{}, 1)
	go func() {
		time.Sleep(PingTimeout)
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
	if err != nil {
		return fmt.Errorf("failed to set write deadline: %w", err)
	}

	err = writeFrame(s.w, frm)
	if err != nil {
		return fmt.Errorf("failed to write to buffer: %w", err)
	}

	err = s.w.Flush()
	if err != nil {
		return fmt.Errorf("failed to flush buffer: %w", err)
	}

	return nil
}

func (s *Session) WritePing() error {
	frm := Frame{Tag: TagPing, Length: 0}
	return s.WriteFrame(frm)
}

func (s *Session) ReadFrame() (Frame, error) {
	return readFrame(s.r)
}

func (s *Session) SendPing() error {
	if err := s.WritePing(); err != nil {
		return err
	}
	s.SetSendPingDeadline()
	return nil
}

func (s *Session) Close() {
	if s == nil {
		return
	}
	defer s.cancelInbox()
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
