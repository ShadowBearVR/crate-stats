use crate::sql_enum;
use quote::ToTokens;
use syn::visit::{self, Visit};
use tracing::trace;
#[cfg(test)]
use tracing_test::traced_test;

sql_enum! {
    enum UnsafeCodeType {
        Function,
        Block,
    }
}

#[derive(Default, Debug)]
struct CallParamList {
    params: Vec<String>,
}

impl Visit<'_> for CallParamList {
    fn visit_generic_argument(&mut self, node: &syn::GenericArgument) {
        match node {
            syn::GenericArgument::Type(t) => self.params.push(t.to_token_stream().to_string()),
            _ => {}
        }
    }
}

impl Visit<'_> for Stats<'_, '_> {
    fn visit_item_fn(&mut self, node: &syn::ItemFn) {
        let mut child = Stats {
            log: self.log.fork(),
            count: 0,
        };
        visit::visit_item_fn(&mut child, node);
        let count = child.count;

        self.log.db.execute(
            r"INSERT INTO unsafe_code (unsafe_code_type, block_count, file_name, version_id) VALUES ($1, $2, $3, $4)",
            &[&UnsafeCodeType::Function, &(count as i32), &self.log.file_name, &self.log.version_id],
        ).unwrap();

        trace!(count = count, ty = "Function");
    }

    fn visit_expr_unsafe(&mut self, node: &syn::ExprUnsafe) {
        visit::visit_expr_unsafe(self, node);

        self.count += 1;

        self.log.db.execute(
            r"INSERT INTO unsafe_code (unsafe_code_type, block_count, file_name, version_id) VALUES ($1, $2, $3, $4)",
            &[&UnsafeCodeType::Block, &None::<i32>, &self.log.file_name, &self.log.version_id],
        ).unwrap();

        trace!(ty = "Block");
    }

    fn visit_expr_call(&mut self, node: &syn::ExprCall) {
        visit::visit_expr_call(self, node);

        let syn::Expr::Path(path) = &*node.func else {
            return
        };

        let mut list = CallParamList { params: Vec::new() };

        let func_name = match path.path.segments.last() {
            Some(seg) => {
                visit::visit_path_segment(&mut list, seg);
                seg.ident.to_string()
            }
            None => return,
        };

        if !(func_name == "transmute" || func_name == "transmute_copy") {
            return;
        }

        let from_type = list.params.get(0);
        let to_type = list.params.get(1);

        self.log.db.execute(
            r"INSERT INTO transmutes (from_type, to_type, file_name, version_id) VALUES ($1, $2, $3, $4)",
            &[&from_type, &to_type, &self.log.file_name, &self.log.version_id],
        ).unwrap();

        trace!(transmute = true, from = from_type, to = to_type);
    }
}

struct Stats<'log, 'db> {
    log: super::Logger<'log, 'db>,
    count: usize,
}

pub const RUNNER: super::Runner = super::Runner {
    collect: |file, log| visit::visit_file(&mut Stats { log, count: 0 }, file),
    init: |db| {
        UnsafeCodeType::init(db);
        db.batch_execute(
            r#"
            CREATE TABLE unsafe_code (
                unsafe_code_type "UnsafeCodeType",
                block_count INT,
                file_name TEXT,
                version_id UUID references versions(id)
            );
            CREATE INDEX usafe_code_version_index ON unsafe_code(version_id);
            CREATE TABLE transmutes (
                from_type TEXT,
                to_type TEXT,
                file_name TEXT,
                version_id UUID references versions(id)
            );
            CREATE INDEX transmutes_version_index ON transmutes(version_id);
        "#,
        )
        .unwrap();
    },
};

#[test]
#[traced_test]
fn test_unsafe_fn() {
    RUNNER.collect_mock("unsafe_fn");
    assert!(logs_contain(r#"ty="Block""#));
    assert!(logs_contain(r#"count=4 ty="Function""#));
}

#[test]
#[traced_test]
fn test_transmute_with_arguments() {
    RUNNER.collect_mock("transmute_with_arguments");
    assert!(logs_contain(r#"transmute=true from="[u8 ; 4]" to="u32""#));
}

#[test]
#[traced_test]
fn test_transmute_without_arguments() {
    RUNNER.collect_mock("transmute_without_arguments");
    assert!(logs_contain(r#"transmute=true"#));
}
