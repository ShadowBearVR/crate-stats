use crate::sql_enum;
use quote::ToTokens;
use syn::{
    spanned::Spanned,
    visit::{self, Visit},
};
use tracing::trace;
#[cfg(test)]
use tracing_test::traced_test;

sql_enum! {
    enum AsyncCodeType {
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
            outermost: false,
        };
        visit::visit_item_fn(&mut child, node);

        if node.sig.asyncness.is_none() {
            return;
        }

        let count = child.count;

        self.log.db.execute(
            r"INSERT INTO async_code (async_code_type, block_count, file_name, first_line_number, last_line_number, outermost, version_id)
            VALUES                   ($1,              $2,          $3,        $4,                $5,               $6,        $7)",
            &[
                &AsyncCodeType::Function,
                &(count as i32),
                &self.log.file_name,
                &(node.span().start().line as i32),
                &(node.span().end().line as i32),
                &self.outermost,
                &self.log.version_id],
        ).unwrap();

        trace!(count = count, ty = "Function", outermost = self.outermost);
    }

    fn visit_expr_async(&mut self, node: &syn::ExprAsync) {
        let mut child = Stats {
            log: self.log.fork(),
            count: 0,
            outermost: false,
        };
        visit::visit_expr_async(&mut child, node);
        self.count += child.count;
        self.count += 1;

        self.log.db.execute(
            r"INSERT INTO async_code (async_code_type, block_count, file_name, first_line_number, last_line_number, outermost, version_id)
            VALUES                   ($1,              $2,          $3,        $4,                $5,               $6,        $7)",
            &[
                &AsyncCodeType::Block,
                &None::<i32>,
                &self.log.file_name,
                &(node.span().start().line as i32),
                &(node.span().end().line as i32),
                &self.outermost,
                &self.log.version_id],
        ).unwrap();

        trace!(ty = "Block", outermost = self.outermost);
    }
}

struct Stats<'log, 'db> {
    log: super::Logger<'log, 'db>,
    count: usize,
    outermost: bool,
}

pub const RUNNER: super::Runner = super::Runner {
    collect: |file, log| {
        visit::visit_file(
            &mut Stats {
                log,
                count: 0,
                outermost: true,
            },
            file,
        )
    },
    init: |db| {
        AsyncCodeType::init(db);
        db.batch_execute(
            r#"
            CREATE TABLE async_code (
                async_code_type "AsyncCodeType",
                block_count INT,
                first_line_number INT,
                last_line_number INT,
                outermost BOOL,
                file_name TEXT,
                version_id UUID references versions(id)
            );
            CREATE INDEX async_code_version_index ON async_code(version_id);
        "#,
        )
        .unwrap();
    },
};

#[test]
#[traced_test]
fn test_async_fn() {
    RUNNER.collect_mock("async_fn");
    assert!(logs_contain(r#"ty="Block" outermost=false"#));
    assert!(!logs_contain(r#"ty="Block" outermost=true"#));
    assert!(logs_contain(r#"count=4 ty="Function" outermost=true"#));
}
