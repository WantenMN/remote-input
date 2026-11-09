//go:build linux

package paste

import "time"

type injector struct {
	clipboard *clipboardOwner
	delay     time.Duration
}

func newInjector(delay time.Duration) Injector {
	return &injector{clipboard: &clipboardOwner{}, delay: delay}
}

func (i *injector) Inject(text string) error {
	if err := i.clipboard.set(text); err != nil {
		return err
	}
	time.Sleep(i.delay)
	return pasteKeystroke()
}

func (i *injector) Close() {
	i.clipboard.close()
}
