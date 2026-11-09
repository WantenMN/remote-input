package access

import (
	"net/netip"
	"sync"
)

type Kind int

const (
	NotWhitelisted Kind = iota + 1
	AtCapacity
)

type Rejection struct {
	Kind    Kind
	IP      netip.Addr
	Current int
	Max     int
}

type State struct {
	mu             sync.Mutex
	connected      map[netip.Addr]struct{}
	MaxConnections int
	Allow          []netip.Addr
}

func New(maxConnections int, allow []netip.Addr) *State {
	return &State{
		connected:      make(map[netip.Addr]struct{}),
		MaxConnections: maxConnections,
		Allow:          allow,
	}
}

func (s *State) Check(ip netip.Addr) *Rejection {
	if len(s.Allow) > 0 && !s.allowed(ip) {
		return &Rejection{Kind: NotWhitelisted, IP: ip, Max: s.MaxConnections}
	}

	s.mu.Lock()
	defer s.mu.Unlock()

	_, present := s.connected[ip]
	if !present && len(s.connected) >= s.MaxConnections {
		return &Rejection{
			Kind:    AtCapacity,
			IP:      ip,
			Current: len(s.connected),
			Max:     s.MaxConnections,
		}
	}
	return nil
}

func (s *State) Add(ip netip.Addr) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.connected[ip] = struct{}{}
}

func (s *State) Remove(ip netip.Addr) {
	s.mu.Lock()
	defer s.mu.Unlock()
	delete(s.connected, ip)
}

func (s *State) Count() int {
	s.mu.Lock()
	defer s.mu.Unlock()
	return len(s.connected)
}

func (s *State) allowed(ip netip.Addr) bool {
	for _, a := range s.Allow {
		if a == ip {
			return true
		}
	}
	return false
}
