import MonacoEditor, { Monaco, OnMount } from "@monaco-editor/react";
import { MarkerSeverity } from "monaco-editor";
import { useCallback, useEffect, useRef } from "react";
import { Diagnostic } from "../pkg";

declare global {
  interface Window {
    __sqruffLastSemanticTokens?: number[];
  }
}

const SEMANTIC_TOKEN_TYPES = [
  "keyword",
  "string",
  "number",
  "comment",
  "operator",
  "function",
  "type",
  "variable",
  "parameter",
  "property",
  "macro",
];

export default function SourceEditor({
  visible,
  source,
  diagnostics,
  semanticTokens,
  onChange,
}: {
  visible: boolean;
  source: string;
  diagnostics: Diagnostic[];
  semanticTokens: Uint32Array;
  onChange: (sqlSource: string) => void;
}) {
  const monacoRef = useRef<Monaco | null>(null);
  const semanticTokensRef = useRef<Uint32Array>(semanticTokens);
  const providerRef = useRef<{ dispose: () => void } | null>(null);

  useEffect(() => {
    semanticTokensRef.current = semanticTokens;
    publishSemanticTokensForTest(semanticTokens);
  }, [semanticTokens]);

  useEffect(() => {
    return () => {
      providerRef.current?.dispose();
      providerRef.current = null;
    };
  }, []);

  useEffect(() => {
    const editor = monacoRef.current;

    if (editor == null) {
      return;
    }

    updateMarkers(editor, diagnostics);
  }, [diagnostics]);

  const handleChange = useCallback(
    (value: string | undefined) => {
      onChange(value ?? "");
    },
    [onChange],
  );

  const handleMount: OnMount = useCallback(
    (_editor, instance) => {
      updateMarkers(instance, diagnostics);
      monacoRef.current = instance;

      if (providerRef.current == null) {
        providerRef.current =
          instance.languages.registerDocumentSemanticTokensProvider("sql", {
            getLegend: () => ({
              tokenTypes: SEMANTIC_TOKEN_TYPES,
              tokenModifiers: [],
            }),
            provideDocumentSemanticTokens: () => {
              const data = semanticTokensRef.current;
              publishSemanticTokensForTest(data);
              return { data };
            },
            releaseDocumentSemanticTokens: () => {},
          });
      }
    },

    [diagnostics],
  );

  return (
    <MonacoEditor
      onMount={handleMount}
      options={{
        fixedOverflowWidgets: true,
        readOnly: false,
        minimap: { enabled: false },
        fontSize: 14,
        roundedSelection: false,
        scrollBeyondLastLine: false,
        contextmenu: false,
      }}
      language={"sql"}
      wrapperProps={visible ? {} : { style: { display: "none" } }}
      value={source}
      onChange={handleChange}
    />
  );
}

function publishSemanticTokensForTest(data: Uint32Array) {
  if (new URLSearchParams(location.search).has("__sqruffSemanticTokenTest")) {
    window.__sqruffLastSemanticTokens = Array.from(data);
  }
}

function updateMarkers(monaco: Monaco, diagnostics: Array<Diagnostic>) {
  const editor = monaco.editor;
  const model = editor?.getModels()[0];

  if (!model) {
    return;
  }

  editor.setModelMarkers(
    model,
    "owner",
    diagnostics.map((diagnostic) => ({
      startLineNumber: diagnostic.start_line_number,
      startColumn: diagnostic.start_column,
      endLineNumber: diagnostic.end_line_number,
      endColumn: diagnostic.end_column,
      message: diagnostic.message,
      severity: MarkerSeverity.Error,
      tags: [],
    })),
  );
}
