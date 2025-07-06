import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { default as Editor, Source } from "./Editor";
import initSqruff from "../pkg";
import Header from "./Header";
import { fromBase64, toBase64 } from "./stateSerializer";
import { debounce } from "lodash-es";

function updateStateUrl(state: {
  sql: string | null;
  settings: string | null;
}) {
  if (!state.sql || !state.settings) {
    return;
  }

  location.hash = "#" + toBase64(JSON.stringify(state));
}

function getStateUrl(): Source | null {
  if (location.hash.length < 2) {
    return null;
  }

  try {
    const state = JSON.parse(fromBase64(location.hash.slice(1)));

    if (!state.sql || !state.settings) {
      return null;
    }

    return {
      sqlSource: state.sql,
      settingsSource: state.settings,
    };
  } catch (error) {
    console.warn("Failed to parse state from URL", error);
    return null;
  }
}

export default function Chrome() {
  const initPromise = useRef<null | Promise<void>>(null);
  const [sqlSource, setSqlSource] = useState<null | string>(null);
  const [settings, setSettings] = useState<null | string>(null);
  const updateStateUrlDebounced = useCallback(
    debounce(updateStateUrl, 100),
    [],
  );

  if (initPromise.current == null) {
    initPromise.current = startPlayground()
      .then(({ sourceCode, settings }) => {
        const stateFromUrl = getStateUrl();

        if (stateFromUrl != null) {
          setSqlSource(stateFromUrl.sqlSource);
          setSettings(stateFromUrl.settingsSource);
        } else {
          setSqlSource(sourceCode);
          setSettings(settings);
        }
      })
      .catch((error) => {
        console.error("Failed to initialize playground.", error);
      });
  }

  const handleSourceChanged = useCallback((source: string) => {
    setSqlSource(source);
  }, []);

  const handleSettingsChanged = useCallback((settings: string) => {
    setSettings(settings);
  }, []);

  const handleNewIssue = useCallback(() => {
    if (settings == null || sqlSource == null) {
      return;
    }

    const bugReport = `
### What Happened

### Expected Behaviour

### How to reproduce

\`\`\`sql
${sqlSource}
\`\`\`

### Configuration
\`\`\`ini
${settings}
\`\`\`
`;
    const github = new URL("https://github.com/quarylabs/sqruff/issues/new");
    github.searchParams.set("body", bugReport);

    const newWindow = window.open(github, "_blank");
    if (newWindow) {
      newWindow.focus();
    }
  }, [sqlSource, settings]);

  const source: Source | null = useMemo(() => {
    if (sqlSource == null || settings == null) {
      return null;
    }

    return { sqlSource: sqlSource, settingsSource: settings };
  }, [settings, sqlSource]);

  useEffect(() => {
    updateStateUrlDebounced({
      sql: source?.sqlSource ?? "",
      settings: source?.settingsSource ?? "",
    });
  }, [source]);

  return (
    <main className="flex flex-col h-full">
      <Header onNewIssue={handleNewIssue} />
      <div className="flex flex-grow">
        {source != null && (
          <Editor
            source={source}
            onSettingsChanged={handleSettingsChanged}
            onSourceChanged={handleSourceChanged}
          />
        )}
      </div>
    </main>
  );
}

async function startPlayground(): Promise<{
  sourceCode: string;
  settings: string;
}> {
  await initSqruff();

  const [settingsSource, sqlSource] = [
    "[sqruff]\ndialect = ansi\nrules = core\n",
    "SELECT name from USERS",
  ];

  return {
    sourceCode: sqlSource,
    settings: settingsSource,
  };
}
