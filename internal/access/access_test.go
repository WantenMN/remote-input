package access

import (
	"net/netip"
	"testing"
)

var (
	localhost = netip.MustParseAddr("127.0.0.1")
	otherIP   = netip.MustParseAddr("10.0.0.99")
)

func TestEmptyWhitelistAllowsEveryone(t *testing.T) {
	state := New(10, nil)
	if r := state.Check(localhost); r != nil {
		t.Errorf("Check(localhost) = %+v, want nil", r)
	}
	if r := state.Check(otherIP); r != nil {
		t.Errorf("Check(other) = %+v, want nil", r)
	}
}

func TestWhitelistAllowsMatchingIP(t *testing.T) {
	state := New(10, []netip.Addr{localhost})
	if r := state.Check(localhost); r != nil {
		t.Errorf("Check = %+v, want nil", r)
	}
}

func TestWhitelistRejectsNonMatchingIP(t *testing.T) {
	state := New(10, []netip.Addr{localhost})
	r := state.Check(otherIP)
	if r == nil {
		t.Fatal("Check = nil, want rejection")
	}
	if r.Kind != NotWhitelisted {
		t.Errorf("Kind = %v, want NotWhitelisted", r.Kind)
	}
}

func TestMultipleWhitelistEntries(t *testing.T) {
	state := New(10, []netip.Addr{localhost, otherIP})
	if r := state.Check(localhost); r != nil {
		t.Errorf("Check(localhost) = %+v", r)
	}
	if r := state.Check(otherIP); r != nil {
		t.Errorf("Check(other) = %+v", r)
	}
	if r := state.Check(netip.MustParseAddr("172.16.0.1")); r == nil {
		t.Error("Check(unknown) = nil, want rejection")
	}
}

func TestAtCapacityRejectsNewIP(t *testing.T) {
	state := New(1, nil)
	state.Add(otherIP)
	r := state.Check(localhost)
	if r == nil {
		t.Fatal("Check = nil, want rejection")
	}
	if r.Kind != AtCapacity {
		t.Errorf("Kind = %v, want AtCapacity", r.Kind)
	}
	if r.Current != 1 || r.Max != 1 {
		t.Errorf("Current/Max = %d/%d, want 1/1", r.Current, r.Max)
	}
}

func TestAlreadyConnectedIPAllowedAtCapacity(t *testing.T) {
	state := New(1, nil)
	state.Add(localhost)
	if r := state.Check(localhost); r != nil {
		t.Errorf("Check = %+v, want nil", r)
	}
}

func TestUnderCapacityAllows(t *testing.T) {
	state := New(3, nil)
	state.Add(otherIP)
	if r := state.Check(localhost); r != nil {
		t.Errorf("Check = %+v, want nil", r)
	}
}

func TestWhitelistCheckedBeforeCapacity(t *testing.T) {
	state := New(2, []netip.Addr{localhost})
	state.Add(localhost)
	r := state.Check(netip.MustParseAddr("10.0.0.88"))
	if r == nil || r.Kind != NotWhitelisted {
		t.Errorf("Check = %+v, want NotWhitelisted", r)
	}
}

func TestRemoveReleasesSlot(t *testing.T) {
	state := New(1, nil)
	state.Add(otherIP)
	state.Remove(otherIP)
	if r := state.Check(localhost); r != nil {
		t.Errorf("Check after Remove = %+v, want nil", r)
	}
}
