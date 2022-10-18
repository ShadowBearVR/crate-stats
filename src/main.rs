use glob::glob;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use syn::visit::{self, Visit};

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory or file to parse
    #[arg(short, long, default_value = "./download/source")]
    source: String,
    /// CSV file to write output
    #[arg(short, long, default_value = "./output/syntax.csv")]
    output: PathBuf,
}

#[derive(Debug)]
struct Stats {
    rows: csv::Writer<File>,
    crate_name: String,
}

#[derive(Debug, serde::Serialize)]
enum SyntaxType {
    Def,
    Impl,
    Dyn,
    Where,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
enum Position {
    Argument,
    Return,
}

#[derive(Debug, serde::Serialize)]
struct Row {
    syntax: SyntaxType,
    position: Option<Position>,
    generic_count: usize,
    atb_count: usize,
    trait_name: String,
    crate_name: String,
}

#[derive(Default, Debug)]
struct TraitParamCounter {
    generic_count: usize,
    atb_count: usize,
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

impl Visit<'_> for TraitParamCounter {
    fn visit_generic_argument(&mut self, node: &syn::GenericArgument) {
        match node {
            syn::GenericArgument::Lifetime(_) => {}
            syn::GenericArgument::Type(_) => self.generic_count += 1,
            syn::GenericArgument::Const(_) => {}
            syn::GenericArgument::Binding(_) => self.atb_count += 1,
            syn::GenericArgument::Constraint(_) => {}
        }
    }

    fn visit_parenthesized_generic_arguments(&mut self, node: &syn::ParenthesizedGenericArguments) {
        self.generic_count += node.inputs.len();
        match node.output {
            syn::ReturnType::Default => {}
            syn::ReturnType::Type(_, _) => self.atb_count += 1,
        }
    }
}

impl Visit<'_> for PositionalStats<'_> {
    fn visit_type_impl_trait(&mut self, node: &syn::TypeImplTrait) {
        for bound in &node.bounds {
            let trait_bound = match bound {
                syn::TypeParamBound::Trait(t) => t,
                syn::TypeParamBound::Lifetime(_) => continue,
            };

            let trait_name = match trait_bound.path.segments.last() {
                Some(seg) => seg.ident.to_string(),
                None => continue,
            };

            match trait_name.as_str() {
                "Sync" | "Send" | "Copy" | "Sized" | "Unpin" => continue,
                _ => (),
            }

            let mut counter = TraitParamCounter::default();

            visit::visit_trait_bound(&mut counter, trait_bound);

            self.stats
                .rows
                .serialize(Row {
                    syntax: SyntaxType::Impl,
                    position: Some(self.position),
                    generic_count: counter.generic_count,
                    atb_count: counter.atb_count,
                    trait_name,
                    crate_name: self.stats.crate_name.clone(),
                })
                .unwrap();
        }
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
    let Args { source, output } = clap::Parser::parse();
    let source_path = Path::new(&source).canonicalize().unwrap();
    match output.parent() {
        Some(dir) => {
            fs::create_dir_all(dir).unwrap();
        }
        None => {}
    };
    let output = csv::Writer::from_path(output).unwrap();
    let mut stats = Stats {
        rows: output,
        crate_name: "".to_string(),
    };
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
}
