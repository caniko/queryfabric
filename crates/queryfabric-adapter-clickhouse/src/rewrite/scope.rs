use std::collections::BTreeSet;

use queryfabric_catalog::{RelationKind, RelationSchema};
use queryfabric_ir::{BoundExprKind, BoundFunctionCall, FunctionRef, QueryDiagnostic, SyntaxNode};

#[derive(Debug, Clone)]
pub(crate) struct ScopeBinding {
    pub(crate) binding_name: String,
    pub(crate) relation_display: String,
    pub(crate) relation: RelationSchema,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct SelectScope {
    pub(crate) bindings: Vec<ScopeBinding>,
}

impl SelectScope {
    pub(crate) fn push(
        &mut self,
        binding_name: String,
        relation_display: String,
        relation: RelationSchema,
    ) {
        self.bindings.push(ScopeBinding {
            binding_name,
            relation_display,
            relation,
        });
    }

    pub(crate) fn target_binding_name(&self, qualifier: Option<&str>) -> Option<&str> {
        match qualifier {
            Some(qualifier) => self
                .bindings
                .iter()
                .find(|binding| binding.binding_name.eq_ignore_ascii_case(qualifier))
                .map(|binding| binding.binding_name.as_str()),
            None if self.bindings.len() == 1 => Some(self.bindings[0].binding_name.as_str()),
            None => None,
        }
    }

    pub(crate) fn resolve_wrapper(
        &self,
        relation: Option<&str>,
        name: &str,
    ) -> Option<ResolvedWrapper> {
        self.bindings
            .iter()
            .filter(|binding| {
                relation.is_none_or(|want| binding.binding_name.eq_ignore_ascii_case(want))
            })
            .filter_map(|binding| {
                binding
                    .relation
                    .columns
                    .iter()
                    .find(|column| column.name.eq_ignore_ascii_case(name))
                    .filter(|_| binding.relation.kind == RelationKind::MaterializedView)
                    .and_then(|column| {
                        column
                            .metadata
                            .extensions
                            .get("clickhouse.mv.merge_fn")
                            .and_then(|merge_fn| WrapperSpec::from_merge_fn(merge_fn))
                            .map(|wrapper| ResolvedWrapper {
                                relation_display: binding.relation_display.clone(),
                                binding_name: binding.binding_name.clone(),
                                column_name: column.name.clone(),
                                wrapper,
                            })
                    })
            })
            .next()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedWrapper {
    pub(crate) relation_display: String,
    pub(crate) binding_name: String,
    pub(crate) column_name: String,
    pub(crate) wrapper: WrapperSpec,
}

#[derive(Debug, Clone)]
pub(crate) struct WrapperNearMiss {
    pub(crate) relation_display: String,
    pub(crate) binding_name: String,
    pub(crate) column_name: String,
    pub(crate) current_wrapper: WrapperSpec,
    pub(crate) expected_wrapper: WrapperSpec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WrapperSpec {
    pub(crate) namespace: Option<&'static str>,
    pub(crate) name: &'static str,
}

impl WrapperSpec {
    pub(crate) fn from_merge_fn(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "summerge" => Some(Self {
                namespace: Some("ch"),
                name: "sum_merge",
            }),
            "countmerge" => Some(Self {
                namespace: Some("ch"),
                name: "count_merge",
            }),
            "avgmerge" => Some(Self {
                namespace: Some("ch"),
                name: "avg_merge",
            }),
            "stddevpopmerge" => Some(Self {
                namespace: Some("ch"),
                name: "stddevpop_merge",
            }),
            "sum" => Some(Self {
                namespace: None,
                name: "sum",
            }),
            "count" => Some(Self {
                namespace: None,
                name: "count",
            }),
            "min" => Some(Self {
                namespace: None,
                name: "min",
            }),
            "max" => Some(Self {
                namespace: None,
                name: "max",
            }),
            "avg" => Some(Self {
                namespace: None,
                name: "avg",
            }),
            _ => None,
        }
    }

    pub(crate) fn merge_fn_name(&self) -> &'static str {
        self.name
    }

    pub(crate) fn display_name(&self) -> &'static str {
        self.name
    }

    pub(crate) fn from_function(function: &FunctionRef) -> Option<Self> {
        match (
            function.namespace.as_deref().map(str::to_ascii_lowercase),
            function.name.to_ascii_lowercase().as_str(),
        ) {
            (Some(namespace), "avg_merge") if namespace == "ch" => Some(Self {
                namespace: Some("ch"),
                name: "avg_merge",
            }),
            (Some(namespace), "count_merge") if namespace == "ch" => Some(Self {
                namespace: Some("ch"),
                name: "count_merge",
            }),
            (Some(namespace), "sum_merge") if namespace == "ch" => Some(Self {
                namespace: Some("ch"),
                name: "sum_merge",
            }),
            (Some(namespace), "stddevpop_merge") if namespace == "ch" => Some(Self {
                namespace: Some("ch"),
                name: "stddevpop_merge",
            }),
            (Some(namespace), "varpop_merge") if namespace == "ch" => Some(Self {
                namespace: Some("ch"),
                name: "varpop_merge",
            }),
            (None, "min") => Some(Self {
                namespace: None,
                name: "min",
            }),
            (None, "max") => Some(Self {
                namespace: None,
                name: "max",
            }),
            _ => None,
        }
    }

    pub(crate) fn function_ref(self) -> FunctionRef {
        FunctionRef {
            namespace: self.namespace.map(str::to_owned),
            name: self.name.to_owned(),
        }
    }

    pub(crate) fn matches(self, function: &FunctionRef) -> bool {
        function.name.eq_ignore_ascii_case(self.name)
            && match (self.namespace, function.namespace.as_deref()) {
                (None, None) => true,
                (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
                _ => false,
            }
    }
}

impl SelectScope {
    pub(crate) fn detect_wrapper_near_miss(
        &self,
        function: &BoundFunctionCall,
    ) -> Option<WrapperNearMiss> {
        if function.distinct
            || function.filter.is_some()
            || function.over.is_some()
            || function.args.len() != 1
        {
            return None;
        }

        let actual = WrapperSpec::from_function(&function.function)?;
        let BoundExprKind::Column(column) = &function.args[0].kind else {
            return None;
        };
        let expected = self.resolve_wrapper(column.relation.as_deref(), &column.name)?;
        (actual != expected.wrapper).then_some(WrapperNearMiss {
            relation_display: expected.relation_display,
            binding_name: expected.binding_name,
            column_name: expected.column_name,
            current_wrapper: actual,
            expected_wrapper: expected.wrapper,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WrapEvent {
    pub(crate) relation_display: String,
    pub(crate) binding_name: String,
    pub(crate) column_name: String,
    pub(crate) wrapper: WrapperSpec,
}

#[derive(Debug, Clone)]
pub(crate) struct NearMissEvent {
    pub(crate) near_miss: WrapperNearMiss,
    pub(crate) column_name: String,
    pub(crate) node: SyntaxNode,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct ClickHouseMvSummary {
    pub(crate) rewritten_relations: BTreeSet<String>,
    pub(crate) wrap_events: Vec<WrapEvent>,
    pub(crate) near_misses: Vec<NearMissEvent>,
}

impl ClickHouseMvSummary {
    pub(crate) fn record_wrap(&mut self, resolved: &ResolvedWrapper, _node: &SyntaxNode) {
        self.rewritten_relations
            .insert(resolved.relation_display.clone());
        self.wrap_events.push(WrapEvent {
            relation_display: resolved.relation_display.clone(),
            binding_name: resolved.binding_name.clone(),
            column_name: resolved.column_name.clone(),
            wrapper: resolved.wrapper,
        });
    }

    pub(crate) fn record_near_miss(&mut self, mismatch: WrapperNearMiss, node: &SyntaxNode) {
        let column_name = mismatch.column_name.clone();
        self.near_misses.push(NearMissEvent {
            near_miss: mismatch,
            column_name,
            node: node.clone(),
        });
    }

    pub(crate) fn rewritten_to_metadata(&self) -> Option<String> {
        (!self.rewritten_relations.is_empty()).then(|| {
            self.rewritten_relations
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(",")
        })
    }

    pub(crate) fn analysis_diagnostics(&self, _backend: &str) -> Vec<QueryDiagnostic> {
        let mut diagnostics = Vec::new();
        let mut seen_wraps = BTreeSet::new();
        for event in &self.wrap_events {
            let key = format!(
                "{}|{}|{}|{}|{}",
                event.relation_display,
                event.binding_name,
                event.column_name,
                event.wrapper.display_name(),
                event.wrapper.merge_fn_name(),
            );
            if seen_wraps.insert(key) {
                diagnostics.push(diagnostic_with_node(
                    QueryDiagnostic::note(
                        "QFCH201",
                        format!(
                            "ClickHouse emission will wrap materialized-view \
                             column `{}.{}` from `{}` with `{}`.",
                            event.binding_name,
                            event.column_name,
                            event.relation_display,
                            event.wrapper.display_name(),
                        ),
                    ),
                    &SyntaxNode::new(None, ""),
                ));
            }
        }
        for miss in &self.near_misses {
            let msg = if miss.near_miss.relation_display.is_empty() {
                format!("MV relation not found for column {}", miss.column_name)
            } else {
                format!(
                    "column {}.{} expects wrapper {:?} but found {:?}",
                    miss.near_miss.binding_name,
                    miss.column_name,
                    miss.near_miss.expected_wrapper.name,
                    miss.near_miss.current_wrapper.name,
                )
            };
            diagnostics.push(diagnostic_with_node(
                QueryDiagnostic::warning("QFCH202", msg),
                &miss.node,
            ));
        }
        diagnostics
    }
}

fn diagnostic_with_node(mut diagnostic: QueryDiagnostic, node: &SyntaxNode) -> QueryDiagnostic {
    diagnostic.node_id = Some(node.node_id.clone());
    diagnostic
}
