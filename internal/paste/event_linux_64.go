//go:build linux && (amd64 || arm64 || riscv64 || loong64 || ppc64 || ppc64le || s390x || mips64 || mips64le)

package paste

type inputEvent struct {
	Sec   int64
	Usec  int64
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
