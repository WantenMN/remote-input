package netinfo

import (
	"net"
	"net/netip"
	"strings"
)

var tunKeywords = []string{
	"tun", "tap", "wireguard", "tailscale", "vpn", "tunnel", "loopback",
}

func DetectLANIP() netip.Addr {
	ifaces, err := net.Interfaces()
	if err == nil {
		for _, iface := range ifaces {
			if iface.Flags&net.FlagUp == 0 {
				continue
			}
			if hasTUNKeyword(iface.Name) {
				continue
			}
			addrs, err := iface.Addrs()
			if err != nil {
				continue
			}
			for _, addr := range addrs {
				ipnet, ok := addr.(*net.IPNet)
				if !ok {
					continue
				}
				ip, ok := netip.AddrFromSlice(ipnet.IP)
				if !ok {
					continue
				}
				ip = ip.Unmap()
				if !ip.Is4() || ip.IsLoopback() || ip.IsLinkLocalUnicast() {
					continue
				}
				return ip
			}
		}
	}
	return netip.AddrFrom4([4]byte{127, 0, 0, 1})
}

func hasTUNKeyword(name string) bool {
	name = strings.ToLower(name)
	for _, kw := range tunKeywords {
		if strings.Contains(name, kw) {
			return true
		}
	}
	return false
}
