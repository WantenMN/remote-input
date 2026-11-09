package netinfo

import "testing"

func TestDetectLANIPIsValidIPv4(t *testing.T) {
	ip := DetectLANIP()
	if !ip.IsValid() {
		t.Fatal("DetectLANIP returned an invalid address")
	}
	if !ip.Is4() {
		t.Errorf("DetectLANIP() = %v, want an IPv4 address", ip)
	}
	if ip.String() == "" {
		t.Error("DetectLANIP().String() is empty")
	}
}
