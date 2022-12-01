use postgres::{Client, NoTls, Transaction};
use std::fs;
use std::path::Path;
use uuid::Uuid;

pub mod traits;
pub mod closures;

pub struct Logger<'a, 'db> {
    pub db: &'a mut Transaction<'db>,
    pub file_name: &'a str,
    pub version_id: Uuid,
}

impl<'a, 'db> Logger<'a, 'db> {
    pub fn fork<'b>(&'b mut self) -> Logger<'b, 'db> {
        Logger {
            db: self.db,
            file_name: self.file_name,
            version_id: self.version_id,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Runner {
    pub init: fn(db: &mut Transaction),
    pub collect: fn(file: &syn::File, log: Logger),
}

impl Runner {
    #[allow(unused)]
    pub fn collect_mock(&self, name: &str) {
        let mut cli = Client::connect("crate-stats-test", NoTls).unwrap();
        let mut tx = cli.transaction().unwrap();
        self.collect_path(
            format!("./mocks/{name}.rs"),
            Logger {
                db: &mut tx,
                file_name: "",
                version_id: Uuid::default(),
            },
        );
        tx.rollback().unwrap();
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

pub const ALL_RUNNERS: &[Runner] = &[traits::RUNNER, closures::RUNNER];
