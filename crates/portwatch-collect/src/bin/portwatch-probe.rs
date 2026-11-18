//! 화면 없이 현재 스냅샷을 JSON으로 출력한다.

fn main() {
    let snapshot = portwatch_collect::collect_snapshot();
    match serde_json::to_string_pretty(&snapshot) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("스냅샷을 JSON으로 바꾸지 못했습니다: {error}");
            std::process::exit(1);
        }
    }
}
