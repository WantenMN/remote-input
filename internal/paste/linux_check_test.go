//go:build linux

package paste

import "testing"

const sampleGroupFile = `root:x:0:
daemon:x:1:
input:x:995:wanten
users:x:100:wanten
`

func TestParseGroupGID(t *testing.T) {
	gid, ok := parseGroupGID(sampleGroupFile, "input")
	if !ok {
		t.Fatal("input group not found")
	}
	if gid != 995 {
		t.Errorf("gid = %d, want 995", gid)
	}
}

func TestParseGroupGIDMissing(t *testing.T) {
	if _, ok := parseGroupGID(sampleGroupFile, "nope"); ok {
		t.Error("found a group that does not exist")
	}
}

func TestParseGroupGIDIgnoresPrefixes(t *testing.T) {
	if _, ok := parseGroupGID(sampleGroupFile, "inpu"); ok {
		t.Error("matched a partial group name")
	}
}
