use syn::visit::{self, Visit};

impl Visit<'_> for Stats<'_, '_> {
    fn visit_expr_closure(&mut self, node: &syn::ExprClosure) {
        self.log
            .db
            .execute(
                r"INSERT INTO closures (file_name, is_try_like, version_id) VALUES ($1, $2, $3)",
                &[&self.log.file_name, &self.is_in_call, &self.log.version_id],
            )
            .unwrap();

        let mut child = Stats {
            log: self.log.fork(),
            is_in_call: false,
        };
        visit::visit_expr_closure(&mut child, node);
    }

    fn visit_expr_call(&mut self, node: &syn::ExprCall) {
        let mut child = Stats {
            log: self.log.fork(),
            is_in_call: true,
        };
        visit::visit_expr(&mut child, &node.func);

        let mut child = Stats {
            log: self.log.fork(),
            is_in_call: false,
        };
        for arg in &node.args {
            visit::visit_expr(&mut child, arg)
        }
    }
}

struct Stats<'log, 'db> {
    log: super::Logger<'log, 'db>,
    is_in_call: bool,
}

pub const RUNNER: super::Runner = super::Runner {
    collect: |file, log| {
        visit::visit_file(
            &mut Stats {
                log,
                is_in_call: false,
            },
            file,
        )
    },
    init: |db| {
        db.batch_execute(
            r#"
            CREATE TABLE closures (
                file_name TEXT,
                is_try_like BOOLEAN,
                version_id UUID references versions(id)
            );
            CREATE INDEX closures_version_index ON closures(version_id);
        "#,
        )
        .unwrap();
    },
};
