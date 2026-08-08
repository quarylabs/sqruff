//go:build tools

// Package tools pins the keep-sorted binary so Bazel can build it from source.
//
// keep-sorted (https://github.com/google/keep-sorted) is Google's language
// agnostic linter for keeping blocks of lines sorted. It is fetched through
// gazelle's go_deps extension, which needs a Go module to resolve the version
// from - this file is that module's only source.
package tools

import _ "github.com/google/keep-sorted"
