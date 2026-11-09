//go:build linux

package paste

import (
	"fmt"
	"os"
	"strconv"
	"strings"
)

func CheckPlatform() error {
	data, err := os.ReadFile("/etc/group")
	if err != nil {
		return nil
	}

	inputGID, ok := parseGroupGID(string(data), "input")
	if !ok {
		return nil
	}
	if os.Getgid() == inputGID {
		return nil
	}
	if groups, err := os.Getgroups(); err == nil {
		for _, gid := range groups {
			if gid == inputGID {
				return nil
			}
		}
	}

	return fmt.Errorf("you are not in the 'input' group (GID %d).\n"+
		"Run: sudo usermod -aG input $USER\n"+
		"Then log out and back in for the change to take effect", inputGID)
}

func parseGroupGID(data, name string) (int, bool) {
	for _, line := range strings.Split(data, "\n") {
		fields := strings.Split(line, ":")
		if len(fields) < 3 || fields[0] != name {
			continue
		}
		gid, err := strconv.Atoi(fields[2])
		if err != nil {
			return 0, false
		}
		return gid, true
	}
	return 0, false
}
