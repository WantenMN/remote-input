package tlscert

import (
	"crypto/x509"
	"encoding/pem"
	"net/netip"
	"os"
	"path/filepath"
	"testing"
)

func TestGeneratesCertForIP(t *testing.T) {
	dir := t.TempDir()
	ip := netip.MustParseAddr("192.168.99.99")

	pair, err := Ensure(dir, ip)
	if err != nil {
		t.Fatalf("Ensure: %v", err)
	}
	if len(pair.Certificate) == 0 {
		t.Fatal("certificate chain is empty")
	}

	for _, name := range []string{certFileName, keyFileName} {
		if _, err := os.Stat(filepath.Join(dir, name)); err != nil {
			t.Errorf("%s not written: %v", name, err)
		}
	}

	cert, err := x509.ParseCertificate(pair.Certificate[0])
	if err != nil {
		t.Fatalf("ParseCertificate: %v", err)
	}
	found := false
	for _, candidate := range cert.IPAddresses {
		if parsed, ok := netip.AddrFromSlice(candidate); ok && parsed.Unmap() == ip {
			found = true
		}
	}
	if !found {
		t.Errorf("certificate does not list %v in IPAddresses", ip)
	}
}

func TestReusesCachedCert(t *testing.T) {
	dir := t.TempDir()
	ip := netip.MustParseAddr("192.168.1.10")

	first, err := Ensure(dir, ip)
	if err != nil {
		t.Fatalf("first Ensure: %v", err)
	}
	second, err := Ensure(dir, ip)
	if err != nil {
		t.Fatalf("second Ensure: %v", err)
	}
	if string(first.Certificate[0]) != string(second.Certificate[0]) {
		t.Error("certificate was regenerated instead of reused")
	}
}

func TestRegeneratesWhenIPChanges(t *testing.T) {
	dir := t.TempDir()
	if _, err := Ensure(dir, netip.MustParseAddr("192.168.1.10")); err != nil {
		t.Fatalf("Ensure: %v", err)
	}

	other := netip.MustParseAddr("10.0.0.5")
	pair, err := Ensure(dir, other)
	if err != nil {
		t.Fatalf("Ensure: %v", err)
	}

	block, _ := pem.Decode(mustRead(t, filepath.Join(dir, certFileName)))
	if block == nil {
		t.Fatal("cert.pem is not valid PEM")
	}
	cert, err := x509.ParseCertificate(block.Bytes)
	if err != nil {
		t.Fatalf("ParseCertificate: %v", err)
	}
	covered := false
	for _, candidate := range cert.IPAddresses {
		if parsed, ok := netip.AddrFromSlice(candidate); ok && parsed.Unmap() == other {
			covered = true
		}
	}
	if !covered {
		t.Errorf("regenerated certificate does not cover %v", other)
	}
	if len(pair.Certificate) == 0 {
		t.Error("returned certificate chain is empty")
	}
}

func mustRead(t *testing.T, path string) []byte {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	return data
}
