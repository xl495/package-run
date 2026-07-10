use serde::Serialize;
use std::process::Command;

#[derive(Serialize, Clone)]
pub struct PortInfo {
    pub port: u16,
    pub pid: u32,
    pub name: String,
}

#[tauri::command]
pub fn port_info(port: u16) -> Result<Option<PortInfo>, String> {
    #[cfg(unix)]
    {
        // -F machine-readable output: "p<pid>" and "c<command>" lines.
        let out = Command::new("lsof")
            .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-Fpc"])
            .output()
            .map_err(|e| e.to_string())?;
        let s = String::from_utf8_lossy(&out.stdout);
        let mut pid: Option<u32> = None;
        let mut name = String::new();
        for line in s.lines() {
            if let Some(p) = line.strip_prefix('p') {
                if pid.is_none() {
                    pid = p.parse().ok();
                }
            } else if let Some(c) = line.strip_prefix('c') {
                if name.is_empty() {
                    name = c.to_string();
                }
            }
        }
        Ok(pid.map(|pid| PortInfo { port, pid, name }))
    }
    #[cfg(windows)]
    {
        let out = Command::new("netstat")
            .args(["-ano", "-p", "TCP"])
            .output()
            .map_err(|e| e.to_string())?;
        let s = String::from_utf8_lossy(&out.stdout);
        let needle = format!(":{port}");
        for line in s.lines() {
            if line.contains("LISTENING") && line.split_whitespace().nth(1).is_some_and(|addr| addr.ends_with(&needle)) {
                if let Some(pid) = line.split_whitespace().last().and_then(|p| p.parse().ok()) {
                    let name = Command::new("tasklist")
                        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
                        .output()
                        .ok()
                        .map(|o| {
                            String::from_utf8_lossy(&o.stdout)
                                .split(',')
                                .next()
                                .unwrap_or("")
                                .trim_matches('"')
                                .to_string()
                        })
                        .unwrap_or_default();
                    return Ok(Some(PortInfo { port, pid, name }));
                }
            }
        }
        Ok(None)
    }
}

#[tauri::command]
pub fn kill_port(port: u16) -> Result<PortInfo, String> {
    let info = port_info(port)?.ok_or("ERR_PORT_FREE")?;
    #[cfg(unix)]
    {
        let ret = unsafe { libc::kill(info.pid as i32, libc::SIGKILL) };
        if ret != 0 {
            return Err(format!("ERR_KILL|{}|{}", info.name, info.pid));
        }
    }
    #[cfg(windows)]
    {
        Command::new("taskkill")
            .args(["/F", "/PID", &info.pid.to_string()])
            .output()
            .map_err(|e| e.to_string())?;
    }
    Ok(info)
}
