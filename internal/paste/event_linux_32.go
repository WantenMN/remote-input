//go:build linux && (386 || arm || mips || mipsle || ppc)

package paste

type inputEvent struct {
	Sec   int32
	Usec  int32
	Type  uint16
	Code  uint16
	Value int32
}

func newKeyEvent(code uint16, value int32) inputEvent {
	return inputEvent{Type: evKey, Code: code, Value: value}
}

func newSyncEvent() inputEvent {
	return inputEvent{Type: evSyn, Code: 0, Value: 0}
}
