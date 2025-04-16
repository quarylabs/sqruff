import MonacoEditor from "@monaco-editor/react";

export enum SecondaryTool {
  "Format" = "Format",
  "Cst" = "Cst",
  "Lineage" = "Lineage",
  "Templater" = "Templater",
}

export default function SecondaryPanel({
  tool,
  result,
}: {
  tool: SecondaryTool;
  result: string;
}) {
  return (
    <div className="flex flex-col h-full">
      <div className="flex-grow">
        <MonacoEditor
          options={{
            readOnly: true,
            minimap: { enabled: false },
            fontSize: 14,
            roundedSelection: false,
            scrollBeyondLastLine: false,
            contextmenu: false,
          }}
          language={tool === "Format" ? "sql" : "yaml"}
          value={result}
        />
      </div>
    </div>
  );
}
