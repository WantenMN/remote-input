package paste

import (
	"log/slog"
	"time"
)

type Injector interface {
	Inject(text string) error
	Close()
}

func Run(texts <-chan string, delay time.Duration, logger *slog.Logger) {
	injector := newInjector(delay)
	defer injector.Close()

	for text := range texts {
		if err := injector.Inject(text); err != nil {
			logger.Error("Injection failed", "error", err)
			continue
		}
		logger.Info("Text delivered")
	}
}
