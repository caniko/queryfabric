use queryfabric_ir::{BoundRelation, BoundTableWithJoins, ResultField, ResultSchema};

#[derive(Debug, Clone)]
pub(super) struct ScopeEntry {
    binding_name: String,
    schema: ResultSchema,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Scope {
    entries: Vec<ScopeEntry>,
}

#[derive(Debug, Clone)]
pub(super) enum ColumnResolution {
    Local(Box<ResolvedColumn>),
    Missing,
    Ambiguous,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedColumn {
    pub(super) relation: Option<String>,
    pub(super) field: ResultField,
}

impl Scope {
    pub(super) fn from_output_schema(schema: &ResultSchema) -> Self {
        Self {
            entries: vec![ScopeEntry {
                binding_name: "__output__".into(),
                schema: schema.clone(),
            }],
        }
    }

    pub(super) fn push_relation(&mut self, relation: &BoundRelation) {
        if let Some(binding) = relation.binding() {
            self.entries.push(ScopeEntry {
                binding_name: binding.binding_name.clone(),
                schema: binding.schema.clone(),
            });
        }
    }

    pub(super) fn merge_table(&mut self, table: &BoundTableWithJoins) {
        self.push_relation(&table.relation);
        for join in &table.joins {
            self.push_relation(&join.relation);
        }
    }

    pub(super) fn make_all_nullable(&mut self) {
        for entry in &mut self.entries {
            make_schema_nullable(&mut entry.schema);
        }
    }

    pub(super) fn expand_all(&self) -> Vec<ResultField> {
        self.entries
            .iter()
            .flat_map(|entry| entry.schema.fields.clone())
            .collect()
    }

    pub(super) fn expand_qualified(&self, qualifier: &str) -> Option<Vec<ResultField>> {
        self.entries
            .iter()
            .find(|entry| entry.binding_name.eq_ignore_ascii_case(qualifier))
            .map(|entry| entry.schema.fields.clone())
    }

    pub(super) fn has_relation(&self, qualifier: &str) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.binding_name.eq_ignore_ascii_case(qualifier))
    }

    pub(super) fn relation_column_names(&self, qualifier: &str) -> Option<Vec<String>> {
        self.expand_qualified(qualifier).map(|fields| {
            fields
                .into_iter()
                .map(|field| field.name)
                .collect::<Vec<_>>()
        })
    }

    pub(super) fn all_column_names(&self) -> Vec<String> {
        self.entries
            .iter()
            .flat_map(|entry| entry.schema.fields.iter().map(|field| field.name.clone()))
            .collect()
    }

    pub(super) fn resolve_column(&self, relation: Option<&str>, name: &str) -> ColumnResolution {
        let mut matches = Vec::new();
        for entry in &self.entries {
            if let Some(relation) = relation
                && !entry.binding_name.eq_ignore_ascii_case(relation)
            {
                continue;
            }
            for field in &entry.schema.fields {
                if field.name.eq_ignore_ascii_case(name) {
                    matches.push(ResolvedColumn {
                        relation: Some(entry.binding_name.clone()),
                        field: field.clone(),
                    });
                }
            }
        }
        match matches.len() {
            0 => ColumnResolution::Missing,
            1 => ColumnResolution::Local(Box::new(matches.remove(0))),
            _ => ColumnResolution::Ambiguous,
        }
    }

    pub(super) fn try_resolve_column(
        &self,
        relation: Option<&str>,
        name: &str,
    ) -> Option<ColumnResolution> {
        let resolution = self.resolve_column(relation, name);
        (!matches!(resolution, ColumnResolution::Missing)).then_some(resolution)
    }
}

pub(super) fn scope_schema_for_table(table: &BoundTableWithJoins) -> ResultSchema {
    let mut fields = Vec::new();
    if let Some(binding) = table.relation.binding() {
        fields.extend(binding.schema.fields.clone());
    }
    for join in &table.joins {
        if let Some(binding) = join.relation.binding() {
            fields.extend(binding.schema.fields.clone());
        }
    }
    ResultSchema::new(fields)
}

fn make_schema_nullable(schema: &mut ResultSchema) {
    for field in &mut schema.fields {
        field.nullable = true;
    }
}
