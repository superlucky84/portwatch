//! `portwatch-probe --raw`: 출력 형식 확인용 원문. 사용자 이름과 홈 경로를 가리고,
//! listener와 그 조상에 해당하는 줄과 형식 확인용 앞부분만 남긴다.

use std::collections::{HashMap, HashSet};

use portwatch_core::privacy::sanitize;
use portwatch_core::resolve::{plan_lookups, Facts};

use crate::{
    detail_args, facts_from_first_pass, first_pass, lsof_args, CommandOutput, Environment, Runner,
    LAUNCHCTL, LSOF, NETSTAT, PS,
};

/// 원문 한 묶음.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSection {
    pub title: String,
    pub body: String,
}

/// listener 프로세스와 그 조상 PID.
fn related_pids(facts: &Facts) -> HashSet<u32> {
    let parents: HashMap<u32, u32> = facts.processes.iter().map(|p| (p.pid, p.ppid)).collect();
    let mut pids = HashSet::from([1]);
    for listener in &facts.listeners {
        let mut current = listener.pid;
        while pids.insert(current) {
            match parents.get(&current) {
                Some(&parent) if parent > 1 => current = parent,
                _ => break,
            }
        }
    }
    pids
}

fn first_number(line: &str) -> Option<u32> {
    line.split_whitespace().next()?.parse().ok()
}

fn netstat_body(text: &str) -> String {
    text.lines()
        .enumerate()
        .filter(|(index, line)| *index < 2 || line.contains(" LISTEN "))
        .map(|(_, line)| line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn ps_body(text: &str, pids: &HashSet<u32>) -> String {
    text.lines()
        .filter(|line| first_number(line).is_some_and(|pid| pids.contains(&pid)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 서비스 목록: 앞부분 15줄, `services` 블록의 처음 3줄과 관련 PID 줄.
fn launchctl_list_body(text: &str, pids: &HashSet<u32>) -> String {
    let mut out = Vec::new();
    let mut inside = false;
    let mut shown_in_block = 0;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if !inside {
            if trimmed == "services = {" {
                inside = true;
                out.push("\t…".to_owned());
                out.push(line.to_owned());
            } else if index < 15 {
                out.push(line.to_owned());
            }
            continue;
        }
        if trimmed == "}" {
            out.push("\t\t…".to_owned());
            out.push(line.to_owned());
            out.push("\t… (이후 생략)".to_owned());
            break;
        }
        let related = first_number(line).is_some_and(|pid| pids.contains(&pid));
        if shown_in_block < 3 || related {
            out.push(line.to_owned());
            shown_in_block += 1;
        }
    }
    out.join("\n")
}

/// 서비스 상세: 환경 변수 블록은 값이 비밀일 수 있어 뺀다.
fn detail_body(text: &str) -> String {
    let mut out = Vec::new();
    let mut skip_depth = 0usize;
    for line in text.lines() {
        let trimmed = line.trim();
        if skip_depth > 0 {
            if trimmed.ends_with('{') {
                skip_depth += 1;
            } else if trimmed == "}" {
                skip_depth -= 1;
            }
            continue;
        }
        if trimmed.ends_with(" = {") && trimmed.contains("environment") {
            out.push(line.replace(" = {", " = { (환경 변수 생략) }"));
            skip_depth = 1;
            continue;
        }
        out.push(line.to_owned());
    }
    out.join("\n")
}

fn body(result: &Result<CommandOutput, String>, shape: impl Fn(&str) -> String) -> String {
    match result {
        Ok(output) if output.success => shape(&output.stdout),
        Ok(output) => format!(
            "실패(종료 코드 0이 아님): {}\n{}",
            output.stderr.trim(),
            shape(&output.stdout)
        ),
        Err(error) => format!("실행 실패: {error}"),
    }
}

/// 원문 묶음을 만든다.
pub fn collect_raw(runner: &dyn Runner, env: &Environment) -> Vec<RawSection> {
    let pass = first_pass(runner, env);
    let facts = facts_from_first_pass(&pass, env);
    let pids = related_pids(&facts);
    let lookups = plan_lookups(&facts);

    let mut sections = vec![
        RawSection {
            title: format!("{} {} (헤더와 LISTEN 행)", NETSTAT.0, NETSTAT.1.join(" ")),
            body: body(&pass.netstat, netstat_body),
        },
        RawSection {
            title: format!("{} {} (listener와 조상)", PS.0, PS.1.join(" ")),
            body: body(&pass.ps, |text| ps_body(text, &pids)),
        },
        RawSection {
            title: format!("{LAUNCHCTL} print system (앞부분과 관련 서비스)"),
            body: body(&pass.system, |text| launchctl_list_body(text, &pids)),
        },
        RawSection {
            title: format!("{LAUNCHCTL} print gui/{} (앞부분과 관련 서비스)", env.uid),
            body: body(&pass.gui, |text| launchctl_list_body(text, &pids)),
        },
    ];

    for target in &lookups.services {
        let args = detail_args(target, env.uid);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        sections.push(RawSection {
            title: format!("{LAUNCHCTL} {}", args.join(" ")),
            body: body(&runner.run(LAUNCHCTL, &refs), detail_body),
        });
    }

    if !lookups.cwd_pids.is_empty() {
        let args = lsof_args(&lookups.cwd_pids);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        sections.push(RawSection {
            title: format!("{LSOF} {}", args.join(" ")),
            body: match runner.run(LSOF, &refs) {
                Ok(output) => output.stdout,
                Err(error) => format!("실행 실패: {error}"),
            },
        });
    }

    sections
        .into_iter()
        .map(|section| RawSection {
            title: sanitize(&section.title, &env.home, &env.user),
            body: sanitize(&section.body, &env.home, &env.user),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::*;

    #[test]
    fn raw_keeps_formats_and_hides_the_user() {
        let sections = collect_raw(&FakeRunner::default(), &env());
        let all = sections
            .iter()
            .map(|s| format!("{}\n{}", s.title, s.body))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(!all.contains("/Users/dev"), "홈 경로를 가린다");
        assert!(all.contains("~/work/api"));
        assert!(all.contains("login -pf <user>"));
        assert!(!all.contains("PATH =>"), "환경 변수 블록을 뺀다");
        assert!(all.contains("default environment = { (환경 변수 생략) }"));

        let netstat = &sections[0].body;
        assert!(netstat.contains("Proto Recv-Q"));
        assert!(!netstat.contains("ESTABLISHED"));

        let ps = &sections[1].body;
        assert!(ps.contains("nginx: worker process"));
        assert!(
            !ps.contains("esbuild"),
            "listener의 조상이 아닌 프로세스는 뺀다"
        );

        let system = &sections[2].body;
        assert!(system.contains("sh.brew.nginx"));
        assert!(system.contains("(이후 생략)"));
        assert!(!system.contains("disabled services"));

        assert_eq!(
            sections.len(),
            4 + 4 + 1,
            "목록 4개, 서비스 상세 4개, lsof 1개"
        );
    }
}
