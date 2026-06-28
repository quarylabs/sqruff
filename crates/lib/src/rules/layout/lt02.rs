use hashbrown::HashMap;
use sqruff_lib_core::templaters::TemplatedFile;

use crate::core::config::Value;
use crate::core::rules::context::RuleContext;
use crate::core::rules::crawlers::{Crawler, RootOnlyCrawler};
use crate::core::rules::{Erased, ErasedRule, LintResult, Rule, RuleGroups};
use crate::utils::reflow::sequence::ReflowSequence;

#[derive(Default, Debug, Clone)]
pub struct RuleLT02;

fn line_bounds(source: &str, pos: usize) -> (usize, usize) {
    let pos = pos.min(source.len());
    let start = source[..pos].rfind('\n').map_or(0, |idx| idx + 1);
    let end = source[pos..]
        .find('\n')
        .map_or(source.len(), |idx| pos + idx);
    (start, end)
}

fn skip_whitespace_forward(source: &str, mut pos: usize) -> usize {
    while pos < source.len() {
        let Some(ch) = source[pos..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        pos += ch.len_utf8();
    }
    pos
}

fn skip_whitespace_backward(source: &str, mut pos: usize) -> usize {
    while pos > 0 {
        let Some(ch) = source[..pos].chars().next_back() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        pos -= ch.len_utf8();
    }
    pos
}

fn source_only_slice_at(templated_file: &TemplatedFile, pos: usize) -> bool {
    templated_file.raw_sliced().iter().any(|slice| {
        slice.slice_kind().is_source_only()
            && slice.source_slice().start <= pos
            && pos < slice.source_slice().end
    })
}

fn line_is_adjacent_to_source_only_slice(templated_file: &TemplatedFile, pos: usize) -> bool {
    let source = templated_file.source_str.as_str();
    let (line_start, line_end) = line_bounds(source, pos);

    let before = skip_whitespace_backward(source, line_start);
    let after = skip_whitespace_forward(source, line_end);

    source_only_slice_at(templated_file, line_start)
        || source_only_slice_at(templated_file, pos)
        || (before > 0 && source_only_slice_at(templated_file, before - 1))
        || source_only_slice_at(templated_file, after)
}

impl Rule for RuleLT02 {
    fn load_from_config(&self, _config: &HashMap<String, Value>) -> Result<ErasedRule, String> {
        Ok(RuleLT02.erased())
    }
    fn name(&self) -> &'static str {
        "layout.indent"
    }

    fn description(&self) -> &'static str {
        "Incorrect Indentation."
    }

    fn long_description(&self) -> &'static str {
        r#"
**Anti-pattern**

The ``•`` character represents a space and the ``→`` character represents a tab.
In this example, the third line contains five spaces instead of four and
the second line contains two spaces and one tab.

```sql
SELECT
••→a,
•••••b
FROM foo
```

**Best practice**

Change the indentation to use a multiple of four spaces. This example also assumes that the indent_unit config value is set to space. If it had instead been set to tab, then the indents would be tabs instead.

```sql
SELECT
••••a,
••••b
FROM foo
```
"#
    }

    fn groups(&self) -> &'static [RuleGroups] {
        &[RuleGroups::All, RuleGroups::Core, RuleGroups::Layout]
    }

    fn eval(&self, context: &RuleContext) -> Vec<LintResult> {
        let results = ReflowSequence::from_root(&context.segment, context.config)
            .reindent(context.tables)
            .results();

        let Some(templated_file) = &context.templated_file else {
            return results;
        };

        results
            .into_iter()
            .filter(|result| {
                !result.anchor.as_ref().is_some_and(|anchor| {
                    anchor.get_position_marker().is_some_and(|marker| {
                        line_is_adjacent_to_source_only_slice(
                            templated_file,
                            marker.source_slice.start,
                        )
                    })
                })
            })
            .collect()
    }

    fn is_fix_compatible(&self) -> bool {
        true
    }

    fn crawl_behaviour(&self) -> Crawler {
        RootOnlyCrawler.into()
    }
}
