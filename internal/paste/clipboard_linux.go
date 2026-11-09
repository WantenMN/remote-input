//go:build linux

package paste

import (
	"fmt"
	"io"
	"os"
	"os/exec"
	"time"
)

type clipboardOwner struct {
	prev *exec.Cmd
}

func (c *clipboardOwner) set(text string) error {
	if os.Getenv("WAYLAND_DISPLAY") != "" {
		return c.setWayland(text)
	}
	return c.setX11(text)
}

func (c *clipboardOwner) setWayland(text string) error {
	c.kill()
	time.Sleep(10 * time.Millisecond)

	cmd := exec.Command("wl-copy", "--foreground")
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return fmt.Errorf("failed to open wl-copy stdin: %w", err)
	}
	if err := cmd.Start(); err != nil {
		return fmt.Errorf("failed to spawn wl-copy: %w", err)
	}
	if _, err := io.WriteString(stdin, text); err != nil {
		cmd.Process.Kill()
		cmd.Wait()
		return fmt.Errorf("failed to write to wl-copy stdin: %w", err)
	}
	stdin.Close()

	time.Sleep(20 * time.Millisecond)
	c.prev = cmd
	return nil
}

func (c *clipboardOwner) setX11(text string) error {
	cmd := exec.Command("xclip", "-selection", "clipboard")
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return fmt.Errorf("failed to open xclip stdin: %w", err)
	}
	if err := cmd.Start(); err != nil {
		return fmt.Errorf("failed to spawn xclip: %w", err)
	}
	if _, err := io.WriteString(stdin, text); err != nil {
		cmd.Process.Kill()
		cmd.Wait()
		return fmt.Errorf("failed to write to xclip stdin: %w", err)
	}
	stdin.Close()
	return cmd.Wait()
}

func (c *clipboardOwner) kill() {
	if c.prev == nil {
		return
	}
	c.prev.Process.Kill()
	c.prev.Wait()
	c.prev = nil
}

func (c *clipboardOwner) close() {
	c.kill()
}
