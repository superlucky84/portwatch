//! 실제 명령 실행과 파일 확인.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::{CommandOutput, Environment, Files, Runner};

/// 셸을 거치지 않고 절대 경로의 명령을 실행한다. 정해진 시간을 넘기면 멈춘다.
#[derive(Debug, Clone)]
pub struct SystemRunner {
    pub timeout: Duration,
}

impl Default for SystemRunner {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
        }
    }
}

impl Runner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String> {
        let mut child = Command::new(program)
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("{program} 실행 실패: {error}"))?;

        // 출력이 파이프 버퍼보다 크면 읽지 않는 동안 명령이 멈추므로 따로 읽는다.
        let mut stdout = child.stdout.take().expect("piped stdout");
        let mut stderr = child.stderr.take().expect("piped stderr");
        let out_reader = std::thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = stdout.read_to_end(&mut buffer);
            buffer
        });
        let err_reader = std::thread::spawn(move || {
            let mut buffer = Vec::new();
            let _ = stderr.read_to_end(&mut buffer);
            buffer
        });

        let deadline = Instant::now() + self.timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "{program}이(가) {}초 안에 끝나지 않았습니다.",
                        self.timeout.as_secs()
                    ));
                }
                Err(error) => return Err(format!("{program} 대기 실패: {error}")),
            }
        };
        let stdout = out_reader.join().unwrap_or_default();
        let stderr = err_reader.join().unwrap_or_default();
        Ok(CommandOutput {
            success: status.success(),
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        })
    }
}

/// 실제 파일 시스템. 읽기만 한다.
#[derive(Debug, Clone, Copy, Default)]
pub struct RealFiles;

impl Files for RealFiles {
    fn exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }

    fn read(&self, path: &str) -> Option<String> {
        let metadata = std::fs::metadata(path).ok()?;
        // package.json·hosts·nginx 설정만 읽으므로 1 MB가 넘는 파일은 건너뛴다.
        if !metadata.is_file() || metadata.len() > 1_000_000 {
            return None;
        }
        std::fs::read_to_string(path).ok()
    }

    fn list_dir(&self, dir: &str) -> Vec<String> {
        let Ok(read) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut names: Vec<String> = read
            .flatten()
            .filter(|entry| entry.path().is_file())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect();
        names.sort();
        names
    }
}

extern "C" {
    fn getuid() -> u32;
}

pub(crate) fn current_environment() -> Environment {
    // SAFETY: getuid는 인수가 없고 실패하지 않는다.
    let uid = unsafe { getuid() };
    Environment {
        home: std::env::var("HOME").unwrap_or_default(),
        user: std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_default(),
        uid,
        self_pid: std::process::id(),
        now_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or(0),
        security_names: Environment::default_security_names(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_a_command_without_a_shell_and_with_c_locale() {
        let output = SystemRunner::default()
            .run("/usr/bin/env", &[])
            .expect("env 실행");
        assert!(output.success);
        assert!(output.stdout.contains("LC_ALL=C"));
        assert!(
            !output.stdout.contains("HOME="),
            "환경을 비우고 필요한 값만 준다"
        );
    }

    #[test]
    fn large_output_does_not_block() {
        let output = SystemRunner::default()
            .run("/bin/sh", &["-c", "yes portwatch | head -n 200000"])
            .expect("큰 출력");
        assert_eq!(output.stdout.lines().count(), 200_000);
    }

    #[test]
    fn slow_commands_time_out() {
        let runner = SystemRunner {
            timeout: Duration::from_millis(200),
        };
        let error = runner.run("/bin/sleep", &["5"]).unwrap_err();
        assert!(error.contains("끝나지 않았습니다"));
    }

    #[test]
    fn current_environment_has_a_uid_and_pid() {
        let env = current_environment();
        assert_eq!(env.self_pid, std::process::id());
        assert!(env.now_ms > 0);
    }
}
