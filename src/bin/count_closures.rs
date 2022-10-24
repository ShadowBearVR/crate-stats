// use rayon::{prelude::*};
use glob::glob;
use std::fs;
use std::path::Path;
use std::thread;
use syn::visit::{self, Visit};

struct Stats {
    closure_count: usize,
}

impl Visit<'_> for Stats {
    fn visit_expr_closure(&mut self, node: &syn::ExprClosure) {
        self.closure_count += 1;
        visit::visit_expr_closure(self, node);
    }
}

impl Stats {
    pub fn collect(&mut self, path: impl AsRef<Path>) -> Result<(), syn::Error> {
        match fs::read_to_string(path.as_ref()) {
            Ok(source) => {
                let file = syn::parse_file(&source)?;
                visit::visit_file(self, &file);
            }
            Err(err) => {
                eprintln!("Error reading {}: {}", path.as_ref().display(), err);
            }
        }
        Ok(())
    }
}

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory or file to parse
    #[arg(short, long, default_value = "./download/source")]
    source: String,
}

fn run(mut stats: Stats, source_path: &Path, source: &str) {
    if Path::new(&source).is_file() {
        stats.collect(&source).unwrap();
    } else {
        for path in glob(&format!("{source}/**/*.rs")).unwrap() {
            let path = path.unwrap().canonicalize().unwrap();
            if let Err(err) = stats.collect(&path) {
                eprintln!(
                    "Error parsing {}:{}: {}",
                    path.display(),
                    err.span().start().line,
                    err
                );
            } else {
                // eprintln!("Parsing {}", path.display());
            }
        }
    }
    println!("Closures are used {} times", stats.closure_count);
}

fn main() {
    let Args { source } = clap::Parser::parse();
    let source_path = Path::new(&source).canonicalize().unwrap();
    let stats = Stats { closure_count: 0 };

    thread::Builder::new()
        .stack_size(16 * 1024 * 1026)
        .spawn(move || {
            run(stats, &source_path, &source);
        })
        .unwrap()
        .join()
        .unwrap()
}
