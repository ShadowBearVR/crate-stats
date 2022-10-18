use anyhow::Error;
use glob::glob;
use std::{fs, path::Path};
use syn::visit::{visit_file, Visit};

#[derive(clap::Parser, Debug)]
#[command(name = "download_crates", author, version, about, long_about = None)]
struct Args {
    /// Directory or file to parse
    #[arg(default_value = "./download/source")]
    source: String,
}

#[derive(Default, Debug)]
struct Stats {
    impl_trait_arguments: usize,
}

impl Visit<'_> for Stats {
    fn visit_type_impl_trait(&mut self, i: &syn::TypeImplTrait) {
        self.impl_trait_arguments += 1;
    }
}

impl Stats {
    pub fn collect(&mut self, path: &impl AsRef<Path>) -> Result<(), Error> {
        let source = fs::read_to_string(path)?;
        let file = syn::parse_file(&source)?;
        visit_file(self, &file);
        Ok(())
    }
}

fn main() {
    let Args { source } = clap::Parser::parse();
    let mut stats = Stats::default();
    if Path::new(&source).is_file() {
        stats.collect(&source).unwrap();
    } else {
        for path in glob(&format!("{source}/**/*.rs")).unwrap() {
            let path = path.unwrap();
            if let Err(err) = stats.collect(&path) {
                eprintln!("Error parsing {}: {:?}", path.display(), err);
            } else {
                eprintln!("Parsing {}", path.display());
            }
        }
    }

    dbg!(stats);
}
