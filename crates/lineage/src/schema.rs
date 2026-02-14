use hashbrown::HashMap;

use sqruff_lib_core::helpers::Config;

use crate::ir::{Expr, ExprKind, Tables};
use crate::trie::{TrieNode, TrieResult};

#[derive(Default)]
pub(crate) struct Schema {
    visible: HashMap<String, String>,
    mapping: HashMap<String, HashMap<String, String>>,
    mapping_trie: HashMap<String, TrieNode>,
}

impl Schema {
    pub(crate) fn new(mapping: HashMap<String, HashMap<String, String>>) -> Self {
        let mapping_trie =
            crate::trie::new(flatten_schema(&mapping).map(|t| t.config(|this| this.reverse())));

        Self {
            mapping,
            mapping_trie,
            ..Default::default()
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.mapping.is_empty()
    }

    fn table_parts(&self, tables: &Tables, table: Expr) -> Vec<String> {
        let ExprKind::TableReference(n, _) = &tables.exprs[table].kind else {
            unimplemented!()
        };
        vec![n.clone()]
    }

    fn nested_get(&self, parts: Vec<String>) -> &HashMap<String, String> {
        nested_get(
            &self.mapping,
            parts.into_iter().map(|path| (path, String::new())),
        )
    }

    pub(crate) fn find(&self, tables: &Tables, table: Expr) -> Option<&HashMap<String, String>> {
        let parts = self.table_parts(tables, table);
        let (trie, _value) = crate::trie::in_(&self.mapping_trie, &parts);

        match trie {
            TrieResult::Failed => None,
            TrieResult::Prefix => todo!(),
            TrieResult::Exists => Some(self.nested_get(parts)),
        }
    }

    pub(crate) fn column_names(
        &self,
        tables: &Tables,
        table: Expr,
        only_visible: bool,
    ) -> Vec<String> {
        let Some(schema) = self.find(tables, table) else {
            return Vec::new();
        };

        if !only_visible || self.visible.is_empty() {
            return schema.keys().cloned().collect();
        }

        todo!()
    }
}

fn flatten_schema(
    schema: &HashMap<String, HashMap<String, String>>,
) -> impl Iterator<Item = Vec<String>> + '_ {
    schema.keys().cloned().map(|key| vec![key])
}

fn nested_get(
    d: &HashMap<String, HashMap<String, String>>,
    paths: impl Iterator<Item = (String, String)>,
) -> &HashMap<String, String> {
    let mut q = None;
    for (name, _key) in paths {
        q = d.get(&name);
    }
    q.unwrap()
}
