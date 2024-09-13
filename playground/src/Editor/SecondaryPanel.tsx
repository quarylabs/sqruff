import MonacoEditor from "@monaco-editor/react";

export enum SecondaryTool {
  "Format" = "Format",
}

export default function SecondaryPanel({ result }: { result: string }) {
  return (
    <div className="flex flex-col h-full">
      <div className="flex-grow">
        <Content result={result} />
      </div>
    </div>
  );
}

function Content({ result }: { result: string }) {
  return (
    <MonacoEditor
      options={{
        readOnly: true,
        minimap: { enabled: false },
        fontSize: 14,
        roundedSelection: false,
        scrollBeyondLastLine: false,
        contextmenu: false,
      }}
      language={"sql"}
      value={result}
    />
  );
}
