use postgres::{Client, NoTls, Transaction};
use std::fs;
use std::path::Path;
use uuid::Uuid;

pub mod async_code;
pub mod closures;
pub mod traits;
pub mod unsafe_code;

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

pub fn global_init(tx: &mut Transaction) {
    tx.batch_execute(
        r#"CREATE TABLE versions (
                id UUID PRIMARY KEY,
                crate_name TEXT,
                date_str TEXT
        );"#,
    )
    .unwrap();
}

#[derive(Clone, Copy)]
pub struct Runner {
    pub init: fn(db: &mut Transaction),
    pub collect: fn(file: &syn::File, log: Logger),
}

impl Runner {
    #[allow(unused)]
    pub fn collect_mock(&self, name: &str) {
        let mut cli = Client::connect(
            "dbname=crate-stats-test host=localhost user=macdonald",
            NoTls,
        )
        .unwrap();
        let mut tx = cli.transaction().unwrap();
        global_init(&mut tx);
        (self.init)(&mut tx);

        let version_id = Uuid::new_v4();

        tx.execute(
            r"INSERT INTO versions (id, crate_name, date_str) VALUES ($1, $2, $3)",
            &[&version_id, &name, &""],
        )
        .unwrap();

        assert!(
            self.collect_path(
                format!("./mocks/{name}.rs"),
                Logger {
                    db: &mut tx,
                    file_name: "",
                    version_id,
                },
            ),
            "could not parse mock"
        );
        tx.rollback().unwrap();
    }

    pub fn collect_path(&self, path: impl AsRef<Path>, log: Logger) -> bool {
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(err) => {
                eprintln!("Error reading {}: {}", path.as_ref().display(), err);
                return false;
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
                return false;
            }
        };
        (self.collect)(&file, log);
        true
    }

    pub fn collect_syntax(&self, file: &syn::File, log: Logger) {
        (self.collect)(file, log)
    }
}

pub const ALL_RUNNERS: &[Runner] = &[
    traits::RUNNER,
    closures::RUNNER,
    unsafe_code::RUNNER,
    async_code::RUNNER,
];
