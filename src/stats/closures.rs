use syn::visit::{self, Visit};

impl Visit<'_> for Stats<'_, '_> {
    fn visit_expr_closure(&mut self, node: &syn::ExprClosure) {
        self.log
            .db
            .execute(
                r"INSERT INTO closures (file_name, version_id) VALUES ($1, $2)",
                &[&self.log.file_name, &self.log.version_id],
            )
            .unwrap();
        visit::visit_expr_closure(self, node);
    }
}

struct Stats<'log, 'db> {
    log: super::Logger<'log, 'db>,
}

pub const RUNNER: super::Runner = super::Runner {
    collect: |file, log| visit::visit_file(&mut Stats { log }, file),
    init: |db| {
        db.batch_execute(
            r#"
            CREATE TABLE closures (
                file_name TEXT,
                version_id UUID references versions(id)
            );
            CREATE INDEX closures_version_index ON closures(version_id);
        "#,
        )
        .unwrap();
    },
};
