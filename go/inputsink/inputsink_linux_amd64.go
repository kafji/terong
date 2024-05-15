package inputsink

/*
 */
import "C"

type Handle struct {
}

func Start(source <-chan any) *Handle {
	h := &Handle{}
	return h
}

func (h *Handle) Stop() {
}
