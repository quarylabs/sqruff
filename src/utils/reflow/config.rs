use std::collections::{HashMap, HashSet};

use crate::core::config::FluffConfig;
use crate::utils::reflow::depth_map::DepthInfo;

type ConfigElementType = HashMap<&'static str, &'static str>;
type ConfigDictType = HashMap<&'static str, ConfigElementType>;

/// Holds spacing config for a block and allows easy manipulation
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BlockConfig {
    pub spacing_before: String,
    pub spacing_after: String,
    pub spacing_within: Option<String>,
    pub line_position: Option<String>,
}

impl BlockConfig {
    pub fn new() -> Self {
        BlockConfig {
            spacing_before: "single".to_string(),
            spacing_after: "single".to_string(),
            spacing_within: None,
            line_position: None,
        }
    }

    /// Mutate the config based on additional information
    pub fn incorporate(
        &mut self,
        before: Option<&str>,
        after: Option<&str>,
        within: Option<&str>,
        line_position: Option<&str>,
        config: Option<&ConfigElementType>,
    ) {
        let empty_config: ConfigElementType = HashMap::new();
        let config = config.unwrap_or(&empty_config);
        self.spacing_before = before
            .or(config.get("spacing_before").copied())
            .unwrap_or(&self.spacing_before)
            .to_string();
        self.spacing_after = after
            .or(config.get("spacing_after").copied())
            .unwrap_or(&self.spacing_after)
            .to_string();
        self.spacing_within = within
            .map(ToString::to_string)
            .or(config.get("spacing_within").copied().map(ToString::to_string));
        self.line_position =
            line_position.or(config.get("line_position").copied()).map(|s| s.to_string());
    }
}

/// An interface onto the configuration of how segments should reflow.
///
/// This acts as the primary translation engine between configuration
/// held either in dicts for testing, or in the FluffConfig in live
/// usage, and the configuration used during reflow operations.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ReflowConfig {
    _config_dict: ConfigDictType,
    config_types: HashSet<String>,
    /// In production, these values are almost _always_ set because we
    /// use `.from_fluff_config`, but the defaults are here to aid in
    /// testing.
    tab_space_size: usize,
    indent_unit: String,
    max_line_length: usize,
    hanging_indents: bool,
    skip_indentation_in: HashSet<String>,
    allow_implicit_indents: bool,
    trailing_comments: String,
}

impl Default for ReflowConfig {
    fn default() -> Self {
        #[rustfmt::skip]
        let config_types = [
            "path_segment", "colon", "start_bracket", "assignment_operator", 
            "groupby_clause", "sql_conf_option", "pattern_expression", 
            "statement_terminator", "set_operator", "array_type", "sqlcmd_operator", 
            "comment", "comparison_operator", "sign_indicator", "orderby_clause", 
            "start_angle_bracket", "comma", "struct_type", "array_accessor", 
            "tilde", "select_clause", "having_clause", "object_reference", 
            "where_clause", "slice", "typed_struct_literal", "typed_array_literal", 
            "bracketed_arguments", "semi_structured_expression", 
            "common_table_expression", "end_square_bracket", "numeric_literal", 
            "dot", "limit_clause", "binary_operator", "template_loop", 
            "colon_delimiter", "sized_array_type", "end_of_file", "casting_operator", 
            "end_angle_bracket", "function_name", "join_clause", 
            "start_square_bracket", "placeholder", "end_bracket", "from_clause"
        ];

        ReflowConfig {
            _config_dict: create_formatting_rules(),
            config_types: config_types.map(ToOwned::to_owned).into_iter().collect(),
            tab_space_size: 4,
            indent_unit: "    ".to_string(),
            max_line_length: 80,
            hanging_indents: false,
            skip_indentation_in: HashSet::new(),
            allow_implicit_indents: false,
            trailing_comments: "before".to_string(),
        }
    }
}

impl ReflowConfig {
    pub fn get_block_config(
        &self,
        block_class_types: HashSet<String>,
        depth_info: Option<&DepthInfo>,
    ) -> BlockConfig {
        let configured_types = self.config_types.intersection(&block_class_types);

        let mut block_config = BlockConfig::new();

        #[allow(unused_variables)]
        if let Some(depth_info) = depth_info {
            let (parent_start, parent_end) = (true, true);

            for (idx, key) in depth_info.stack_hashes.iter().rev().enumerate() {}
        }

        for seg_type in configured_types {
            block_config.incorporate(
                None,
                None,
                None,
                None,
                self._config_dict.get(seg_type.as_str()),
            );
        }

        block_config
    }

    pub fn from_fluff_config(_config: FluffConfig) -> ReflowConfig {
        panic!("Not implemented yet");
    }
}

fn create_formatting_rules() -> HashMap<&'static str, HashMap<&'static str, &'static str>> {
    let mut rules = HashMap::new();

    // Directly insert the tuples into the HashMaps for each key
    rules.insert(
        "comma",
        [("spacing_before", "touch"), ("line_position", "trailing")].iter().cloned().collect(),
    );
    rules.insert(
        "binary_operator",
        [("spacing_within", "touch"), ("line_position", "leading")].iter().cloned().collect(),
    );
    rules.insert(
        "statement_terminator",
        [("spacing_before", "touch"), ("line_position", "trailing")].iter().cloned().collect(),
    );
    rules.insert("end_of_file", [("spacing_before", "touch")].iter().cloned().collect());
    rules.insert("set_operator", [("line_position", "alone:strict")].iter().cloned().collect());
    rules.insert("start_bracket", [("spacing_after", "touch")].iter().cloned().collect());
    rules.insert("end_bracket", [("spacing_before", "touch")].iter().cloned().collect());
    rules.insert("start_square_bracket", [("spacing_after", "touch")].iter().cloned().collect());
    rules.insert("end_square_bracket", [("spacing_before", "touch")].iter().cloned().collect());
    rules.insert("start_angle_bracket", [("spacing_after", "touch")].iter().cloned().collect());
    rules.insert("end_angle_bracket", [("spacing_before", "touch")].iter().cloned().collect());
    rules.insert(
        "casting_operator",
        [("spacing_before", "touch"), ("spacing_after", "touch:inline")].iter().cloned().collect(),
    );
    rules.insert(
        "slice",
        [("spacing_before", "touch"), ("spacing_after", "touch")].iter().cloned().collect(),
    );
    rules.insert(
        "dot",
        [("spacing_before", "touch"), ("spacing_after", "touch")].iter().cloned().collect(),
    );
    rules.insert(
        "comparison_operator",
        [("spacing_within", "touch"), ("line_position", "leading")].iter().cloned().collect(),
    );
    rules.insert(
        "assignment_operator",
        [("spacing_within", "touch"), ("line_position", "leading")].iter().cloned().collect(),
    );
    rules
        .insert("object_reference", [("spacing_within", "touch:inline")].iter().cloned().collect());
    rules.insert("numeric_literal", [("spacing_within", "touch:inline")].iter().cloned().collect());
    rules.insert("sign_indicator", [("spacing_after", "touch:inline")].iter().cloned().collect());
    rules.insert("tilde", [("spacing_after", "touch:inline")].iter().cloned().collect());
    rules.insert(
        "function_name",
        [("spacing_within", "touch:inline"), ("spacing_after", "touch:inline")]
            .iter()
            .cloned()
            .collect(),
    );
    rules.insert("array_type", [("spacing_within", "touch:inline")].iter().cloned().collect());
    rules.insert("typed_array_literal", [("spacing_within", "touch")].iter().cloned().collect());
    rules.insert("sized_array_type", [("spacing_within", "touch")].iter().cloned().collect());
    rules.insert("struct_type", [("spacing_within", "touch:inline")].iter().cloned().collect());
    rules.insert(
        "bracketed_arguments",
        [("spacing_before", "touch:inline")].iter().cloned().collect(),
    );
    rules.insert("typed_struct_literal", [("spacing_within", "touch")].iter().cloned().collect());
    rules.insert(
        "semi_structured_expression",
        [("spacing_within", "touch:inline"), ("spacing_before", "touch:inline")]
            .iter()
            .cloned()
            .collect(),
    );
    rules.insert("array_accessor", [("spacing_before", "touch:inline")].iter().cloned().collect());
    rules.insert("colon", [("spacing_before", "touch")].iter().cloned().collect());
    rules.insert(
        "colon_delimiter",
        [("spacing_before", "touch"), ("spacing_after", "touch")].iter().cloned().collect(),
    );
    rules.insert("path_segment", [("spacing_within", "touch")].iter().cloned().collect());
    rules.insert("sql_conf_option", [("spacing_within", "touch")].iter().cloned().collect());
    rules.insert("sqlcmd_operator", [("spacing_before", "touch")].iter().cloned().collect());
    rules.insert(
        "comment",
        [("spacing_before", "any"), ("spacing_after", "any")].iter().cloned().collect(),
    );
    rules.insert("pattern_expression", [("spacing_within", "any")].iter().cloned().collect());
    rules.insert(
        "placeholder",
        [("spacing_before", "any"), ("spacing_after", "any")].iter().cloned().collect(),
    );
    rules.insert(
        "common_table_expression",
        [("spacing_within", "single:inline")].iter().cloned().collect(),
    );
    rules.insert("select_clause", [("line_position", "alone")].iter().cloned().collect());
    rules.insert("where_clause", [("line_position", "alone")].iter().cloned().collect());
    rules.insert("from_clause", [("line_position", "alone")].iter().cloned().collect());
    rules.insert("join_clause", [("line_position", "alone")].iter().cloned().collect());
    rules.insert("groupby_clause", [("line_position", "alone")].iter().cloned().collect());
    rules.insert("orderby_clause", [("line_position", "leading")].iter().cloned().collect());
    rules.insert("having_clause", [("line_position", "alone")].iter().cloned().collect());
    rules.insert("limit_clause", [("line_position", "alone")].iter().cloned().collect());
    rules.insert(
        "template_loop",
        [("spacing_before", "any"), ("spacing_after", "any")].iter().cloned().collect(),
    );

    rules
}
