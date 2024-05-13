use std::borrow::Cow;
use std::os::windows::raw;

use ahash::AHashMap;
use itertools::{enumerate, Itertools as _};
use minijinja::machinery::{Token, Tokenizer};
use minijinja::Environment;

use crate::core::templaters::base::{RawFileSlice, TemplatedFileSlice};

#[derive(Debug)]
pub struct JinjaTrace {
    pub(crate) templated_str: String,
    pub(crate) raw_sliced: Vec<RawFileSlice>,
    pub(crate) sliced_file: Vec<TemplatedFileSlice>,
}

#[derive(Debug)]
pub struct RawSliceInfo {
    pub(crate) unique_alternate_id: Option<String>,
    pub(crate) alternate_code: Option<String>,
    pub(crate) next_slice_indices: Vec<i32>,
    pub(crate) inside_block: bool,
}

pub struct JinjaTracer {
    pub(crate) raw_str: String,
    pub(crate) raw_sliced: Vec<RawFileSlice>,
    pub(crate) raw_slice_info: AHashMap<RawFileSlice, RawSliceInfo>,
    pub(crate) sliced_file: Vec<TemplatedFileSlice>,
    pub(crate) program_counter: i32,
    pub(crate) source_idx: i32,
}

impl JinjaTracer {
    pub fn new(
        raw_str: String,
        raw_sliced: Vec<RawFileSlice>,
        raw_slice_info: AHashMap<RawFileSlice, RawSliceInfo>,
        sliced_file: Vec<TemplatedFileSlice>,
    ) -> Self {
        JinjaTracer {
            raw_str,
            raw_sliced,
            raw_slice_info,
            sliced_file,
            program_counter: 0,
            source_idx: 0,
        }
    }

    pub fn trace(mut self, env: Environment, append_to_templated: &str) -> JinjaTrace {
        let trace_template_str = self
            .raw_sliced
            .iter()
            .map(|rs| {
                if let Some(code) = &self.raw_slice_info[rs].alternate_code {
                    dbg!(code)
                } else {
                    dbg!(&rs.raw)
                }
            })
            .join("");

        let trace_template_output = env.render_str(&trace_template_str, ()).unwrap();
        let trace_entries: Vec<regex::Match> =
            lazy_regex::regex!(r"\x00").find_iter(&trace_template_output).collect();

        if trace_entries.is_empty() {
            unimplemented!()
        }

        for (match_idx, matches) in enumerate(trace_entries) {
            
        }

        unimplemented!()
    }
}

pub struct JinjaAnalyzer<'me> {
    raw_str: &'me str,
    raw_sliced: Vec<RawFileSlice>,
    raw_slice_info: AHashMap<RawFileSlice, RawSliceInfo>,
    sliced_file: Vec<TemplatedFileSlice>,
    slice_id: i32,
    inside_set_macro_or_call: bool,
    inside_block: bool,
    stack: Vec<i32>,
    idx_raw: usize,
}

impl<'me> JinjaAnalyzer<'me> {
    pub fn new(raw_str: &'me str) -> Self {
        JinjaAnalyzer {
            raw_str,
            raw_sliced: Vec::new(),
            raw_slice_info: AHashMap::new(),
            sliced_file: Vec::new(),
            slice_id: 0,
            inside_set_macro_or_call: false,
            inside_block: false,
            stack: Vec::new(),
            idx_raw: 0,
        }
    }

    fn next_slice_id(&mut self) -> String {
        let result = format!("{:032x}", self.slice_id);
        self.slice_id += 1;
        result
    }

    fn make_raw_slice_info(
        &self,
        unique_alternate_id: Option<String>,
        alternate_code: Option<String>,
        inside_block: bool,
    ) -> RawSliceInfo {
        if !self.inside_set_macro_or_call {
            RawSliceInfo {
                unique_alternate_id,
                alternate_code,
                inside_block,
                next_slice_indices: Vec::new(),
            }
        } else {
            RawSliceInfo {
                unique_alternate_id: None,
                alternate_code: None,
                next_slice_indices: Vec::new(),
                inside_block: false,
            }
        }
    }

    fn track_templated(
        &mut self,
        m_open: &fancy_regex::Match,
        m_close: &regex::Match,
        tag_contents: &[Cow<str>],
    ) -> RawSliceInfo {
        let unique_alternate_id = self.next_slice_id();
        let open_ = m_open.as_str();
        let close_ = m_close.as_str();
        let alternate_code =
            format!("\0{} {} {} {}", unique_alternate_id, open_, tag_contents.join(" "), close_);

        self.make_raw_slice_info(unique_alternate_id.into(), alternate_code.into(), false)
    }

    fn update_inside_set_call_macro_or_block<'a>(
        &mut self,
        mut block_type: &'a str,
        trimmed_parts: &[Cow<str>],
        m_open: Option<&str>,
        m_close: Option<&str>,
        tag_contents: &[Cow<str>],
    ) -> (Option<RawSliceInfo>, &'a str) {
        if block_type == "block_start"
            && ["block", "call", "macro", "set"].contains(&trimmed_parts[0].as_ref())
        {
            unimplemented!();
        } else if block_type == "block_end" {
            match trimmed_parts[0].as_ref() {
                "endcall" | "endmacro" | "endset" => self.inside_set_macro_or_call = false,
                "endblock" => self.inside_block = false,
                _ => {}
            }
        }

        (None, block_type)
    }

    pub fn analyze(mut self) -> JinjaTracer {
        let re_open_tag = {
            static RE: lazy_regex::Lazy<fancy_regex::Regex> = lazy_regex::Lazy::new(|| {
                fancy_regex::Regex::new(r"^\s*({[{%])[\+\-]?\s*").unwrap()
            });
            &RE
        };
        let re_close_tag = lazy_regex::regex!(r"\s*[\+\-]?([}%]})\s*$");

        let mut tokenizer = Tokenizer::new(&self.raw_str, false, <_>::default(), <_>::default());

        let mut str_buff = String::new();
        let mut str_parts: Vec<Cow<str>> = Vec::new();

        let block_idx = 0;
        while let Ok(Some((token, span))) = tokenizer.next_token() {
            let raw = &self.raw_str[span.start_offset as usize..span.end_offset as usize];

            if let Token::TemplateData(data) = token {
                self.raw_sliced.push(RawFileSlice {
                    raw: data.to_string(),
                    slice_type: "literal".to_string(),
                    source_idx: self.idx_raw,
                    slice_subtype: None,
                    block_idx,
                });

                let slice_info = self.slice_info_for_literal(data.len(), "");
                self.raw_slice_info.insert(self.raw_sliced.last().unwrap().clone(), slice_info);

                self.idx_raw += data.len();
                continue;
            }

            str_buff.push_str(raw);
            str_parts.push(raw.into());

            if let Token::VariableStart | Token::BlockStart = token {
                self.handle_left_whitespace_stripping(raw, block_idx);
            }

            let mut raw_slice_info = self.make_raw_slice_info(None, None, false);
            let mut tag_contents = Vec::new();

            let m_open = None;
            let m_close = None;

            if let Token::VariableEnd | Token::BlockEnd = token {
                let mut block_type = match token {
                    Token::VariableEnd => "templated",
                    Token::BlockEnd => "block",
                    _ => unreachable!(),
                };

                let mut block_subtype = None;
                if matches!(block_type, "block" | "templated") {
                    let m_open = re_open_tag.find(&str_parts[0]).unwrap();
                    let m_close = re_close_tag.find(&str_parts[str_parts.len() - 1]);

                    if let Some((open, close)) = m_open.zip(m_close) {
                        tag_contents = extract_tag_contents(&str_parts, &close, &open, &str_buff);
                    }

                    if block_type == "block" && !tag_contents.is_empty() {
                        (block_type, block_subtype) =
                            extract_block_type(&tag_contents[0], block_subtype);
                    }

                    if block_type == "templated" && !tag_contents.is_empty() {
                        raw_slice_info = self.track_templated(
                            &m_open.unwrap(),
                            &m_close.unwrap(),
                            &tag_contents,
                        );

                        dbg!(&raw_slice_info);
                    }
                }

                (_, block_type) = self.update_inside_set_call_macro_or_block(
                    block_type,
                    &tag_contents,
                    m_open.map(|m: fancy_regex::Match| m.as_str()),
                    m_close.map(|m: regex::Match| m.as_str()),
                    &tag_contents,
                );

                let m_strip_right = lazy_regex::regex_find!(r"\s+$", raw);
                if let Token::VariableEnd | Token::BlockEnd = token
                    && let Some(_m) = m_strip_right
                {
                    unimplemented!()
                } else {
                    self.raw_sliced.push(RawFileSlice {
                        raw: str_buff.to_string(),
                        slice_type: block_type.to_string(),
                        source_idx: self.idx_raw,
                        slice_subtype: block_subtype.map(|_| unimplemented!()),
                        block_idx,
                    });

                    self.raw_slice_info
                        .insert(self.raw_sliced.last().unwrap().clone(), raw_slice_info);

                    let slice_idx = self.raw_sliced.len() - 1;
                    self.idx_raw += str_buff.len();

                    str_buff.clear();
                    str_parts.clear();
                }
            }

            let whitespace = self.raw_str[span.end_offset as usize..]
                .chars()
                .take_while(|n: &char| n.is_whitespace())
                .collect::<String>();

            if !whitespace.is_empty() {
                self.idx_raw += whitespace.len();
                str_buff.push_str(&whitespace);
                str_parts.push(whitespace.into());
            }
        }

        JinjaTracer {
            raw_str: self.raw_str.to_string(),
            raw_sliced: self.raw_sliced,
            raw_slice_info: self.raw_slice_info,
            sliced_file: self.sliced_file,
            program_counter: 0,
            source_idx: 0,
        }
    }

    fn handle_left_whitespace_stripping(&mut self, token: &str, block_idx: usize) {
        let num_chars_skipped = match self.raw_str[self.idx_raw..].find(token) {
            Some(index) => index + self.idx_raw,
            None => self.raw_str.len(),
        } - self.idx_raw;

        if num_chars_skipped == 0 {
            return;
        }

        let skipped_str = &self.raw_str[self.idx_raw..self.idx_raw + num_chars_skipped];

        if !skipped_str.chars().all(char::is_whitespace) {
            tracing::warn!("Jinja lex() skipped non-whitespace: {skipped_str}");
        }

        let slice_info = self.slice_info_for_literal(0, "");
        self.raw_slice_info.insert(self.raw_sliced.last().unwrap().clone(), slice_info);
        self.idx_raw += num_chars_skipped;
    }

    fn slice_info_for_literal(&mut self, length: usize, prefix: &str) -> RawSliceInfo {
        let unique_alternate_id = self.next_slice_id();
        let alternate_code = format!("\0{}{}_{}", prefix, unique_alternate_id, length);
        self.make_raw_slice_info(
            unique_alternate_id.into(),
            alternate_code.into(),
            self.inside_block,
        )
    }
}

fn extract_tag_contents<'a>(
    str_parts: &'a [Cow<str>],
    m_close: &regex::Match,
    m_open: &fancy_regex::Match,
    str_buff: &'a str,
) -> Vec<Cow<'a, str>> {
    if str_parts.len() >= 3 {
        let mut trimmed_parts = str_parts[1..str_parts.len() - 1].to_vec();
        if trimmed_parts[0].trim().is_empty() {
            trimmed_parts.remove(0);
        }
        if trimmed_parts.last().unwrap().trim().is_empty() {
            trimmed_parts.pop();
        }
        trimmed_parts
    } else {
        let start = m_open.end();
        let end = str_buff.len() - m_close.start();
        let trimmed_content = &str_buff[start..end];
        trimmed_content.split_whitespace().map(Cow::Borrowed).collect()
    }
}

fn extract_block_type<'a>(
    tag_name: &str,
    mut block_subtype: Option<&'a str>,
) -> (&'static str, Option<&'a str>) {
    let block_type = if ["include", "import", "from", "do"].contains(&tag_name) {
        "templated"
    } else if tag_name.starts_with("end") {
        "block_end"
    } else if tag_name.starts_with("el") {
        // Handles 'else', 'elif'
        "block_mid"
    } else {
        if tag_name == "for" {
            block_subtype = Some("loop");
        }
        "block_start"
    };

    (block_type, block_subtype)
}
