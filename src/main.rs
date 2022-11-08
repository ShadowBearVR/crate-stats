use glob::glob;
use humantime::format_rfc3339_seconds;
use syn::spanned::Spanned;
// use rayon::{prelude::*};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;
use syn::visit::{self, Visit};

#[derive(Debug, serde::Serialize, PartialEq, Eq)]
enum SyntaxType {
    TraitDef,
    ImplFor,
    TypeImpl,
    TypeDyn,
    WhereClause,
}

#[derive(Debug, Clone, Copy, serde::Serialize, PartialEq, Eq)]
enum Position {
    Argument,
    Return,
}

#[derive(Debug, serde::Serialize, PartialEq, Eq)]
struct Row {
    syntax: SyntaxType,
    position: Option<Position>,
    generic_count: usize,
    at_count: usize,
    trait_name: String,
    crate_name: String,
    location: String,
}

#[derive(Default, Debug)]
struct TraitParamCounter {
    generic_count: usize,
    at_count: usize,
}

impl Visit<'_> for TraitParamCounter {
    fn visit_generic_argument(&mut self, node: &syn::GenericArgument) {
        match node {
            syn::GenericArgument::Lifetime(_) => {}
            syn::GenericArgument::Type(_) => self.generic_count += 1,
            syn::GenericArgument::Const(_) => {}
            syn::GenericArgument::Binding(_) => self.at_count += 1,
            syn::GenericArgument::Constraint(_) => {}
        }
    }

    fn visit_parenthesized_generic_arguments(&mut self, node: &syn::ParenthesizedGenericArguments) {
        self.generic_count += node.inputs.len();
        match node.output {
            syn::ReturnType::Default => {}
            syn::ReturnType::Type(_, _) => self.at_count += 1,
        }
    }
}

struct PositionalStats<'stats, R: Rows> {
    stats: &'stats mut Stats<R>,
    position: Position,
}

impl<R: Rows> Visit<'_> for PositionalStats<'_, R> {
    fn visit_type_impl_trait(&mut self, node: &syn::TypeImplTrait) {
        for bound in &node.bounds {
            self.stats
                .collect_type_param_bound(bound, SyntaxType::TypeImpl, Some(self.position))
        }
    }

    fn visit_type_trait_object(&mut self, node: &syn::TypeTraitObject) {
        for bound in &node.bounds {
            self.stats
                .collect_type_param_bound(bound, SyntaxType::TypeDyn, Some(self.position))
        }
    }
}

trait Rows {
    fn push(&mut self, row: Row);
}

impl<W: Write> Rows for csv::Writer<W> {
    fn push(&mut self, row: Row) {
        self.serialize(row).unwrap();
    }
}

impl Rows for Vec<Row> {
    fn push(&mut self, row: Row) {
        Vec::push(self, row);
    }
}

impl<R: Rows> Rows for Arc<Mutex<R>> {
    fn push(&mut self, row: Row) {
        self.lock().unwrap().push(row);
    }
}

struct Stats<R: Rows> {
    rows: R,
    crate_name: String,
    file_name: String,
}

impl<R: Rows> Visit<'_> for Stats<R> {
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

    fn visit_predicate_type(&mut self, node: &syn::PredicateType) {
        for bound in &node.bounds {
            self.collect_type_param_bound(bound, SyntaxType::WhereClause, None)
        }
    }

    fn visit_item_impl(&mut self, node: &syn::ItemImpl) {
        let (_, path, _) = match &node.trait_ {
            Some(t) => t,
            None => return,
        };

        let base_at_count = node
            .items
            .iter()
            .filter(|i| matches!(i, syn::ImplItem::Type(_)))
            .count();

        self.collect_trait_path(path, SyntaxType::ImplFor, None, base_at_count)
    }

    fn visit_item_trait(&mut self, node: &syn::ItemTrait) {
        let at_count = node
            .items
            .iter()
            .filter(|i| matches!(i, syn::TraitItem::Type(_)))
            .count();

        let generic_count = node
            .generics
            .params
            .iter()
            .filter(|i| matches!(i, syn::GenericParam::Type(_)))
            .count();

        self.rows.push(Row {
            syntax: SyntaxType::TraitDef,
            position: None,
            generic_count,
            at_count,
            trait_name: node.ident.to_string(),
            crate_name: self.crate_name.clone(),
            location: format!("{}:{}", &self.file_name, node.ident.span().start().line),
        });
    }
}

impl<R: Rows> Stats<R> {
    fn collect_type_param_bound(
        &mut self,
        bound: &syn::TypeParamBound,
        syntax: SyntaxType,
        position: Option<Position>,
    ) {
        let trait_bound = match bound {
            syn::TypeParamBound::Trait(t) => t,
            syn::TypeParamBound::Lifetime(_) => return,
        };

        self.collect_trait_path(&trait_bound.path, syntax, position, 0)
    }

    fn collect_trait_path(
        &mut self,
        path: &syn::Path,
        syntax: SyntaxType,
        position: Option<Position>,
        base_at_count: usize,
    ) {
        let trait_name = match path.segments.last() {
            Some(seg) => seg.ident.to_string(),
            None => return,
        };

        match trait_name.as_str() {
            "Sync" | "Send" | "Copy" | "Sized" | "Unpin" => return,
            _ => (),
        }

        let mut counter = TraitParamCounter::default();

        visit::visit_path(&mut counter, path);

        self.rows.push(Row {
            syntax,
            position,
            generic_count: counter.generic_count,
            at_count: counter.at_count + base_at_count,
            trait_name,
            crate_name: self.crate_name.clone(),
            location: format!("{}:{}", &self.file_name, path.span().start().line),
        });
    }

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

#[cfg(test)]
fn collect_mock(name: &str) -> Vec<Row> {
    let file_name = format!("./mocks/{name}.rs");
    let mut stats = Stats {
        rows: Vec::<Row>::new(),
        crate_name: name.to_string(),
        file_name: file_name.clone(),
    };
    stats.collect(&file_name).unwrap();
    return stats.rows;
}

#[test]
fn test_impl_for() {
    assert_eq!(
        collect_mock("impl_for"),
        vec![Row {
            syntax: SyntaxType::ImplFor,
            position: None,
            generic_count: 0,
            at_count: 1,
            trait_name: "Iterator".to_string(),
            crate_name: "impl_for".to_string(),
            location: "./mocks/impl_for.rs:3".to_string(),
        }]
    )
}

#[test]
fn test_iterator_arg() {
    assert_eq!(
        collect_mock("iterator_arg"),
        vec![Row {
            syntax: SyntaxType::TypeImpl,
            position: Some(Position::Argument),
            generic_count: 0,
            at_count: 1,
            trait_name: "Iterator".to_string(),
            crate_name: "iterator_arg".to_string(),
            location: "./mocks/iterator_arg.rs:1".to_string(),
        }]
    )
}

#[test]
fn test_iterator_ret() {
    assert_eq!(
        collect_mock("iterator_ret"),
        vec![Row {
            syntax: SyntaxType::TypeImpl,
            position: Some(Position::Return),
            generic_count: 0,
            at_count: 1,
            trait_name: "Iterator".to_string(),
            crate_name: "iterator_ret".to_string(),
            location: "./mocks/iterator_ret.rs:1".to_string(),
        }]
    )
}

#[test]
fn test_dyn_iterator_arg() {
    assert_eq!(
        collect_mock("dyn_iterator_arg"),
        vec![Row {
            syntax: SyntaxType::TypeDyn,
            position: Some(Position::Argument),
            generic_count: 0,
            at_count: 1,
            trait_name: "Iterator".to_string(),
            crate_name: "dyn_iterator_arg".to_string(),
            location: "./mocks/dyn_iterator_arg.rs:1".to_string(),
        }]
    )
}

#[test]
fn test_many_generics() {
    assert_eq!(
        collect_mock("many_generics"),
        vec![
            Row {
                syntax: SyntaxType::TraitDef,
                position: None,
                generic_count: 3,
                at_count: 1,
                trait_name: "Mock".to_string(),
                crate_name: "many_generics".to_string(),
                location: "./mocks/many_generics.rs:1".to_string(),
            },
            Row {
                syntax: SyntaxType::TypeImpl,
                position: Some(Position::Argument),
                generic_count: 3,
                at_count: 0,
                trait_name: "Mock".to_string(),
                crate_name: "many_generics".to_string(),
                location: "./mocks/many_generics.rs:5".to_string(),
            }
        ]
    )
}

#[test]
fn test_where_clause() {
    assert_eq!(
        collect_mock("where_clause"),
        vec![Row {
            syntax: SyntaxType::WhereClause,
            position: None,
            generic_count: 0,
            at_count: 1,
            trait_name: "Iterator".to_string(),
            crate_name: "where_clause".to_string(),
            location: "./mocks/where_clause.rs:3".to_string(),
        }]
    )
}

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

fn run(mut stats: Stats<impl Rows>, source_path: &Path, source: &str) {
    if Path::new(&source).is_file() {
        stats.collect(&source).unwrap();
    } else {
        for path in glob(&format!("{source}/**/*.rs")).unwrap() {
            let path = path.unwrap().canonicalize().unwrap();
            let mut components = path.strip_prefix(&source_path).unwrap().components();
            let crate_name = components.next().unwrap();
            let file_name = path.strip_prefix(&source_path.join(crate_name)).unwrap();
            if components.any(|s| {
                let s = s.as_os_str().to_str().unwrap();
                let s = s.trim_end_matches(".rs");
                s == "test" || s == "tests" || s == "test_data"
            }) {
                // Don't include test files
                continue;
            }
            stats.crate_name = crate_name.as_os_str().to_str().unwrap().to_string();
            stats.file_name = file_name.display().to_string();
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

fn main() {
    let Args { source, output } = clap::Parser::parse();
    let source_path = Path::new(&source).canonicalize().unwrap();
    match output.parent() {
        Some(dir) => {
            fs::create_dir_all(dir).unwrap();
        }
        None => {}
    };

    let now = SystemTime::now();
    let now_timestamp = format_rfc3339_seconds(now.into());

    let hostname = hostname::get().unwrap();
    let hostname = hostname.to_str().unwrap();
    let hostname = hostname.to_string();
    let hostname = hostname.split(".").next().unwrap_or("");

    let mut output_filename = output.file_stem().unwrap().to_os_string();
    output_filename.push("_");
    output_filename.push(now_timestamp.to_string());
    output_filename.push("_");
    output_filename.push(hostname);
    output_filename.push(".csv");

    let mut output = output.clone();
    output.set_file_name(output_filename);

    let output = csv::Writer::from_path(output).unwrap();
    let stats = Stats {
        rows: output,
        crate_name: "".to_string(),
        file_name: "".to_string(),
    };

    thread::Builder::new()
        .stack_size(16 * 1024 * 1026)
        .spawn(move || {
            run(stats, &source_path, &source);
        })
        .unwrap()
        .join()
        .unwrap()
}
