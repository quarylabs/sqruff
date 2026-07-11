import { useDeferredValue, useMemo, useState } from "react";
import { Group, Panel } from "react-resizable-panels";
import { Linter, Result } from "../pkg";
import PrimarySideBar from "./PrimarySideBar";
import { HorizontalResizeHandle } from "./ResizeHandle";
import SecondaryPanel, { SecondaryTool } from "./SecondaryPanel";
import SecondarySideBar from "./SecondarySideBar";
import SettingsEditor from "./SettingsEditor";
import SourceEditor from "./SourceEditor";

type Tab = "Source" | "Settings";

export interface Source {
  sqlSource: string;
  settingsSource: string;
}

type Props = {
  source: Source;

  onSourceChanged(source: string): void;
  onSettingsChanged(settings: string): void;
};

export default function Editor({
  source,
  onSourceChanged,
  onSettingsChanged,
}: Props) {
  const [tab, setTab] = useState<Tab>("Source");
  const [secondaryTool, setSecondaryTool] = useState<SecondaryTool | null>(
    () => {
      const secondaryValue = new URLSearchParams(location.search).get(
        "secondary",
      );
      if (secondaryValue == null) {
        return null;
      } else {
        return parseSecondaryTool(secondaryValue);
      }
    },
  );

  const handleSecondaryToolSelected = (tool: SecondaryTool | null) => {
    if (tool === secondaryTool) {
      tool = null;
    }

    const url = new URL(location.href);

    if (tool == null) {
      url.searchParams.delete("secondary");
    } else {
      url.searchParams.set("secondary", tool);
    }

    history.replaceState(null, "", url);

    setSecondaryTool(tool);
  };

  const deferredSource = useDeferredValue(source);

  const analysis: { checkResult: Result; semanticTokens: Uint32Array } =
    useMemo(() => {
      const { sqlSource, settingsSource } = deferredSource;
      try {
        const linter = new Linter(settingsSource);
        return {
          checkResult: linter.check(sqlSource, secondaryTool ?? "Format"),
          semanticTokens: linter.semanticTokens(sqlSource),
        };
      } catch (error) {
        console.log(error);
        return {
          checkResult: new Result(),
          semanticTokens: new Uint32Array(),
        };
      }
    }, [deferredSource, secondaryTool]);

  return (
    <>
      <Group orientation="horizontal" className="flex-1">
        <PrimarySideBar onSelectTool={(tool) => setTab(tool)} selected={tab} />
        <Panel id="main" className="my-2" minSize={10}>
          <SourceEditor
            visible={tab === "Source"}
            source={source.sqlSource}
            diagnostics={analysis.checkResult.diagnostics}
            semanticTokens={analysis.semanticTokens}
            onChange={onSourceChanged}
          />
          <SettingsEditor
            visible={tab === "Settings"}
            source={source.settingsSource}
            onChange={onSettingsChanged}
          />
        </Panel>
        {secondaryTool != null && (
          <>
            <HorizontalResizeHandle />
            <Panel id="secondary-panel" className={"my-2"} minSize={10}>
              <SecondaryPanel
                tool={secondaryTool}
                result={analysis.checkResult.secondary}
              />
            </Panel>
          </>
        )}
        <SecondarySideBar
          selected={secondaryTool}
          onSelected={handleSecondaryToolSelected}
        />
      </Group>
    </>
  );
}

function parseSecondaryTool(tool: string): SecondaryTool | null {
  if (Object.hasOwn(SecondaryTool, tool)) {
    return tool as SecondaryTool;
  }
  return null;
}
