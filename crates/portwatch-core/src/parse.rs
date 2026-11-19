//! macOS 명령 출력 파서. 입력은 명령의 표준 출력 텍스트뿐이다.

use std::collections::HashMap;

use crate::snapshot::Listener;

/// `ps -ww -axo pid=,ppid=,pgid=,uid=,tty=,user=,lstart=,command=`의 한 줄.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRow {
    pub pid: u32,
    pub ppid: u32,
    pub pgid: u32,
    pub uid: u32,
    /// 터미널 이름. 터미널이 없으면 `??`.
    pub tty: String,
    pub user: String,
    /// `lstart` 다섯 칸을 공백 하나로 이은 값(예: "Wed Nov 18 09:00:00 2026").
    pub started_at: String,
    pub command: String,
}

/// launchd 서비스 목록의 실행 중인 항목 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchdJob {
    pub pid: u32,
    pub label: String,
}

/// `launchctl print <도메인>/<라벨>`에서 쓰는 값.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceDetail {
    pub path: Option<String>,
    pub program: Option<String>,
    pub arguments: Vec<String>,
    pub working_directory: Option<String>,
    pub state: Option<String>,
    pub pid: Option<u32>,
}

impl ServiceDetail {
    /// 실행 파일 경로. `program`이 없으면 첫 인수.
    pub fn executable(&self) -> Option<&str> {
        self.program
            .as_deref()
            .or_else(|| self.arguments.first().map(String::as_str))
    }
}

/// 헤더의 열 이름 목록. `Local Address`처럼 두 단어인 이름은 하나로 합친다.
fn netstat_columns(header: &str) -> Vec<String> {
    let mut columns: Vec<String> = Vec::new();
    for token in header.split_whitespace() {
        let merge = token == "Address"
            && matches!(
                columns.last().map(String::as_str),
                Some("Local" | "Foreign")
            );
        match columns.last_mut() {
            Some(last) if merge => {
                last.push(' ');
                last.push_str(token);
            }
            _ => columns.push(token.to_owned()),
        }
    }
    columns
}

/// `netstat -anv -p tcp`에서 LISTEN 행을 읽는다.
///
/// 같은 PID·포트·주소의 행(예: tcp4와 tcp6)은 하나로 합치고 `family`를 쉼표로 잇는다.
pub fn netstat_listeners(text: &str) -> Result<Vec<Listener>, String> {
    let mut lines = text.lines();
    let header = lines
        .by_ref()
        .find(|line| line.trim_start().starts_with("Proto"))
        .ok_or("netstat 출력에서 헤더를 찾지 못했습니다.")?;
    let columns = netstat_columns(header);
    let find = |name: &str| {
        columns
            .iter()
            .position(|column| column == name)
            .ok_or(format!("netstat 헤더에 `{name}` 열이 없습니다."))
    };
    let proto_index = find("Proto")?;
    let local_index = find("Local Address")?;
    let state_index = find("(state)")?;
    let pid_index = find("pid")?;

    let mut listeners: Vec<Listener> = Vec::new();
    for line in lines {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let (Some(proto), Some(local), Some(state), Some(pid)) = (
            tokens.get(proto_index),
            tokens.get(local_index),
            tokens.get(state_index),
            tokens.get(pid_index),
        ) else {
            continue;
        };
        if !proto.starts_with("tcp") || *state != "LISTEN" {
            continue;
        }
        let Some((address, port)) = local.rsplit_once('.') else {
            continue;
        };
        let (Ok(port), Ok(pid)) = (port.parse::<u16>(), pid.parse::<u32>()) else {
            continue;
        };

        if let Some(existing) = listeners
            .iter_mut()
            .find(|l| l.pid == pid && l.port == port && l.address == address)
        {
            if !existing.family.split(',').any(|family| family == *proto) {
                existing.family.push(',');
                existing.family.push_str(proto);
            }
            continue;
        }
        listeners.push(Listener {
            port,
            address: address.to_owned(),
            family: (*proto).to_owned(),
            pid,
        });
    }
    Ok(listeners)
}

/// 공백으로 나뉜 앞 `count`개 칸과 나머지 원문을 돌려준다. 나머지의 안쪽 공백은 그대로 둔다.
fn split_fields(line: &str, count: usize) -> Option<(Vec<&str>, &str)> {
    let mut rest = line;
    let mut fields = Vec::with_capacity(count);
    for _ in 0..count {
        rest = rest.trim_start();
        let end = rest.find(char::is_whitespace)?;
        fields.push(&rest[..end]);
        rest = &rest[end..];
    }
    Some((fields, rest.trim()))
}

/// ps 출력을 읽는다. 형식이 맞지 않는 줄 수를 함께 돌려준다.
pub fn ps_rows(text: &str) -> (Vec<ProcessRow>, usize) {
    let mut rows = Vec::new();
    let mut skipped = 0;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        match ps_row(line) {
            Some(row) => rows.push(row),
            None => skipped += 1,
        }
    }
    (rows, skipped)
}

fn ps_row(line: &str) -> Option<ProcessRow> {
    // pid ppid pgid uid tty user, lstart 다섯 칸, 나머지는 명령줄.
    let (fields, command) = split_fields(line, 11)?;
    if command.is_empty() {
        return None;
    }
    Some(ProcessRow {
        pid: fields[0].parse().ok()?,
        ppid: fields[1].parse().ok()?,
        pgid: fields[2].parse().ok()?,
        // macOS의 nobody는 uid가 -2로 나온다. 커널 값(4294967294)과 같게 32비트로 감는다.
        uid: fields[3].parse::<i64>().ok()? as u32,
        tty: fields[4].to_owned(),
        user: fields[5].to_owned(),
        started_at: fields[6..11].join(" "),
        command: command.to_owned(),
    })
}

/// `launchctl print system` 또는 `gui/<uid>`의 `services = { … }` 안에서 실행 중인 항목을 읽는다.
pub fn launchctl_services(text: &str) -> Vec<LaunchdJob> {
    let mut jobs = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if !inside {
            inside = trimmed == "services = {";
            continue;
        }
        if trimmed == "}" {
            break;
        }
        let tokens: Vec<&str> = trimmed.split_whitespace().collect();
        if let [pid, _status, label] = tokens.as_slice() {
            if let Ok(pid) = pid.parse::<u32>() {
                if pid > 0 {
                    jobs.push(LaunchdJob {
                        pid,
                        label: (*label).to_owned(),
                    });
                }
            }
        }
    }
    jobs
}

/// `launchctl print <도메인>/<라벨>`의 맨 바깥 값들을 읽는다.
pub fn launchctl_detail(text: &str) -> ServiceDetail {
    let mut detail = ServiceDetail::default();
    let mut depth = 0usize;
    let mut in_arguments = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "}" {
            depth = depth.saturating_sub(1);
            in_arguments = false;
            continue;
        }
        if in_arguments && depth == 2 {
            detail.arguments.push(trimmed.to_owned());
            continue;
        }
        if let Some(key) = trimmed.strip_suffix(" = {") {
            depth += 1;
            in_arguments = depth == 2 && key == "arguments";
            continue;
        }
        if depth != 1 {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(" = ") else {
            continue;
        };
        let value = value.trim().to_owned();
        match key {
            "path" => detail.path = Some(value),
            "program" => detail.program = Some(value),
            "working directory" => detail.working_directory = Some(value),
            "state" => detail.state = Some(value),
            "pid" => detail.pid = value.parse().ok(),
            _ => {}
        }
    }
    detail
}

/// `lsof -a -p <PID 목록> -d cwd -Fn`에서 PID별 작업 폴더를 읽는다.
pub fn lsof_cwd(text: &str) -> HashMap<u32, String> {
    let mut cwd = HashMap::new();
    let mut current: Option<u32> = None;
    for line in text.lines() {
        if let Some(pid) = line.strip_prefix('p') {
            current = pid.parse().ok();
        } else if let (Some(path), Some(pid)) = (line.strip_prefix('n'), current) {
            cwd.insert(pid, path.to_owned());
        }
    }
    cwd
}

/// `docker ps --no-trunc --format '{{json .}}'`의 컨테이너 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerContainer {
    pub id: String,
    pub name: String,
    /// 호스트 쪽 tcp 포트(중복 없이 오름차순).
    pub host_ports: Vec<u16>,
}

/// `docker ps`의 JSON 줄들을 읽는다. 읽지 못한 줄 수를 함께 돌려준다.
pub fn docker_ps(text: &str) -> (Vec<DockerContainer>, usize) {
    let mut containers = Vec::new();
    let mut skipped = 0;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        match docker_row(line) {
            Some(container) => containers.push(container),
            None => skipped += 1,
        }
    }
    (containers, skipped)
}

fn docker_row(line: &str) -> Option<DockerContainer> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let field = |name: &str| value.get(name).and_then(serde_json::Value::as_str);
    let id = field("ID")?.to_owned();
    // 여러 이름이 있으면 쉼표로 나뉜다. 첫 이름을 쓴다.
    let name = field("Names")?.split(',').next()?.trim().to_owned();
    if id.is_empty() || name.is_empty() {
        return None;
    }
    let mut host_ports: Vec<u16> = field("Ports")
        .unwrap_or("")
        .split(',')
        .flat_map(docker_host_ports)
        .collect();
    host_ports.sort_unstable();
    host_ports.dedup();
    Some(DockerContainer {
        id,
        name,
        host_ports,
    })
}

/// `0.0.0.0:5432->5432/tcp`, `[::]:8000-8001->8000-8001/tcp` 한 칸의 호스트 포트. 호스트 연결이 없거나 tcp가 아니면 없음.
fn docker_host_ports(mapping: &str) -> Vec<u16> {
    let Some((host, container)) = mapping.trim().split_once("->") else {
        return Vec::new();
    };
    if !container.ends_with("/tcp") {
        return Vec::new();
    }
    let Some((_, range)) = host.rsplit_once(':') else {
        return Vec::new();
    };
    let (start, end) = range.split_once('-').unwrap_or((range, range));
    match (start.parse::<u16>(), end.parse::<u16>()) {
        // 잘못된 범위가 수많은 포트로 펼쳐지지 않게 크기를 제한한다.
        (Ok(start), Ok(end)) if start <= end && end - start < 1024 => (start..=end).collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_ps_reads_ids_names_and_tcp_host_ports() {
        let text = concat!(
            r#"{"ID":"a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2","Names":"shop-db","Ports":"0.0.0.0:5432->5432/tcp, [::]:5432->5432/tcp"}"#,
            "\n",
            r#"{"ID":"b2c3d4e5f6a1","Names":"web,alias","Ports":"127.0.0.1:8000-8001->8000-8001/tcp, 0.0.0.0:5353->53/udp, 9000/tcp"}"#,
            "\n",
            r#"{"ID":"c3d4e5f6a1b2","Names":"quiet","Ports":""}"#,
            "\nnot json\n",
        );
        let (containers, skipped) = docker_ps(text);
        assert_eq!(skipped, 1);
        assert_eq!(containers.len(), 3);
        assert_eq!(containers[0].name, "shop-db");
        assert_eq!(containers[0].host_ports, [5432]);
        assert_eq!(containers[1].name, "web");
        assert_eq!(containers[1].host_ports, [8000, 8001]);
        assert!(containers[2].host_ports.is_empty());
    }

    #[test]
    fn docker_port_ranges_are_bounded() {
        assert_eq!(
            docker_host_ports("0.0.0.0:1-65535->1-65535/tcp"),
            Vec::<u16>::new()
        );
        assert_eq!(docker_host_ports("0.0.0.0:9-8->9-8/tcp"), Vec::<u16>::new());
        assert_eq!(docker_host_ports("garbage"), Vec::<u16>::new());
    }

    const NETSTAT: &str = "\
Active Internet connections (including servers)
Proto Recv-Q Send-Q  Local Address          Foreign Address        (state)          rxbytes      txbytes  rhiwat  shiwat    pid   epid state  options           gencnt    flags   flags1 usecnt rtncnt fltrs
tcp4       0      0  127.0.0.1.5173         127.0.0.1.50122        ESTABLISHED          512         2048  131072  131072   2102      0 00102 00000000 0000000000000a01 00000080 04000900      1      0 000001
tcp4       0      0  *.8080                 *.*                    LISTEN                 0            0  131072  131072   1101      0 00180 00000006 0000000000000b01 00000000 00000900      1      0 000000
tcp6       0      0  *.22                   *.*                    LISTEN                 0            0  131072  131072      1      0 00180 00000006 00000000000000ab 00000000 00000800      1      0 000000
tcp4       0      0  *.22                   *.*                    LISTEN                 0            0  131072  131072      1      0 00180 00000006 00000000000000ac 00000000 00000900      1      0 000000
tcp6       0      0  ::1.6379               *.*                    LISTEN                 0            0  131072  131072   1300      0 00100 00000006 0000000000000c02 00000000 00000800      1      0 000000
";

    #[test]
    fn netstat_reads_listen_rows_by_header_names() {
        let listeners = netstat_listeners(NETSTAT).unwrap();
        assert_eq!(listeners.len(), 3);
        assert_eq!(
            listeners[0],
            Listener {
                port: 8080,
                address: "*".into(),
                family: "tcp4".into(),
                pid: 1101
            }
        );
        assert_eq!(listeners[1].family, "tcp6,tcp4");
        assert_eq!(listeners[1].pid, 1);
        assert_eq!(listeners[2].address, "::1");
        assert_eq!(listeners[2].port, 6379);
    }

    #[test]
    fn netstat_without_pid_column_is_an_error() {
        let error =
            netstat_listeners("Proto Recv-Q Send-Q  Local Address  Foreign Address  (state)\n")
                .unwrap_err();
        assert!(error.contains("pid"));
        assert!(netstat_listeners("").is_err());
    }

    #[test]
    fn ps_keeps_command_spacing_and_joins_lstart() {
        let text =
            "  2101  2100  2100   501 ttys001  dev  Wed Nov  4 09:00:01 2026 sh -c vite  --host\n\
                     bad line\n";
        let (rows, skipped) = ps_rows(text);
        assert_eq!(skipped, 1);
        assert_eq!(
            rows[0],
            ProcessRow {
                pid: 2101,
                ppid: 2100,
                pgid: 2100,
                uid: 501,
                tty: "ttys001".into(),
                user: "dev".into(),
                started_at: "Wed Nov 4 09:00:01 2026".into(),
                command: "sh -c vite  --host".into(),
            }
        );
    }

    #[test]
    fn launchctl_services_reads_only_running_jobs_in_the_services_block() {
        let text = "system = {\n\
                    \tactive count = 3\n\
                    \tservices = {\n\
                    \t\t       0      -         com.example.idle\n\
                    \t\t    1100      -         sh.brew.nginx\n\
                    \t\t     147     78         com.apple.Kerberos.kdc\n\
                    \t}\n\
                    \tdisabled services = {\n\
                    \t\t\"sh.brew.nginx\" => enabled\n\
                    \t}\n\
                    }\n";
        assert_eq!(
            launchctl_services(text),
            vec![
                LaunchdJob {
                    pid: 1100,
                    label: "sh.brew.nginx".into()
                },
                LaunchdJob {
                    pid: 147,
                    label: "com.apple.Kerberos.kdc".into()
                },
            ]
        );
    }

    #[test]
    fn launchctl_detail_reads_top_level_values_and_arguments() {
        let text = "system/sh.brew.nginx = {\n\
                    \tactive count = 1\n\
                    \tpath = /Library/LaunchDaemons/sh.brew.nginx.plist\n\
                    \ttype = LaunchDaemon\n\
                    \tstate = running\n\
                    \n\
                    \tprogram = /opt/homebrew/opt/nginx/bin/nginx\n\
                    \targuments = {\n\
                    \t\t/opt/homebrew/opt/nginx/bin/nginx\n\
                    \t\t-g\n\
                    \t\tdaemon off;\n\
                    \t}\n\
                    \n\
                    \tworking directory = /opt/homebrew\n\
                    \tenvironment = {\n\
                    \t\tpid => 99\n\
                    \t}\n\
                    \tpid = 1100\n\
                    }\n";
        let detail = launchctl_detail(text);
        assert_eq!(
            detail.path.as_deref(),
            Some("/Library/LaunchDaemons/sh.brew.nginx.plist")
        );
        assert_eq!(
            detail.executable(),
            Some("/opt/homebrew/opt/nginx/bin/nginx")
        );
        assert_eq!(
            detail.arguments,
            ["/opt/homebrew/opt/nginx/bin/nginx", "-g", "daemon off;"]
        );
        assert_eq!(detail.working_directory.as_deref(), Some("/opt/homebrew"));
        assert_eq!(detail.state.as_deref(), Some("running"));
        assert_eq!(detail.pid, Some(1100));
    }

    #[test]
    fn lsof_maps_pids_to_paths() {
        let cwd =
            lsof_cwd("p2102\nfcwd\nn/Users/dev/work/shop web\np2202\nfcwd\nn/Users/dev/work/api\n");
        assert_eq!(cwd[&2102], "/Users/dev/work/shop web");
        assert_eq!(cwd[&2202], "/Users/dev/work/api");
    }
}
