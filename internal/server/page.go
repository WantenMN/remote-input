package server

import (
	"fmt"
	"mime"
	"net"
	"net/http"
	"net/netip"
	"path"

	"github.com/WantenMN/remote-input/internal/access"
)

func clientIP(r *http.Request) netip.Addr {
	host, _, err := net.SplitHostPort(r.RemoteAddr)
	if err != nil {
		host = r.RemoteAddr
	}
	ip, err := netip.ParseAddr(host)
	if err != nil {
		return netip.Addr{}
	}
	return ip.Unmap()
}

var extraTypes = map[string]string{
	".woff2":       "font/woff2",
	".woff":        "font/woff",
	".ttf":         "font/ttf",
	".otf":         "font/otf",
	".ico":         "image/x-icon",
	".svg":         "image/svg+xml",
	".webmanifest": "application/manifest+json",
	".map":         "application/json",
	".wasm":        "application/wasm",
}

func contentType(name string) string {
	ext := path.Ext(name)
	if ct, ok := extraTypes[ext]; ok {
		return ct
	}
	if ct := mime.TypeByExtension(ext); ct != "" {
		return ct
	}
	return "application/octet-stream"
}

func rejectPage(rejection *access.Rejection) string {
	var body string
	if rejection.Kind == access.NotWhitelisted {
		ip := rejection.IP
		body = fmt.Sprintf(
			"<h2>Access Denied</h2>"+
				"<p>Your IP (<b>%s</b>) is not in the whitelist.</p>"+
				"<p>To allow it, restart with:</p>"+
				"<pre>remote-input -a %s</pre>"+
				"<p>For multiple IPs:</p>"+
				"<pre>remote-input -a %s,&lt;other-ip&gt;</pre>",
			ip, ip, ip,
		)
	} else {
		body = fmt.Sprintf(
			"<h2>Server Full</h2>"+
				"<p>At capacity (%d/%d connections).</p>"+
				"<p>To allow more, restart with:</p>"+
				"<pre>remote-input -m &lt;number&gt;</pre>"+
				"<p>For example:</p>"+
				"<pre>remote-input -m 3</pre>",
			rejection.Current, rejection.Max,
		)
	}

	return "<!DOCTYPE html>" +
		"<html>" +
		"<head>" +
		`<meta charset="utf-8">` +
		`<meta name="viewport" content="width=device-width, initial-scale=1">` +
		"<title>Remote Input - Rejected</title>" +
		"<style>" +
		"body { font-family: -apple-system, sans-serif; padding: 2em; max-width: 600px; margin: auto; color: #333; }" +
		"h2 { color: #c00; }" +
		"pre { background: #f4f4f4; padding: 0.8em; border-radius: 4px; overflow-x: auto; font-size: 0.95em; }" +
		"</style>" +
		"</head>" +
		"<body>" + body + "</body>" +
		"</html>"
}
