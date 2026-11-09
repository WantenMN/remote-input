//go:build windows

package paste

import (
	"fmt"
	"runtime"
	"syscall"
	"time"
	"unicode/utf16"
	"unsafe"
)

func CheckPlatform() error { return nil }

type injector struct{}

func newInjector(time.Duration) Injector {
	return &injector{}
}

func (i *injector) Inject(text string) error {
	return typeText(text)
}

func (i *injector) Close() {}

const (
	inputKeyboard = 1

	keyeventfKeyup   = 0x0002
	keyeventfUnicode = 0x0004

	vkBack   = 0x08
	vkTab    = 0x09
	vkReturn = 0x0D

	sendInputBatch = 256
)

type keyboardInput struct {
	WVk         uint16
	WScan       uint16
	DwFlags     uint32
	Time        uint32
	DwExtraInfo uintptr
}

type mouseInput struct {
	Dx          int32
	Dy          int32
	MouseData   uint32
	DwFlags     uint32
	Time        uint32
	DwExtraInfo uintptr
}

const (
	inputTypeSize = uintptr(4)
	inputAlign    = unsafe.Alignof(uintptr(0))
	inputUnion    = unsafe.Sizeof(mouseInput{})
	inputOffset   = (inputTypeSize + inputAlign - 1) / inputAlign * inputAlign
	inputStride   = inputOffset + inputUnion
)

var procSendInput = syscall.NewLazyDLL("user32.dll").NewProc("SendInput")

func typeText(text string) error {
	buf := make([]byte, 0, len(text)*2*int(inputStride))

	appendKey := func(vk, scan uint16, flags uint32) {
		start := len(buf)
		buf = append(buf, make([]byte, inputStride)...)
		*(*uint32)(unsafe.Pointer(&buf[start])) = inputKeyboard
		ki := (*keyboardInput)(unsafe.Pointer(&buf[start+int(inputOffset)]))
		ki.WVk = vk
		ki.WScan = scan
		ki.DwFlags = flags
	}
	tap := func(vk uint16) {
		appendKey(vk, 0, 0)
		appendKey(vk, 0, keyeventfKeyup)
	}

	runes := []rune(text)
	for i := 0; i < len(runes); i++ {
		switch r := runes[i]; r {
		case '\r':
			tap(vkReturn)
			if i+1 < len(runes) && runes[i+1] == '\n' {
				i++
			}
		case '\n':
			tap(vkReturn)
		case '\t':
			tap(vkTab)
		case '\b':
			tap(vkBack)
		default:
			for _, unit := range utf16.Encode([]rune{r}) {
				appendKey(0, unit, keyeventfUnicode)
				appendKey(0, unit, keyeventfUnicode|keyeventfKeyup)
			}
		}
	}

	for offset := 0; offset < len(buf); offset += sendInputBatch * int(inputStride) {
		end := min(offset+sendInputBatch*int(inputStride), len(buf))
		count := (end - offset) / int(inputStride)
		sent, _, err := procSendInput.Call(
			uintptr(count),
			uintptr(unsafe.Pointer(&buf[offset])),
			inputStride,
		)
		if sent != uintptr(count) {
			if errno, ok := err.(syscall.Errno); ok && errno != 0 {
				return fmt.Errorf("SendInput failed: %w", errno)
			}
			return fmt.Errorf("SendInput failed: sent %d of %d events", sent, count)
		}
	}
	runtime.KeepAlive(buf)
	return nil
}
