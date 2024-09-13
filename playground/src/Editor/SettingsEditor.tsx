/**
 * Editor for the settings JSON.
 */

import MonacoEditor, { useMonaco } from "@monaco-editor/react";
import { useCallback, useEffect } from "react";

export default function SettingsEditor({
  visible,
  source,
  onChange,
}: {
  visible: boolean;
  source: string;
  onChange: (source: string) => void;
}) {
  const monaco = useMonaco();

  useEffect(() => {}, [monaco]);

  const handleChange = useCallback(
    (value: string | undefined) => {
      onChange(value ?? "");
    },
    [onChange],
  );
  return (
    <MonacoEditor
      options={{
        readOnly: false,
        minimap: { enabled: false },
        fontSize: 14,
        roundedSelection: false,
        scrollBeyondLastLine: false,
        contextmenu: false,
      }}
      wrapperProps={visible ? {} : { style: { display: "none" } }}
      language={"ini"}
      value={source}
      onChange={handleChange}
    />
  );
}
