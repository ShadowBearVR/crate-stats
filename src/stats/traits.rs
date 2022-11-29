use crate::sql_enum;
use rusqlite::Connection;
use std::{fmt::Display, rc::Rc};
use syn::visit::{self, Visit};
use tracing::trace;

#[cfg(test)]
use super::DynStats as _;
#[cfg(test)]
use tracing_test::traced_test;

sql_enum! {
    enum SyntaxType {
        TraitDef,
        ImplFor,
        TypeImpl,
        TypeDyn,
        WhereClause,
    }
}

sql_enum! {
    enum Position {
        Argument,
        Return,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Row {
    syntax: SyntaxType,
    position: Option<Position>,
    generic_count: usize,
    at_count: usize,
    trait_name: String,
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

struct PositionalStats<'stats> {
    stats: &'stats mut Stats,
    position: Position,
}

impl Visit<'_> for PositionalStats<'_> {
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

pub struct Stats {
    db: Rc<Connection>,
    crate_name: String,
    file_name: String,
    date_str: String,
}

impl Stats {
    pub fn push(&self, row: Row) {
        trace!(row = ?row);
        self.db
            .execute(
                "INSERT INTO traits
                (syntax, position, at_count, generic_count, trait_name, crate_name, file_name, data_str)
                VALUES
                (?,      ?,        ?,        ?,             ?,          ?,          ?,         ?)",
                (
                    row.syntax,
                    row.position,
                    row.at_count,
                    row.generic_count,
                    row.trait_name,
                    &self.crate_name,
                    &self.file_name,
                    &self.date_str,
                ),
            )
            .unwrap();
    }
}

impl super::Stats for Stats {
    fn new(db: Rc<Connection>) -> Self {
        Stats {
            db,
            crate_name: "".to_string(),
            file_name: "".to_string(),
            date_str: "".to_string(),
        }
    }

    fn set_location(
        &mut self,
        crate_name: impl Display,
        file_name: impl Display,
        date_str: impl Display,
    ) {
        self.crate_name = crate_name.to_string();
        self.file_name = file_name.to_string();
        self.date_str = date_str.to_string();
    }

    fn init(&self) {
        self.db
            .execute_batch(
                "CREATE TABLE traits (
                    syntax TEXT,
                    position TEXT,
                    at_count INT,
                    generic_count INT,
                    trait_name TEXT,
                    crate_name TEXT,
                    file_name TEXT,
                    data_str TEXT
                )",
            )
            .unwrap();
    }
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

        self.push(Row {
            syntax: SyntaxType::TraitDef,
            position: None,
            generic_count,
            at_count,
            trait_name: node.ident.to_string(),
        });
    }
}

impl Stats {
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

        self.push(Row {
            syntax,
            position,
            generic_count: counter.generic_count,
            at_count: counter.at_count + base_at_count,
            trait_name,
        });
    }
}

#[cfg(test)]
fn collect_mock(name: &str) {
    let file_name = format!("./mocks/{name}.rs");
    let mut stats = Stats {
        db: Rc::new(Connection::open_in_memory().unwrap()),
        crate_name: name.to_string(),
        file_name: file_name.clone(),
        date_str: "".to_string(),
    };
    stats.init();
    stats.collect(file_name.as_ref()).unwrap();
}

#[test]
#[traced_test]
fn test_impl_for() {
    collect_mock("impl_for");
    vec![Row {
        syntax: SyntaxType::ImplFor,
        position: None,
        generic_count: 0,
        at_count: 1,
        trait_name: "Iterator".to_string(),
    }];
}

#[test]
#[traced_test]
fn test_iterator_arg() {
    collect_mock("iterator_arg");
    assert!(logs_contain(
        r#"row=Row { syntax: TypeImpl, position: Some(Argument), generic_count: 0, at_count: 1, trait_name: "Iterator" }"#,
    ));
}

#[test]
#[traced_test]
fn test_iterator_ret() {
    collect_mock("iterator_ret");
    assert!(logs_contain(
        r#"row=Row { syntax: TypeImpl, position: Some(Return), generic_count: 0, at_count: 1, trait_name: "Iterator" }"#,
    ));
}

#[test]
#[traced_test]
fn test_dyn_iterator_arg() {
    collect_mock("dyn_iterator_arg");
    assert!(logs_contain(
        r#"row=Row { syntax: TypeDyn, position: Some(Argument), generic_count: 0, at_count: 1, trait_name: "Iterator" }"#,
    ));
}

#[test]
#[traced_test]
fn test_many_generics() {
    collect_mock("many_generics");
    assert!(logs_contain(
        r#"row=Row { syntax: TraitDef, position: None, generic_count: 3, at_count: 1, trait_name: "Mock" }"#,
    ));
    assert!(logs_contain(
        r#"row=Row { syntax: TypeImpl, position: Some(Argument), generic_count: 3, at_count: 0, trait_name: "Mock" }"#,
    ));
}

#[test]
#[traced_test]
fn test_where_clause() {
    collect_mock("where_clause");
    assert!(logs_contain(
        r#"row=Row { syntax: WhereClause, position: None, generic_count: 0, at_count: 1, trait_name: "Iterator" }"#,
    ));
}
