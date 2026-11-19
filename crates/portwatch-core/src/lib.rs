//! portwatch 코어.
//!
//! 명령 출력 텍스트를 받아 값을 돌려주는 순수 함수만 둔다.
//! 프로세스 실행, 파일 시스템, 네트워크, macOS API를 쓰지 않는다.

pub mod actions;
pub mod cert;
pub mod domains;
pub mod hosts;
pub mod nginx;
pub mod parse;
pub mod prefs;
pub mod privacy;
pub mod project;
pub mod resolve;
pub mod settings;
pub mod snapshot;
pub mod stop;

pub use snapshot::{
    Category, Cert, Domain, DomainVia, Entry, Listener, Manager, ManagerKind, ProcessInfo, Project,
    Protection, Scheme, Snapshot,
};
