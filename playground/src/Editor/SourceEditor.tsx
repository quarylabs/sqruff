import MonacoEditor, { Monaco, OnMount } from "@monaco-editor/react";
import { MarkerSeverity } from "monaco-editor";
import { useCallback, useEffect, useRef } from "react";
import { Diagnostic } from "../pkg";

export default function SourceEditor({
  visible,
  source,
  diagnostics,
  onChange,
}: {
  visible: boolean;
  source: string;
  diagnostics: Diagnostic[];
  onChange: (sqlSource: string) => void;
}) {
  const monacoRef = useRef<Monaco | null>(null);

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
