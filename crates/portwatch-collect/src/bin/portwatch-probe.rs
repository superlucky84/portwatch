//! 화면 없이 수집을 한 번 실행한다.
//!
//! - 기본: 분류별 표
//! - `--json`: 스냅샷 JSON
//! - `--raw`: 출력 형식 확인용 원문(사용자 이름·홈 경로를 가림)

use portwatch_collect::raw::collect_raw;
use portwatch_collect::report::render_table;
use portwatch_collect::{collect_snapshot, Environment, SystemRunner};

const HELP: &str = "\
portwatch-probe: 로컬 개발 서버 탐지 결과를 출력합니다.

사용법: portwatch-probe [--json | --raw | --help]
  (없음)   분류별 표
  --json   스냅샷 JSON
  --raw    출력 형식 확인용 원문. 사용자 이름은 <user>, 홈 경로는 ~로 가립니다.";

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let env = Environment::current();
    match mode.as_str() {
        "" => print!(
            "{}",
            render_table(&collect_snapshot(), &env.home, &env.user)
        ),
        "--json" => match serde_json::to_string_pretty(&collect_snapshot()) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("스냅샷을 JSON으로 바꾸지 못했습니다: {error}");
                std::process::exit(1);
            }
        },
        "--raw" => {
            if !cfg!(target_os = "macos") {
                eprintln!("--raw는 macOS에서만 의미가 있습니다.");
            }
            for section in collect_raw(&SystemRunner::default(), &env) {
                println!("===== {} =====\n{}\n", section.title, section.body);
            }
        }
        "--help" | "-h" => println!("{HELP}"),
        other => {
            eprintln!("알 수 없는 옵션: {other}\n\n{HELP}");
            std::process::exit(2);
        }
    }
}
