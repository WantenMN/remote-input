package cli

import (
	"errors"
	"flag"
	"fmt"
	"net/netip"
	"os"
	"strings"
)

const (
	DefaultPort           = 48732
	DefaultPasteDelay     = 20
	DefaultMaxConnections = 1
)

var ErrHelp = flag.ErrHelp

var ErrVersion = errors.New("version requested")

var ErrSilent = errors.New("error already reported")

type Args struct {
	Port           uint16
	PasteDelay     uint64
	HTTP           bool
	MaxConnections int
	Allow          []netip.Addr
}

func Parse(argv []string) (Args, error) {
	fs := flag.NewFlagSet("remote-input", flag.ContinueOnError)
	fs.SetOutput(os.Stderr)
	fs.Usage = func() {
		fmt.Fprintf(os.Stderr, "Usage: remote-input [options]\n\n")
		fmt.Fprintf(os.Stderr, "Type on your phone, paste into your desktop.\n\n")
		fmt.Fprintf(os.Stderr, "Options:\n")
		fs.PrintDefaults()
	}

	var (
		port  uint   = DefaultPort
		p     uint   = DefaultPort
		delay uint64 = DefaultPasteDelay
		d     uint64 = DefaultPasteDelay
		http  bool
		maxc  int = DefaultMaxConnections
		m     int = DefaultMaxConnections
		allow addrList
	)

	fs.UintVar(&port, "port", DefaultPort, "port to listen on")
	fs.UintVar(&p, "p", DefaultPort, "port to listen on (shorthand)")
	fs.Uint64Var(&delay, "paste-delay", DefaultPasteDelay, "delay in milliseconds between clipboard write and paste simulation (Linux only)")
	fs.Uint64Var(&d, "D", DefaultPasteDelay, "same as --paste-delay (shorthand)")
	fs.BoolVar(&http, "http", false, "use HTTP instead of HTTPS (insecure, not recommended)")
	fs.IntVar(&maxc, "max-connections", DefaultMaxConnections, "maximum number of distinct client IPs allowed at once")
	fs.IntVar(&m, "m", DefaultMaxConnections, "same as --max-connections (shorthand)")
	fs.Var(&allow, "allow", "only allow connections from these IPs (comma-separated)")
	fs.Var(&allow, "a", "same as --allow (shorthand)")
	showVersion := fs.Bool("version", false, "print version and exit")

	if err := fs.Parse(argv); err != nil {
		if errors.Is(err, flag.ErrHelp) {
			return Args{}, ErrHelp
		}
		return Args{}, ErrSilent
	}
	if *showVersion {
		return Args{}, ErrVersion
	}

	set := map[string]bool{}
	fs.Visit(func(f *flag.Flag) { set[f.Name] = true })
	if set["p"] {
		port = p
	}
	if set["D"] {
		delay = d
	}
	if set["m"] {
		maxc = m
	}

	if port > 65535 {
		fmt.Fprintf(os.Stderr, "remote-input: port %d out of range (0-65535)\n", port)
		return Args{}, ErrSilent
	}
	if maxc < 0 {
		fmt.Fprintln(os.Stderr, "remote-input: --max-connections must not be negative")
		return Args{}, ErrSilent
	}

	return Args{
		Port:           uint16(port),
		PasteDelay:     delay,
		HTTP:           http,
		MaxConnections: maxc,
		Allow:          allow,
	}, nil
}

type addrList []netip.Addr

func (a *addrList) String() string {
	parts := make([]string, len(*a))
	for i, ip := range *a {
		parts[i] = ip.String()
	}
	return strings.Join(parts, ",")
}

func (a *addrList) Set(value string) error {
	for _, part := range strings.Split(value, ",") {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}
		ip, err := netip.ParseAddr(part)
		if err != nil {
			return fmt.Errorf("invalid IP address %q", part)
		}
		*a = append(*a, ip.Unmap())
	}
	return nil
}
