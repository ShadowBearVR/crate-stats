use glob::glob;
use std::{fs, path::Path};
use syn::visit::{self, Visit};

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory or file to parse
    #[arg(default_value = "./download/source")]
    source: String,
}

#[derive(Default, Debug)]
struct Stats {
    rows: Vec<Row>,
    crate_name: String,
}

#[derive(Debug)]
enum SyntaxType {
    Def,
    Impl,
    Dyn,
    Where,
}

#[derive(Debug, Clone, Copy)]
enum Position {
    Argument,
    Return,
}

#[derive(Debug)]
struct Row {
    syntax: SyntaxType,
    position: Option<Position>,
    generic_count: usize,
    atb_count: usize,
    trait_name: String,
    crate_name: String,
}

struct PositionalStats<'stats> {
    stats: &'stats mut Stats,
    position: Position,
}

impl Visit<'_> for Stats {
    fn visit_fn_arg(&mut self, node: &syn::FnArg) {
        visit::visit_fn_arg(
            &mut PositionalStats {
                stats: self,
                position: Position::Argument,
            },
            node,
        )
    }

    fn visit_return_type(&mut self, node: &syn::ReturnType) {
        visit::visit_return_type(
            &mut PositionalStats {
                stats: self,
                position: Position::Return,
            },
            node,
        )
    }
}

impl Visit<'_> for PositionalStats<'_> {
    fn visit_type_impl_trait(&mut self, i: &syn::TypeImplTrait) {
        self.stats.rows.push(Row {
            syntax: SyntaxType::Impl,
            position: Some(self.position),
            generic_count: 0,
            atb_count: 0,
            trait_name: "".to_string(),
            crate_name: self.stats.crate_name.clone(),
        })
    }
}

impl Stats {
    pub fn collect(&mut self, path: &impl AsRef<Path>) -> Result<(), syn::Error> {
        let source = fs::read_to_string(path).unwrap();
        let file = syn::parse_file(&source)?;
        visit::visit_file(self, &file);
        Ok(())
    }
}

fn main() {
    let Args { source } = clap::Parser::parse();
    let source_path = Path::new(&source).canonicalize().unwrap();
    let mut stats = Stats::default();
    if Path::new(&source).is_file() {
        stats.collect(&source).unwrap();
    } else {
        for path in glob(&format!("{source}/**/*.rs")).unwrap() {
            let path = path.unwrap().canonicalize().unwrap();
            let crate_name = path
                .strip_prefix(&source_path)
                .unwrap()
                .components()
                .next()
                .unwrap();
            stats.crate_name = crate_name.as_os_str().to_str().unwrap().to_string();
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

    dbg!(stats);
}
