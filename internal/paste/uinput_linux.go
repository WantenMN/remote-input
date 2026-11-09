//go:build linux

package paste

import (
	"fmt"
	"os"
	"runtime"
	"syscall"
	"time"
	"unsafe"
)

const (
	uinputPath = "/dev/uinput"

	evSyn = 0x00
	evKey = 0x01

	keyLeftCtrl  = 29
	keyLeftShift = 42
	keyV         = 47

	busUSB = 0x03

	uiDevCreate  = 0x5501
	uiDevDestroy = 0x5502
	uiSetEvBit   = 0x40045564
	uiSetKeyBit  = 0x40045565
)

var uiDevSetup = uintptr(0x40000000 | (uintptr(unsafe.Sizeof(uinputSetup{})) << 16) | ('U' << 8) | 3)

type inputID struct {
	BusType uint16
	Vendor  uint16
	Product uint16
	Version uint16
}

type uinputSetup struct {
	ID           inputID
	Name         [80]byte
	FFEffectsMax uint32
}

func pasteKeystroke() error {
	dev, err := os.OpenFile(uinputPath, os.O_WRONLY|syscall.O_NONBLOCK, 0)
	if err != nil {
		return fmt.Errorf("cannot open /dev/uinput: %w. Are you in the 'input' group?", err)
	}
	defer dev.Close()

	fd := dev.Fd()

	for _, eventType := range []uintptr{evKey, evSyn} {
		if err := ioctlArg(fd, uiSetEvBit, eventType); err != nil {
			return fmt.Errorf("failed to enable event type %d: %w", eventType, err)
		}
	}
	for _, code := range []uintptr{keyLeftCtrl, keyLeftShift, keyV} {
		if err := ioctlArg(fd, uiSetKeyBit, code); err != nil {
			return fmt.Errorf("failed to register key %d: %w", code, err)
		}
	}

	var setup uinputSetup
	setup.ID.BusType = busUSB
	copy(setup.Name[:], "remote-input virtual keyboard")
	err = ioctlPtr(fd, uiDevSetup, unsafe.Pointer(&setup))
	runtime.KeepAlive(&setup)
	if err != nil {
		return fmt.Errorf("cannot configure uinput device: %w", err)
	}

	if err := ioctlArg(fd, uiDevCreate, 0); err != nil {
		return fmt.Errorf("cannot create uinput device: %w. Are you in the 'input' group?", err)
	}
	defer ioctlArg(fd, uiDevDestroy, 0)

	time.Sleep(100 * time.Millisecond)

	events := []inputEvent{
		newKeyEvent(keyLeftCtrl, 1),
		newKeyEvent(keyLeftShift, 1),
		newKeyEvent(keyV, 1),
		newKeyEvent(keyV, 0),
		newKeyEvent(keyLeftShift, 0),
		newKeyEvent(keyLeftCtrl, 0),
		newSyncEvent(),
	}
	if err := writeEvents(dev, events); err != nil {
		return fmt.Errorf("failed to emit key events: %w", err)
	}

	time.Sleep(10 * time.Millisecond)
	return nil
}

func writeEvents(dev *os.File, events []inputEvent) error {
	if len(events) == 0 {
		return nil
	}
	size := int(unsafe.Sizeof(events[0])) * len(events)
	buf := unsafe.Slice((*byte)(unsafe.Pointer(&events[0])), size)
	_, err := dev.Write(buf)
	runtime.KeepAlive(events)
	return err
}

func ioctlArg(fd, req, arg uintptr) error {
	if _, _, errno := syscall.Syscall(syscall.SYS_IOCTL, fd, req, arg); errno != 0 {
		return errno
	}
	return nil
}

func ioctlPtr(fd, req uintptr, ptr unsafe.Pointer) error {
	if _, _, errno := syscall.Syscall(syscall.SYS_IOCTL, fd, req, uintptr(ptr)); errno != 0 {
		return errno
	}
	return nil
}
