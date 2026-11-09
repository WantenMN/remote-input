//go:generate pnpm --dir web install
//go:generate pnpm --dir web run build
package main

import "embed"

//go:embed all:web-dist
var webDist embed.FS
