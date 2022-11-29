use rusqlite::Connection;
use std::{fmt::Display, fs, rc::Rc};
use syn::visit::{self, Visit};

pub mod traits;

pub trait Stats: for<'a> Visit<'a> + Sized + 'static {
    const NEW: NewStats = |db| Box::new(Self::new(db.clone()));

    fn new(db: Rc<Connection>) -> Self;
    fn init(&self);
    fn set_location(
        &mut self,
        crate_name: impl Display,
        file_name: impl Display,
        date_str: impl Display,
    );
}

pub trait DynStats {
    fn init(&self);
    fn set_location(&mut self, crate_name: String, file_name: String, date_str: String);
    fn collect(&mut self, path: &std::path::Path) -> Result<(), syn::Error>;
    fn collect_syntax(&mut self, file: &syn::File);
}

impl<S> DynStats for S
where
    S: Stats,
{
    fn init(&self) {
        Stats::init(self);
    }

    fn set_location(&mut self, crate_name: String, file_name: String, date_str: String) {
        Stats::set_location(self, crate_name, file_name, date_str);
    }

    fn collect_syntax(&mut self, file: &syn::File) {
        self.visit_file(file);
    }

    fn collect(&mut self, path: &std::path::Path) -> Result<(), syn::Error> {
        match fs::read_to_string(path) {
            Ok(source) => {
                let file = syn::parse_file(&source)?;
                visit::visit_file(self, &file);
            }
            Err(err) => {
                eprintln!("Error reading {}: {}", path.display(), err);
            }
        }
        Ok(())
    }
}

pub type AnyStats = Box<dyn DynStats>;
pub type NewStats = fn(&Rc<Connection>) -> AnyStats;
pub const ALL_STATS: &[NewStats] = &[traits::Stats::NEW];
