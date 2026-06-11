/// Verify the current user is in the `input` group (required for /dev/uinput).
pub fn check_input_group() -> Result<(), String> {
    let status = std::fs::read_to_string("/proc/self/status")
        .map_err(|e| format!("Cannot read /proc/self/status: {e}"))?;

    let groups_line = status
        .lines()
        .find(|l| l.starts_with("Groups:"))
        .ok_or("Cannot find Groups in /proc/self/status")?;

    let sup_gids: Vec<u32> = groups_line
        .split_once(':')
        .map(|(_, v)| v)
        .unwrap_or("")
        .split_whitespace()
        .filter_map(|g| g.parse().ok())
        .collect();

    let group_file = std::fs::read_to_string("/etc/group")
        .map_err(|e| format!("Cannot read /etc/group: {e}"))?;

    let input_gid: Option<u32> = group_file
        .lines()
        .filter_map(|line| {
            let mut parts = line.split(':');
            let name = parts.next()?;
            if name == "input" {
                parts.nth(1)?.parse().ok()
            } else {
                None
            }
        })
        .next();

    let input_gid = match input_gid {
        Some(g) => g,
        None => return Ok(()),
    };

    if sup_gids.contains(&input_gid) {
        return Ok(());
    }

    if get_primary_gid() == Some(input_gid) {
        return Ok(());
    }

    Err(format!(
        "You are not in the 'input' group (GID {input_gid}).\n\
         Run: sudo usermod -aG input $USER\n\
         Then log out and back in for the change to take effect."
    ))
}

/// Get the current user's primary GID from /etc/passwd.
fn get_primary_gid() -> Option<u32> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let uid: u32 = status
        .lines()
        .find(|l| l.starts_with("Uid:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())?;

    let passwd = std::fs::read_to_string("/etc/passwd").ok()?;
    for line in passwd.lines() {
        let mut parts = line.split(':');
        parts.next()?; // name
        parts.next()?; // password
        let uid_str = parts.next()?;
        if uid_str.parse::<u32>().ok() == Some(uid) {
            return parts.next()?.parse().ok(); // GID
        }
    }
    None
}
