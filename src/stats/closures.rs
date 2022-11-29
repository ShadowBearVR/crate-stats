use syn::visit::{self, Visit};

impl Visit<'_> for Stats<'_, '_> {
    fn visit_expr_closure(&mut self, node: &syn::ExprClosure) {
        self.log.db.execute(
            r"INSERT INTO closures (crate_name, file_name, date_str) VALUES ($1, $2, $3)",
            &[&self.log.crate_name, &self.log.file_name, &self.log.date_str],
        ).unwrap();
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
            r#"CREATE TABLE closures (
                    crate_name TEXT,
                    file_name TEXT,
                    date_str TEXT
            )"#,
        )
        .unwrap();
    },
};
