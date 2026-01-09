use itertools::Itertools;
use sqruff_lib_core::dialects::Dialect;
use sqruff_lib_core::dialects::init::DialectKind;
use sqruff_lib_core::dialects::syntax::SyntaxKind;
use sqruff_lib_core::helpers::{Config, ToMatchable};
use sqruff_lib_core::parser::grammar::anyof::{AnyNumberOf, one_of, optionally_bracketed};
use sqruff_lib_core::parser::grammar::delimited::Delimited;
use sqruff_lib_core::parser::grammar::sequence::{Bracketed, Sequence};
use sqruff_lib_core::parser::grammar::{Anything, Ref};
use sqruff_lib_core::parser::lexer::Matcher;
use sqruff_lib_core::parser::matchable::{Matchable, MatchableTrait};
use sqruff_lib_core::parser::node_matcher::NodeMatcher;
use sqruff_lib_core::parser::parsers::{RegexParser, StringParser, TypedParser};
use sqruff_lib_core::parser::segments::generator::SegmentGenerator;
use sqruff_lib_core::parser::segments::meta::MetaSegment;
use sqruff_lib_core::parser::types::ParseMode;

use super::mysql_keywords::{MYSQL_RESERVED_KEYWORDS, MYSQL_UNRESERVED_KEYWORDS};

pub fn dialect() -> Dialect {
    raw_dialect().config(|this| this.expand())
}

#[rustfmt::skip]
pub fn raw_dialect() -> Dialect {
    let ansi_dialect = super::ansi::raw_dialect();
    let mut mysql_dialect = ansi_dialect.clone();
    mysql_dialect.name = DialectKind::Mysql;
    
    mysql_dialect.patch_lexer_matchers(vec![
        Matcher::regex("inline_comment", r#"(^--|-- |#)[^\n]*"#, SyntaxKind::InlineComment),
        Matcher::legacy("single_quote", |_| true, r#"(?s)('(?:\\'|''|\\\\|[^'])*'(?!'))"#, SyntaxKind::SingleQuote),
        Matcher::legacy("double_quote", |_| true, r#"(?s)("(?:\\"|""|\\\\|[^"])*"(?!"))"#, SyntaxKind::DoubleQuote),
    ]);
    
    mysql_dialect.insert_lexer_matchers(vec![
        Matcher::regex("hexadecimal_literal", r#"([xX]'([\da-fA-F][\da-fA-F])+'|0x[\da-fA-F]+)"#, SyntaxKind::NumericLiteral),
        Matcher::regex("bit_value_literal", r#"([bB]'[01]+'|0b[01]+)"#, SyntaxKind::NumericLiteral),
    ], "numeric_literal");
    
    mysql_dialect.update_keywords_set_from_multiline_string("unreserved_keywords", MYSQL_UNRESERVED_KEYWORDS);
    
    mysql_dialect.sets_mut("reserved_keywords").clear();
    
    mysql_dialect.update_keywords_set_from_multiline_string("reserved_keywords", MYSQL_RESERVED_KEYWORDS);
    
    mysql_dialect.sets_mut("datetime_units").clear();
    
    mysql_dialect.sets_mut("datetime_units").extend(["DAY_HOUR", "DAY_MICROSECOND", "DAY_MINUTE", "DAY_SECOND", "HOUR_MICROSECOND", "HOUR_MINUTE", "HOUR_SECOND", "MINUTE_MICROSECOND", "MINUTE_SECOND", "SECOND_MICROSECOND", "YEAR_MONTH", "DAY", "WEEK", "HOUR", "MINUTE", "MONTH", "QUARTER", "SECOND", "MICROSECOND", "YEAR"]);
    
    mysql_dialect.sets_mut("date_part_function_name").clear();
    
    mysql_dialect.sets_mut("date_part_function_name").extend(["EXTRACT", "TIMESTAMPADD", "TIMESTAMPDIFF"]);
    
    mysql_dialect.add([
        (
            "NakedIdentifierSegment".into(),
            SegmentGenerator::new(|dialect| {
                let reserved_keywords = dialect.sets("reserved_keywords");
                let pattern = reserved_keywords.iter().join("|");
                let anti_template = format!("^({pattern})$");
                RegexParser::new(r#"([A-Z0-9_]*[A-Z][A-Z0-9_]*)|_"#, SyntaxKind::NakedIdentifier)
                    .anti_template(&anti_template)
                    .to_matchable()
            })
                .into()
        ),
    ]);
    
    mysql_dialect.replace_grammar(
        "QuotedIdentifierSegment",
        TypedParser::new(SyntaxKind::BackQuote, SyntaxKind::QuotedIdentifier)
            .to_matchable()
    );
    
    mysql_dialect.replace_grammar(
        "LiteralGrammar",
        ansi_dialect.grammar("LiteralGrammar").copy(Some(vec![Ref::new("DoubleQuotedLiteralSegment") .to_matchable(), Ref::new("SystemVariableSegment") .to_matchable()]), None, None, None, vec![], false)
    );
    
    mysql_dialect.replace_grammar(
        "PostTableExpressionGrammar",
        one_of(vec![Ref::new("IndexHintClauseSegment") .to_matchable(), Ref::new("SelectPartitionClauseSegment") .to_matchable()])
            .to_matchable()
    );
    
    mysql_dialect.replace_grammar(
        "FromClauseTerminatorGrammar",
        ansi_dialect.grammar("FromClauseTerminatorGrammar").copy(Some(vec![Ref::new("ForClauseSegment") .to_matchable(), Ref::new("SetOperatorSegment") .to_matchable(), Ref::new("WithNoSchemaBindingClauseSegment") .to_matchable(), Ref::new("WithCheckOptionSegment") .to_matchable(), Ref::new("IntoClauseSegment") .to_matchable()]), None, None, None, vec![], false)
    );
    
    mysql_dialect.replace_grammar(
        "WhereClauseTerminatorGrammar",
        ansi_dialect.grammar("WhereClauseTerminatorGrammar").copy(Some(vec![Ref::new("IntoClauseSegment") .to_matchable()]), None, None, None, vec![], false)
    );
    
    mysql_dialect.replace_grammar(
        "BaseExpressionElementGrammar",
        ansi_dialect.grammar("BaseExpressionElementGrammar").copy(Some(vec![Ref::new("SessionVariableNameSegment") .to_matchable(), Ref::new("LocalVariableNameSegment") .to_matchable(), Ref::new("VariableAssignmentSegment") .to_matchable()]), None, None, None, vec![], false)
    );
    
    mysql_dialect.replace_grammar(
        "Expression_D_Potential_Select_Statement_Without_Brackets",
        ansi_dialect.grammar("Expression_D_Potential_Select_Statement_Without_Brackets").copy(Some(vec![Ref::new("SessionVariableNameSegment") .to_matchable()]), Some(0), None, None, vec![], false)
    );
    
    mysql_dialect.replace_grammar(
        "BinaryOperatorGrammar",
        ansi_dialect.grammar("BinaryOperatorGrammar").copy(Some(vec![Ref::new("ColumnPathOperatorSegment") .to_matchable(), Ref::new("InlinePathOperatorSegment") .to_matchable()]), None, None, None, vec![], false)
    );
    
    mysql_dialect.replace_grammar(
        "ArithmeticBinaryOperatorGrammar",
        ansi_dialect.grammar("ArithmeticBinaryOperatorGrammar").copy(Some(vec![Ref::new("DivOperatorSegment") .to_matchable(), Ref::new("ModOperatorSegment") .to_matchable()]), None, None, None, vec![], false)
    );
    
    mysql_dialect.replace_grammar(
        "DateTimeLiteralGrammar",
        Sequence::new(vec![one_of(vec![Ref::keyword("DATE") .to_matchable(), Ref::keyword("TIME") .to_matchable(), Ref::keyword("TIMESTAMP") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::DateConstructorLiteral) .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable()])
            .to_matchable()
    );
    
    mysql_dialect.replace_grammar(
        "QuotedLiteralSegment",
        AnyNumberOf::new(vec![TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::QuotedLiteral) .to_matchable(), Ref::new("DoubleQuotedLiteralSegment") .to_matchable()])
            .config(|this| {
                this.min_times(1);
            })
            .to_matchable()
    );
    
    mysql_dialect.replace_grammar(
        "UniqueKeyGrammar",
        Sequence::new(vec![Ref::keyword("UNIQUE") .to_matchable(), Ref::keyword("KEY") .optional() .to_matchable()])
            .to_matchable()
    );
    
    mysql_dialect.replace_grammar(
        "CharCharacterSetGrammar",
        Ref::keyword("BINARY")
            .to_matchable()
    );
    
    mysql_dialect.replace_grammar(
        "DelimiterGrammar",
        one_of(vec![Ref::new("SemicolonSegment") .to_matchable(), Ref::new("TildeSegment") .to_matchable()])
            .to_matchable()
    );
    
    mysql_dialect.replace_grammar(
        "TildeSegment",
        StringParser::new("~", SyntaxKind::StatementTerminator)
            .to_matchable()
    );
    
    mysql_dialect.replace_grammar(
        "ParameterNameSegment",
        RegexParser::new(r#"`?[A-Za-z0-9_]*`?"#, SyntaxKind::Parameter)
            .to_matchable()
    );
    
    mysql_dialect.replace_grammar(
        "SingleIdentifierGrammar",
        ansi_dialect.grammar("SingleIdentifierGrammar").copy(Some(vec![Ref::new("SessionVariableNameSegment") .to_matchable()]), None, None, None, vec![], false)
    );
    
    mysql_dialect.replace_grammar(
        "AndOperatorGrammar",
        one_of(vec![StringParser::new("AND", SyntaxKind::BinaryOperator) .to_matchable(), StringParser::new("&&", SyntaxKind::BinaryOperator) .to_matchable()])
            .to_matchable()
    );
    
    mysql_dialect.replace_grammar(
        "OrOperatorGrammar",
        one_of(vec![StringParser::new("OR", SyntaxKind::BinaryOperator) .to_matchable(), StringParser::new("||", SyntaxKind::BinaryOperator) .to_matchable(), StringParser::new("XOR", SyntaxKind::BinaryOperator) .to_matchable()])
            .to_matchable()
    );
    
    mysql_dialect.replace_grammar(
        "NotOperatorGrammar",
        one_of(vec![StringParser::new("NOT", SyntaxKind::Keyword) .to_matchable(), StringParser::new("!", SyntaxKind::NotOperator) .to_matchable()])
            .to_matchable()
    );
    
    mysql_dialect.replace_grammar(
        "Expression_C_Grammar",
        Sequence::new(vec![Sequence::new(vec![Ref::new("SessionVariableNameSegment") .to_matchable(), Ref::new("WalrusOperatorSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), ansi_dialect.grammar("Expression_C_Grammar")])
            .to_matchable()
    );
    
    mysql_dialect.replace_grammar(
        "ColumnConstraintDefaultGrammar",
        one_of(vec![Bracketed::new(vec![ansi_dialect.grammar("ColumnConstraintDefaultGrammar")]) .to_matchable(), ansi_dialect.grammar("ColumnConstraintDefaultGrammar")])
            .to_matchable()
    );
    
    mysql_dialect.replace_grammar(
        "LikeGrammar",
        one_of(vec![Ref::keyword("LIKE") .to_matchable(), Ref::keyword("RLIKE") .to_matchable(), Ref::keyword("REGEXP") .to_matchable()])
            .to_matchable()
    );
    
    mysql_dialect.replace_grammar(
        "CollateGrammar",
        Sequence::new(vec![Ref::keyword("COLLATE") .to_matchable(), Ref::new("CollationReferenceSegment") .to_matchable()])
            .to_matchable()
    );
    
    mysql_dialect.replace_grammar(
        "ComparisonOperatorGrammar",
        ansi_dialect.grammar("ComparisonOperatorGrammar").copy(Some(vec![Ref::new("NullSafeEqualsSegment") .to_matchable()]), None, None, None, vec![], false)
    );
    
    mysql_dialect.add([
        (
            "DoubleQuotedLiteralSegment".into(),
            TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedLiteral)
                .to_matchable()
                .into()
        ),
        (
            "DoubleQuotedIdentifierSegment".into(),
            TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::QuotedIdentifier)
                .to_matchable()
                .into()
        ),
        (
            "AtSignLiteralSegment".into(),
            TypedParser::new(SyntaxKind::AtSignLiteral, SyntaxKind::AtSignLiteral)
                .to_matchable()
                .into()
        ),
        (
            "SystemVariableSegment".into(),
            RegexParser::new(r#"@@((session|global|local|persist|persist_only)\.)?[A-Za-z0-9_]+"#, SyntaxKind::SystemVariable)
                .to_matchable()
                .into()
        ),
        (
            "DivOperatorSegment".into(),
            StringParser::new("DIV", SyntaxKind::BinaryOperator)
                .to_matchable()
                .into()
        ),
        (
            "ModOperatorSegment".into(),
            StringParser::new("MOD", SyntaxKind::BinaryOperator)
                .to_matchable()
                .into()
        ),
        (
            "DoubleQuotedJSONPath".into(),
            TypedParser::new(SyntaxKind::DoubleQuote, SyntaxKind::JsonPath)
                .to_matchable()
                .into()
        ),
        (
            "SingleQuotedJSONPath".into(),
            TypedParser::new(SyntaxKind::SingleQuote, SyntaxKind::JsonPath)
                .to_matchable()
                .into()
        ),
    ]);
    
    mysql_dialect.add([
        (
            "AliasExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::AliasExpression, |_dialect| {
                Sequence::new(vec![MetaSegment::indent() .to_matchable(), Ref::new("AsAliasOperatorSegment") .optional() .to_matchable(), one_of(vec![Sequence::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), Bracketed::new(vec![Ref::new("SingleIdentifierListSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::new("SingleQuotedIdentifierSegment") .to_matchable(), Ref::new("DoubleQuotedIdentifierSegment") .to_matchable()]) .to_matchable(), MetaSegment::dedent() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ColumnDefinitionSegment".into(),
            NodeMatcher::new(SyntaxKind::ColumnDefinition, |_dialect| {
                Sequence::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), one_of(vec![Ref::new("DatatypeSegment") .exclude(one_of(vec![Ref::keyword("DATETIME") .to_matchable(), Ref::keyword("TIMESTAMP") .to_matchable()])) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("DATETIME") .to_matchable(), Ref::keyword("TIMESTAMP") .to_matchable()]) .to_matchable(), Ref::new("BracketedArguments") .optional() .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Sequence::new(vec![Ref::keyword("NOT") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("NULL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("DEFAULT") .to_matchable(), one_of(vec![Sequence::new(vec![one_of(vec![Ref::keyword("CURRENT_TIMESTAMP") .to_matchable(), Ref::keyword("NOW") .to_matchable()]) .to_matchable(), Bracketed::new(vec![Ref::new("NumericLiteralSegment") .optional() .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::keyword("NULL") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("UPDATE") .to_matchable(), one_of(vec![Ref::keyword("CURRENT_TIMESTAMP") .to_matchable(), Ref::keyword("NOW") .to_matchable(), Bracketed::new(vec![Ref::new("NumericLiteralSegment") .optional() .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable(), Bracketed::new(vec![Anything::new() .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), AnyNumberOf::new(vec![Ref::new("ColumnConstraintSegment") .optional() .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateTableStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::new("OrReplaceGrammar") .optional() .to_matchable(), Ref::new("TemporaryTransientGrammar") .optional() .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Ref::new("IfNotExistsGrammar") .optional() .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), one_of(vec![Sequence::new(vec![Bracketed::new(vec![Delimited::new(vec![one_of(vec![Ref::new("TableConstraintSegment") .to_matchable(), Ref::new("ColumnDefinitionSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .optional() .to_matchable(), optionally_bracketed(vec![Ref::new("SelectableGrammar") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("CommentClauseSegment") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .optional() .to_matchable(), optionally_bracketed(vec![Ref::new("SelectableGrammar") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("LIKE") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("TableEndClauseSegment") .optional() .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("DEFAULT") .optional() .to_matchable(), one_of(vec![Ref::new("ParameterNameSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("CHARACTER") .to_matchable(), Ref::keyword("SET") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("DATA") .to_matchable(), Ref::keyword("INDEX") .to_matchable()]) .to_matchable(), Ref::keyword("DIRECTORY") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("SYSTEM") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::new("LiteralGrammar") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("SingleQuotedIdentifierSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("PARTITION") .to_matchable(), Ref::keyword("BY") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("LINEAR") .optional() .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("HASH") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("KEY") .to_matchable(), Sequence::new(vec![Ref::keyword("ALGORITHM") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Delimited::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("RANGE") .to_matchable(), Ref::keyword("LIST") .to_matchable()]) .to_matchable(), one_of(vec![Ref::new("ExpressionSegment") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("PARTITIONS") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("SUBPARTITION") .to_matchable(), Ref::keyword("BY") .to_matchable(), Sequence::new(vec![Ref::keyword("LINEAR") .optional() .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("HASH") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("KEY") .to_matchable(), Sequence::new(vec![Ref::keyword("ALGORITHM") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Bracketed::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SUBPARTITIONS") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), AnyNumberOf::new(vec![Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::keyword("PARTITION") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("VALUES") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("LESS") .to_matchable(), Ref::keyword("THAN") .to_matchable(), one_of(vec![Ref::keyword("MAXVALUE") .to_matchable(), Bracketed::new(vec![one_of(vec![Ref::new("ExpressionSegment") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("LiteralGrammar") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("IN") .to_matchable(), Bracketed::new(vec![Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::new("ParameterNameSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("CHARACTER") .to_matchable(), Ref::keyword("SET") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("DATA") .to_matchable(), Ref::keyword("INDEX") .to_matchable()]) .to_matchable(), Ref::keyword("DIRECTORY") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("SYSTEM") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::new("LiteralGrammar") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("SingleQuotedIdentifierSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SUBPARTITION") .optional() .to_matchable(), Ref::new("LiteralGrammar") .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("VALUES") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("LESS") .to_matchable(), Ref::keyword("THAN") .to_matchable(), one_of(vec![Ref::keyword("MAXVALUE") .to_matchable(), Bracketed::new(vec![Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), Bracketed::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("IN") .to_matchable(), Bracketed::new(vec![Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::new("ParameterNameSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("CHARACTER") .to_matchable(), Ref::keyword("SET") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("DATA") .to_matchable(), Ref::keyword("INDEX") .to_matchable()]) .to_matchable(), Ref::keyword("DIRECTORY") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("SYSTEM") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::new("LiteralGrammar") .to_matchable(), Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("SingleQuotedIdentifierSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateUserStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateUserStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::keyword("USER") .to_matchable(), Ref::new("IfNotExistsGrammar") .optional() .to_matchable(), Delimited::new(vec![Sequence::new(vec![Ref::new("RoleReferenceSegment") .to_matchable(), Sequence::new(vec![Delimited::new(vec![Sequence::new(vec![Ref::keyword("IDENTIFIED") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("BY") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("RANDOM") .to_matchable(), Ref::keyword("PASSWORD") .to_matchable()]) .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Sequence::new(vec![one_of(vec![Sequence::new(vec![Ref::keyword("BY") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("RANDOM") .to_matchable(), Ref::keyword("PASSWORD") .to_matchable()]) .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("AS") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("INITIAL") .to_matchable(), Ref::keyword("AUTHENTICATION") .to_matchable(), Ref::keyword("IDENTIFIED") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("BY") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("RANDOM") .to_matchable(), Ref::keyword("PASSWORD") .to_matchable()]) .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::keyword("AS") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.delimiter(Ref::keyword("AND")); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DEFAULT") .to_matchable(), Ref::keyword("ROLE") .to_matchable(), Delimited::new(vec![Ref::new("RoleReferenceSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("REQUIRE") .to_matchable(), one_of(vec![Ref::keyword("NONE") .to_matchable(), Delimited::new(vec![one_of(vec![Ref::keyword("SSL") .to_matchable(), Ref::keyword("X509") .to_matchable(), Sequence::new(vec![Ref::keyword("CIPHER") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ISSUER") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SUBJECT") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.delimiter(Ref::keyword("AND")); }) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![one_of(vec![Ref::keyword("MAX_QUERIES_PER_HOUR") .to_matchable(), Ref::keyword("MAX_UPDATES_PER_HOUR") .to_matchable(), Ref::keyword("MAX_CONNECTIONS_PER_HOUR") .to_matchable(), Ref::keyword("MAX_USER_CONNECTIONS") .to_matchable()]) .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("PASSWORD") .to_matchable(), Ref::keyword("EXPIRE") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("DEFAULT") .to_matchable(), Ref::keyword("NEVER") .to_matchable(), Sequence::new(vec![Ref::keyword("INTERVAL") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::keyword("DAY") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("PASSWORD") .to_matchable(), Ref::keyword("HISTORY") .to_matchable(), one_of(vec![Ref::keyword("DEFAULT") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("PASSWORD") .to_matchable(), Ref::keyword("REUSE") .to_matchable(), Ref::keyword("INTERVAL") .to_matchable(), one_of(vec![Ref::keyword("DEFAULT") .to_matchable(), Sequence::new(vec![Ref::new("NumericLiteralSegment") .to_matchable(), Ref::keyword("DAY") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("PASSWORD") .to_matchable(), Ref::keyword("REQUIRE") .to_matchable(), Ref::keyword("CURRENT") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("DEFAULT") .to_matchable(), Ref::keyword("OPTIONAL") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FAILED_LOGIN_ATTEMPTS") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("PASSWORD_LOCK_TIME") .to_matchable(), one_of(vec![Ref::new("NumericLiteralSegment") .to_matchable(), Ref::keyword("UNBOUNDED") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("ACCOUNT") .to_matchable(), one_of(vec![Ref::keyword("UNLOCK") .to_matchable(), Ref::keyword("LOCK") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("COMMENT") .to_matchable(), Ref::keyword("ATTRIBUTE") .to_matchable()]) .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "UpsertClauseListSegment".into(),
            NodeMatcher::new(SyntaxKind::UpsertClauseList, |_dialect| {
                Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("DUPLICATE") .to_matchable(), Ref::keyword("KEY") .to_matchable(), Ref::keyword("UPDATE") .to_matchable(), Delimited::new(vec![Ref::new("SetClauseSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "InsertRowAliasSegment".into(),
            NodeMatcher::new(SyntaxKind::InsertRowAlias, |_dialect| {
                Sequence::new(vec![Ref::keyword("AS") .to_matchable(), Ref::new("SingleIdentifierGrammar") .to_matchable(), Bracketed::new(vec![Ref::new("SingleIdentifierListSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "InsertStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::InsertStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("INSERT") .to_matchable(), one_of(vec![Ref::keyword("LOW_PRIORITY") .to_matchable(), Ref::keyword("DELAYED") .to_matchable(), Ref::keyword("HIGH_PRIORITY") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("IGNORE") .optional() .to_matchable(), Ref::keyword("INTO") .optional() .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("PARTITION") .to_matchable(), Bracketed::new(vec![Ref::new("SingleIdentifierListSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .optional() .to_matchable(), AnyNumberOf::new(vec![one_of(vec![Ref::new("ValuesClauseSegment") .to_matchable(), Ref::new("SetClauseListSegment") .to_matchable(), Sequence::new(vec![one_of(vec![Ref::new("SelectableGrammar") .to_matchable(), Sequence::new(vec![Ref::keyword("TABLE") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("InsertRowAliasSegment") .optional() .to_matchable(), Ref::new("UpsertClauseListSegment") .optional() .to_matchable()]) .config(|this| { this.max_times_per_element = Some(1); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DeleteTargetTableSegment".into(),
            NodeMatcher::new(SyntaxKind::DeleteTargetTable, |_dialect| {
                Sequence::new(vec![Ref::new("TableReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::new("DotSegment") .to_matchable(), Ref::new("StarSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DeleteUsingClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::UsingClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("USING") .to_matchable(), Delimited::new(vec![Ref::new("FromExpressionSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DeleteStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DeleteStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("DELETE") .to_matchable(), Ref::keyword("LOW_PRIORITY") .optional() .to_matchable(), Ref::keyword("QUICK") .optional() .to_matchable(), Ref::keyword("IGNORE") .optional() .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("FROM") .to_matchable(), Delimited::new(vec![Ref::new("DeleteTargetTableSegment") .to_matchable()]) .config(|this| { this.terminators = vec![Ref::keyword("USING") .to_matchable()]; }) .to_matchable(), Ref::new("DeleteUsingClauseSegment") .to_matchable(), Ref::new("WhereClauseSegment") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Delimited::new(vec![Ref::new("DeleteTargetTableSegment") .to_matchable()]) .config(|this| { this.terminators = vec![Ref::keyword("FROM") .to_matchable()]; }) .to_matchable(), Ref::new("FromClauseSegment") .to_matchable(), Ref::new("WhereClauseSegment") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("FromClauseSegment") .to_matchable(), Ref::new("SelectPartitionClauseSegment") .optional() .to_matchable(), Ref::new("WhereClauseSegment") .optional() .to_matchable(), Ref::new("OrderByClauseSegment") .optional() .to_matchable(), Ref::new("LimitClauseSegment") .optional() .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ColumnConstraintSegment".into(),
            NodeMatcher::new(SyntaxKind::ColumnConstraintSegment, |_dialect| {
                one_of(vec![{ let dialect = super::ansi::raw_dialect(); dialect.grammar("ColumnConstraintSegment").match_grammar(&dialect).unwrap() }, Sequence::new(vec![Ref::keyword("CHARACTER") .to_matchable(), Ref::keyword("SET") .to_matchable(), one_of(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), Ref::new("SingleQuotedIdentifierSegment") .to_matchable(), Ref::new("DoubleQuotedIdentifierSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::new("CollateGrammar") .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::keyword("GENERATED") .to_matchable(), Ref::keyword("ALWAYS") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("AS") .to_matchable(), Bracketed::new(vec![Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("STORED") .to_matchable(), Ref::keyword("VIRTUAL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SRID") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("INVISIBLE") .to_matchable(), Ref::keyword("VISIBLE") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "IndexTypeGrammar".into(),
            NodeMatcher::new(SyntaxKind::IndexType, |_dialect| {
                Sequence::new(vec![Ref::keyword("USING") .to_matchable(), one_of(vec![Ref::keyword("BTREE") .to_matchable(), Ref::keyword("HASH") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "IndexOptionsSegment".into(),
            NodeMatcher::new(SyntaxKind::IndexOption, |_dialect| {
                AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("KEY_BLOCK_SIZE") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Ref::new("IndexTypeGrammar") .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("PARSER") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Ref::new("CommentClauseSegment") .to_matchable(), one_of(vec![Ref::keyword("VISIBLE") .to_matchable(), Ref::keyword("INVISIBLE") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ENGINE_ATTRIBUTE") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SECONDARY_ENGINE_ATTRIBUTE") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()])
                    .config(|this| {
                        this.max_times_per_element = Some(1);
                    })
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "TableConstraintSegment".into(),
            NodeMatcher::new(SyntaxKind::TableConstraint, |_dialect| {
                one_of(vec![Sequence::new(vec![Sequence::new(vec![Ref::keyword("CONSTRAINT") .to_matchable(), Ref::new("ObjectReferenceSegment") .optional() .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("UNIQUE") .to_matchable(), one_of(vec![Ref::keyword("INDEX") .to_matchable(), Ref::keyword("KEY") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("IndexReferenceSegment") .optional() .to_matchable(), Ref::new("IndexTypeGrammar") .optional() .to_matchable(), Ref::new("BracketedKeyPartListGrammar") .to_matchable(), Ref::new("IndexOptionsSegment") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("PrimaryKeyGrammar") .to_matchable(), Ref::new("IndexTypeGrammar") .optional() .to_matchable(), Ref::new("BracketedKeyPartListGrammar") .to_matchable(), Ref::new("IndexOptionsSegment") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("ForeignKeyGrammar") .to_matchable(), Ref::new("IndexReferenceSegment") .optional() .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .to_matchable(), Ref::keyword("REFERENCES") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("ON") .to_matchable(), one_of(vec![Ref::keyword("DELETE") .to_matchable(), Ref::keyword("UPDATE") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("RESTRICT") .to_matchable(), Ref::keyword("CASCADE") .to_matchable(), Sequence::new(vec![Ref::keyword("SET") .to_matchable(), Ref::keyword("NULL") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("NO") .to_matchable(), Ref::keyword("ACTION") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SET") .to_matchable(), Ref::keyword("DEFAULT") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CHECK") .to_matchable(), Bracketed::new(vec![Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("ENFORCED") .to_matchable(), Sequence::new(vec![Ref::keyword("NOT") .to_matchable(), Ref::keyword("ENFORCED") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("INDEX") .to_matchable(), Ref::keyword("KEY") .to_matchable()]) .to_matchable(), Ref::new("IndexReferenceSegment") .optional() .to_matchable(), Ref::new("IndexTypeGrammar") .optional() .to_matchable(), Ref::new("BracketedKeyPartListGrammar") .to_matchable(), Ref::new("IndexOptionsSegment") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("FULLTEXT") .to_matchable(), Ref::keyword("SPATIAL") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("INDEX") .to_matchable(), Ref::keyword("KEY") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("IndexReferenceSegment") .optional() .to_matchable(), Ref::new("BracketedKeyPartListGrammar") .to_matchable(), Ref::new("IndexOptionsSegment") .optional() .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateIndexStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateIndexStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), one_of(vec![Ref::keyword("UNIQUE") .to_matchable(), Ref::keyword("FULLTEXT") .to_matchable(), Ref::keyword("SPATIAL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("INDEX") .to_matchable(), Ref::new("IndexReferenceSegment") .to_matchable(), Ref::new("IndexTypeGrammar") .optional() .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("BracketedKeyPartListGrammar") .to_matchable(), Ref::new("IndexOptionsSegment") .optional() .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("ALGORITHM") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::keyword("DEFAULT") .to_matchable(), Ref::keyword("INPLACE") .to_matchable(), Ref::keyword("COPY") .to_matchable(), Ref::keyword("NOCOPY") .to_matchable(), Ref::keyword("INSTANT") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("LOCK") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::keyword("DEFAULT") .to_matchable(), Ref::keyword("NONE") .to_matchable(), Ref::keyword("SHARED") .to_matchable(), Ref::keyword("EXCLUSIVE") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.max_times_per_element = Some(1); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "IntervalExpressionSegment".into(),
            NodeMatcher::new(SyntaxKind::IntervalExpression, |_dialect| {
                Sequence::new(vec![Ref::keyword("INTERVAL") .to_matchable(), one_of(vec![Ref::new("DatetimeUnitSegment") .to_matchable(), Sequence::new(vec![Ref::new("ExpressionSegment") .to_matchable(), Ref::new("DatetimeUnitSegment") .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
    ]);
    
    mysql_dialect.add([
        (
            "OutputParameterSegment".into(),
            StringParser::new("OUT", SyntaxKind::ParameterDirection)
                .to_matchable()
                .into()
        ),
        (
            "InputParameterSegment".into(),
            StringParser::new("IN", SyntaxKind::ParameterDirection)
                .to_matchable()
                .into()
        ),
        (
            "InputOutputParameterSegment".into(),
            StringParser::new("INOUT", SyntaxKind::ParameterDirection)
                .to_matchable()
                .into()
        ),
        (
            "ProcedureParameterGrammar".into(),
            one_of(vec![Sequence::new(vec![one_of(vec![Ref::new("OutputParameterSegment") .to_matchable(), Ref::new("InputParameterSegment") .to_matchable(), Ref::new("InputOutputParameterSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("ParameterNameSegment") .optional() .to_matchable(), Ref::new("DatatypeSegment") .to_matchable()]) .to_matchable(), Ref::new("DatatypeSegment") .to_matchable()])
                .to_matchable()
                .into()
        ),
        (
            "LocalVariableNameSegment".into(),
            RegexParser::new(r#"`?[a-zA-Z0-9_$]*`?"#, SyntaxKind::Variable)
                .to_matchable()
                .into()
        ),
        (
            "SessionVariableNameSegment".into(),
            RegexParser::new(r#"[@][a-zA-Z0-9_$]*"#, SyntaxKind::Variable)
                .to_matchable()
                .into()
        ),
        (
            "WalrusOperatorSegment".into(),
            StringParser::new(":=", SyntaxKind::AssignmentOperator)
                .to_matchable()
                .into()
        ),
        (
            "VariableAssignmentSegment".into(),
            Sequence::new(vec![Ref::new("SessionVariableNameSegment") .to_matchable(), Ref::new("WalrusOperatorSegment") .to_matchable(), Ref::new("BaseExpressionElementGrammar") .to_matchable()])
                .to_matchable()
                .into()
        ),
        (
            "ColumnPathOperatorSegment".into(),
            StringParser::new("->", SyntaxKind::ColumnPathOperator)
                .to_matchable()
                .into()
        ),
        (
            "InlinePathOperatorSegment".into(),
            StringParser::new("->>", SyntaxKind::ColumnPathOperator)
                .to_matchable()
                .into()
        ),
        (
            "BooleanDynamicSystemVariablesGrammar".into(),
            one_of(vec![one_of(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("OFF") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("TRUE") .to_matchable(), Ref::keyword("FALSE") .to_matchable()]) .to_matchable()])
                .to_matchable()
                .into()
        ),
        (
            "BracketedKeyPartListGrammar".into(),
            Bracketed::new(vec![Delimited::new(vec![Sequence::new(vec![one_of(vec![Ref::new("ColumnReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable(), Bracketed::new(vec![Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Bracketed::new(vec![Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("ASC") .to_matchable(), Ref::keyword("DESC") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()])
                .to_matchable()
                .into()
        ),
    ]);
    
    mysql_dialect.insert_lexer_matchers(vec![
        Matcher::regex("at_sign", r#"@@?[a-zA-Z0-9_$]*(\.[a-zA-Z0-9_$]+)?"#, SyntaxKind::AtSignLiteral),
    ], "word");
    
    mysql_dialect.insert_lexer_matchers(vec![
        Matcher::string("double_ampersand", r#"&&"#, SyntaxKind::DoubleAmpersand),
    ], "ampersand");
    
    mysql_dialect.insert_lexer_matchers(vec![
        Matcher::string("double_vertical_bar", r#"||"#, SyntaxKind::DoubleVerticalBar),
    ], "vertical_bar");
    
    mysql_dialect.insert_lexer_matchers(vec![
        Matcher::string("walrus_operator", r#":="#, SyntaxKind::WalrusOperator),
    ], "equals");
    
    mysql_dialect.insert_lexer_matchers(vec![
        Matcher::string("inline_path_operator", r#"->>"#, SyntaxKind::InlinePathOperator),
        Matcher::string("column_path_operator", r#"->"#, SyntaxKind::ColumnPathOperator),
    ], "greater_than");
    
    mysql_dialect.add([
        (
            "RoleReferenceSegment".into(),
            NodeMatcher::new(SyntaxKind::RoleReference, |_dialect| {
                one_of(vec![Sequence::new(vec![one_of(vec![Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::new("QuotedIdentifierSegment") .to_matchable(), Ref::new("SingleQuotedIdentifierSegment") .to_matchable(), Ref::new("DoubleQuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("AtSignLiteralSegment") .to_matchable(), one_of(vec![Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::new("QuotedIdentifierSegment") .to_matchable(), Ref::new("SingleQuotedIdentifierSegment") .to_matchable(), Ref::new("DoubleQuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); this.disallow_gaps(); }) .to_matchable()]) .to_matchable(), Ref::keyword("CURRENT_USER") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DeclareStatement".into(),
            NodeMatcher::new(SyntaxKind::DeclareStatement, |_dialect| {
                one_of(vec![Sequence::new(vec![Ref::keyword("DECLARE") .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::keyword("CURSOR") .to_matchable(), Ref::keyword("FOR") .to_matchable(), Ref::new("StatementSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DECLARE") .to_matchable(), one_of(vec![Ref::keyword("CONTINUE") .to_matchable(), Ref::keyword("EXIT") .to_matchable(), Ref::keyword("UNDO") .to_matchable()]) .to_matchable(), Ref::keyword("HANDLER") .to_matchable(), Ref::keyword("FOR") .to_matchable(), one_of(vec![Ref::keyword("SQLEXCEPTION") .to_matchable(), Ref::keyword("SQLWARNING") .to_matchable(), Sequence::new(vec![Ref::keyword("NOT") .to_matchable(), Ref::keyword("FOUND") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SQLSTATE") .to_matchable(), Ref::keyword("VALUE") .optional() .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("StatementSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DECLARE") .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::keyword("CONDITION") .to_matchable(), Ref::keyword("FOR") .to_matchable(), one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DECLARE") .to_matchable(), Ref::new("LocalVariableNameSegment") .to_matchable(), Ref::new("DatatypeSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("DEFAULT") .to_matchable(), one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("FunctionSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "StatementSegment".into(),
            NodeMatcher::new(SyntaxKind::Statement, |_dialect| {
                { let dialect = super::ansi::raw_dialect(); dialect.grammar("StatementSegment").match_grammar(&dialect).unwrap() }.copy(Some(vec![Ref::new("DelimiterStatement") .to_matchable(), Ref::new("CreateProcedureStatementSegment") .to_matchable(), Ref::new("DeclareStatement") .to_matchable(), Ref::new("SetTransactionStatementSegment") .to_matchable(), Ref::new("SetAssignmentStatementSegment") .to_matchable(), Ref::new("IfExpressionStatement") .to_matchable(), Ref::new("WhileStatementSegment") .to_matchable(), Ref::new("IterateStatementSegment") .to_matchable(), Ref::new("RepeatStatementSegment") .to_matchable(), Ref::new("LoopStatementSegment") .to_matchable(), Ref::new("CallStoredProcedureSegment") .to_matchable(), Ref::new("PrepareSegment") .to_matchable(), Ref::new("ExecuteSegment") .to_matchable(), Ref::new("DeallocateSegment") .to_matchable(), Ref::new("GetDiagnosticsSegment") .to_matchable(), Ref::new("ResignalSegment") .to_matchable(), Ref::new("CursorOpenCloseSegment") .to_matchable(), Ref::new("CursorFetchSegment") .to_matchable(), Ref::new("DropProcedureStatementSegment") .to_matchable(), Ref::new("AlterTableStatementSegment") .to_matchable(), Ref::new("AlterViewStatementSegment") .to_matchable(), Ref::new("CreateViewStatementSegment") .to_matchable(), Ref::new("RenameTableStatementSegment") .to_matchable(), Ref::new("ResetMasterStatementSegment") .to_matchable(), Ref::new("PurgeBinaryLogsStatementSegment") .to_matchable(), Ref::new("HelpStatementSegment") .to_matchable(), Ref::new("CheckTableStatementSegment") .to_matchable(), Ref::new("ChecksumTableStatementSegment") .to_matchable(), Ref::new("AnalyzeTableStatementSegment") .to_matchable(), Ref::new("RepairTableStatementSegment") .to_matchable(), Ref::new("OptimizeTableStatementSegment") .to_matchable(), Ref::new("UpsertClauseListSegment") .to_matchable(), Ref::new("InsertRowAliasSegment") .to_matchable(), Ref::new("FlushStatementSegment") .to_matchable(), Ref::new("LoadDataSegment") .to_matchable(), Ref::new("ReplaceSegment") .to_matchable(), Ref::new("AlterDatabaseStatementSegment") .to_matchable(), Ref::new("ReturnStatementSegment") .to_matchable(), Ref::new("SetNamesStatementSegment") .to_matchable(), Ref::new("CreateEventStatementSegment") .to_matchable(), Ref::new("AlterEventStatementSegment") .to_matchable(), Ref::new("DropEventStatementSegment") .to_matchable()]), None, None, Some(vec![Ref::new("CreateSchemaStatementSegment") .to_matchable()]), vec![], false)
            })
                .to_matchable()
                .into()
        ),
        (
            "DelimiterStatement".into(),
            NodeMatcher::new(SyntaxKind::DelimiterStatement, |_dialect| {
                Ref::keyword("DELIMITER")
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateProcedureStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateProcedureStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::new("DefinerSegment") .optional() .to_matchable(), Ref::keyword("PROCEDURE") .to_matchable(), Ref::new("IfNotExistsGrammar") .optional() .to_matchable(), Ref::new("FunctionNameSegment") .to_matchable(), Ref::new("ProcedureParameterListGrammar") .optional() .to_matchable(), Ref::new("CommentClauseSegment") .optional() .to_matchable(), Ref::new("CharacteristicStatement") .optional() .to_matchable(), Ref::new("FunctionDefinitionGrammar") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FunctionDefinitionGrammar".into(),
            NodeMatcher::new(SyntaxKind::FunctionDefinition, |_dialect| {
                Ref::new("TransactionStatementSegment")
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CharacteristicStatement".into(),
            NodeMatcher::new(SyntaxKind::CharacteristicStatement, |_dialect| {
                Sequence::new(vec![one_of(vec![Ref::keyword("DETERMINISTIC") .to_matchable(), Sequence::new(vec![Ref::keyword("NOT") .to_matchable(), Ref::keyword("DETERMINISTIC") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("LANGUAGE") .to_matchable(), Ref::keyword("SQL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("CONTAINS") .to_matchable(), Ref::keyword("SQL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("NO") .to_matchable(), Ref::keyword("SQL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("READS") .to_matchable(), Ref::keyword("SQL") .to_matchable(), Ref::keyword("DATA") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("MODIFIES") .to_matchable(), Ref::keyword("SQL") .to_matchable(), Ref::keyword("DATA") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("SQL") .to_matchable(), Ref::keyword("SECURITY") .to_matchable(), one_of(vec![Ref::keyword("DEFINER") .to_matchable(), Ref::keyword("INVOKER") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateFunctionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateFunctionStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::new("DefinerSegment") .optional() .to_matchable(), Ref::keyword("FUNCTION") .to_matchable(), Ref::new("FunctionNameSegment") .to_matchable(), Ref::new("FunctionParameterListGrammar") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("RETURNS") .to_matchable(), Ref::new("DatatypeSegment") .to_matchable()]) .to_matchable(), Ref::new("CommentClauseSegment") .optional() .to_matchable(), Ref::new("CharacteristicStatement") .to_matchable(), Ref::new("FunctionDefinitionGrammar") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AlterTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterTableStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("ALTER") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Delimited::new(vec![one_of(vec![Sequence::new(vec![Ref::new("ParameterNameSegment") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::new("LiteralGrammar") .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ADD") .to_matchable(), Ref::keyword("COLUMN") .optional() .to_matchable(), Ref::new("IfNotExistsGrammar") .optional() .to_matchable(), Ref::new("ColumnDefinitionSegment") .to_matchable(), one_of(vec![Ref::keyword("FIRST") .to_matchable(), Sequence::new(vec![Ref::keyword("AFTER") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ALTER") .to_matchable(), Ref::keyword("COLUMN") .optional() .to_matchable(), Ref::new("SingleIdentifierGrammar") .to_matchable(), AnyNumberOf::new(vec![one_of(vec![Sequence::new(vec![Ref::keyword("SET") .to_matchable(), Ref::keyword("DEFAULT") .to_matchable(), one_of(vec![Ref::new("LiteralGrammar") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), Ref::keyword("DEFAULT") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SET") .to_matchable(), one_of(vec![Ref::keyword("INVISIBLE") .to_matchable(), Ref::keyword("VISIBLE") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.max_times_per_element = Some(1); this.min_times(1); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("MODIFY") .to_matchable(), Ref::keyword("COLUMN") .optional() .to_matchable(), Ref::new("ColumnDefinitionSegment") .to_matchable(), one_of(vec![Ref::keyword("FIRST") .to_matchable(), Sequence::new(vec![Ref::keyword("AFTER") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ADD") .to_matchable(), Ref::new("TableConstraintSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CHANGE") .to_matchable(), Ref::keyword("COLUMN") .optional() .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::new("ColumnDefinitionSegment") .to_matchable(), one_of(vec![Sequence::new(vec![one_of(vec![Ref::keyword("FIRST") .to_matchable(), Sequence::new(vec![Ref::keyword("AFTER") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("COLUMN") .optional() .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("INDEX") .to_matchable(), Ref::keyword("KEY") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("IndexReferenceSegment") .to_matchable()]) .to_matchable(), Ref::new("PrimaryKeyGrammar") .to_matchable(), Sequence::new(vec![Ref::new("ForeignKeyGrammar") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("CHECK") .to_matchable(), Ref::keyword("CONSTRAINT") .to_matchable()]) .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ALTER") .to_matchable(), one_of(vec![Ref::keyword("CHECK") .to_matchable(), Ref::keyword("CONSTRAINT") .to_matchable()]) .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), one_of(vec![Ref::keyword("ENFORCED") .to_matchable(), Sequence::new(vec![Ref::keyword("NOT") .to_matchable(), Ref::keyword("ENFORCED") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ALTER") .to_matchable(), Ref::keyword("INDEX") .to_matchable(), Ref::new("IndexReferenceSegment") .to_matchable(), one_of(vec![Ref::keyword("VISIBLE") .to_matchable(), Ref::keyword("INVISIBLE") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("RENAME") .to_matchable(), one_of(vec![Sequence::new(vec![one_of(vec![Ref::keyword("AS") .to_matchable(), Ref::keyword("TO") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("INDEX") .to_matchable(), Ref::keyword("KEY") .to_matchable()]) .to_matchable(), Ref::new("IndexReferenceSegment") .to_matchable(), Ref::keyword("TO") .to_matchable(), Ref::new("IndexReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("COLUMN") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable(), Ref::keyword("TO") .to_matchable(), Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("DISABLE") .to_matchable(), Ref::keyword("ENABLE") .to_matchable()]) .to_matchable(), Ref::keyword("KEYS") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("CONVERT") .to_matchable(), Ref::keyword("TO") .to_matchable(), AnyNumberOf::new(vec![Ref::new("AlterOptionSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("ADD") .to_matchable(), Ref::keyword("DROP") .to_matchable(), Ref::keyword("DISCARD") .to_matchable(), Ref::keyword("IMPORT") .to_matchable(), Ref::keyword("TRUNCATE") .to_matchable(), Ref::keyword("COALESCE") .to_matchable(), Ref::keyword("REORGANIZE") .to_matchable(), Ref::keyword("EXCHANGE") .to_matchable(), Ref::keyword("ANALYZE") .to_matchable(), Ref::keyword("CHECK") .to_matchable(), Ref::keyword("OPTIMIZE") .to_matchable(), Ref::keyword("REBUILD") .to_matchable(), Ref::keyword("REPAIR") .to_matchable(), Ref::keyword("REMOVE") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("PARTITION") .to_matchable(), Ref::keyword("PARTITIONING") .to_matchable()]) .to_matchable(), one_of(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::keyword("ALL") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Ref::keyword("TABLESPACE") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Ref::new("TableReference") .to_matchable(), one_of(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("WITHOUT") .to_matchable()]) .to_matchable(), Ref::keyword("VALIDATION") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("INTO") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "WithCheckOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::WithCheckOptions, |_dialect| {
                Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), one_of(vec![Ref::keyword("CASCADED") .to_matchable(), Ref::keyword("LOCAL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("CHECK") .to_matchable(), Ref::keyword("OPTION") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AlterViewStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterViewStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("ALTER") .to_matchable(), Sequence::new(vec![Ref::keyword("ALGORITHM") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("UNDEFINED") .to_matchable(), Ref::keyword("MERGE") .to_matchable(), Ref::keyword("TEMPTABLE") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("DefinerSegment") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("SQL") .to_matchable(), Ref::keyword("SECURITY") .to_matchable(), one_of(vec![Ref::keyword("DEFINER") .to_matchable(), Ref::keyword("INVOKER") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("VIEW") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .optional() .to_matchable(), Ref::keyword("AS") .to_matchable(), optionally_bracketed(vec![Ref::new("SelectableGrammar") .to_matchable()]) .to_matchable(), Ref::new("WithCheckOptionSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateViewStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateViewStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::new("OrReplaceGrammar") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("ALGORITHM") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("UNDEFINED") .to_matchable(), Ref::keyword("MERGE") .to_matchable(), Ref::keyword("TEMPTABLE") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("DefinerSegment") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("SQL") .to_matchable(), Ref::keyword("SECURITY") .to_matchable(), one_of(vec![Ref::keyword("DEFINER") .to_matchable(), Ref::keyword("INVOKER") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("VIEW") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("BracketedColumnReferenceListGrammar") .optional() .to_matchable(), Ref::keyword("AS") .to_matchable(), optionally_bracketed(vec![Ref::new("SelectableGrammar") .to_matchable()]) .to_matchable(), Ref::new("WithCheckOptionSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ProcedureParameterListGrammar".into(),
            NodeMatcher::new(SyntaxKind::ProcedureParameterList, |_dialect| {
                Bracketed::new(vec![Delimited::new(vec![Ref::new("ProcedureParameterGrammar") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SetAssignmentStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SetStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("SET") .to_matchable(), Delimited::new(vec![Sequence::new(vec![Sequence::new(vec![one_of(vec![Ref::keyword("NEW") .to_matchable(), Ref::keyword("OLD") .to_matchable()]) .to_matchable(), Ref::new("DotSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Ref::keyword("GLOBAL") .to_matchable(), Ref::keyword("PERSIST") .to_matchable(), Ref::keyword("PERSIST_ONLY") .to_matchable(), Ref::keyword("SESSION") .to_matchable(), Ref::keyword("LOCAL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Ref::new("SessionVariableNameSegment") .to_matchable(), Ref::new("LocalVariableNameSegment") .to_matchable(), Ref::new("SystemVariableSegment") .to_matchable()]) .to_matchable(), one_of(vec![Ref::new("EqualsSegment") .to_matchable(), Ref::new("WalrusOperatorSegment") .to_matchable()]) .to_matchable(), AnyNumberOf::new(vec![Ref::new("NumericLiteralSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("DoubleQuotedLiteralSegment") .to_matchable(), Ref::new("SessionVariableNameSegment") .to_matchable(), Ref::new("SystemVariableSegment") .to_matchable(), Ref::new("BooleanDynamicSystemVariablesGrammar") .to_matchable(), Ref::new("LocalVariableNameSegment") .to_matchable(), Ref::new("FunctionSegment") .to_matchable(), Ref::new("ArithmeticBinaryOperatorGrammar") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "TransactionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::TransactionStatement, |_dialect| {
                one_of(vec![Sequence::new(vec![Ref::keyword("START") .to_matchable(), Ref::keyword("TRANSACTION") .to_matchable()]) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), Ref::new("ColonSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("BEGIN") .to_matchable(), Ref::keyword("WORK") .optional() .to_matchable(), Ref::new("StatementSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("LEAVE") .to_matchable(), Ref::new("SingleIdentifierGrammar") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("COMMIT") .to_matchable(), Ref::keyword("WORK") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("AND") .to_matchable(), Ref::keyword("NO") .optional() .to_matchable(), Ref::keyword("CHAIN") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ROLLBACK") .to_matchable(), Ref::keyword("WORK") .optional() .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("END") .to_matchable(), Ref::new("SingleIdentifierGrammar") .optional() .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "IfExpressionStatement".into(),
            NodeMatcher::new(SyntaxKind::IfThenStatement, |_dialect| {
                AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("IF") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable(), Ref::keyword("THEN") .to_matchable(), Ref::new("StatementSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ELSEIF") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable(), Ref::keyword("THEN") .to_matchable(), Ref::new("StatementSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ELSE") .to_matchable(), Ref::new("StatementSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("END") .to_matchable(), Ref::keyword("IF") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DefinerSegment".into(),
            NodeMatcher::new(SyntaxKind::DefinerSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("DEFINER") .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), Ref::new("RoleReferenceSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SelectClauseModifierSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectClauseModifier, |_dialect| {
                Sequence::new(vec![one_of(vec![Ref::keyword("DISTINCT") .to_matchable(), Ref::keyword("ALL") .to_matchable(), Ref::keyword("DISTINCTROW") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("HIGH_PRIORITY") .optional() .to_matchable(), Ref::keyword("STRAIGHT_JOIN") .optional() .to_matchable(), Ref::keyword("SQL_SMALL_RESULT") .optional() .to_matchable(), Ref::keyword("SQL_BIG_RESULT") .optional() .to_matchable(), Ref::keyword("SQL_BUFFER_RESULT") .optional() .to_matchable(), Ref::keyword("SQL_CACHE") .optional() .to_matchable(), Ref::keyword("SQL_NO_CACHE") .optional() .to_matchable(), Ref::keyword("SQL_CALC_FOUND_ROWS") .optional() .to_matchable()])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "IntoClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::IntoClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("INTO") .to_matchable(), one_of(vec![Delimited::new(vec![AnyNumberOf::new(vec![Ref::new("SessionVariableNameSegment") .to_matchable(), Ref::new("LocalVariableNameSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DUMPFILE") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("OUTFILE") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("CHARACTER") .to_matchable(), Ref::keyword("SET") .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("FIELDS") .to_matchable(), Ref::keyword("COLUMNS") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("TERMINATED") .to_matchable(), Ref::keyword("BY") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("OPTIONALLY") .optional() .to_matchable(), Ref::keyword("ENCLOSED") .to_matchable(), Ref::keyword("BY") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("ESCAPED") .to_matchable(), Ref::keyword("BY") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("LINES") .to_matchable(), Sequence::new(vec![Ref::keyword("STARTING") .to_matchable(), Ref::keyword("BY") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("TERMINATED") .to_matchable(), Ref::keyword("BY") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .config(|this| {
                        this.parse_mode(ParseMode::GreedyOnceStarted);
                        this.terminators = vec![Ref::new("SelectClauseTerminatorGrammar") .to_matchable()];
                    })
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "UnorderedSelectStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectStatement, |_dialect| {
                { let dialect = super::ansi::raw_dialect(); dialect.grammar("UnorderedSelectStatementSegment").match_grammar(&dialect).unwrap() }.copy(Some(vec![Ref::new("IntoClauseSegment") .optional() .to_matchable()]), None, Some(Ref::new("FromClauseSegment") .optional() .to_matchable()), None, vec![], false).copy(Some(vec![Ref::new("ForClauseSegment") .optional() .to_matchable()]), None, None, None, vec![], false).copy(Some(vec![Ref::new("IndexHintClauseSegment") .optional() .to_matchable()]), None, Some(Ref::new("WhereClauseSegment") .optional() .to_matchable()), None, vec![], false).copy(Some(vec![Ref::new("SelectPartitionClauseSegment") .optional() .to_matchable()]), None, Some(Ref::new("WhereClauseSegment") .optional() .to_matchable()), None, vec![Ref::new("IntoClauseSegment") .to_matchable(), Ref::new("ForClauseSegment") .to_matchable(), Ref::new("IndexHintClauseSegment") .to_matchable(), Ref::new("WithCheckOptionSegment") .to_matchable(), Ref::new("SelectPartitionClauseSegment") .to_matchable(), Ref::new("UpsertClauseListSegment") .to_matchable()], false)
            })
                .to_matchable()
                .into()
        ),
        (
            "SelectClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectClause, |_dialect| {
                { let dialect = super::ansi::raw_dialect(); dialect.grammar("SelectClauseSegment").match_grammar(&dialect).unwrap() }.copy(None, None, None, None, vec![Ref::keyword("INTO") .to_matchable()], false)
            })
                .to_matchable()
                .into()
        ),
        (
            "SelectStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SelectStatement, |_dialect| {
                get_unordered_select_statement_segment_grammar().copy(Some(vec![Ref::new("OrderByClauseSegment") .optional() .to_matchable(), Ref::new("LimitClauseSegment") .optional() .to_matchable(), Ref::new("NamedWindowSegment") .optional() .to_matchable(), Ref::new("IntoClauseSegment") .optional() .to_matchable()]), None, None, None, vec![Ref::new("SetOperatorSegment") .to_matchable(), Ref::new("UpsertClauseListSegment") .to_matchable(), Ref::new("WithCheckOptionSegment") .to_matchable()], true)
            })
                .to_matchable()
                .into()
        ),
        (
            "ForClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::ForClause, |_dialect| {
                one_of(vec![Sequence::new(vec![Sequence::new(vec![Ref::keyword("FOR") .to_matchable(), one_of(vec![Ref::keyword("UPDATE") .to_matchable(), Ref::keyword("SHARE") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("OF") .to_matchable(), Delimited::new(vec![Ref::new("NakedIdentifierSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Ref::keyword("NOWAIT") .to_matchable(), Sequence::new(vec![Ref::keyword("SKIP") .to_matchable(), Ref::keyword("LOCKED") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("LOCK") .to_matchable(), Ref::keyword("IN") .to_matchable(), Ref::keyword("SHARE") .to_matchable(), Ref::keyword("MODE") .to_matchable()]) .to_matchable()])
                    .config(|this| {
                        this.optional();
                    })
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "IndexHintClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::IndexHintClause, |_dialect| {
                Sequence::new(vec![one_of(vec![Ref::keyword("USE") .to_matchable(), Ref::keyword("IGNORE") .to_matchable(), Ref::keyword("FORCE") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("INDEX") .to_matchable(), Ref::keyword("KEY") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FOR") .to_matchable(), one_of(vec![Ref::keyword("JOIN") .to_matchable(), Sequence::new(vec![Ref::keyword("ORDER") .to_matchable(), Ref::keyword("BY") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("GROUP") .to_matchable(), Ref::keyword("BY") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Bracketed::new(vec![Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable(), Ref::new("JoinOnConditionSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CallStoredProcedureSegment".into(),
            NodeMatcher::new(SyntaxKind::CallStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CALL") .to_matchable(), Ref::new("FunctionSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SelectPartitionClauseSegment".into(),
            NodeMatcher::new(SyntaxKind::PartitionClause, |_dialect| {
                Sequence::new(vec![Ref::keyword("PARTITION") .to_matchable(), Bracketed::new(vec![Delimited::new(vec![Ref::new("ObjectReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "WhileStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::WhileStatement, |_dialect| {
                one_of(vec![Sequence::new(vec![Sequence::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), Ref::new("ColonSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("WHILE") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable(), Ref::keyword("DO") .to_matchable(), AnyNumberOf::new(vec![Ref::new("StatementSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("END") .to_matchable(), Ref::keyword("WHILE") .to_matchable(), Ref::new("SingleIdentifierGrammar") .optional() .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "PrepareSegment".into(),
            NodeMatcher::new(SyntaxKind::PrepareSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("PREPARE") .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::keyword("FROM") .to_matchable(), one_of(vec![Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("SessionVariableNameSegment") .to_matchable(), Ref::new("LocalVariableNameSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "GetDiagnosticsSegment".into(),
            NodeMatcher::new(SyntaxKind::GetDiagnosticsSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("GET") .to_matchable(), Sequence::new(vec![Ref::keyword("CURRENT") .to_matchable(), Ref::keyword("STACKED") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("DIAGNOSTICS") .to_matchable(), Delimited::new(vec![Sequence::new(vec![one_of(vec![Ref::new("SessionVariableNameSegment") .to_matchable(), Ref::new("LocalVariableNameSegment") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("NUMBER") .to_matchable(), Ref::keyword("ROW_COUNT") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("CONDITION") .to_matchable(), one_of(vec![Ref::new("SessionVariableNameSegment") .to_matchable(), Ref::new("LocalVariableNameSegment") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable(), Delimited::new(vec![Sequence::new(vec![one_of(vec![Ref::new("SessionVariableNameSegment") .to_matchable(), Ref::new("LocalVariableNameSegment") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::keyword("CLASS_ORIGIN") .to_matchable(), Ref::keyword("SUBCLASS_ORIGIN") .to_matchable(), Ref::keyword("RETURNED_SQLSTATE") .to_matchable(), Ref::keyword("MESSAGE_TEXT") .to_matchable(), Ref::keyword("MYSQL_ERRNO") .to_matchable(), Ref::keyword("CONSTRAINT_CATALOG") .to_matchable(), Ref::keyword("CONSTRAINT_SCHEMA") .to_matchable(), Ref::keyword("CONSTRAINT_NAME") .to_matchable(), Ref::keyword("CATALOG_NAME") .to_matchable(), Ref::keyword("SCHEMA_NAME") .to_matchable(), Ref::keyword("TABLE_NAME") .to_matchable(), Ref::keyword("COLUMN_NAME") .to_matchable(), Ref::keyword("CURSOR_NAME") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "LoopStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::LoopStatement, |_dialect| {
                one_of(vec![Sequence::new(vec![Sequence::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), Ref::new("ColonSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("LOOP") .to_matchable(), Delimited::new(vec![Ref::new("StatementSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("END") .to_matchable(), Ref::keyword("LOOP") .to_matchable(), Ref::new("SingleIdentifierGrammar") .optional() .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CursorOpenCloseSegment".into(),
            NodeMatcher::new(SyntaxKind::CursorOpenCloseSegment, |_dialect| {
                Sequence::new(vec![one_of(vec![Ref::keyword("CLOSE") .to_matchable(), Ref::keyword("OPEN") .to_matchable()]) .to_matchable(), one_of(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), Ref::new("QuotedIdentifierSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "IterateStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::IterateStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("ITERATE") .to_matchable(), Ref::new("SingleIdentifierGrammar") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ExecuteSegment".into(),
            NodeMatcher::new(SyntaxKind::ExecuteSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("EXECUTE") .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("USING") .to_matchable(), Delimited::new(vec![Ref::new("SessionVariableNameSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "RepeatStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::RepeatStatement, |_dialect| {
                one_of(vec![Sequence::new(vec![Sequence::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), Ref::new("ColonSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("REPEAT") .to_matchable(), AnyNumberOf::new(vec![Ref::new("StatementSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("UNTIL") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("END") .to_matchable(), Ref::keyword("REPEAT") .to_matchable(), Ref::new("SingleIdentifierGrammar") .optional() .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DeallocateSegment".into(),
            NodeMatcher::new(SyntaxKind::DeallocateSegment, |_dialect| {
                Sequence::new(vec![Sequence::new(vec![one_of(vec![Ref::keyword("DEALLOCATE") .to_matchable(), Ref::keyword("DROP") .to_matchable()]) .to_matchable(), Ref::keyword("PREPARE") .to_matchable()]) .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ResignalSegment".into(),
            NodeMatcher::new(SyntaxKind::ResignalSegment, |_dialect| {
                Sequence::new(vec![one_of(vec![Ref::keyword("SIGNAL") .to_matchable(), Ref::keyword("RESIGNAL") .to_matchable()]) .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("SQLSTATE") .to_matchable(), Ref::keyword("VALUE") .optional() .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("SET") .to_matchable(), Delimited::new(vec![Sequence::new(vec![one_of(vec![Ref::keyword("CLASS_ORIGIN") .to_matchable(), Ref::keyword("SUBCLASS_ORIGIN") .to_matchable(), Ref::keyword("RETURNED_SQLSTATE") .to_matchable(), Ref::keyword("MESSAGE_TEXT") .to_matchable(), Ref::keyword("MYSQL_ERRNO") .to_matchable(), Ref::keyword("CONSTRAINT_CATALOG") .to_matchable(), Ref::keyword("CONSTRAINT_SCHEMA") .to_matchable(), Ref::keyword("CONSTRAINT_NAME") .to_matchable(), Ref::keyword("CATALOG_NAME") .to_matchable(), Ref::keyword("SCHEMA_NAME") .to_matchable(), Ref::keyword("TABLE_NAME") .to_matchable(), Ref::keyword("COLUMN_NAME") .to_matchable(), Ref::keyword("CURSOR_NAME") .to_matchable()]) .to_matchable(), Ref::new("EqualsSegment") .to_matchable(), one_of(vec![Ref::new("SessionVariableNameSegment") .to_matchable(), Ref::new("LocalVariableNameSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CursorFetchSegment".into(),
            NodeMatcher::new(SyntaxKind::CursorFetchSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("FETCH") .to_matchable(), Sequence::new(vec![Ref::keyword("NEXT") .optional() .to_matchable(), Ref::keyword("FROM") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::keyword("INTO") .to_matchable(), Delimited::new(vec![Ref::new("SessionVariableNameSegment") .to_matchable(), Ref::new("LocalVariableNameSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DropIndexStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropIndexStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), Ref::keyword("INDEX") .to_matchable(), Ref::new("IndexReferenceSegment") .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("ALGORITHM") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::keyword("DEFAULT") .to_matchable(), Ref::keyword("INPLACE") .to_matchable(), Ref::keyword("COPY") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("LOCK") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::keyword("DEFAULT") .to_matchable(), Ref::keyword("NONE") .to_matchable(), Ref::keyword("SHARED") .to_matchable(), Ref::keyword("EXCLUSIVE") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DropProcedureStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropProcedureStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), one_of(vec![Ref::keyword("PROCEDURE") .to_matchable(), Ref::keyword("FUNCTION") .to_matchable()]) .to_matchable(), Ref::new("IfExistsGrammar") .optional() .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DropFunctionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropFunctionStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), Ref::keyword("FUNCTION") .to_matchable(), Ref::new("IfExistsGrammar") .optional() .to_matchable(), Ref::new("FunctionNameSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "RenameTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::RenameTableStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("RENAME") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Delimited::new(vec![Sequence::new(vec![Ref::new("TableReferenceSegment") .to_matchable(), Ref::keyword("TO") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ResetMasterStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ResetMasterStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("RESET") .to_matchable(), Ref::keyword("MASTER") .to_matchable(), Sequence::new(vec![Ref::keyword("TO") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "PurgeBinaryLogsStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::PurgeBinaryLogsStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("PURGE") .to_matchable(), one_of(vec![Ref::keyword("BINARY") .to_matchable(), Ref::keyword("MASTER") .to_matchable()]) .to_matchable(), Ref::keyword("LOGS") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("TO") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("BEFORE") .to_matchable(), one_of(vec![Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "HelpStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::HelpStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("HELP") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CheckTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CheckTableStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CHECK") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Delimited::new(vec![Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![Ref::keyword("FOR") .to_matchable(), Ref::keyword("UPGRADE") .to_matchable()]) .to_matchable(), Ref::keyword("QUICK") .to_matchable(), Ref::keyword("FAST") .to_matchable(), Ref::keyword("MEDIUM") .to_matchable(), Ref::keyword("EXTENDED") .to_matchable(), Ref::keyword("CHANGED") .to_matchable()]) .config(|this| { this.min_times(1); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ChecksumTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ChecksumTableStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CHECKSUM") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Delimited::new(vec![Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("QUICK") .to_matchable(), Ref::keyword("EXTENDED") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AnalyzeTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AnalyzeTableStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("ANALYZE") .to_matchable(), one_of(vec![Ref::keyword("NO_WRITE_TO_BINLOG") .to_matchable(), Ref::keyword("LOCAL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("TABLE") .to_matchable(), one_of(vec![Sequence::new(vec![Delimited::new(vec![Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("TableReferenceSegment") .to_matchable(), Ref::keyword("UPDATE") .to_matchable(), Ref::keyword("HISTOGRAM") .to_matchable(), Ref::keyword("ON") .to_matchable(), Delimited::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), Ref::keyword("BUCKETS") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::new("TableReferenceSegment") .to_matchable(), Ref::keyword("DROP") .to_matchable(), Ref::keyword("HISTOGRAM") .to_matchable(), Ref::keyword("ON") .to_matchable(), Delimited::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "RepairTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::RepairTableStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("REPAIR") .to_matchable(), one_of(vec![Ref::keyword("NO_WRITE_TO_BINLOG") .to_matchable(), Ref::keyword("LOCAL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Delimited::new(vec![Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable(), AnyNumberOf::new(vec![Ref::keyword("QUICK") .to_matchable(), Ref::keyword("EXTENDED") .to_matchable(), Ref::keyword("USE_FRM") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "OptimizeTableStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::OptimizeTableStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("OPTIMIZE") .to_matchable(), one_of(vec![Ref::keyword("NO_WRITE_TO_BINLOG") .to_matchable(), Ref::keyword("LOCAL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Delimited::new(vec![Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "UpdateStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::UpdateStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("UPDATE") .to_matchable(), Ref::keyword("LOW_PRIORITY") .optional() .to_matchable(), Ref::keyword("IGNORE") .optional() .to_matchable(), MetaSegment::indent() .to_matchable(), Delimited::new(vec![Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("FromExpressionSegment") .to_matchable()]) .to_matchable(), MetaSegment::dedent() .to_matchable(), Ref::new("SetClauseListSegment") .to_matchable(), Ref::new("WhereClauseSegment") .optional() .to_matchable(), Ref::new("OrderByClauseSegment") .optional() .to_matchable(), Ref::new("LimitClauseSegment") .optional() .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "FlushStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::FlushStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("FLUSH") .to_matchable(), one_of(vec![Ref::keyword("NO_WRITE_TO_BINLOG") .to_matchable(), Ref::keyword("LOCAL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Delimited::new(vec![Sequence::new(vec![Ref::keyword("BINARY") .to_matchable(), Ref::keyword("LOGS") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ENGINE") .to_matchable(), Ref::keyword("LOGS") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ERROR") .to_matchable(), Ref::keyword("LOGS") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("GENERAL") .to_matchable(), Ref::keyword("LOGS") .to_matchable()]) .to_matchable(), Ref::keyword("HOSTS") .to_matchable(), Ref::keyword("LOGS") .to_matchable(), Ref::keyword("PRIVILEGES") .to_matchable(), Ref::keyword("OPTIMIZER_COSTS") .to_matchable(), Sequence::new(vec![Ref::keyword("RELAY") .to_matchable(), Ref::keyword("LOGS") .to_matchable(), Sequence::new(vec![Ref::keyword("FOR") .to_matchable(), Ref::keyword("CHANNEL") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("SLOW") .to_matchable(), Ref::keyword("LOGS") .to_matchable()]) .to_matchable(), Ref::keyword("STATUS") .to_matchable(), Ref::keyword("USER_RESOURCES") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("TABLES") .to_matchable(), Sequence::new(vec![Delimited::new(vec![Ref::new("TableReferenceSegment") .to_matchable()]) .config(|this| { this.terminators = vec![Ref::keyword("WITH") .to_matchable()]; }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("WITH") .to_matchable(), Ref::keyword("READ") .to_matchable(), Ref::keyword("LOCK") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("TABLES") .to_matchable(), Sequence::new(vec![Delimited::new(vec![Ref::new("TableReferenceSegment") .to_matchable()]) .config(|this| { this.terminators = vec![Ref::keyword("FOR") .to_matchable()]; }) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("FOR") .to_matchable(), Ref::keyword("EXPORT") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "LoadDataSegment".into(),
            NodeMatcher::new(SyntaxKind::LoadDataStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("LOAD") .to_matchable(), Ref::keyword("DATA") .to_matchable(), one_of(vec![Ref::keyword("LOW_PRIORITY") .to_matchable(), Ref::keyword("CONCURRENT") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("LOCAL") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("INFILE") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable(), one_of(vec![Ref::keyword("REPLACE") .to_matchable(), Ref::keyword("IGNORE") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("INTO") .to_matchable(), Ref::keyword("TABLE") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("SelectPartitionClauseSegment") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("CHARACTER") .to_matchable(), Ref::keyword("SET") .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("FIELDS") .to_matchable(), Ref::keyword("COLUMNS") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("TERMINATED") .to_matchable(), Ref::keyword("BY") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::keyword("OPTIONALLY") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("ENCLOSED") .to_matchable(), Ref::keyword("BY") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("ESCAPED") .to_matchable(), Ref::keyword("BY") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("LINES") .to_matchable(), Sequence::new(vec![Ref::keyword("STARTING") .to_matchable(), Ref::keyword("BY") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("TERMINATED") .to_matchable(), Ref::keyword("BY") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("IGNORE") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable(), one_of(vec![Ref::keyword("LINES") .to_matchable(), Ref::keyword("ROWS") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Bracketed::new(vec![Delimited::new(vec![Ref::new("ColumnReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("SET") .to_matchable(), Ref::new("Expression_B_Grammar") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ReplaceSegment".into(),
            NodeMatcher::new(SyntaxKind::ReplaceStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("REPLACE") .to_matchable(), one_of(vec![Ref::keyword("LOW_PRIORITY") .to_matchable(), Ref::keyword("DELAYED") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("INTO") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Ref::new("SelectPartitionClauseSegment") .optional() .to_matchable(), one_of(vec![Sequence::new(vec![Ref::new("BracketedColumnReferenceListGrammar") .optional() .to_matchable(), Ref::new("ValuesClauseSegment") .to_matchable()]) .to_matchable(), Ref::new("SetClauseListSegment") .to_matchable(), Sequence::new(vec![Ref::new("BracketedColumnReferenceListGrammar") .optional() .to_matchable(), one_of(vec![Ref::new("SelectableGrammar") .to_matchable(), Sequence::new(vec![Ref::keyword("TABLE") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateTriggerStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateTriggerStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::new("DefinerSegment") .optional() .to_matchable(), Ref::keyword("TRIGGER") .to_matchable(), Ref::new("IfNotExistsGrammar") .optional() .to_matchable(), Ref::new("TriggerReferenceSegment") .to_matchable(), one_of(vec![Ref::keyword("BEFORE") .to_matchable(), Ref::keyword("AFTER") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("INSERT") .to_matchable(), Ref::keyword("UPDATE") .to_matchable(), Ref::keyword("DELETE") .to_matchable()]) .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::new("TableReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("FOR") .to_matchable(), Ref::keyword("EACH") .to_matchable(), Ref::keyword("ROW") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Ref::keyword("FOLLOWS") .to_matchable(), Ref::keyword("PRECEDES") .to_matchable()]) .to_matchable(), Ref::new("SingleIdentifierGrammar") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Ref::new("StatementSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("BEGIN") .to_matchable(), Ref::new("StatementSegment") .to_matchable(), Ref::keyword("END") .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DropTriggerStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropTriggerStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), Ref::keyword("TRIGGER") .to_matchable(), Ref::new("IfExistsGrammar") .optional() .to_matchable(), Ref::new("TriggerReferenceSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateDatabaseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateDatabaseStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), one_of(vec![Ref::keyword("DATABASE") .to_matchable(), Ref::keyword("SCHEMA") .to_matchable()]) .to_matchable(), Ref::new("IfNotExistsGrammar") .optional() .to_matchable(), Ref::new("DatabaseReferenceSegment") .to_matchable(), AnyNumberOf::new(vec![Ref::new("CreateOptionSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateOptionSegment, |_dialect| {
                Sequence::new(vec![Ref::keyword("DEFAULT") .optional() .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("CHARACTER") .to_matchable(), Ref::keyword("SET") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::new("NakedIdentifierSegment") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("COLLATE") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), Ref::new("CollationReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("ENCRYPTION") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AlterDatabaseStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterDatabaseStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("ALTER") .to_matchable(), one_of(vec![Ref::keyword("DATABASE") .to_matchable(), Ref::keyword("SCHEMA") .to_matchable()]) .to_matchable(), Ref::new("DatabaseReferenceSegment") .optional() .to_matchable(), AnyNumberOf::new(vec![Ref::new("AlterOptionSegment") .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AlterOptionSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterOptionSegment, |_dialect| {
                Sequence::new(vec![one_of(vec![Sequence::new(vec![Ref::keyword("DEFAULT") .optional() .to_matchable(), Ref::keyword("CHARACTER") .to_matchable(), Ref::keyword("SET") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), Ref::new("SingleQuotedIdentifierSegment") .to_matchable(), Ref::new("DoubleQuotedIdentifierSegment") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DEFAULT") .optional() .to_matchable(), Ref::keyword("COLLATE") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), Ref::new("CollationReferenceSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("DEFAULT") .optional() .to_matchable(), Ref::keyword("ENCRYPTION") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("READ") .to_matchable(), Ref::keyword("ONLY") .to_matchable(), Ref::new("EqualsSegment") .optional() .to_matchable(), one_of(vec![Ref::keyword("DEFAULT") .to_matchable(), Ref::new("NumericLiteralSegment") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "ReturnStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::ReturnStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("RETURN") .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SetTransactionStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SetTransactionStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("SET") .to_matchable(), one_of(vec![Ref::keyword("GLOBAL") .to_matchable(), Ref::keyword("SESSION") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::keyword("TRANSACTION") .to_matchable(), Delimited::new(vec![Sequence::new(vec![Ref::keyword("ISOLATION") .to_matchable(), Ref::keyword("LEVEL") .to_matchable(), one_of(vec![Sequence::new(vec![Ref::keyword("READ") .to_matchable(), one_of(vec![Ref::keyword("COMMITTED") .to_matchable(), Ref::keyword("UNCOMMITTED") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("REPEATABLE") .to_matchable(), Ref::keyword("READ") .to_matchable()]) .to_matchable(), Ref::keyword("SERIALIZABLE") .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("READ") .to_matchable(), one_of(vec![Ref::keyword("WRITE") .to_matchable(), Ref::keyword("ONLY") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "SetNamesStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::SetNamesStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("SET") .to_matchable(), Ref::keyword("NAMES") .to_matchable(), one_of(vec![Ref::keyword("DEFAULT") .to_matchable(), Ref::new("QuotedLiteralSegment") .to_matchable(), Ref::new("NakedIdentifierSegment") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("COLLATE") .to_matchable(), Ref::new("CollationReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "CreateEventStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::CreateEventStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("CREATE") .to_matchable(), Ref::new("DefinerSegment") .optional() .to_matchable(), Ref::keyword("EVENT") .to_matchable(), Ref::new("IfNotExistsGrammar") .optional() .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Ref::keyword("ON") .to_matchable(), Ref::keyword("SCHEDULE") .to_matchable(), one_of(vec![Ref::keyword("AT") .to_matchable(), Ref::keyword("EVERY") .to_matchable()]) .to_matchable(), Ref::new("ExpressionSegment") .to_matchable(), one_of(vec![Ref::new("DatetimeUnitSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![one_of(vec![Ref::keyword("STARTS") .to_matchable(), Ref::keyword("ENDS") .to_matchable()]) .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("COMPLETION") .to_matchable(), Ref::keyword("NOT") .optional() .to_matchable(), Ref::keyword("PRESERVE") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Ref::keyword("ENABLE") .to_matchable(), Ref::keyword("DISABLE") .to_matchable(), Sequence::new(vec![Ref::keyword("DISABLE") .to_matchable(), Ref::keyword("ON") .to_matchable(), one_of(vec![Ref::keyword("REPLICA") .to_matchable(), Ref::keyword("SLAVE") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("CommentClauseSegment") .optional() .to_matchable(), Ref::keyword("DO") .to_matchable(), Ref::new("StatementSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "AlterEventStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::AlterEventStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("ALTER") .to_matchable(), Ref::new("DefinerSegment") .optional() .to_matchable(), Ref::keyword("EVENT") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable(), Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("SCHEDULE") .to_matchable(), one_of(vec![Ref::keyword("AT") .to_matchable(), Ref::keyword("EVERY") .to_matchable()]) .to_matchable(), Ref::new("ExpressionSegment") .to_matchable(), one_of(vec![Ref::new("DatetimeUnitSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), AnyNumberOf::new(vec![Sequence::new(vec![one_of(vec![Ref::keyword("STARTS") .to_matchable(), Ref::keyword("ENDS") .to_matchable()]) .to_matchable(), Ref::new("ExpressionSegment") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("ON") .to_matchable(), Ref::keyword("COMPLETION") .to_matchable(), Ref::keyword("NOT") .optional() .to_matchable(), Ref::keyword("PRESERVE") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Sequence::new(vec![Ref::keyword("RENAME") .to_matchable(), Ref::keyword("TO") .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), one_of(vec![Ref::keyword("ENABLE") .to_matchable(), Ref::keyword("DISABLE") .to_matchable(), Sequence::new(vec![Ref::keyword("DISABLE") .to_matchable(), Ref::keyword("ON") .to_matchable(), one_of(vec![Ref::keyword("REPLICA") .to_matchable(), Ref::keyword("SLAVE") .to_matchable()]) .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable(), Ref::new("CommentClauseSegment") .optional() .to_matchable(), Sequence::new(vec![Ref::keyword("DO") .to_matchable(), Ref::new("StatementSegment") .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DropEventStatementSegment".into(),
            NodeMatcher::new(SyntaxKind::DropEventStatement, |_dialect| {
                Sequence::new(vec![Ref::keyword("DROP") .to_matchable(), Ref::keyword("EVENT") .to_matchable(), Ref::new("IfExistsGrammar") .optional() .to_matchable(), Ref::new("ObjectReferenceSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "DatatypeSegment".into(),
            NodeMatcher::new(SyntaxKind::DataType, |_dialect| {
                one_of(vec![Ref::new("TimeWithTZGrammar") .to_matchable(), Sequence::new(vec![Ref::keyword("DOUBLE") .to_matchable(), Ref::keyword("PRECISION") .to_matchable()]) .to_matchable(), Sequence::new(vec![one_of(vec![Sequence::new(vec![one_of(vec![Ref::keyword("CHARACTER") .to_matchable(), Ref::keyword("BINARY") .to_matchable()]) .to_matchable(), one_of(vec![Ref::keyword("VARYING") .to_matchable(), Sequence::new(vec![Ref::keyword("LARGE") .to_matchable(), Ref::keyword("OBJECT") .to_matchable()]) .to_matchable()]) .to_matchable()]) .to_matchable(), Sequence::new(vec![Sequence::new(vec![Ref::new("SingleIdentifierGrammar") .to_matchable(), Ref::new("DotSegment") .to_matchable()]) .config(|this| { this.optional(); this.disallow_gaps(); }) .to_matchable(), Ref::new("DatatypeIdentifierSegment") .to_matchable()]) .config(|this| { this.disallow_gaps(); }) .to_matchable()]) .to_matchable(), Ref::new("BracketedArguments") .optional() .to_matchable(), one_of(vec![Ref::new("CharCharacterSetGrammar") .to_matchable(), Ref::keyword("SIGNED") .to_matchable(), Ref::keyword("UNSIGNED") .to_matchable(), Ref::keyword("ZEROFILL") .to_matchable(), Sequence::new(vec![Ref::keyword("ZEROFILL") .to_matchable(), Ref::keyword("UNSIGNED") .to_matchable()]) .to_matchable(), Sequence::new(vec![Ref::keyword("UNSIGNED") .to_matchable(), Ref::keyword("ZEROFILL") .to_matchable()]) .to_matchable()]) .config(|this| { this.optional(); }) .to_matchable()]) .to_matchable(), Ref::new("ArrayTypeSegment") .to_matchable()])
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
        (
            "NullSafeEqualsSegment".into(),
            NodeMatcher::new(SyntaxKind::ComparisonOperator, |_dialect| {
                Sequence::new(vec![Ref::new("RawLessThanSegment") .to_matchable(), Ref::new("RawEqualsSegment") .to_matchable(), Ref::new("RawGreaterThanSegment") .to_matchable()])
                    .config(|this| {
                        this.disallow_gaps();
                    })
                    .to_matchable()
            })
                .to_matchable()
                .into()
        ),
    ]);
    
    mysql_dialect
}

pub fn get_unordered_select_statement_segment_grammar() -> Matchable {
    {
        let dialect = super::ansi::raw_dialect();
        dialect
            .grammar("UnorderedSelectStatementSegment")
            .match_grammar(&dialect)
            .unwrap()
    }
    .copy(
        Some(vec![
            Ref::new("IntoClauseSegment").optional().to_matchable(),
        ]),
        None,
        Some(Ref::new("FromClauseSegment").optional().to_matchable()),
        None,
        vec![],
        false,
    )
    .copy(
        Some(vec![Ref::new("ForClauseSegment").optional().to_matchable()]),
        None,
        None,
        None,
        vec![],
        false,
    )
    .copy(
        Some(vec![
            Ref::new("IndexHintClauseSegment").optional().to_matchable(),
        ]),
        None,
        Some(Ref::new("WhereClauseSegment").optional().to_matchable()),
        None,
        vec![],
        false,
    )
    .copy(
        Some(vec![
            Ref::new("SelectPartitionClauseSegment")
                .optional()
                .to_matchable(),
        ]),
        None,
        Some(Ref::new("WhereClauseSegment").optional().to_matchable()),
        None,
        vec![
            Ref::new("IntoClauseSegment").to_matchable(),
            Ref::new("ForClauseSegment").to_matchable(),
            Ref::new("IndexHintClauseSegment").to_matchable(),
            Ref::new("WithCheckOptionSegment").to_matchable(),
            Ref::new("SelectPartitionClauseSegment").to_matchable(),
            Ref::new("UpsertClauseListSegment").to_matchable(),
        ],
        false,
    )
}
