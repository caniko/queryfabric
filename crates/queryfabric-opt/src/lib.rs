use queryfabric_catalog::Catalog;
use queryfabric_ir::{BoundQuery, QueryDiagnostic, Result};

pub mod federation;
pub mod jobs;
mod syntax;

/// Conservative rewrite advisory emitted by normalization passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteAdvisory {
    pub code: String,
    pub message: String,
}

pub trait OptimizationPass: Send + Sync {
    fn name(&self) -> &'static str;
    fn apply(&self, query: BoundQuery, catalog: &dyn Catalog) -> Result<BoundQuery>;
}

pub use self::syntax::{
    SyntaxTransformer, apply_selection_overrides, flatten_boolean_and, rebuild_boolean_and,
    transform_expr_children, transform_join_children, transform_order_by_expr_children,
    transform_projection_item_children, transform_query_children, transform_relation_children,
    transform_select_children, transform_set_expr_children, transform_table_with_joins_children,
    transform_when_then_children,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct IdentityPass;

impl OptimizationPass for IdentityPass {
    fn name(&self) -> &'static str {
        "identity"
    }

    fn apply(&self, query: BoundQuery, _catalog: &dyn Catalog) -> Result<BoundQuery> {
        Ok(query)
    }
}

#[derive(Default)]
pub struct OptimizationPipeline {
    passes: Vec<Box<dyn OptimizationPass>>,
}

impl OptimizationPipeline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_pass(mut self, pass: impl OptimizationPass + 'static) -> Self {
        self.passes.push(Box::new(pass));
        self
    }

    pub fn normalize(&self, mut query: BoundQuery, catalog: &dyn Catalog) -> Result<BoundQuery> {
        let mut advisories = Vec::new();
        for pass in &self.passes {
            advisories.push(RewriteAdvisory {
                code: format!("QFNORM:{}", pass.name()),
                message: format!("Applied normalization pass `{}`.", pass.name()),
            });
            query = pass.apply(query, catalog)?;
        }
        if !advisories.is_empty() {
            let mut diagnostics = query.diagnostics().to_vec();
            diagnostics.extend(
                advisories
                    .into_iter()
                    .map(|advisory| QueryDiagnostic::note(advisory.code, advisory.message)),
            );
            query = query.with_diagnostics(diagnostics);
        }
        Ok(query)
    }
}
