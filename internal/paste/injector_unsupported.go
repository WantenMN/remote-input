//go:build !linux && !windows

package paste

import (
	"errors"
	"time"
)

func CheckPlatform() error { return nil }

type injector struct{}

func newInjector(time.Duration) Injector {
	return &injector{}
}

func (i *injector) Inject(string) error {
	return errors.New("text injection is only supported on Linux and Windows")
}

func (i *injector) Close() {}
