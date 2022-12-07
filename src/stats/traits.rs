use crate::sql_enum;
use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use tracing::trace;

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
    enum PositionType {
        Argument,
        Return,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Row {
    syntax: SyntaxType,
    position: Option<PositionType>,
    generic_count: usize,
    at_count: usize,
    gat_count: Option<usize>,
    trait_name: String,
    trait_bounds_count: usize,
    lifetime_bounds_count: usize,
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

struct PositionalStats<'log, 'db> {
    stats: Stats<'log, 'db>,
    position: PositionType,
}

impl Visit<'_> for PositionalStats<'_, '_> {
    fn visit_type_impl_trait(&mut self, node: &syn::TypeImplTrait) {
        let trait_bounds_count = node
            .bounds
            .iter()
            .filter(|b| matches!(b, syn::TypeParamBound::Trait(_)))
            .count();
        let lifetime_bounds_count = node.bounds.len() - trait_bounds_count;
        for bound in &node.bounds {
            // TODO: Deal with Lifetime
            self.stats.collect_type_param_bound(
                bound,
                SyntaxType::TypeImpl,
                Some(self.position),
                trait_bounds_count,
                lifetime_bounds_count,
            )
        }
    }

    fn visit_type_trait_object(&mut self, node: &syn::TypeTraitObject) {
        let trait_bounds_count = node
            .bounds
            .iter()
            .filter(|b| matches!(b, syn::TypeParamBound::Trait(_)))
            .count();
        let lifetime_bounds_count = node.bounds.len() - trait_bounds_count;
        for bound in &node.bounds {
            // TODO: Deal with Lifetime
            self.stats.collect_type_param_bound(
                bound,
                SyntaxType::TypeDyn,
                Some(self.position),
                trait_bounds_count,
                lifetime_bounds_count,
            )
        }
    }
}

pub struct Stats<'log, 'db> {
    log: super::Logger<'log, 'db>,
}

impl<'log, 'db> Stats<'log, 'db> {
    pub fn push(&mut self, row: Row, span: Span) {
        trace!(row = ?row);
        self.log
            .db
            .execute(
                "INSERT INTO traits
                (syntax, position, at_count, gat_count, generic_count, trait_bounds_count, lifetime_bounds_count, trait_name, file_name, line_number, version_id)
                VALUES
                ($1,     $2,       $3,       $4,        $5,            $6,                $7,                    $8,         $9,        $10,         $11)",
                &[
                    &row.syntax,
                    &row.position,
                    &(row.at_count as i32),
                    &(row.gat_count.map(|i| i as i32)),
                    &(row.generic_count as i32),
                    &(row.trait_bounds_count as i32),
                    &(row.lifetime_bounds_count as i32),
                    &row.trait_name,
                    &self.log.file_name,
                    &(span.start().line as i32),
                    &self.log.version_id,
                ],
            )
            .unwrap();
    }
}

impl Visit<'_> for Stats<'_, '_> {
    fn visit_fn_arg(&mut self, node: &syn::FnArg) {
        visit::visit_fn_arg(
            &mut PositionalStats {
                stats: Stats {
                    log: self.log.fork(),
                },
                position: PositionType::Argument,
            },
            node,
        )
    }

    fn visit_return_type(&mut self, node: &syn::ReturnType) {
        visit::visit_return_type(
            &mut PositionalStats {
                stats: Stats {
                    log: self.log.fork(),
                },
                position: PositionType::Return,
            },
            node,
        )
    }

    fn visit_predicate_type(&mut self, node: &syn::PredicateType) {
        let trait_bounds_count = node
            .bounds
            .iter()
            .filter(|b| matches!(b, syn::TypeParamBound::Trait(_)))
            .count();
        let lifetime_bounds_count = node.bounds.len() - trait_bounds_count;
        for bound in &node.bounds {
            // TODO: Deal with Lifetime
            self.collect_type_param_bound(
                bound,
                SyntaxType::WhereClause,
                None,
                trait_bounds_count,
                lifetime_bounds_count,
            )
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

        let gat_count = node
            .items
            .iter()
            .filter(|i| {
                let syn::ImplItem::Type(x) = i else {
                    return false
                };

                !x.generics.params.is_empty()
            })
            .count();

        self.collect_trait_path(
            path,
            SyntaxType::ImplFor,
            None,
            base_at_count,
            Some(gat_count),
            0,
            0,
        )
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

        let gat_count = node
            .items
            .iter()
            .filter(|i| {
                let syn::TraitItem::Type(x) = i else {
                    return false
                };

                !x.generics.params.is_empty()
            })
            .count();

        self.push(
            Row {
                syntax: SyntaxType::TraitDef,
                position: None,
                generic_count,
                gat_count: Some(gat_count),
                at_count,
                trait_name: node.ident.to_string(),
                trait_bounds_count: 0,
                lifetime_bounds_count: 0,
            },
            node.ident.span(),
        );
    }
}

impl Stats<'_, '_> {
    fn collect_type_param_bound(
        &mut self,
        bound: &syn::TypeParamBound,
        syntax: SyntaxType,
        position: Option<PositionType>,
        trait_bounds_count: usize,
        lifetime_bounds_count: usize,
    ) {
        let trait_bound = match bound {
            syn::TypeParamBound::Trait(t) => t,
            syn::TypeParamBound::Lifetime(_) => return,
        };

        self.collect_trait_path(
            &trait_bound.path,
            syntax,
            position,
            0,
            None,
            trait_bounds_count,
            lifetime_bounds_count,
        )
    }

    fn collect_trait_path(
        &mut self,
        path: &syn::Path,
        syntax: SyntaxType,
        position: Option<PositionType>,
        base_at_count: usize,
        gat_count: Option<usize>,
        trait_bounds_count: usize,
        lifetime_bounds_count: usize,
    ) {
        let trait_name = match path.segments.last() {
            Some(seg) => seg.ident.to_string(),
            None => return,
        };

        let mut counter = TraitParamCounter::default();

        visit::visit_path(&mut counter, path);

        self.push(
            Row {
                syntax,
                position,
                generic_count: counter.generic_count,
                at_count: counter.at_count + base_at_count,
                gat_count,
                trait_name,
                trait_bounds_count,
                lifetime_bounds_count,
            },
            path.span(),
        );
    }
}

pub const RUNNER: super::Runner = super::Runner {
    collect: |file, log| visit::visit_file(&mut Stats { log }, file),
    init: |db| {
        SyntaxType::init(db);
        PositionType::init(db);
        db.batch_execute(
            r#"
            CREATE TABLE traits (
                syntax "SyntaxType",
                position "PositionType",
                at_count INT,
                gat_count INT,
                generic_count INT,
                trait_bounds_count INT,
                lifetime_bounds_count INT,
                trait_name TEXT,
                line_number INT,
                file_name TEXT,
                version_id UUID references versions(id)
            );
            CREATE INDEX traits_version_index ON traits(version_id);
            CREATE INDEX traits_name_index ON traits(trait_name);
        "#,
        )
        .unwrap();
    },
};

#[test]
#[traced_test]
fn test_impl_for() {
    RUNNER.collect_mock("impl_for");
    assert!(logs_contain(
        r#"row=Row { syntax: ImplFor, position: None, generic_count: 0, at_count: 1, gat_count: Some(0), trait_name: "Iterator", trait_bounds_count: 0, lifetime_bounds_count: 0 }"#,
    ));
}

#[test]
#[traced_test]
fn test_iterator_arg() {
    RUNNER.collect_mock("iterator_arg");
    assert!(logs_contain(
        r#"row=Row { syntax: TypeImpl, position: Some(Argument), generic_count: 0, at_count: 1, gat_count: None, trait_name: "Iterator", trait_bounds_count: 1, lifetime_bounds_count: 0 }"#,
    ));
}

#[test]
#[traced_test]
fn test_iterator_ret() {
    RUNNER.collect_mock("iterator_ret");
    assert!(logs_contain(
        r#"row=Row { syntax: TypeImpl, position: Some(Return), generic_count: 0, at_count: 1, gat_count: None, trait_name: "Iterator", trait_bounds_count: 1, lifetime_bounds_count: 0 }"#,
    ));
}

#[test]
#[traced_test]
fn test_iterator_ret_lifetime_bounds() {
    RUNNER.collect_mock("iterator_ret");
    assert!(logs_contain(
        r#"row=Row { syntax: TypeImpl, position: Some(Return), generic_count: 0, at_count: 1, gat_count: None, trait_name: "Iterator", trait_bounds_count: 1, lifetime_bounds_count: 2 }"#,
    ));
}

#[test]
#[traced_test]
fn test_dyn_iterator_arg() {
    RUNNER.collect_mock("dyn_iterator_arg");
    assert!(logs_contain(
        r#"row=Row { syntax: TypeDyn, position: Some(Argument), generic_count: 0, at_count: 1, gat_count: None, trait_name: "Iterator", trait_bounds_count: 1, lifetime_bounds_count: 0 }"#,
    ));
}

#[test]
#[traced_test]
fn test_many_generics() {
    RUNNER.collect_mock("many_generics");
    assert!(logs_contain(
        r#"row=Row { syntax: TraitDef, position: None, generic_count: 3, at_count: 1, gat_count: Some(0), trait_name: "Mock", trait_bounds_count: 0, lifetime_bounds_count: 0 }"#,
    ));
    assert!(logs_contain(
        r#"row=Row { syntax: TypeImpl, position: Some(Argument), generic_count: 3, at_count: 0, gat_count: None, trait_name: "Mock", trait_bounds_count: 1, lifetime_bounds_count: 0 }"#,
    ));
}

#[test]
#[traced_test]
fn test_define_gat() {
    RUNNER.collect_mock("define_gat");
    assert!(logs_contain(
        r#"row=Row { syntax: TraitDef, position: None, generic_count: 0, at_count: 1, gat_count: Some(1), trait_name: "LendingIterator", trait_bounds_count: 0, lifetime_bounds_count: 0 }"#,
    ));
}

#[test]
#[traced_test]
fn test_where_clause() {
    RUNNER.collect_mock("where_clause");
    assert!(logs_contain(
        r#"row=Row { syntax: WhereClause, position: None, generic_count: 0, at_count: 1, gat_count: None, trait_name: "Iterator", trait_bounds_count: 1, lifetime_bounds_count: 0 }"#,
    ));
}
