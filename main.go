package main

import (
	"context"
	"crypto/tls"
	"errors"
	"fmt"
	"io/fs"
	"log/slog"
	"net"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"time"

	"github.com/WantenMN/remote-input/internal/access"
	"github.com/WantenMN/remote-input/internal/banner"
	"github.com/WantenMN/remote-input/internal/cli"
	"github.com/WantenMN/remote-input/internal/netinfo"
	"github.com/WantenMN/remote-input/internal/paste"
	"github.com/WantenMN/remote-input/internal/server"
	"github.com/WantenMN/remote-input/internal/tlscert"
)

var version = "0.3.0"

func main() {
	if err := run(); err != nil {
		if !errors.Is(err, cli.ErrSilent) {
			fmt.Fprintf(os.Stderr, "\n  \x1b[1m\x1b[31mError:\x1b[0m %v\n\n", err)
		}
		os.Exit(1)
	}
}

func run() error {
	args, err := cli.Parse(os.Args[1:])
	switch {
	case errors.Is(err, cli.ErrHelp), errors.Is(err, cli.ErrSilent):
		return err
	case errors.Is(err, cli.ErrVersion):
		fmt.Printf("remote-input %s\n", version)
		return nil
	case err != nil:
		return err
	}

	logger := newLogger()

	if err := paste.CheckPlatform(); err != nil {
		return err
	}

	localIP := netinfo.DetectLANIP()
	delay := time.Duration(args.PasteDelay) * time.Millisecond

	texts := make(chan string, 64)
	pasteDone := make(chan struct{})
	go func() {
		defer close(pasteDone)
		paste.Run(texts, delay, logger)
	}()

	web, err := fs.Sub(webDist, "web-dist")
	if err != nil {
		return fmt.Errorf("embedded frontend missing: %w", err)
	}

	handler, err := server.New(server.Config{
		Web:    web,
		Access: access.New(args.MaxConnections, args.Allow),
		Paste:  texts,
		Logger: logger,
	})
	if err != nil {
		return err
	}

	addr := net.JoinHostPort("0.0.0.0", strconv.Itoa(int(args.Port)))
	listener, err := net.Listen("tcp", addr)
	if err != nil {
		return fmt.Errorf("cannot listen on port %d: %w", args.Port, err)
	}

	srv := &http.Server{
		Handler:           handler,
		ReadHeaderTimeout: 10 * time.Second,
	}

	if !args.HTTP {
		cert, err := tlscert.Ensure(certDir(), localIP)
		if err != nil {
			listener.Close()
			return err
		}
		srv.TLSConfig = &tls.Config{
			Certificates: []tls.Certificate{cert},
			MinVersion:   tls.VersionTLS12,
		}
	}

	banner.Print(os.Stderr, localIP, args.Port, args.HTTP)

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	serveErr := make(chan error, 1)
	go func() {
		if args.HTTP {
			serveErr <- srv.Serve(listener)
		} else {
			serveErr <- srv.ServeTLS(listener, "", "")
		}
	}()

	select {
	case err := <-serveErr:
		if err != nil && !errors.Is(err, http.ErrServerClosed) {
			return err
		}
	case <-ctx.Done():
		logger.Info("Shutdown signal received")
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		defer cancel()
		if err := srv.Shutdown(shutdownCtx); err != nil {
			logger.Warn("Graceful shutdown failed", "error", err)
		}
	}

	close(texts)
	<-pasteDone
	logger.Info("Shutting down.")
	return nil
}

func certDir() string {
	exe, err := os.Executable()
	if err != nil {
		return "remote-input-cert"
	}
	return filepath.Join(filepath.Dir(exe), "remote-input-cert")
}

func newLogger() *slog.Logger {
	level := slog.LevelInfo
	switch strings.ToLower(os.Getenv("LOG_LEVEL")) {
	case "debug":
		level = slog.LevelDebug
	case "warn", "warning":
		level = slog.LevelWarn
	case "error":
		level = slog.LevelError
	}
	logger := slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: level}))
	slog.SetDefault(logger)
	return logger
}
