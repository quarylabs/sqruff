#![allow(clippy::too_many_lines)]
use std::simd::{Simd, cmp::SimdPartialEq, cmp::SimdPartialOrd};

/// A simple token kind used by the experimental SIMD tokenizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Identifier,
    Number,
    Symbol,
    Whitespace,
    EndOfFile,
}

/// A token returned by the SIMD tokenizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token<'a> {
    pub kind: TokenKind,
    pub text: &'a str,
}

/// Experimental tokenizer using SIMD operations to scan the input string.
///
/// This is a simplified implementation inspired by the tokenizer used in
/// [`db25-sql-parser`](https://github.com/space-rf-org/db25-sql-parser).
/// It demonstrates how wide SIMD registers can be used to accelerate
/// common scanning tasks such as skipping whitespace and consuming
/// identifier characters.
#[derive(Debug, Clone)]
pub struct SimdTokenizer<'a> {
    source: &'a str,
}

impl<'a> SimdTokenizer<'a> {
    /// Create a new tokenizer from an input string.
    pub fn new(source: &'a str) -> Self {
        Self { source }
    }

    /// Tokenize the input string and return all tokens.
    pub fn tokenize(self) -> Vec<Token<'a>> {
        let bytes = self.source.as_bytes();
        let mut pos = 0;
        let mut tokens = Vec::new();

        while pos < bytes.len() {
            let skipped = skip_whitespace(&bytes[pos..]);
            if skipped > 0 {
                let text = &self.source[pos..pos + skipped];
                tokens.push(Token {
                    kind: TokenKind::Whitespace,
                    text,
                });
                pos += skipped;
                continue;
            }

            let ch = bytes[pos];
            if is_ident_start(ch) {
                let len = take_identifier(&bytes[pos..]);
                let text = &self.source[pos..pos + len];
                tokens.push(Token {
                    kind: TokenKind::Identifier,
                    text,
                });
                pos += len;
                continue;
            }

            if ch.is_ascii_digit() {
                let len = take_number(&bytes[pos..]);
                let text = &self.source[pos..pos + len];
                tokens.push(Token {
                    kind: TokenKind::Number,
                    text,
                });
                pos += len;
                continue;
            }

            // Anything else is treated as a single symbol token.
            let text = &self.source[pos..pos + 1];
            tokens.push(Token {
                kind: TokenKind::Symbol,
                text,
            });
            pos += 1;
        }

        tokens.push(Token {
            kind: TokenKind::EndOfFile,
            text: "",
        });
        tokens
    }
}

const CHUNK: usize = 64;

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x80
}

fn skip_whitespace(bytes: &[u8]) -> usize {
    let mut i = 0;
    while i + CHUNK <= bytes.len() {
        let chunk = Simd::<u8, CHUNK>::from_slice(&bytes[i..i + CHUNK]);
        let is_space = chunk.simd_eq(Simd::splat(b' '))
            | chunk.simd_eq(Simd::splat(b'\n'))
            | chunk.simd_eq(Simd::splat(b'\t'))
            | chunk.simd_eq(Simd::splat(b'\r'));
        let mask: u64 = is_space.to_bitmask();
        if mask == u64::MAX {
            i += CHUNK;
        } else {
            let first_non = (!mask).trailing_zeros() as usize;
            return i + first_non;
        }
    }
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\n' | b'\t' | b'\r') {
        i += 1;
    }
    i
}

fn take_identifier(bytes: &[u8]) -> usize {
    let mut i = 0;
    while i + CHUNK <= bytes.len() {
        let chunk = Simd::<u8, CHUNK>::from_slice(&bytes[i..i + CHUNK]);
        let upper = chunk.simd_ge(Simd::splat(b'A')) & chunk.simd_le(Simd::splat(b'Z'));
        let lower = chunk.simd_ge(Simd::splat(b'a')) & chunk.simd_le(Simd::splat(b'z'));
        let digit = chunk.simd_ge(Simd::splat(b'0')) & chunk.simd_le(Simd::splat(b'9'));
        let underscore = chunk.simd_eq(Simd::splat(b'_'));
        let ident = upper | lower | digit | underscore;
        let mask: u64 = ident.to_bitmask();
        if mask == u64::MAX {
            i += CHUNK;
        } else {
            let first_non = (!mask).trailing_zeros() as usize;
            return i + first_non;
        }
    }
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
    }
    i
}

fn take_number(bytes: &[u8]) -> usize {
    let mut i = 0;
    while i + CHUNK <= bytes.len() {
        let chunk = Simd::<u8, CHUNK>::from_slice(&bytes[i..i + CHUNK]);
        let digit = chunk.simd_ge(Simd::splat(b'0')) & chunk.simd_le(Simd::splat(b'9'));
        let mask: u64 = digit.to_bitmask();
        if mask == u64::MAX {
            i += CHUNK;
        } else {
            let first_non = (!mask).trailing_zeros() as usize;
            return i + first_non;
        }
    }
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_tokenization() {
        let sql = "SELECT foo, 42 FROM bar";
        let tokenizer = SimdTokenizer::new(sql);
        let tokens = tokenizer.tokenize();
        let kinds: Vec<TokenKind> = tokens.iter().map(|t| t.kind).collect();
        assert!(kinds.contains(&TokenKind::Identifier));
        assert!(kinds.contains(&TokenKind::Number));
        assert!(matches!(tokens.last().unwrap().kind, TokenKind::EndOfFile));
    }
}
