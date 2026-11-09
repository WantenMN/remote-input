package cli

import (
	"errors"
	"net/netip"
	"testing"
)

func TestDefaults(t *testing.T) {
	args, err := Parse(nil)
	if err != nil {
		t.Fatalf("Parse: %v", err)
	}
	if args.Port != 48732 {
		t.Errorf("Port = %d, want 48732", args.Port)
	}
	if args.PasteDelay != 20 {
		t.Errorf("PasteDelay = %d, want 20", args.PasteDelay)
	}
	if args.HTTP {
		t.Error("HTTP = true, want false")
	}
	if args.MaxConnections != 1 {
		t.Errorf("MaxConnections = %d, want 1", args.MaxConnections)
	}
	if len(args.Allow) != 0 {
		t.Errorf("Allow = %v, want empty", args.Allow)
	}
}

func TestAllFlags(t *testing.T) {
	args, err := Parse([]string{
		"-p", "9999",
		"-D", "50",
		"--http",
		"-m", "5",
		"-a", "192.168.1.5,192.168.1.10",
	})
	if err != nil {
		t.Fatalf("Parse: %v", err)
	}
	if args.Port != 9999 {
		t.Errorf("Port = %d, want 9999", args.Port)
	}
	if args.PasteDelay != 50 {
		t.Errorf("PasteDelay = %d, want 50", args.PasteDelay)
	}
	if !args.HTTP {
		t.Error("HTTP = false, want true")
	}
	if args.MaxConnections != 5 {
		t.Errorf("MaxConnections = %d, want 5", args.MaxConnections)
	}
	want := []netip.Addr{
		netip.MustParseAddr("192.168.1.5"),
		netip.MustParseAddr("192.168.1.10"),
	}
	if len(args.Allow) != len(want) {
		t.Fatalf("Allow = %v, want %v", args.Allow, want)
	}
	for i := range want {
		if args.Allow[i] != want[i] {
			t.Errorf("Allow[%d] = %v, want %v", i, args.Allow[i], want[i])
		}
	}
}

func TestLongFlags(t *testing.T) {
	args, err := Parse([]string{"--port", "8080", "--max-connections", "10", "--paste-delay", "5"})
	if err != nil {
		t.Fatalf("Parse: %v", err)
	}
	if args.Port != 8080 || args.MaxConnections != 10 || args.PasteDelay != 5 {
		t.Errorf("got %+v", args)
	}
}

func TestInvalidPortFails(t *testing.T) {
	if _, err := Parse([]string{"-p", "notanumber"}); err == nil {
		t.Fatal("Parse succeeded, want error")
	}
}

func TestPortOutOfRangeFails(t *testing.T) {
	if _, err := Parse([]string{"-p", "70000"}); err == nil {
		t.Fatal("Parse succeeded, want error")
	}
}

func TestInvalidAllowAddressFails(t *testing.T) {
	if _, err := Parse([]string{"-a", "not-an-ip"}); err == nil {
		t.Fatal("Parse succeeded, want error")
	}
}

func TestHelpAndVersion(t *testing.T) {
	if _, err := Parse([]string{"-h"}); !errors.Is(err, ErrHelp) {
		t.Errorf("err = %v, want ErrHelp", err)
	}
	if _, err := Parse([]string{"--version"}); !errors.Is(err, ErrVersion) {
		t.Errorf("err = %v, want ErrVersion", err)
	}
}
