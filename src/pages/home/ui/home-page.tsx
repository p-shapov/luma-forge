import type {
  EndpointProfile,
  GpuCloudProviderSetup,
  NativeCommandError,
  ProviderInventory,
  ProvisioningProfile,
  WorkflowCatalog,
  WorkspaceCatalog,
} from "@/generated/commands";
import {
  Add01Icon,
  ArrowReloadHorizontalIcon,
  CloudServerIcon,
  DatabaseSyncIcon,
  Delete02Icon,
  Key01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Badge } from "@shared/components/ui/badge";

import { Button } from "@shared/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@shared/components/ui/card";
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@shared/components/ui/field";
import { Input } from "@shared/components/ui/input";
import {
  NativeSelect,
  NativeSelectOption,
} from "@shared/components/ui/native-select";
import { Separator } from "@shared/components/ui/separator";
import { Slider } from "@shared/components/ui/slider";
import { useMemo, useState } from "react";
import { commands } from "@/generated/commands";
import {
  isNativeCommandError,
  presentNativeCommandError,
} from "@/shared/lib/native-command-error-presenter";

const GIB = 1024 ** 3;
const MIN_STORAGE_SIZE_GB = 1;
const DEFAULT_MAX_STORAGE_SIZE_GB = 100;

type CommandResult<T>
  = | { status: "ok"; data: T }
    | { status: "error"; error: NativeCommandError };

interface LogEntry {
  id: number;
  label: string;
  status: "ok" | "error";
  payload: unknown;
}

function formatJson(value: unknown) {
  return JSON.stringify(value, null, 2);
}

function errorPayload(error: unknown) {
  if (error instanceof Error) {
    return { message: error.message };
  }

  return { message: "Command failed", error };
}

export function HomePage() {
  const [providerApiKey, setProviderApiKey] = useState("");
  const [providerSetup, setProviderSetup] = useState<GpuCloudProviderSetup | null>(null);
  const [workflowCatalog, setWorkflowCatalog] = useState<WorkflowCatalog | null>(null);
  const [provisioningProfiles, setProvisioningProfiles] = useState<ProvisioningProfile[]>([]);
  const [endpointProfiles, setEndpointProfiles] = useState<EndpointProfile[]>([]);
  const [providerInventory, setProviderInventory] = useState<ProviderInventory | null>(null);
  const [workspaceCatalog, setWorkspaceCatalog] = useState<WorkspaceCatalog | null>(null);
  const [workspaceName, setWorkspaceName] = useState("Default workspace");
  const [storageSizeGb, setStorageSizeGb] = useState(20);
  const [workflowPresetId, setWorkflowPresetId] = useState("");
  const [provisioningProfileId, setProvisioningProfileId] = useState("");
  const [endpointProfileId, setEndpointProfileId] = useState("");
  const [datacenterId, setDatacenterId] = useState("");
  const [gpuId, setGpuId] = useState("");
  const [pendingCommand, setPendingCommand] = useState<string | null>(null);
  const [logEntries, setLogEntries] = useState<LogEntry[]>([]);

  const workflowPresets = workflowCatalog?.workflow_presets ?? [];
  const datacenters = providerInventory?.datacenters ?? [];
  const selectedWorkflowPreset = workflowPresets.find(({ id }) => id === workflowPresetId)
    ?? workflowPresets[0];
  const selectedProvisioningProfile = provisioningProfiles.find(({ id }) => id === provisioningProfileId)
    ?? provisioningProfiles[0];
  const selectedEndpointProfile = endpointProfiles.find(({ id }) => id === endpointProfileId)
    ?? endpointProfiles[0];
  const selectedDatacenter = datacenters.find(({ id }) => id === datacenterId)
    ?? datacenters[0];
  const selectedGpu = selectedDatacenter?.gpu_options.find(({ id }) => id === gpuId)
    ?? selectedDatacenter?.gpu_options[0];
  const maxStorageSizeGb = providerInventory?.max_persistent_storage_volume_size_bytes !== null
    && providerInventory?.max_persistent_storage_volume_size_bytes !== undefined
    ? Math.max(
        MIN_STORAGE_SIZE_GB,
        Math.floor(providerInventory.max_persistent_storage_volume_size_bytes / GIB),
      )
    : DEFAULT_MAX_STORAGE_SIZE_GB;
  const selectedStorageSizeGb = Math.min(storageSizeGb, maxStorageSizeGb);

  const canCreateWorkspace = Boolean(
    workspaceName.trim().length > 0
    && selectedWorkflowPreset !== undefined
    && selectedProvisioningProfile !== undefined
    && selectedEndpointProfile !== undefined
    && selectedDatacenter !== undefined
    && selectedGpu !== undefined
    && selectedStorageSizeGb >= MIN_STORAGE_SIZE_GB,
  );

  const latestEntry = logEntries[0];
  const latestPayload = useMemo(() => latestEntry?.payload ?? {
    message: "Run a command to see the native response.",
  }, [latestEntry]);
  const latestErrorPresentation = latestEntry?.status === "error"
    && isNativeCommandError(latestEntry.payload)
    ? presentNativeCommandError(latestEntry.payload)
    : null;

  async function runCommand<T>(
    label: string,
    action: () => Promise<CommandResult<T>>,
    onSuccess?: (data: T) => void,
  ) {
    setPendingCommand(label);

    try {
      const result = await action();

      if (result.status === "ok") {
        onSuccess?.(result.data);
        setLogEntries(entries => [
          { id: Date.now(), label, status: "ok", payload: result.data },
          ...entries,
        ]);
        return;
      }

      setLogEntries(entries => [
        { id: Date.now(), label, status: "error", payload: result.error },
        ...entries,
      ]);
    }
    catch (error) {
      setLogEntries(entries => [
        { id: Date.now(), label, status: "error", payload: errorPayload(error) },
        ...entries,
      ]);
    }
    finally {
      setPendingCommand(null);
    }
  }

  function refreshProviderSetup() {
    void runCommand(
      "getGpuCloudProviderSetup",
      async () => commands.getGpuCloudProviderSetup({ gpu_cloud_provider_id: "runpod" }),
      ({ gpu_cloud_provider_setup }) => setProviderSetup(gpu_cloud_provider_setup),
    );
  }

  function setupProvider() {
    void runCommand(
      "setupGpuCloudProvider",
      async () => commands.setupGpuCloudProvider({
        gpu_cloud_provider_id: "runpod",
        provider_api_key: providerApiKey,
      }),
      ({ gpu_cloud_provider_setup }) => {
        setProviderSetup(gpu_cloud_provider_setup);
        setProviderApiKey("");
      },
    );
  }

  function deleteProviderSetup() {
    void runCommand(
      "deleteGpuCloudProviderSetup",
      async () => commands.deleteGpuCloudProviderSetup({ gpu_cloud_provider_id: "runpod" }),
      ({ gpu_cloud_provider_setup }) => setProviderSetup(gpu_cloud_provider_setup),
    );
  }

  function fetchWorkflowCatalog() {
    void runCommand(
      "getWorkflowCatalog",
      async () => commands.getWorkflowCatalog(),
      ({ workflow_catalog }) => setWorkflowCatalog(workflow_catalog),
    );
  }

  function fetchProvisioningProfiles() {
    void runCommand(
      "getProvisioningProfiles",
      async () => commands.getProvisioningProfiles(),
      ({ provisioning_profiles }) => setProvisioningProfiles(provisioning_profiles),
    );
  }

  function fetchEndpointProfiles() {
    void runCommand(
      "getEndpointProfiles",
      async () => commands.getEndpointProfiles(),
      ({ endpoint_profiles }) => setEndpointProfiles(endpoint_profiles),
    );
  }

  function fetchProviderInventory() {
    void runCommand(
      "getProviderInventory",
      async () => commands.getProviderInventory({ gpu_cloud_provider_id: "runpod" }),
      ({ provider_inventory }) => setProviderInventory(provider_inventory),
    );
  }

  function fetchWorkspaceCatalog() {
    void runCommand(
      "getWorkspaceCatalog",
      async () => commands.getWorkspaceCatalog(),
      ({ workspace_catalog }) => setWorkspaceCatalog(workspace_catalog),
    );
  }

  function createWorkspace() {
    if (
      selectedWorkflowPreset === undefined
      || selectedProvisioningProfile === undefined
      || selectedEndpointProfile === undefined
      || selectedDatacenter === undefined
      || selectedGpu === undefined
    ) {
      return;
    }

    void runCommand(
      "createWorkspace",
      async () => commands.createWorkspace({
        workspace_id: crypto.randomUUID(),
        name: workspaceName.trim(),
        gpu_cloud_provider_id: "runpod",
        placement_plan: {
          gpu_cloud_provider_id: "runpod",
          selected_datacenter_id: selectedDatacenter.id,
          selected_gpu_id: selectedGpu.id,
          persistent_storage_volume_size_bytes: Math.round(selectedStorageSizeGb * GIB),
          selected_workflow_preset: selectedWorkflowPreset,
          selected_provisioning_profile: selectedProvisioningProfile,
          selected_endpoint_profile: selectedEndpointProfile,
        },
      }),
      ({ workspace }) => setWorkspaceCatalog(catalog => ({
        workspaces: [
          workspace,
          ...(catalog?.workspaces.filter(({ id }) => id !== workspace.id) ?? []),
        ],
      })),
    );
  }

  return (
    <main className="min-h-svh bg-background text-foreground">
      <section className="mx-auto flex w-full max-w-6xl flex-col gap-6 px-6 py-8">
        <header className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
          <div className="flex flex-col gap-2">
            <p className="text-sm font-medium text-muted-foreground">Luma Forge</p>
            <h1 className="text-3xl font-semibold tracking-normal md:text-5xl">
              Native command console
            </h1>
          </div>
          <Badge variant={providerSetup !== null ? "default" : "secondary"}>
            {providerSetup !== null ? "RunPod configured" : "RunPod not configured"}
          </Badge>
        </header>

        <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(320px,420px)]">
          <div className="flex flex-col gap-6">
            <Card>
              <CardHeader>
                <CardTitle>Provider setup</CardTitle>
                <CardDescription>
                  Validate, store, inspect, or delete the RunPod setup.
                </CardDescription>
                <CardAction>
                  <Badge variant="outline">runpod</Badge>
                </CardAction>
              </CardHeader>
              <CardContent>
                <FieldGroup>
                  <Field>
                    <FieldLabel htmlFor="provider-api-key">Provider API key</FieldLabel>
                    <Input
                      id="provider-api-key"
                      type="password"
                      autoComplete="off"
                      value={providerApiKey}
                      placeholder="RunPod API key"
                      disabled={pendingCommand !== null}
                      onChange={event => setProviderApiKey(event.target.value)}
                    />
                    <FieldDescription>
                      The key is submitted to the native layer and cleared after setup succeeds.
                    </FieldDescription>
                  </Field>
                  <div className="flex flex-wrap gap-3">
                    <Button disabled={!providerApiKey.trim() || pendingCommand !== null} onClick={setupProvider}>
                      <HugeiconsIcon icon={Key01Icon} strokeWidth={2} data-icon="inline-start" />
                      Setup
                    </Button>
                    <Button variant="outline" disabled={pendingCommand !== null} onClick={refreshProviderSetup}>
                      <HugeiconsIcon icon={ArrowReloadHorizontalIcon} strokeWidth={2} data-icon="inline-start" />
                      Refresh
                    </Button>
                    <Button variant="destructive" disabled={pendingCommand !== null} onClick={deleteProviderSetup}>
                      <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} data-icon="inline-start" />
                      Delete
                    </Button>
                  </div>
                </FieldGroup>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Catalog commands</CardTitle>
                <CardDescription>
                  Load native-owned workflow, profile, provider, and workspace state.
                </CardDescription>
              </CardHeader>
              <CardContent className="flex flex-wrap gap-3">
                <Button variant="outline" disabled={pendingCommand !== null} onClick={fetchWorkflowCatalog}>
                  <HugeiconsIcon icon={DatabaseSyncIcon} strokeWidth={2} data-icon="inline-start" />
                  Workflows
                </Button>
                <Button variant="outline" disabled={pendingCommand !== null} onClick={fetchProvisioningProfiles}>
                  <HugeiconsIcon icon={DatabaseSyncIcon} strokeWidth={2} data-icon="inline-start" />
                  Provisioning profiles
                </Button>
                <Button variant="outline" disabled={pendingCommand !== null} onClick={fetchEndpointProfiles}>
                  <HugeiconsIcon icon={DatabaseSyncIcon} strokeWidth={2} data-icon="inline-start" />
                  Endpoint profiles
                </Button>
                <Button variant="outline" disabled={pendingCommand !== null} onClick={fetchProviderInventory}>
                  <HugeiconsIcon icon={CloudServerIcon} strokeWidth={2} data-icon="inline-start" />
                  Provider inventory
                </Button>
                <Button variant="outline" disabled={pendingCommand !== null} onClick={fetchWorkspaceCatalog}>
                  <HugeiconsIcon icon={DatabaseSyncIcon} strokeWidth={2} data-icon="inline-start" />
                  Workspaces
                </Button>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Create workspace</CardTitle>
                <CardDescription>
                  Uses loaded catalog/profile/inventory objects to build a placement plan.
                </CardDescription>
                <CardAction>
                  <Badge variant="secondary">
                    {workspaceCatalog?.workspaces.length ?? 0}
                    {" "}
                    saved
                  </Badge>
                </CardAction>
              </CardHeader>
              <CardContent>
                <FieldGroup>
                  <div className="grid gap-4 md:grid-cols-2">
                    <Field>
                      <FieldLabel htmlFor="workspace-name">Workspace name</FieldLabel>
                      <Input
                        id="workspace-name"
                        value={workspaceName}
                        disabled={pendingCommand !== null}
                        onChange={event => setWorkspaceName(event.target.value)}
                      />
                    </Field>
                  </div>

                  <div className="grid gap-4 md:grid-cols-2">
                    <Field>
                      <FieldLabel htmlFor="workflow-preset">Workflow preset</FieldLabel>
                      <NativeSelect
                        id="workflow-preset"
                        className="w-full"
                        value={selectedWorkflowPreset?.id ?? ""}
                        disabled={workflowPresets.length === 0 || pendingCommand !== null}
                        onChange={event => setWorkflowPresetId(event.target.value)}
                      >
                        {workflowPresets.length === 0 && <NativeSelectOption value="">Load workflows first</NativeSelectOption>}
                        {workflowPresets.map(preset => (
                          <NativeSelectOption key={preset.id} value={preset.id}>
                            {preset.name}
                          </NativeSelectOption>
                        ))}
                      </NativeSelect>
                    </Field>
                    <Field>
                      <FieldLabel htmlFor="provisioning-profile">Provisioning profile</FieldLabel>
                      <NativeSelect
                        id="provisioning-profile"
                        className="w-full"
                        value={selectedProvisioningProfile?.id ?? ""}
                        disabled={provisioningProfiles.length === 0 || pendingCommand !== null}
                        onChange={event => setProvisioningProfileId(event.target.value)}
                      >
                        {provisioningProfiles.length === 0 && <NativeSelectOption value="">Load profiles first</NativeSelectOption>}
                        {provisioningProfiles.map(profile => (
                          <NativeSelectOption key={profile.id} value={profile.id}>
                            {profile.name}
                          </NativeSelectOption>
                        ))}
                      </NativeSelect>
                    </Field>
                  </div>

                  <div className="grid gap-4 md:grid-cols-2">
                    <Field>
                      <FieldLabel htmlFor="endpoint-profile">Endpoint profile</FieldLabel>
                      <NativeSelect
                        id="endpoint-profile"
                        className="w-full"
                        value={selectedEndpointProfile?.id ?? ""}
                        disabled={endpointProfiles.length === 0 || pendingCommand !== null}
                        onChange={event => setEndpointProfileId(event.target.value)}
                      >
                        {endpointProfiles.length === 0 && <NativeSelectOption value="">Load endpoint profiles first</NativeSelectOption>}
                        {endpointProfiles.map(profile => (
                          <NativeSelectOption key={profile.id} value={profile.id}>
                            {profile.name}
                          </NativeSelectOption>
                        ))}
                      </NativeSelect>
                    </Field>
                    <Field>
                      <FieldLabel htmlFor="storage-size">Storage size, GiB</FieldLabel>
                      <div className="flex items-center gap-3">
                        <Slider
                          id="storage-size"
                          min={MIN_STORAGE_SIZE_GB}
                          max={maxStorageSizeGb}
                          step={1}
                          value={[selectedStorageSizeGb]}
                          disabled={pendingCommand !== null}
                          onValueChange={([value]) => {
                            setStorageSizeGb(value ?? MIN_STORAGE_SIZE_GB);
                          }}
                        />
                        <Input
                          className="w-20"
                          type="number"
                          min={MIN_STORAGE_SIZE_GB}
                          max={maxStorageSizeGb}
                          value={selectedStorageSizeGb}
                          disabled={pendingCommand !== null}
                          aria-label="Storage size in GiB"
                          onChange={(event) => {
                            const nextValue = Number(event.target.value);

                            if (!Number.isNaN(nextValue)) {
                              setStorageSizeGb(Math.min(
                                maxStorageSizeGb,
                                Math.max(MIN_STORAGE_SIZE_GB, nextValue),
                              ));
                            }
                          }}
                        />
                      </div>
                      <FieldDescription>
                        {MIN_STORAGE_SIZE_GB}
                        -
                        {maxStorageSizeGb}
                        {" "}
                        GiB
                      </FieldDescription>
                    </Field>
                  </div>

                  <div className="grid gap-4 md:grid-cols-2">
                    <Field>
                      <FieldLabel htmlFor="datacenter">Datacenter</FieldLabel>
                      <NativeSelect
                        id="datacenter"
                        className="w-full"
                        value={selectedDatacenter?.id ?? ""}
                        disabled={datacenters.length === 0 || pendingCommand !== null}
                        onChange={(event) => {
                          setDatacenterId(event.target.value);
                          setGpuId("");
                        }}
                      >
                        {datacenters.length === 0 && <NativeSelectOption value="">Load inventory first</NativeSelectOption>}
                        {datacenters.map(datacenter => (
                          <NativeSelectOption key={datacenter.id} value={datacenter.id}>
                            {datacenter.name}
                          </NativeSelectOption>
                        ))}
                      </NativeSelect>
                    </Field>
                    <Field>
                      <FieldLabel htmlFor="gpu">GPU</FieldLabel>
                      <NativeSelect
                        id="gpu"
                        className="w-full"
                        value={selectedGpu?.id ?? ""}
                        disabled={(selectedDatacenter?.gpu_options.length ?? 0) === 0 || pendingCommand !== null}
                        onChange={event => setGpuId(event.target.value)}
                      >
                        {(selectedDatacenter?.gpu_options.length ?? 0) === 0 && (
                          <NativeSelectOption value="">Load inventory first</NativeSelectOption>
                        )}
                        {selectedDatacenter?.gpu_options.map(gpu => (
                          <NativeSelectOption key={gpu.id} value={gpu.id}>
                            {gpu.name}
                          </NativeSelectOption>
                        ))}
                      </NativeSelect>
                    </Field>
                  </div>

                  <Button disabled={!canCreateWorkspace || pendingCommand !== null} onClick={createWorkspace}>
                    <HugeiconsIcon icon={Add01Icon} strokeWidth={2} data-icon="inline-start" />
                    Create workspace
                  </Button>
                </FieldGroup>
              </CardContent>
            </Card>
          </div>

          <aside className="flex flex-col gap-6">
            <Card>
              <CardHeader>
                <CardTitle>Last response</CardTitle>
                <CardDescription>
                  {pendingCommand !== null ? `Running ${pendingCommand}` : "Native command output"}
                </CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-4">
                {latestErrorPresentation !== null && (
                  <div className="flex flex-col gap-3 rounded-md border border-destructive/30 bg-destructive/5 p-4">
                    <div className="flex items-start justify-between gap-3">
                      <div className="flex flex-col gap-1">
                        <p className="text-sm font-medium text-destructive">
                          {latestErrorPresentation.title}
                        </p>
                        <p className="text-sm text-muted-foreground">
                          {latestErrorPresentation.description}
                        </p>
                      </div>
                      {latestErrorPresentation.retryable && (
                        <Badge variant="outline">retryable</Badge>
                      )}
                    </div>
                    {latestErrorPresentation.recoveryHint !== null && (
                      <p className="text-sm text-foreground">
                        {latestErrorPresentation.recoveryHint}
                      </p>
                    )}
                    <dl className="grid gap-2 text-xs text-muted-foreground">
                      {latestErrorPresentation.details.map(detail => (
                        <div key={detail.label} className="grid grid-cols-[88px_minmax(0,1fr)] gap-3">
                          <dt>{detail.label}</dt>
                          <dd className="break-words font-mono">{detail.value}</dd>
                        </div>
                      ))}
                    </dl>
                  </div>
                )}
                <pre className="max-h-[440px] overflow-auto rounded-md bg-muted p-4 text-xs leading-5 text-muted-foreground">
                  {formatJson(latestPayload)}
                </pre>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Command log</CardTitle>
                <CardDescription>Latest calls in this session.</CardDescription>
              </CardHeader>
              <CardContent className="flex flex-col gap-3">
                {logEntries.length === 0 && (
                  <p className="text-sm text-muted-foreground">No commands have been run yet.</p>
                )}
                {logEntries.map((entry, index) => {
                  const errorPresentation = entry.status === "error"
                    && isNativeCommandError(entry.payload)
                    ? presentNativeCommandError(entry.payload)
                    : null;

                  return (
                    <div key={entry.id} className="flex flex-col gap-3">
                      {index > 0 && <Separator />}
                      <div className="flex items-center justify-between gap-3">
                        <span className="truncate text-sm font-medium">{entry.label}</span>
                        <Badge variant={entry.status === "ok" ? "secondary" : "destructive"}>
                          {entry.status}
                        </Badge>
                      </div>
                      {errorPresentation !== null && (
                        <div className="flex flex-col gap-1">
                          <p className="text-sm font-medium text-destructive">
                            {errorPresentation.title}
                          </p>
                          <p className="line-clamp-2 text-xs text-muted-foreground">
                            {errorPresentation.recoveryHint ?? errorPresentation.description}
                          </p>
                        </div>
                      )}
                    </div>
                  );
                })}
              </CardContent>
            </Card>
          </aside>
        </div>
      </section>
    </main>
  );
}
