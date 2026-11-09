package server

import (
	"encoding/json"
	"errors"
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"net/netip"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/gorilla/websocket"

	"github.com/WantenMN/remote-input/internal/access"
)

var otherIP = netip.MustParseAddr("10.0.0.99")

func testLogger() *slog.Logger {
	return slog.New(slog.NewTextHandler(io.Discard, nil))
}

func newTestServer(t *testing.T, maxConnections int, allow []netip.Addr) (*httptest.Server, chan string) {
	t.Helper()
	state := access.New(maxConnections, allow)
	return newTestServerWith(t, state)
}

func newTestServerWith(t *testing.T, state *access.State) (*httptest.Server, chan string) {
	t.Helper()

	web := os.DirFS(filepath.Join("testdata", "web"))
	pasteCh := make(chan string, 16)

	handler, err := New(Config{
		Web:    web,
		Access: state,
		Paste:  pasteCh,
		Logger: testLogger(),
	})
	if err != nil {
		t.Fatalf("New: %v", err)
	}

	srv := httptest.NewServer(handler)
	t.Cleanup(srv.Close)
	return srv, pasteCh
}

func get(t *testing.T, url string) (*http.Response, string) {
	t.Helper()
	resp, err := http.Get(url)
	if err != nil {
		t.Fatalf("GET %s: %v", url, err)
	}
	t.Cleanup(func() { resp.Body.Close() })
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		t.Fatalf("read body: %v", err)
	}
	return resp, string(body)
}

func firstAsset(t *testing.T, ext string) string {
	t.Helper()
	entries, err := os.ReadDir(filepath.Join("testdata", "web", "assets"))
	if err != nil {
		t.Fatalf("testdata assets not found: %v", err)
	}
	for _, entry := range entries {
		if strings.HasSuffix(entry.Name(), "."+ext) {
			return "/assets/" + entry.Name()
		}
	}
	t.Fatalf("no .%s asset found in testdata", ext)
	return ""
}

func dialWS(t *testing.T, srv *httptest.Server) *websocket.Conn {
	t.Helper()
	url := "ws" + strings.TrimPrefix(srv.URL, "http") + "/ws"
	conn, _, err := websocket.DefaultDialer.Dial(url, nil)
	if err != nil {
		t.Fatalf("dial %s: %v", url, err)
	}
	t.Cleanup(func() { conn.Close() })
	return conn
}

func readAck(t *testing.T, conn *websocket.Conn) (ok bool, length int) {
	t.Helper()
	conn.SetReadDeadline(time.Now().Add(3 * time.Second))
	_, data, err := conn.ReadMessage()
	if err != nil {
		t.Fatalf("read message: %v", err)
	}
	var ack struct {
		OK  bool `json:"ok"`
		Len int  `json:"len"`
	}
	if err := json.Unmarshal(data, &ack); err != nil {
		t.Fatalf("ack %q is not JSON: %v", data, err)
	}
	return ack.OK, ack.Len
}

func waitForClose(t *testing.T, conn *websocket.Conn) *websocket.CloseError {
	t.Helper()
	conn.SetReadDeadline(time.Now().Add(5 * time.Second))
	for {
		_, _, err := conn.ReadMessage()
		if err == nil {
			continue
		}
		var closeErr *websocket.CloseError
		if errors.As(err, &closeErr) {
			return closeErr
		}
		t.Fatalf("expected close frame, got %v", err)
	}
}

func TestIndexReturnsHTML(t *testing.T) {
	srv, _ := newTestServer(t, 10, nil)

	resp, body := get(t, srv.URL+"/")
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("status = %d, want 200", resp.StatusCode)
	}
	if ct := resp.Header.Get("Content-Type"); !strings.Contains(ct, "text/html") {
		t.Errorf("Content-Type = %q, want text/html", ct)
	}
	if !strings.Contains(body, `<div id="root">`) {
		t.Error("index.html does not contain the root div")
	}
}

func TestCSSAsset(t *testing.T) {
	srv, _ := newTestServer(t, 10, nil)
	resp, _ := get(t, srv.URL+firstAsset(t, "css"))
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("status = %d, want 200", resp.StatusCode)
	}
	if ct := resp.Header.Get("Content-Type"); !strings.Contains(ct, "text/css") {
		t.Errorf("Content-Type = %q, want text/css", ct)
	}
}

func TestJSAsset(t *testing.T) {
	srv, _ := newTestServer(t, 10, nil)
	resp, _ := get(t, srv.URL+firstAsset(t, "js"))
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("status = %d, want 200", resp.StatusCode)
	}
	if ct := resp.Header.Get("Content-Type"); !strings.Contains(ct, "javascript") {
		t.Errorf("Content-Type = %q, want javascript", ct)
	}
}

func TestFontAsset(t *testing.T) {
	srv, _ := newTestServer(t, 10, nil)
	resp, _ := get(t, srv.URL+firstAsset(t, "woff2"))
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("status = %d, want 200", resp.StatusCode)
	}
	if ct := resp.Header.Get("Content-Type"); !strings.Contains(ct, "font") {
		t.Errorf("Content-Type = %q, want font", ct)
	}
}

func TestUnknownAssetReturns404(t *testing.T) {
	srv, _ := newTestServer(t, 10, nil)
	resp, _ := get(t, srv.URL+"/assets/nonexistent.txt")
	if resp.StatusCode != http.StatusNotFound {
		t.Errorf("status = %d, want 404", resp.StatusCode)
	}
}

func TestNonexistentRouteReturns404(t *testing.T) {
	srv, _ := newTestServer(t, 10, nil)
	resp, _ := get(t, srv.URL+"/api/foo")
	if resp.StatusCode != http.StatusNotFound {
		t.Errorf("status = %d, want 404", resp.StatusCode)
	}
}

func TestWhitelistRejectsHTTP(t *testing.T) {
	srv, _ := newTestServer(t, 10, []netip.Addr{otherIP})

	resp, body := get(t, srv.URL+"/")
	if resp.StatusCode != http.StatusForbidden {
		t.Fatalf("status = %d, want 403", resp.StatusCode)
	}
	if !strings.Contains(body, "Access Denied") {
		t.Error("rejection page does not mention Access Denied")
	}
}

func TestFreshServerAllowsFirstConnection(t *testing.T) {
	srv, _ := newTestServer(t, 1, nil)
	resp, _ := get(t, srv.URL+"/")
	if resp.StatusCode != http.StatusOK {
		t.Errorf("status = %d, want 200", resp.StatusCode)
	}
}

func TestConnectAndSendText(t *testing.T) {
	srv, _ := newTestServer(t, 10, nil)
	conn := dialWS(t, srv)

	if err := conn.WriteMessage(websocket.TextMessage, []byte("hello world")); err != nil {
		t.Fatalf("write: %v", err)
	}
	ok, length := readAck(t, conn)
	if !ok || length != 11 {
		t.Errorf("ack = ok:%v len:%d, want ok:true len:11", ok, length)
	}
}

func TestUnicodeCharCount(t *testing.T) {
	srv, _ := newTestServer(t, 10, nil)
	conn := dialWS(t, srv)

	if err := conn.WriteMessage(websocket.TextMessage, []byte("你好")); err != nil {
		t.Fatalf("write: %v", err)
	}
	ok, length := readAck(t, conn)
	if !ok || length != 2 {
		t.Errorf("ack = ok:%v len:%d, want ok:true len:2", ok, length)
	}
}

func TestWhitespaceOnlyMessageSkipped(t *testing.T) {
	srv, _ := newTestServer(t, 10, nil)
	conn := dialWS(t, srv)

	if err := conn.WriteMessage(websocket.TextMessage, []byte("   ")); err != nil {
		t.Fatalf("write: %v", err)
	}
	if err := conn.WriteMessage(websocket.TextMessage, []byte("real")); err != nil {
		t.Fatalf("write: %v", err)
	}

	ok, length := readAck(t, conn)
	if !ok || length != 4 {
		t.Errorf("ack = ok:%v len:%d, want ok:true len:4", ok, length)
	}
}

func TestTextIsTrimmed(t *testing.T) {
	srv, _ := newTestServer(t, 10, nil)
	conn := dialWS(t, srv)

	if err := conn.WriteMessage(websocket.TextMessage, []byte("  hello  ")); err != nil {
		t.Fatalf("write: %v", err)
	}
	_, length := readAck(t, conn)
	if length != 5 {
		t.Errorf("len = %d, want 5", length)
	}
}

func TestWhitelistRejectsWebSocket(t *testing.T) {
	srv, _ := newTestServer(t, 10, []netip.Addr{otherIP})
	conn := dialWS(t, srv)

	closeErr := waitForClose(t, conn)
	if closeErr.Code != websocket.CloseTryAgainLater {
		t.Errorf("close code = %d, want %d", closeErr.Code, websocket.CloseTryAgainLater)
	}
	if !strings.Contains(closeErr.Text, "not in the whitelist") {
		t.Errorf("close reason = %q", closeErr.Text)
	}
}

func TestCapacityRejectsWebSocket(t *testing.T) {
	state := access.New(1, nil)
	state.Add(otherIP)
	srv, _ := newTestServerWith(t, state)
	conn := dialWS(t, srv)

	closeErr := waitForClose(t, conn)
	if closeErr.Code != websocket.CloseTryAgainLater {
		t.Errorf("close code = %d, want %d", closeErr.Code, websocket.CloseTryAgainLater)
	}
	if !strings.Contains(closeErr.Text, "capacity") {
		t.Errorf("close reason = %q", closeErr.Text)
	}
}

func TestMultipleMessages(t *testing.T) {
	srv, _ := newTestServer(t, 10, nil)
	conn := dialWS(t, srv)

	for _, text := range []string{"msg0", "msg1", "msg2", "msg3", "msg4"} {
		if err := conn.WriteMessage(websocket.TextMessage, []byte(text)); err != nil {
			t.Fatalf("write %q: %v", text, err)
		}
		ok, length := readAck(t, conn)
		if !ok || length != len(text) {
			t.Errorf("text %q: ack ok:%v len:%d", text, ok, length)
		}
	}
}

func TestPasteChannelReceivesText(t *testing.T) {
	srv, pasteCh := newTestServer(t, 10, nil)
	conn := dialWS(t, srv)

	if err := conn.WriteMessage(websocket.TextMessage, []byte("paste me")); err != nil {
		t.Fatalf("write: %v", err)
	}

	select {
	case text := <-pasteCh:
		if text != "paste me" {
			t.Errorf("paste channel got %q, want %q", text, "paste me")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("timeout waiting for text on the paste channel")
	}
}

func TestConnectionSlotHeldWhileConnected(t *testing.T) {
	state := access.New(1, nil)
	srv, _ := newTestServerWith(t, state)

	conn := dialWS(t, srv)
	if err := conn.WriteMessage(websocket.TextMessage, []byte("hi")); err != nil {
		t.Fatalf("write: %v", err)
	}
	readAck(t, conn)

	if got := state.Count(); got != 1 {
		t.Errorf("connected count = %d, want 1 while the socket is open", got)
	}
}

func TestConnectionSlotReleasedOnDisconnect(t *testing.T) {
	state := access.New(1, nil)
	srv, _ := newTestServerWith(t, state)

	conn := dialWS(t, srv)
	if err := conn.WriteMessage(websocket.TextMessage, []byte("hi")); err != nil {
		t.Fatalf("write: %v", err)
	}
	readAck(t, conn)
	conn.Close()

	deadline := time.Now().Add(3 * time.Second)
	for state.Count() != 0 {
		if time.Now().After(deadline) {
			t.Fatalf("connection slot not released, count = %d", state.Count())
		}
		time.Sleep(10 * time.Millisecond)
	}
}
