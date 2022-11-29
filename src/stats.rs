use rusqlite::Connection;
use std::fs;
use std::path::Path;

pub mod traits;

#[derive(Copy, Clone)]
pub struct Logger<'a> {
    pub tx: &'a Connection,
    pub crate_name: &'a str,
    pub file_name: &'a str,
    pub date_str: &'a str,
}

impl<'a> std::ops::Deref for Logger<'a> {
    type Target = Connection;

    fn deref(&self) -> &Connection {
        self.tx
    }
}

#[derive(Clone, Copy)]
pub struct Runner {
    pub init: fn(con: &Connection),
    pub collect: fn(file: &syn::File, log: Logger),
}

impl Runner {
#[allow(unused)]
    pub fn collect_mock(&self, name: &str) {
        let con = Connection::open_in_memory().unwrap();
        self.collect_path(
            format!("./mocks/{name}.rs"),
            Logger {
                tx: &con,
                crate_name: "",
                file_name: "",
                date_str: "",
            },
        );
    }

    pub fn collect_path(&self, path: impl AsRef<Path>, log: Logger) {
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(err) => {
                eprintln!("Error reading {}: {}", path.as_ref().display(), err);
                return;
            }
        };
        let file = match syn::parse_file(&source) {
            Ok(f) => f,
            Err(err) => {
                eprintln!(
                    "Error parsing {}:{}: {}",
                    path.as_ref().display(),
                    err.span().start().line,
                    err
                );
                return;
            }
        };
        (self.collect)(&file, log);
    }

    pub fn collect_syntax(&self, file: &syn::File, log: Logger) {
        (self.collect)(file, log)
    }
}

pub const ALL_RUNNERS: &[Runner] = &[traits::RUNNER];
