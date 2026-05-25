import type {
  GpuCloudProviderSetup,
  HuggingFaceApiKeySetup,
  NativeCommandError,
  ProviderInventory,
  ProviderPlacementCapabilities,
  WorkflowCatalog,
  Workspace,
  WorkspaceCatalog,
  WorkspaceProvisioningFailure,
  WorkspaceProvisioningProgress,
  WorkspaceProvisioningResponse,
} from "@/generated/commands";
import {
  Add01Icon,
  ArrowReloadHorizontalIcon,
  CloudServerIcon,
  DatabaseSyncIcon,
  Delete02Icon,
  Key01Icon,
  PlayIcon,
  RefreshIcon,
  StopIcon,
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
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@shared/components/ui/field";
import { Input } from "@shared/components/ui/input";
import {
  NativeSelect,
  NativeSelectOption,
} from "@shared/components/ui/native-select";
import { Progress } from "@shared/components/ui/progress";
import { Separator } from "@shared/components/ui/separator";
import { Slider } from "@shared/components/ui/slider";
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { commands } from "@/generated/commands";
import {
  isNativeCommandError,
  presentNativeCommandError,
} from "@/shared/lib/native-command-error-presenter";

const GIB = 1024 ** 3;
const MIN_STORAGE_SIZE_GB = 1;
const DEFAULT_MAX_STORAGE_SIZE_GB = 100;
const DEFAULT_ENDPOINT_KEEP_ALIVE_SECONDS = 5;
const MIN_ENDPOINT_KEEP_ALIVE_SECONDS = 5;
const MAX_ENDPOINT_KEEP_ALIVE_SECONDS = 3600;
const PROVISIONING_SYNC_INTERVAL_MS = 1000;

type CommandResult<T>
  = | { status: "ok"; data: T }
    | { status: "error"; error: NativeCommandError };

interface LogEntry {
  id: number;
  label: string;
  status: "ok" | "error";
  payload: unknown;
}

const COMMAND_TOAST_COPY = {
  getGpuCloudProviderSetup: {
    loading: "Refreshing provider setup...",
    success: "Provider setup refreshed",
  },
  setupGpuCloudProvider: {
    loading: "Validating provider key...",
    success: "Provider setup completed",
  },
  deleteGpuCloudProviderSetup: {
    loading: "Deleting provider setup...",
    success: "Provider setup deleted",
  },
  getHuggingFaceApiKeySetup: {
    loading: "Refreshing Hugging Face setup...",
    success: "Hugging Face setup refreshed",
  },
  setupHuggingFaceApiKey: {
    loading: "Validating Hugging Face key...",
    success: "Hugging Face setup completed",
  },
  deleteHuggingFaceApiKeySetup: {
    loading: "Deleting Hugging Face setup...",
    success: "Hugging Face setup deleted",
  },
  getWorkflowCatalog: {
    loading: "Loading workflow catalog...",
    success: "Workflow catalog loaded",
  },
  getProviderPlacementOptions: {
    loading: "Loading placement options...",
    success: "Placement options loaded",
  },
  getWorkspaceCatalog: {
    loading: "Loading workspace catalog...",
    success: "Workspace catalog loaded",
  },
  createWorkspace: {
    loading: "Creating workspace...",
    success: "Workspace created",
  },
  deleteWorkspace: {
    loading: "Removing workspace...",
    success: "Workspace removed",
  },
  initiateWorkspaceProvisioning: {
    loading: "Starting workspace provisioning...",
    success: "Workspace provisioning started",
  },
  syncWorkspaceProvisioning: {
    loading: "Syncing workspace provisioning...",
    success: "Workspace provisioning synced",
  },
  cancelWorkspaceProvisioning: {
    loading: "Cancelling workspace provisioning...",
    success: "Workspace provisioning cancelled",
  },
} satisfies Record<string, {
  loading: string;
  success: string;
}>;

type CommandLabel = keyof typeof COMMAND_TOAST_COPY;

function formatJson(value: unknown) {
  return JSON.stringify(value, null, 2);
}

function errorPayload(error: unknown) {
  if (error instanceof Error) {
    return { message: error.message };
  }

  return { message: "Command failed", error };
}

function toastErrorPayload(error: unknown) {
  if (isNativeCommandError(error)) {
    const presentation = presentNativeCommandError(error);

    return {
      message: presentation.title,
      description: presentation.recoveryHint ?? presentation.description,
    };
  }

  if (error instanceof Error) {
    return {
      message: "Command failed",
      description: error.message,
    };
  }

  return {
    message: "Command failed",
    description: "Review the latest response for details.",
  };
}

function clampNumber(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

function endpointKeepAliveRange(capabilities: ProviderPlacementCapabilities | null) {
  const capability = capabilities?.endpoint_keep_alive;

  if (capability?.supported === "true") {
    return {
      supported: true,
      defaultSeconds: capability.default_seconds,
      minSeconds: capability.min_seconds,
      maxSeconds: capability.max_seconds,
    };
  }

  return {
    supported: false,
    defaultSeconds: DEFAULT_ENDPOINT_KEEP_ALIVE_SECONDS,
    minSeconds: MIN_ENDPOINT_KEEP_ALIVE_SECONDS,
    maxSeconds: MAX_ENDPOINT_KEEP_ALIVE_SECONDS,
  };
}

function gpuAvailabilityLabel(score: number) {
  if (score >= 80) {
    return "High availability";
  }
  if (score >= 50) {
    return "Medium availability";
  }
  if (score > 0) {
    return "Low availability";
  }

  return "Unavailable";
}

function gpuOptionLabel(gpu: { name: string; availability_score: number }) {
  return `${gpu.name} - ${gpuAvailabilityLabel(gpu.availability_score)}`;
}

function isGpuAvailable(gpu: { availability_score: number } | undefined) {
  return gpu !== undefined && gpu.availability_score > 0;
}

function workspacePlacementGpu(
  providerInventory: ProviderInventory | null,
  workspace: Workspace | undefined,
) {
  if (workspace === undefined) {
    return undefined;
  }

  const datacenter = providerInventory?.datacenters.find(
    ({ id }) => id === workspace.placement_plan.selected_datacenter_id,
  );

  return datacenter?.gpu_options.find(
    ({ id }) => id === workspace.placement_plan.selected_gpu_id,
  );
}

function upsertWorkspace(catalog: WorkspaceCatalog | null, workspace: Workspace): WorkspaceCatalog {
  return {
    workspaces: [
      workspace,
      ...(catalog?.workspaces.filter(({ id }) => id !== workspace.id) ?? []),
    ],
  };
}

function isTerminalProvisioningResponse(response: WorkspaceProvisioningResponse) {
  return response.progress.status === "idle"
    || response.progress.status === "completed"
    || response.progress.status === "failed"
    || response.workspace.lifecycle_state === "draft"
    || response.workspace.lifecycle_state === "ready"
    || response.workspace.lifecycle_state === "failed";
}

function formatProvisioningLabel(value: string) {
  return value
    .split("_")
    .map(part => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function provisioningFailureText(failure: WorkspaceProvisioningFailure | null) {
  if (failure === null) {
    return null;
  }

  return [
    `${formatProvisioningLabel(failure.source)} failure`,
    formatProvisioningLabel(failure.code),
    provisioningRecoveryHint(failure.recovery_action),
  ].join(" - ");
}

function provisioningRecoveryHint(recoveryAction: WorkspaceProvisioningFailure["recovery_action"]) {
  switch (recoveryAction) {
    case "retry":
      return "Retry when the provider is available.";
    case "recover_provider_setup":
      return "Recover provider setup before retrying.";
    case "configure_hugging_face_setup":
      return "Configure Hugging Face setup before retrying.";
    case "reselect_placement":
      return "Reselect placement before retrying.";
    case "cleanup_workspace_resources":
      return "Clean up workspace resources before retrying.";
    case "inspect_workspace_provisioning":
      return "Inspect provisioning state before retrying.";
  }
}

function provisioningProgressValue(
  progress: WorkspaceProvisioningProgress | undefined,
  workspace: Workspace | undefined,
) {
  if (typeof progress?.percent === "number") {
    return clampNumber(progress.percent, 0, 100);
  }

  if (progress?.status === "completed" || workspace?.lifecycle_state === "ready") {
    return 100;
  }

  return 0;
}

export function HomePage() {
  const [providerApiKey, setProviderApiKey] = useState("");
  const [providerSetup, setProviderSetup] = useState<GpuCloudProviderSetup | null>(null);
  const [huggingFaceApiKey, setHuggingFaceApiKey] = useState("");
  const [huggingFaceSetup, setHuggingFaceSetup] = useState<HuggingFaceApiKeySetup | null>(null);
  const [workflowCatalog, setWorkflowCatalog] = useState<WorkflowCatalog | null>(null);
  const [providerInventory, setProviderInventory] = useState<ProviderInventory | null>(null);
  const [providerPlacementCapabilities, setProviderPlacementCapabilities]
    = useState<ProviderPlacementCapabilities | null>(null);
  const [workspaceCatalog, setWorkspaceCatalog] = useState<WorkspaceCatalog | null>(null);
  const [workspaceName, setWorkspaceName] = useState("Default workspace");
  const [additionalStorageSizeGb, setAdditionalStorageSizeGb] = useState(0);
  const [endpointKeepAliveSeconds, setEndpointKeepAliveSeconds]
    = useState(DEFAULT_ENDPOINT_KEEP_ALIVE_SECONDS);
  const [workflowPresetId, setWorkflowPresetId] = useState("");
  const [datacenterId, setDatacenterId] = useState("");
  const [gpuId, setGpuId] = useState("");
  const [provisioningWorkspaceId, setProvisioningWorkspaceId] = useState("");
  const [autoSyncWorkspaceId, setAutoSyncWorkspaceId] = useState<string | null>(null);
  const [provisioningProgressByWorkspaceId, setProvisioningProgressByWorkspaceId]
    = useState<Record<string, WorkspaceProvisioningProgress>>({});
  const [pendingCommand, setPendingCommand] = useState<string | null>(null);
  const [logEntries, setLogEntries] = useState<LogEntry[]>([]);
  const autoSyncInFlightRef = useRef(false);

  const workflowPresets = workflowCatalog?.workflow_presets ?? [];
  const datacenters = providerInventory?.datacenters ?? [];
  const workspaces = workspaceCatalog?.workspaces ?? [];
  const selectedWorkflowPreset = workflowPresets.find(({ id }) => id === workflowPresetId)
    ?? workflowPresets[0];
  const selectedDatacenter = datacenters.find(({ id }) => id === datacenterId)
    ?? datacenters[0];
  const gpuOptions = selectedDatacenter?.gpu_options ?? [];
  const selectedGpu = gpuOptions.find(({ id }) => id === gpuId)
    ?? gpuOptions[0];
  const selectedGpuAvailable = isGpuAvailable(selectedGpu);
  const placementOptionsLoaded = providerInventory !== null;
  const noGpuAvailable = placementOptionsLoaded
    && datacenters.length > 0
    && !datacenters.some(datacenter => datacenter.gpu_options.some(isGpuAvailable));
  const keepAliveRange = endpointKeepAliveRange(providerPlacementCapabilities);
  const selectedEndpointKeepAliveSeconds = clampNumber(
    endpointKeepAliveSeconds,
    keepAliveRange.minSeconds,
    keepAliveRange.maxSeconds,
  );
  const maxTotalStorageSizeGb = providerInventory?.max_persistent_storage_volume_size_bytes !== null
    && providerInventory?.max_persistent_storage_volume_size_bytes !== undefined
    ? Math.max(
        MIN_STORAGE_SIZE_GB,
        Math.floor(providerInventory.max_persistent_storage_volume_size_bytes / GIB),
      )
    : DEFAULT_MAX_STORAGE_SIZE_GB;
  const requiredBaseStorageSizeBytes = selectedWorkflowPreset?.required_base_volume_size_bytes ?? 0;
  const requiredBaseStorageSizeGb = Math.ceil(requiredBaseStorageSizeBytes / GIB);
  const maxAdditionalStorageSizeGb = Math.max(0, maxTotalStorageSizeGb - requiredBaseStorageSizeGb);
  const selectedAdditionalStorageSizeGb = Math.min(additionalStorageSizeGb, maxAdditionalStorageSizeGb);
  const requestedStorageSizeBytes = requiredBaseStorageSizeBytes
    + Math.round(selectedAdditionalStorageSizeGb * GIB);
  const requestedStorageSizeGb = Math.ceil(requestedStorageSizeBytes / GIB);
  const selectedProvisioningWorkspaceId = provisioningWorkspaceId || workspaces[0]?.id || "";
  const canRunProvisioningCommand = selectedProvisioningWorkspaceId.trim().length > 0;
  const selectedProvisioningWorkspace = workspaces.find(({ id }) => id === selectedProvisioningWorkspaceId);
  const selectedProvisioningGpu = workspacePlacementGpu(providerInventory, selectedProvisioningWorkspace);
  const selectedProvisioningPlacementChecked = providerInventory !== null
    && selectedProvisioningWorkspace !== undefined;
  const selectedProvisioningGpuUnavailable = selectedProvisioningPlacementChecked
    && !isGpuAvailable(selectedProvisioningGpu);
  const canStartProvisioningCommand = canRunProvisioningCommand && !selectedProvisioningGpuUnavailable;
  const selectedProvisioningProgress = provisioningProgressByWorkspaceId[selectedProvisioningWorkspaceId];
  const selectedProvisioningFailureText = provisioningFailureText(
    selectedProvisioningProgress?.failure ?? selectedProvisioningWorkspace?.last_provisioning_failure ?? null,
  );
  const selectedProvisioningProgressValue = provisioningProgressValue(
    selectedProvisioningProgress,
    selectedProvisioningWorkspace,
  );
  const canRemoveWorkspace = selectedProvisioningWorkspace !== undefined
    && selectedProvisioningWorkspace.lifecycle_state !== "provisioning";
  const autoSyncWorkspace = workspaces.find(({ id }) => id === autoSyncWorkspaceId);
  const autoSyncActive = autoSyncWorkspaceId !== null;

  const latestEntry = logEntries[0];
  const latestPayload = useMemo(() => latestEntry?.payload ?? {
    message: "Run a command to see the native response.",
  }, [latestEntry]);
  const latestErrorPresentation = latestEntry?.status === "error"
    && isNativeCommandError(latestEntry.payload)
    ? presentNativeCommandError(latestEntry.payload)
    : null;

  function rememberProvisioningResponse(response: WorkspaceProvisioningResponse) {
    setWorkspaceCatalog(catalog => upsertWorkspace(catalog, response.workspace));
    setProvisioningProgressByWorkspaceId(progressByWorkspaceId => ({
      ...progressByWorkspaceId,
      [response.workspace.id]: response.progress,
    }));
  }

  useEffect(() => {
    if (autoSyncWorkspaceId === null) {
      return;
    }

    let disposed = false;

    async function syncOnce() {
      if (autoSyncInFlightRef.current || autoSyncWorkspaceId === null) {
        return;
      }

      autoSyncInFlightRef.current = true;

      try {
        const result = await commands.syncWorkspaceProvisioning({
          workspace_id: autoSyncWorkspaceId,
        });

        if (disposed) {
          return;
        }

        if (result.status === "ok") {
          setWorkspaceCatalog(catalog => upsertWorkspace(catalog, result.data.workspace));
          setProvisioningProgressByWorkspaceId(progressByWorkspaceId => ({
            ...progressByWorkspaceId,
            [result.data.workspace.id]: result.data.progress,
          }));
          setLogEntries(entries => [
            {
              id: Date.now(),
              label: "syncWorkspaceProvisioning",
              status: "ok",
              payload: result.data,
            },
            ...entries,
          ]);

          if (isTerminalProvisioningResponse(result.data)) {
            setAutoSyncWorkspaceId(null);
          }

          return;
        }

        setLogEntries(entries => [
          {
            id: Date.now(),
            label: "syncWorkspaceProvisioning",
            status: "error",
            payload: result.error,
          },
          ...entries,
        ]);

        if (!result.error.retryable) {
          setAutoSyncWorkspaceId(null);
        }
      }
      catch (error) {
        if (!disposed) {
          setLogEntries(entries => [
            {
              id: Date.now(),
              label: "syncWorkspaceProvisioning",
              status: "error",
              payload: errorPayload(error),
            },
            ...entries,
          ]);
          setAutoSyncWorkspaceId(null);
        }
      }
      finally {
        autoSyncInFlightRef.current = false;
      }
    }

    void syncOnce();
    const intervalId = window.setInterval(() => {
      void syncOnce();
    }, PROVISIONING_SYNC_INTERVAL_MS);

    return () => {
      disposed = true;
      window.clearInterval(intervalId);
    };
  }, [autoSyncWorkspaceId]);

  async function runCommand<T>(
    label: CommandLabel,
    action: () => Promise<CommandResult<T>>,
    onSuccess?: (data: T) => void,
  ) {
    setPendingCommand(label);

    let loggedError = false;
    const toastCopy = COMMAND_TOAST_COPY[label];
    const commandPromise = (async () => {
      try {
        const result = await action();

        if (result.status === "ok") {
          onSuccess?.(result.data);
          setLogEntries(entries => [
            { id: Date.now(), label, status: "ok", payload: result.data },
            ...entries,
          ]);
          return result.data;
        }

        loggedError = true;
        setLogEntries(entries => [
          { id: Date.now(), label, status: "error", payload: result.error },
          ...entries,
        ]);
        throw result.error;
      }
      catch (error) {
        if (!loggedError) {
          setLogEntries(entries => [
            { id: Date.now(), label, status: "error", payload: errorPayload(error) },
            ...entries,
          ]);
        }

        throw error;
      }
      finally {
        setPendingCommand(null);
      }
    })();

    toast.promise(commandPromise, {
      loading: toastCopy.loading,
      success: toastCopy.success,
      error: toastErrorPayload,
    });

    await commandPromise.catch(() => undefined);
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

  function refreshHuggingFaceSetup() {
    void runCommand(
      "getHuggingFaceApiKeySetup",
      async () => commands.getHuggingFaceApiKeySetup(null),
      ({ hugging_face_api_key_setup }) => setHuggingFaceSetup(hugging_face_api_key_setup),
    );
  }

  function setupHuggingFaceApiKey() {
    void runCommand(
      "setupHuggingFaceApiKey",
      async () => commands.setupHuggingFaceApiKey({
        hugging_face_api_key: huggingFaceApiKey,
      }),
      ({ hugging_face_api_key_setup }) => {
        setHuggingFaceSetup(hugging_face_api_key_setup);
        setHuggingFaceApiKey("");
      },
    );
  }

  function deleteHuggingFaceSetup() {
    void runCommand(
      "deleteHuggingFaceApiKeySetup",
      async () => commands.deleteHuggingFaceApiKeySetup(null),
      ({ hugging_face_api_key_setup }) => setHuggingFaceSetup(hugging_face_api_key_setup),
    );
  }

  function fetchWorkflowCatalog() {
    void runCommand(
      "getWorkflowCatalog",
      async () => commands.getWorkflowCatalog(),
      ({ workflow_catalog }) => setWorkflowCatalog(workflow_catalog),
    );
  }

  function fetchProviderPlacementOptions() {
    void runCommand(
      "getProviderPlacementOptions",
      async () => commands.getProviderPlacementOptions({ gpu_cloud_provider_id: "runpod" }),
      ({ provider_inventory, placement_capabilities }) => {
        setProviderInventory(provider_inventory);
        setProviderPlacementCapabilities(placement_capabilities);

        const range = endpointKeepAliveRange(placement_capabilities);
        setEndpointKeepAliveSeconds(value =>
          clampNumber(
            value || range.defaultSeconds,
            range.minSeconds,
            range.maxSeconds,
          ),
        );
      },
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
          persistent_storage_volume_size_bytes: requestedStorageSizeBytes,
          endpoint_keep_alive_seconds: selectedEndpointKeepAliveSeconds,
          selected_workflow_preset: selectedWorkflowPreset,
        },
      }),
      ({ workspace }) => {
        setWorkspaceCatalog(catalog => upsertWorkspace(catalog, workspace));
        setProvisioningWorkspaceId(workspace.id);
      },
    );
  }

  function runProvisioningCommand(
    label: Extract<
      CommandLabel,
      "initiateWorkspaceProvisioning"
      | "syncWorkspaceProvisioning"
      | "cancelWorkspaceProvisioning"
    >,
  ) {
    const workspaceId = selectedProvisioningWorkspaceId.trim();

    if (!workspaceId) {
      return;
    }

    const action = {
      initiateWorkspaceProvisioning: commands.initiateWorkspaceProvisioning,
      syncWorkspaceProvisioning: commands.syncWorkspaceProvisioning,
      cancelWorkspaceProvisioning: commands.cancelWorkspaceProvisioning,
    }[label];

    void runCommand(
      label,
      async () => {
        if (label === "cancelWorkspaceProvisioning") {
          setAutoSyncWorkspaceId(null);
        }

        return action({ workspace_id: workspaceId });
      },
      (response) => {
        rememberProvisioningResponse(response);

        if (isTerminalProvisioningResponse(response)) {
          setAutoSyncWorkspaceId(null);
          return;
        }

        if (label === "initiateWorkspaceProvisioning") {
          setAutoSyncWorkspaceId(response.workspace.id);
        }
      },
    );
  }

  function removeWorkspace() {
    const workspaceId = selectedProvisioningWorkspaceId.trim();

    if (!workspaceId) {
      return;
    }

    void runCommand(
      "deleteWorkspace",
      async () => commands.deleteWorkspace({ workspace_id: workspaceId }),
      ({ workspace_catalog }) => {
        setWorkspaceCatalog(workspace_catalog);
        setAutoSyncWorkspaceId(value => value === workspaceId ? null : value);
        setProvisioningWorkspaceId(value => value === workspaceId ? "" : value);
        setProvisioningProgressByWorkspaceId((progressByWorkspaceId) => {
          const nextProgressByWorkspaceId = { ...progressByWorkspaceId };
          delete nextProgressByWorkspaceId[workspaceId];
          return nextProgressByWorkspaceId;
        });
      },
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
                <CardTitle>Hugging Face setup</CardTitle>
                <CardDescription>
                  Validate, store, inspect, or delete the optional model asset key.
                </CardDescription>
                <CardAction>
                  <Badge variant={huggingFaceSetup !== null ? "default" : "secondary"}>
                    {huggingFaceSetup !== null ? "configured" : "not configured"}
                  </Badge>
                </CardAction>
              </CardHeader>
              <CardContent>
                <FieldGroup>
                  <Field>
                    <FieldLabel htmlFor="hugging-face-api-key">Hugging Face API key</FieldLabel>
                    <Input
                      id="hugging-face-api-key"
                      type="password"
                      autoComplete="off"
                      value={huggingFaceApiKey}
                      placeholder="Hugging Face API key"
                      disabled={pendingCommand !== null}
                      onChange={event => setHuggingFaceApiKey(event.target.value)}
                    />
                    <FieldDescription>
                      Stored only in the native keyring and cleared after setup succeeds.
                    </FieldDescription>
                  </Field>
                  {huggingFaceSetup !== null && (
                    <div className="grid gap-2 rounded-md border bg-muted/30 p-3 text-sm md:grid-cols-3">
                      <div>
                        <p className="text-xs font-medium uppercase text-muted-foreground">Token</p>
                        <p className="break-all">{huggingFaceSetup.token_name}</p>
                      </div>
                      <div>
                        <p className="text-xs font-medium uppercase text-muted-foreground">User</p>
                        <p className="break-all">{huggingFaceSetup.user_name}</p>
                      </div>
                      <div>
                        <p className="text-xs font-medium uppercase text-muted-foreground">Email</p>
                        <p className="break-all">{huggingFaceSetup.user_email ?? "Not provided"}</p>
                      </div>
                    </div>
                  )}
                  <div className="flex flex-wrap gap-3">
                    <Button
                      disabled={!huggingFaceApiKey.trim() || pendingCommand !== null}
                      onClick={setupHuggingFaceApiKey}
                    >
                      <HugeiconsIcon icon={Key01Icon} strokeWidth={2} data-icon="inline-start" />
                      Setup
                    </Button>
                    <Button variant="outline" disabled={pendingCommand !== null} onClick={refreshHuggingFaceSetup}>
                      <HugeiconsIcon icon={ArrowReloadHorizontalIcon} strokeWidth={2} data-icon="inline-start" />
                      Refresh
                    </Button>
                    <Button variant="destructive" disabled={pendingCommand !== null} onClick={deleteHuggingFaceSetup}>
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
                  Load native-owned workflow, provider, and workspace state.
                </CardDescription>
              </CardHeader>
              <CardContent className="flex flex-wrap gap-3">
                <Button variant="outline" disabled={pendingCommand !== null} onClick={fetchWorkflowCatalog}>
                  <HugeiconsIcon icon={DatabaseSyncIcon} strokeWidth={2} data-icon="inline-start" />
                  Workflows
                </Button>
                <Button variant="outline" disabled={pendingCommand !== null} onClick={fetchProviderPlacementOptions}>
                  <HugeiconsIcon icon={CloudServerIcon} strokeWidth={2} data-icon="inline-start" />
                  Placement options
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
                  Uses loaded workflow and placement objects to build a placement plan.
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
                      <FieldLabel htmlFor="storage-size">Additional storage, GiB</FieldLabel>
                      <div className="flex items-center gap-3">
                        <Slider
                          id="storage-size"
                          min={0}
                          max={maxAdditionalStorageSizeGb}
                          step={1}
                          value={[selectedAdditionalStorageSizeGb]}
                          disabled={pendingCommand !== null}
                          onValueChange={([value]) => {
                            setAdditionalStorageSizeGb(value ?? 0);
                          }}
                        />
                        <Input
                          className="w-20"
                          type="number"
                          min={0}
                          max={maxAdditionalStorageSizeGb}
                          value={selectedAdditionalStorageSizeGb}
                          disabled={pendingCommand !== null}
                          aria-label="Additional storage size in GiB"
                          onChange={(event) => {
                            const nextValue = Number(event.target.value);

                            if (!Number.isNaN(nextValue)) {
                              setAdditionalStorageSizeGb(Math.min(
                                maxAdditionalStorageSizeGb,
                                Math.max(0, nextValue),
                              ));
                            }
                          }}
                        />
                      </div>
                      <FieldDescription>
                        Required base:
                        {" "}
                        {requiredBaseStorageSizeGb}
                        {" "}
                        GiB. Requested total:
                        {" "}
                        {requestedStorageSizeGb}
                        {" "}
                        GiB. Additional range:
                        {" "}
                        0
                        -
                        {maxAdditionalStorageSizeGb}
                        {" "}
                        GiB
                      </FieldDescription>
                    </Field>
                  </div>

                  <div className="grid gap-4 md:grid-cols-2">
                    <Field>
                      <FieldLabel htmlFor="endpoint-keep-alive">Endpoint keep-alive, seconds</FieldLabel>
                      <div className="flex items-center gap-3">
                        <Slider
                          id="endpoint-keep-alive"
                          min={keepAliveRange.minSeconds}
                          max={keepAliveRange.maxSeconds}
                          step={5}
                          value={[selectedEndpointKeepAliveSeconds]}
                          disabled={!keepAliveRange.supported || pendingCommand !== null}
                          onValueChange={([value]) => {
                            setEndpointKeepAliveSeconds(value ?? keepAliveRange.defaultSeconds);
                          }}
                        />
                        <Input
                          className="w-24"
                          type="number"
                          min={keepAliveRange.minSeconds}
                          max={keepAliveRange.maxSeconds}
                          value={selectedEndpointKeepAliveSeconds}
                          disabled={!keepAliveRange.supported || pendingCommand !== null}
                          aria-label="Endpoint keep-alive seconds"
                          onChange={(event) => {
                            const nextValue = Number(event.target.value);

                            if (!Number.isNaN(nextValue)) {
                              setEndpointKeepAliveSeconds(clampNumber(
                                nextValue,
                                keepAliveRange.minSeconds,
                                keepAliveRange.maxSeconds,
                              ));
                            }
                          }}
                        />
                      </div>
                      <FieldDescription>
                        Range:
                        {" "}
                        {keepAliveRange.minSeconds}
                        -
                        {keepAliveRange.maxSeconds}
                        {" "}
                        seconds. Default:
                        {" "}
                        {keepAliveRange.defaultSeconds}
                        {" "}
                        seconds.
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
                        {datacenters.length === 0 && (
                          <NativeSelectOption value="">
                            {placementOptionsLoaded ? "No available datacenters" : "Load placement options first"}
                          </NativeSelectOption>
                        )}
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
                        disabled={gpuOptions.length === 0 || pendingCommand !== null}
                        onChange={event => setGpuId(event.target.value)}
                      >
                        {gpuOptions.length === 0 && (
                          <NativeSelectOption value="">
                            {placementOptionsLoaded ? "No available GPUs" : "Load placement options first"}
                          </NativeSelectOption>
                        )}
                        {gpuOptions.map(gpu => (
                          <NativeSelectOption key={gpu.id} value={gpu.id}>
                            {gpuOptionLabel(gpu)}
                          </NativeSelectOption>
                        ))}
                      </NativeSelect>
                      {selectedGpu !== undefined && (
                        <FieldDescription>
                          {gpuAvailabilityLabel(selectedGpu.availability_score)}
                          {" "}
                          (
                          {selectedGpu.availability_score}
                          /100).
                          {!selectedGpuAvailable && (
                            <>
                              {" "}
                              Workspace creation is disabled until this GPU is available.
                            </>
                          )}
                        </FieldDescription>
                      )}
                    </Field>
                  </div>

                  {noGpuAvailable && (
                    <FieldError>
                      No GPU is currently available in any loaded datacenter.
                      {" "}
                      Refresh placement options and try again later.
                    </FieldError>
                  )}

                  <Button disabled={pendingCommand !== null} onClick={createWorkspace}>
                    <HugeiconsIcon icon={Add01Icon} strokeWidth={2} data-icon="inline-start" />
                    Create workspace
                  </Button>
                </FieldGroup>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Provision workspace</CardTitle>
                <CardDescription>
                  Starts, syncs, or cancels native-owned workspace provisioning.
                </CardDescription>
                <CardAction>
                  <Badge variant={autoSyncActive ? "default" : "secondary"}>
                    {autoSyncActive ? "Auto-sync active" : "Manual sync"}
                  </Badge>
                </CardAction>
              </CardHeader>
              <CardContent>
                <FieldGroup>
                  <Field>
                    <FieldLabel htmlFor="provisioning-workspace">Workspace</FieldLabel>
                    <NativeSelect
                      id="provisioning-workspace"
                      className="w-full"
                      value={selectedProvisioningWorkspaceId}
                      disabled={workspaces.length === 0 || pendingCommand !== null}
                      onChange={(event) => {
                        setProvisioningWorkspaceId(event.target.value);
                        setAutoSyncWorkspaceId(null);
                      }}
                    >
                      {workspaces.length === 0 && (
                        <NativeSelectOption value="">Load or create a workspace first</NativeSelectOption>
                      )}
                      {workspaces.map(workspace => (
                        <NativeSelectOption key={workspace.id} value={workspace.id}>
                          {workspace.name}
                          {" - "}
                          {workspace.lifecycle_state}
                        </NativeSelectOption>
                      ))}
                    </NativeSelect>
                    <FieldDescription>
                      Commands use the selected workspace ID and return the authoritative workspace snapshot.
                      {selectedProvisioningGpu !== undefined && (
                        <>
                          {" "}
                          GPU:
                          {" "}
                          {gpuAvailabilityLabel(selectedProvisioningGpu.availability_score)}
                          {" "}
                          (
                          {selectedProvisioningGpu.availability_score}
                          /100).
                        </>
                      )}
                      {selectedProvisioningGpuUnavailable && (
                        <>
                          {" "}
                          Start is disabled until the selected workspace GPU is available in loaded placement options.
                        </>
                      )}
                      {autoSyncWorkspace !== undefined && (
                        <>
                          {" "}
                          Syncing:
                          {" "}
                          {autoSyncWorkspace.name}
                          .
                        </>
                      )}
                    </FieldDescription>
                  </Field>

                  <Field>
                    <div className="flex items-center justify-between gap-3">
                      <FieldLabel>Provisioning progress</FieldLabel>
                      <Badge variant={selectedProvisioningProgress?.status === "failed" ? "destructive" : "secondary"}>
                        {formatProvisioningLabel(
                          selectedProvisioningProgress?.status
                          ?? selectedProvisioningWorkspace?.lifecycle_state
                          ?? "idle",
                        )}
                      </Badge>
                    </div>
                    <Progress
                      value={selectedProvisioningProgressValue}
                      aria-label="Provisioning progress"
                    />
                    <FieldDescription>
                      {selectedProvisioningProgress === undefined
                        ? "Run or sync provisioning to load progress."
                        : (
                            <>
                              {formatProvisioningLabel(selectedProvisioningProgress.phase)}
                              {" - "}
                              {selectedProvisioningProgress.percent === null
                                ? "Progress pending"
                                : `${selectedProvisioningProgressValue}%`}
                              {selectedProvisioningFailureText !== null && (
                                <>
                                  {" - "}
                                  {selectedProvisioningFailureText}
                                </>
                              )}
                            </>
                          )}
                    </FieldDescription>
                  </Field>

                  <div className="flex flex-wrap gap-3">
                    <Button
                      disabled={!canStartProvisioningCommand || pendingCommand !== null}
                      onClick={() => runProvisioningCommand("initiateWorkspaceProvisioning")}
                    >
                      <HugeiconsIcon icon={PlayIcon} strokeWidth={2} data-icon="inline-start" />
                      Start
                    </Button>
                    <Button
                      variant="outline"
                      disabled={!canRunProvisioningCommand || pendingCommand !== null}
                      onClick={() => runProvisioningCommand("syncWorkspaceProvisioning")}
                    >
                      <HugeiconsIcon icon={RefreshIcon} strokeWidth={2} data-icon="inline-start" />
                      Sync
                    </Button>
                    <Button
                      variant="destructive"
                      disabled={!canRunProvisioningCommand || pendingCommand !== null}
                      onClick={() => runProvisioningCommand("cancelWorkspaceProvisioning")}
                    >
                      <HugeiconsIcon icon={StopIcon} strokeWidth={2} data-icon="inline-start" />
                      Cancel
                    </Button>
                    <Button
                      variant="destructive"
                      disabled={!canRemoveWorkspace || pendingCommand !== null}
                      onClick={removeWorkspace}
                    >
                      <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} data-icon="inline-start" />
                      Remove
                    </Button>
                  </div>
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
