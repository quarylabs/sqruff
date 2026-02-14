use hashbrown::HashMap;

pub(crate) fn new(keywords: impl Iterator<Item = Vec<String>>) -> HashMap<String, TrieNode> {
    let mut trie = HashMap::default();

    for key_vec in keywords {
        for key in key_vec {
            let mut current = &mut trie;
            current = current
                .entry(key.clone())
                .or_insert_with(TrieNode::new)
                .children
                .as_mut()
                .unwrap();
            current.insert("0".to_string(), TrieNode { children: None });
        }
    }

    trie
}

#[derive(Default, Debug)]
pub(crate) struct TrieNode {
    children: Option<HashMap<String, TrieNode>>,
}

impl TrieNode {
    fn new() -> Self {
        TrieNode {
            children: Some(HashMap::new()),
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum TrieResult {
    Failed,
    Prefix,
    Exists,
}

pub(crate) fn in_<'a>(
    trie: &'a HashMap<String, TrieNode>,
    key: &[String],
) -> (TrieResult, &'a HashMap<String, TrieNode>) {
    if key.is_empty() {
        return (TrieResult::Failed, trie);
    }

    let mut current = trie;
    for part in key {
        match current.get(part) {
            Some(node) => {
                if let Some(children) = &node.children {
                    current = children;
                } else {
                    return (TrieResult::Failed, current);
                }
            }
            None => {
                return (TrieResult::Failed, current);
            }
        }
    }

    if current.contains_key("0") {
        (TrieResult::Exists, current)
    } else {
        (TrieResult::Prefix, current)
    }
}
