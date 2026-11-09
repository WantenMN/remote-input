package server

import (
	"fmt"
	"net/http"
	"net/netip"
	"runtime"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/gorilla/websocket"

	"github.com/WantenMN/remote-input/internal/access"
)

var upgrader = websocket.Upgrader{
	ReadBufferSize:  1024,
	WriteBufferSize: 1024,
	CheckOrigin:     func(*http.Request) bool { return true },
}

func (h *handler) serveWebSocket(w http.ResponseWriter, r *http.Request) {
	ip := clientIP(r)

	if rejection := h.cfg.Access.Check(ip); rejection != nil {
		h.cfg.Logger.Warn("Rejected WebSocket",
			"ip", ip, "reason", reasonText(rejection))
		h.closeAfterUpgrade(w, r, rejection)
		return
	}

	h.cfg.Access.Add(ip)

	conn, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		h.cfg.Access.Remove(ip)
		h.cfg.Logger.Error("WebSocket upgrade failed", "ip", ip, "error", err)
		return
	}

	h.handleConn(conn, ip)
}

func (h *handler) closeAfterUpgrade(w http.ResponseWriter, r *http.Request, rejection *access.Rejection) {
	conn, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		return
	}
	defer conn.Close()

	var reason string
	if rejection.Kind == access.NotWhitelisted {
		reason = fmt.Sprintf("your IP (%s) is not in the whitelist", rejection.IP)
	} else {
		reason = "server is at capacity"
	}

	_ = conn.WriteControl(
		websocket.CloseMessage,
		websocket.FormatCloseMessage(websocket.CloseTryAgainLater, reason),
		time.Now().Add(time.Second),
	)
}

func (h *handler) handleConn(conn *websocket.Conn, ip netip.Addr) {
	defer conn.Close()
	defer h.cfg.Access.Remove(ip)
	conn.SetReadLimit(64 << 20)
	h.cfg.Logger.Info("Client connected", "ip", ip)
	defer h.cfg.Logger.Info("Client disconnected", "ip", ip)

	for {
		messageType, data, err := conn.ReadMessage()
		if err != nil {
			return
		}
		if messageType != websocket.TextMessage {
			continue
		}

		text := strings.TrimSpace(string(data))
		if text == "" {
			continue
		}

		action := "pasting"
		if runtime.GOOS == "windows" {
			action = "typing"
		}
		length := utf8.RuneCountInString(text)
		h.cfg.Logger.Info("Received text", "ip", ip, "chars", length, "action", action)

		h.cfg.Paste <- text

		ack := fmt.Sprintf(`{"ok":true,"len":%d}`, length)
		if err := conn.WriteMessage(websocket.TextMessage, []byte(ack)); err != nil {
			return
		}
	}
}
