package server

import (
	"fmt"
	"io/fs"
	"log/slog"
	"net/http"

	"github.com/WantenMN/remote-input/internal/access"
)

type Config struct {
	Web    fs.FS
	Access *access.State
	Paste  chan<- string
	Logger *slog.Logger
}

type handler struct {
	cfg    Config
	assets fs.FS
	index  []byte
}

func New(cfg Config) (http.Handler, error) {
	index, err := fs.ReadFile(cfg.Web, "index.html")
	if err != nil {
		return nil, fmt.Errorf("embedded frontend is missing index.html: %w", err)
	}
	assets, err := fs.Sub(cfg.Web, "assets")
	if err != nil {
		return nil, fmt.Errorf("embedded frontend is missing assets/: %w", err)
	}

	h := &handler{cfg: cfg, assets: assets, index: index}

	mux := http.NewServeMux()
	mux.HandleFunc("GET /{$}", h.serveIndex)
	mux.HandleFunc("GET /assets/{path...}", h.serveAsset)
	mux.HandleFunc("GET /ws", h.serveWebSocket)
	return mux, nil
}

func (h *handler) serveIndex(w http.ResponseWriter, r *http.Request) {
	if rejection := h.cfg.Access.Check(clientIP(r)); rejection != nil {
		h.reject(w, r, rejection)
		return
	}
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.Write(h.index)
}

func (h *handler) serveAsset(w http.ResponseWriter, r *http.Request) {
	if rejection := h.cfg.Access.Check(clientIP(r)); rejection != nil {
		h.reject(w, r, rejection)
		return
	}

	name := r.PathValue("path")
	data, err := fs.ReadFile(h.assets, name)
	if err != nil {
		http.NotFound(w, r)
		return
	}

	w.Header().Set("Content-Type", contentType(name))
	w.Write(data)
}

func (h *handler) reject(w http.ResponseWriter, r *http.Request, rejection *access.Rejection) {
	h.cfg.Logger.Warn("Rejected request",
		"ip", rejection.IP, "reason", reasonText(rejection))

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	w.WriteHeader(http.StatusForbidden)
	fmt.Fprint(w, rejectPage(rejection))
}

func reasonText(rejection *access.Rejection) string {
	if rejection.Kind == access.NotWhitelisted {
		return "not in whitelist"
	}
	return fmt.Sprintf("limit reached (%d/%d)", rejection.Current, rejection.Max)
}
