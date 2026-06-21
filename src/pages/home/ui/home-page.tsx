import type { UnlistenFn } from "@tauri-apps/api/event";
import type { ReactNode } from "react";
import type {
  CommandError,
  CreateRunpodWorkspaceRequest,
  NativeStartupStatusResponse,
  WorkspaceIdRequest,
} from "@/generated/commands";

import Prism from "prismjs";
import { useEffect, useMemo, useRef, useState } from "react";
import { commands, events } from "@/generated/commands";
import { Button } from "@/shared/components/ui/button";
import "prismjs/components/prism-json";

type CommandStatus = "idle" | "running" | "ok" | "error";

type CommandProbe
  = | {
    id: string;
    label: string;
    inputType: "none";
    run: () => Promise<unknown>;
  }
  | {
    id: string;
    label: string;
    inputType: "json";
    initialInput: string;
    run: (request: unknown) => Promise<unknown>;
  };

interface CommandResult {
  id: string;
  label: string;
  status: CommandStatus;
  output: unknown;
}

interface EventLogEntry {
  id: string;
  name: string;
  payload: unknown;
  receivedAt: string;
}

const workspaceIdRequest = stringifyJson({
  workspaceId: "",
});

const commandProbes: CommandProbe[] = [
  {
    id: "get-workflow-catalog",
    label: "getWorkflowCatalog",
    inputType: "none",
    run: commands.getWorkflowCatalog,
  },
  {
    id: "get-runpod-placement-options",
    label: "getRunpodPlacementOptions",
    inputType: "none",
    run: commands.getRunpodPlacementOptions,
  },
  {
    id: "get-workspace-catalog",
    label: "getWorkspaceCatalog",
    inputType: "none",
    run: commands.getWorkspaceCatalog,
  },
  {
    id: "setup-runpod-api-key",
    label: "setupRunpodApiKey",
    inputType: "json",
    initialInput: stringifyJson({
      apiKey: "",
    }),
    run: async (request) => {
      const apiKey = typeof request === "string"
        ? request
        : (request as { apiKey?: string }).apiKey ?? "";
      return commands.setupRunpodApiKey({ apiKey });
    },
  },
  {
    id: "get-runpod-api-key-identity",
    label: "getRunpodApiKeyIdentity",
    inputType: "none",
    run: commands.getRunpodApiKeyIdentity,
  },
  {
    id: "delete-runpod-api-key",
    label: "deleteRunpodApiKey",
    inputType: "none",
    run: commands.deleteRunpodApiKey,
  },
  {
    id: "setup-hugging-face-api-key",
    label: "setupHuggingFaceApiKey",
    inputType: "json",
    initialInput: stringifyJson({
      apiKey: "",
    }),
    run: async (request) => {
      const apiKey = typeof request === "string"
        ? request
        : (request as { apiKey?: string }).apiKey ?? "";
      return commands.setupHuggingFaceApiKey({ apiKey });
    },
  },
  {
    id: "get-hugging-face-api-key-identity",
    label: "getHuggingFaceApiKeyIdentity",
    inputType: "none",
    run: commands.getHuggingFaceApiKeyIdentity,
  },
  {
    id: "delete-hugging-face-api-key",
    label: "deleteHuggingFaceApiKey",
    inputType: "none",
    run: commands.deleteHuggingFaceApiKey,
  },
  {
    id: "create-runpod-workspace",
    label: "createRunpodWorkspace",
    inputType: "json",
    initialInput: stringifyJson({
      workflowPresetId: "",
      placement: {
        datacenterId: "",
        gpuId: "",
        volumeSizeGb: 0,
      },
    }),
    run: async request =>
      commands.createRunpodWorkspace(request as CreateRunpodWorkspaceRequest),
  },
  {
    id: "provision-workspace",
    label: "provisionWorkspace",
    inputType: "json",
    initialInput: workspaceIdRequest,
    run: async request =>
      commands.provisionWorkspace(request as WorkspaceIdRequest),
  },
  {
    id: "cleanup-workspace",
    label: "cleanupWorkspace",
    inputType: "json",
    initialInput: workspaceIdRequest,
    run: async request =>
      commands.cleanupWorkspace(request as WorkspaceIdRequest),
  },
  {
    id: "delete-workspace",
    label: "deleteWorkspace",
    inputType: "json",
    initialInput: workspaceIdRequest,
    run: async request =>
      commands.deleteWorkspace(request as WorkspaceIdRequest),
  },
  {
    id: "get-running-lifecycle-operations",
    label: "getRunningLifecycleOperations",
    inputType: "none",
    run: commands.getRunningLifecycleOperations,
  },
  {
    id: "get-latest-lifecycle-operation",
    label: "getLatestLifecycleOperation",
    inputType: "json",
    initialInput: workspaceIdRequest,
    run: async request =>
      commands.getLatestLifecycleOperation(request as WorkspaceIdRequest),
  },
];

export function HomePage() {
  const [nativeStartupStatus, setNativeStartupStatus] = useState<
    NativeStartupStatusResponse | "loading"
  >("loading");
  const [commandInputs, setCommandInputs] = useState(() =>
    Object.fromEntries(
      commandProbes.map(probe => [
        probe.id,
        probe.inputType === "json" ? probe.initialInput : "",
      ]),
    ),
  );
  const [commandResults, setCommandResults] = useState<CommandResult[]>(
    commandProbes.map(probe => ({
      id: probe.id,
      label: probe.label,
      status: "idle",
      output: null,
    })),
  );
  const [eventLog, setEventLog] = useState<EventLogEntry[]>([]);
  const nextEventLogIdRef = useRef(0);

  const [expandedCommands, setExpandedCommands] = useState<
    Record<string, boolean>
  >(() => Object.fromEntries(commandProbes.map(probe => [probe.id, false])));
  const [latestCommandId, setLatestCommandId] = useState<string | null>(null);

  const latestCommandResult = useMemo(() => {
    if (latestCommandId !== null) {
      const result = findCommandResult(commandResults, latestCommandId);

      if (result.status !== "idle") {
        return result;
      }
    }

    for (let index = commandResults.length - 1; index >= 0; index -= 1) {
      const result = commandResults[index];

      if (result.status !== "idle") {
        return result;
      }
    }

    return commandResults[0];
  }, [commandResults, latestCommandId]);

  useEffect(() => {
    let isStopped = false;

    async function loadNativeStartupStatus() {
      const result = await commands.getNativeStartupStatus();

      if (isStopped) {
        return;
      }

      if (result.status === "ok") {
        setNativeStartupStatus(result.data);
      }
      else {
        setNativeStartupStatus({
          status: "failed",
          error: result.error,
        });
      }
    }

    void loadNativeStartupStatus();

    return () => {
      isStopped = true;
    };
  }, []);

  useEffect(() => {
    if (nativeStartupStatus === "loading" || nativeStartupStatus.status !== "ready") {
      return;
    }

    let isStopped = false;
    const unlistenFns: UnlistenFn[] = [];

    async function listenToNativeEvents() {
      try {
        const lifecycleUnlisten = await events.lifecycleOperationChangedEvent.listen(
          (event) => {
            appendEventLog("lifecycle-operation-changed-event", event.payload);
          },
        );
        const workspaceChangedUnlisten = await events.workspaceChangedEvent.listen(
          (event) => {
            appendEventLog("workspace-changed-event", event.payload);
          },
        );
        const workspaceDeletedUnlisten = await events.workspaceDeletedEvent.listen(
          (event) => {
            appendEventLog("workspace-deleted-event", event.payload);
          },
        );

        if (isStopped) {
          lifecycleUnlisten();
          workspaceChangedUnlisten();
          workspaceDeletedUnlisten();
          return;
        }

        unlistenFns.push(
          lifecycleUnlisten,
          workspaceChangedUnlisten,
          workspaceDeletedUnlisten,
        );
      }
      catch (error) {
        console.error(error);
      }
    }

    void listenToNativeEvents();

    return () => {
      isStopped = true;
      for (const unlisten of unlistenFns) {
        unlisten();
      }
    };
  }, [nativeStartupStatus]);

  if (nativeStartupStatus === "loading") {
    return <StartupLoadingPage />;
  }

  if (nativeStartupStatus.status === "failed") {
    return <StartupErrorPage error={nativeStartupStatus.error} />;
  }

  async function runCommand(probe: CommandProbe) {
    setLatestCommandId(probe.id);
    setCommandResults(results =>
      updateCommandResult(results, {
        id: probe.id,
        label: probe.label,
        status: "running",
        output: null,
      }),
    );

    try {
      const output = await executeCommandProbe(probe, commandInputs[probe.id]);

      setCommandResults(results =>
        updateCommandResult(results, {
          id: probe.id,
          label: probe.label,
          status: "ok",
          output,
        }),
      );
    }
    catch (error) {
      setCommandResults(results =>
        updateCommandResult(results, {
          id: probe.id,
          label: probe.label,
          status: "error",
          output: formatError(error),
        }),
      );
    }
  }

  function appendEventLog(name: string, payload: unknown) {
    const id = `${name}-${nextEventLogIdRef.current}`;
    nextEventLogIdRef.current += 1;

    setEventLog(entries =>
      [
        {
          id,
          name,
          payload,
          receivedAt: new Date().toLocaleTimeString(),
        },
        ...entries,
      ].slice(0, 20),
    );
  }

  function toggleCommandExpanded(id: string) {
    setExpandedCommands(states => ({ ...states, [id]: !states[id] }));
  }

  return (
    <main className="min-h-svh bg-background text-foreground">
      <section className="mx-auto flex w-full max-w-6xl flex-col gap-6 py-10">
        <header className="flex flex-col gap-2">
          <p className="text-sm font-medium text-muted-foreground">
            src-tauri diagnostics
          </p>
          <h1 className="text-2xl font-semibold tracking-normal">
            Native command and event tests
          </h1>
        </header>

        <section className="grid items-start gap-4 lg:grid-cols-[minmax(360px,0.95fr)_minmax(0,1.05fr)]">
          <Panel
            title="Commands"
            description="All generated native command bindings."
          >
            <div className="grid gap-2">
              {commandProbes.map((probe) => {
                const result = findCommandResult(commandResults, probe.id);
                const running = result.status === "running";
                const inputValue = commandInputs[probe.id] ?? "";
                const expanded = expandedCommands[probe.id] ?? false;

                return (
                  <article
                    key={probe.id}
                    className="overflow-hidden rounded-lg border"
                  >
                    <div className="flex flex-wrap items-center justify-between gap-2 border-b bg-muted/20 p-3">
                      <div className="min-w-0">
                        <h3 className="flex items-center gap-2 text-sm font-medium">
                          <span className="truncate font-mono text-xs">
                            {probe.label}
                          </span>
                        </h3>
                      </div>
                      <div className="flex gap-2">
                        <Button
                          size="sm"
                          type="button"
                          variant="ghost"
                          onClick={() => toggleCommandExpanded(probe.id)}
                        >
                          {expanded ? "Hide" : "Expand"}
                        </Button>
                        <Button
                          disabled={running}
                          size="sm"
                          type="button"
                          variant="outline"
                          onClick={() => void runCommand(probe)}
                        >
                          {running ? "Running" : "Run"}
                        </Button>
                      </div>
                    </div>

                    {expanded
                      ? (
                          <div className="grid gap-2 bg-card p-3">
                            {probe.inputType === "json"
                              ? (
                                  <JsonInput
                                    value={inputValue}
                                    onChange={(nextValue) => {
                                      setCommandInputs(inputs => ({
                                        ...inputs,
                                        [probe.id]: nextValue,
                                      }));
                                    }}
                                  />
                                )
                              : null}

                            {probe.inputType === "none"
                              ? (
                                  <p className="text-xs text-muted-foreground">
                                    No input needed.
                                  </p>
                                )
                              : (
                                  <p className="text-xs text-muted-foreground">
                                    Edit payload and run.
                                  </p>
                                )}
                          </div>
                        )
                      : null}
                  </article>
                );
              })}
            </div>
          </Panel>

          <section className="grid min-h-0 flex-1 gap-4">
            <Panel
              title="Latest command response"
              description={latestCommandResult.label}
            >
              <JsonBlock
                className="min-h-[360px] max-h-[640px]"
                resizable
                value={latestCommandResult.output}
              />
            </Panel>

            <Panel title="Event log" description="Most recent 20 events.">
              {eventLog.length > 0
                ? (
                    <div className="grid gap-3">
                      {eventLog.map(entry => (
                        <article
                          key={entry.id}
                          className="rounded-lg border bg-muted/20 p-3"
                        >
                          <div className="mb-2 flex items-center justify-between gap-3">
                            <p className="text-sm font-medium">{entry.name}</p>
                            <time className="text-xs text-muted-foreground">
                              {entry.receivedAt}
                            </time>
                          </div>
                          <JsonBlock
                            defaultCollapsed
                            value={entry.payload}
                          />
                        </article>
                      ))}
                    </div>
                  )
                : (
                    <p className="rounded-lg border bg-muted/30 p-3 text-sm text-muted-foreground">
                      No events received yet.
                    </p>
                  )}
            </Panel>
          </section>
        </section>
      </section>
    </main>
  );
}

function StartupLoadingPage() {
  return (
    <main className="min-h-svh bg-background text-foreground">
      <section className="mx-auto flex w-full max-w-3xl flex-col gap-6 py-10">
        <Panel title="Native startup" description="Checking native initialization.">
          <p className="text-sm text-muted-foreground">Loading startup status.</p>
        </Panel>
      </section>
    </main>
  );
}

function StartupErrorPage({
  error,
}: {
  error: CommandError<string>;
}) {
  const storagePath = extractStoragePath(error.message);

  return (
    <main className="min-h-svh bg-background text-foreground">
      <section className="mx-auto flex w-full max-w-3xl flex-col gap-6 py-10">
        <header className="flex flex-col gap-2">
          <p className="text-sm font-medium text-muted-foreground">
            src-tauri diagnostics
          </p>
          <h1 className="text-2xl font-semibold tracking-normal">
            Native startup failed
          </h1>
        </header>

        <Panel
          title="Startup error"
          description="Native state was not initialized."
        >
          <div className="grid gap-4">
            <div className="grid gap-1">
              <p className="text-xs font-medium text-muted-foreground">Code</p>
              <p className="font-mono text-sm">{error.code}</p>
            </div>
            <div className="grid gap-1">
              <p className="text-xs font-medium text-muted-foreground">Message</p>
              <p className="break-words font-mono text-sm">{error.message}</p>
            </div>
            {storagePath !== null
              ? (
                  <div className="grid gap-1">
                    <p className="text-xs font-medium text-muted-foreground">
                      Local storage
                    </p>
                    <p className="break-words font-mono text-sm">{storagePath}</p>
                  </div>
                )
              : null}
            <p className="rounded-lg border bg-muted/30 p-3 text-sm text-muted-foreground">
              Move or delete the incompatible local storage file, then restart the app.
            </p>
          </div>
        </Panel>
      </section>
    </main>
  );
}

interface PanelProps {
  title: string;
  description: string;
  children: ReactNode;
  className?: string;
}

function Panel({ title, description, children, className = "" }: PanelProps) {
  return (
    <section
      className={`min-h-0 min-w-0 rounded-lg border bg-card p-5 text-card-foreground ${className}`}
    >
      <div className="mb-4 flex flex-col gap-1">
        <h2 className="text-base font-semibold tracking-normal">{title}</h2>
        <p className="text-sm text-muted-foreground">{description}</p>
      </div>
      {children}
    </section>
  );
}

function JsonInput({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="relative">
      <textarea
        className="min-h-28 w-full rounded-lg border bg-background p-3 font-mono text-xs leading-5 whitespace-pre-wrap break-words token string rounded-lg caret-foreground outline-none focus-visible:ring-3 focus-visible:ring-ring/30"
        spellCheck={false}
        value={value}
        onChange={event => onChange(event.target.value)}
      />
    </div>
  );
}

function JsonBlock({
  value,
  className = "",
  resizable = false,
  defaultCollapsed = false,
}: {
  value: unknown;
  className?: string;
  resizable?: boolean;
  defaultCollapsed?: boolean;
}) {
  const resizeClass = resizable ? "resize-y" : "";
  const rootClassName = [
    "json-foldable min-h-0 min-w-0 max-h-[640px] overflow-auto rounded-lg border bg-background p-3 font-mono text-xs leading-5 whitespace-pre-wrap break-words language-json",
    resizeClass,
    className,
  ].join(" ");

  return (
    <div className={rootClassName}>
      {renderFoldableJson(value, defaultCollapsed)}
    </div>
  );
}

interface JsonPrismLanguage extends Record<string, unknown> {}

interface JsonPrismRuntime {
  languages: {
    json: JsonPrismLanguage;
  };
}

interface JsonPrismRuntimeWithHighlight extends JsonPrismRuntime {
  highlight: (code: string, grammar: JsonPrismLanguage, language: string) => string;
}

const prism = Prism as unknown as JsonPrismRuntimeWithHighlight;

function jsonHighlightedHtml(code: string) {
  return prism.highlight(code, prism.languages.json, "json");
}

function JsonFoldablePrimitive({ raw }: { raw: string }) {
  const highlighted = useMemo(() => jsonHighlightedHtml(raw || ""), [raw]);
  return (
    <code
      // className="language-json"
      dangerouslySetInnerHTML={{ __html: highlighted }}
    />
  );
}

function renderFoldableJson(value: unknown, defaultCollapsed: boolean) {
  return (
    <JsonFoldNode
      value={value}
      depth={0}
      isLast={true}
      defaultCollapsed={defaultCollapsed}
    />
  );
}

function JsonFoldNode({
  value,
  depth,
  isLast = true,
  defaultCollapsed = false,
}: {
  value: unknown;
  depth: number;
  isLast?: boolean;
  defaultCollapsed?: boolean;
}) {
  if (value === null) {
    return (
      <>
        <JsonFoldablePrimitive raw="null" />
        {!isLast ? <span className="token punctuation">,</span> : null}
      </>
    );
  }

  const type = typeof value;

  if (type === "string") {
    return (
      <>
        <JsonFoldablePrimitive raw={JSON.stringify(value)} />
        {!isLast ? <span className="token punctuation">,</span> : null}
      </>
    );
  }

  if (type === "number" || type === "boolean" || type === "undefined") {
    return (
      <>
        <JsonFoldablePrimitive raw={String(value)} />
        {!isLast ? <span className="token punctuation">,</span> : null}
      </>
    );
  }

  if (type === "bigint" || type === "symbol" || type === "function") {
    return (
      <>
        <JsonFoldablePrimitive raw={String(value)} />
        {!isLast ? <span className="token punctuation">,</span> : null}
      </>
    );
  }

  if (Array.isArray(value)) {
    return (
      <JsonFoldableArray
        values={value}
        depth={depth}
        isLast={isLast}
        defaultCollapsed={defaultCollapsed}
      />
    );
  }

  return (
    <JsonFoldableObject
      value={value as Record<string, unknown>}
      depth={depth}
      isLast={isLast}
      defaultCollapsed={defaultCollapsed}
    />
  );
}

function JsonFoldableArray({
  values,
  depth,
  isLast,
  defaultCollapsed = false,
}: {
  values: unknown[];
  depth: number;
  isLast: boolean;
  defaultCollapsed?: boolean;
}) {
  const [isOpen, setIsOpen] = useState(!defaultCollapsed && depth < 2);
  if (values.length === 0) {
    return (
      <>
        <span className="token punctuation">[]</span>
        {!isLast ? <span className="token punctuation">,</span> : null}
      </>
    );
  }

  const itemKeys = new Map<string, number>();

  return (
    <>
      <details
        className="json-details"
        open={isOpen}
        onToggle={(event) => {
          const details = event.currentTarget;

          setIsOpen(details.open);
        }}
      >
        <summary className="json-summary">
          <span className="token punctuation">[</span>
          {!isOpen
            ? (
                <>
                  <span className="json-collapsed-bracket"> ... ]</span>
                  {!isLast ? <span className="token punctuation">,</span> : null}
                </>
              )
            : null}
        </summary>
        {isOpen
          ? (
              <>
                <div className="json-children">
                  {values.map((item, index) => (
                    <div
                      key={getArrayItemKey(item, itemKeys)}
                      className="json-item"
                    >
                      <JsonFoldNode
                        value={item}
                        depth={depth + 1}
                        defaultCollapsed={defaultCollapsed}
                        isLast={index === values.length - 1}
                      />
                    </div>
                  ))}
                </div>
                <span className="token punctuation">
                  ]
                  {!isLast ? "," : null}
                </span>
              </>
            )
          : null}
      </details>
    </>
  );
}

function JsonFoldableObject({
  value,
  depth,
  isLast,
  defaultCollapsed = false,
}: {
  value: Record<string, unknown>;
  depth: number;
  isLast: boolean;
  defaultCollapsed?: boolean;
}) {
  const [isOpen, setIsOpen] = useState(!defaultCollapsed && depth < 2);
  const entries = Object.entries(value);

  if (entries.length === 0) {
    return (
      <>
        <span className="token punctuation">{`{}`}</span>
        {!isLast ? <span className="token punctuation">,</span> : null}
      </>
    );
  }

  return (
    <>
      <details
        className="json-details"
        open={isOpen}
        onToggle={(event) => {
          const details = event.currentTarget;

          setIsOpen(details.open);
        }}
      >
        <summary className="json-summary">
          <span className="token punctuation">{`{`}</span>
          {!isOpen
            ? (
                <>
                  <span className="json-collapsed-bracket">{` ... }`}</span>
                  {!isLast ? <span className="token punctuation">,</span> : null}
                </>
              )
            : null}
        </summary>
        {isOpen
          ? (
              <>
                <div className="json-children">
                  {entries.map(([name, item], index) => (
                    <div key={name} className="json-item">
                      <span
                        className="token string"
                      >
                        "
                        {name}
                        "
                      </span>
                      <span className="token punctuation">
                        :
                        {" "}
                      </span>
                      <JsonFoldNode
                        value={item}
                        depth={depth + 1}
                        defaultCollapsed={defaultCollapsed}
                        isLast={index === entries.length - 1}
                      />
                    </div>
                  ))}
                </div>
                <span className="token punctuation">
                  {`}`}
                  {!isLast ? "," : null}
                </span>
              </>
            )
          : null}
      </details>
    </>
  );
}

function describeValueForJsonKey(value: unknown) {
  const jsonText = stringifyJson(value);

  return jsonText !== "" ? jsonText : String(value);
}

function getArrayItemKey(value: unknown, itemKeys: Map<string, number>) {
  const key = describeValueForJsonKey(value);
  const serial = itemKeys.get(key) ?? 0;

  itemKeys.set(key, serial + 1);

  return `${serial}-${key}`;
}

function findCommandResult(results: CommandResult[], id: string) {
  const result = results.find(item => item.id === id);

  if (result !== undefined) {
    return result;
  }

  return {
    id,
    label: id,
    status: "idle" as const,
    output: null,
  };
}

function updateCommandResult(
  results: CommandResult[],
  nextResult: CommandResult,
) {
  return results.map(result =>
    result.id === nextResult.id ? nextResult : result,
  );
}

async function executeCommandProbe(
  probe: CommandProbe,
  input: string | undefined,
) {
  if (probe.inputType === "none") {
    return probe.run();
  }

  return probe.run(parseJsonInput(input ?? ""));
}

function parseJsonInput(input: string) {
  return JSON.parse(input) as unknown;
}

function stringifyJson(value: unknown) {
  const json = JSON.stringify(value, null, 2);

  if (json !== undefined) {
    return json;
  }

  return "";
}

function formatError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  return stringifyJson(error);
}

function extractStoragePath(message: string) {
  const match = message.match(/ at (.*native\.sqlite): /);

  return match?.[1] ?? null;
}
