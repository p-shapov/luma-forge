import type { FormEvent } from 'react'
import type { GpuCloudProviderSetup } from '@/generated/commands'
import { Button } from '@shared/components/ui/button'
import { useEffect, useState } from 'react'
import { commands } from '@/generated/commands'

const RUNPOD_PROVIDER_ID = 'runpod'

export function HomePage() {
  const [providerSetup, setProviderSetup] = useState<GpuCloudProviderSetup | null>(null)
  const [providerApiKey, setProviderApiKey] = useState('')
  const [statusMessage, setStatusMessage] = useState('Checking provider setup...')
  const [errorMessage, setErrorMessage] = useState<string | null>(null)
  const [isStatusLoading, setIsStatusLoading] = useState(true)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [isSyncing, setIsSyncing] = useState(false)
  const [isDeleting, setIsDeleting] = useState(false)

  useEffect(() => {
    let isMounted = true

    async function loadProviderSetup() {
      setIsStatusLoading(true)
      setErrorMessage(null)

      try {
        const result = await commands.getGpuCloudProviderSetup({
          gpu_cloud_provider_id: RUNPOD_PROVIDER_ID,
        })

        if (!isMounted) {
          return
        }

        if (result.status === 'ok') {
          setProviderSetup(result.data.gpu_cloud_provider_setup)
          setStatusMessage(
            result.data.gpu_cloud_provider_setup
              ? 'RunPod provider access is configured.'
              : 'RunPod provider access is not configured.',
          )
        }
        else {
          setErrorMessage(result.error.message)
          setStatusMessage('Provider setup status is unavailable.')
        }
      }
      catch (error) {
        if (!isMounted) {
          return
        }

        setErrorMessage(error instanceof Error ? error.message : 'Provider setup status failed.')
        setStatusMessage('Provider setup status is unavailable.')
      }
      finally {
        if (isMounted) {
          setIsStatusLoading(false)
        }
      }
    }

    void loadProviderSetup()

    return () => {
      isMounted = false
    }
  }, [])

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()

    if (providerSetup || !providerApiKey.trim()) {
      setProviderApiKey('')
      return
    }

    setIsSubmitting(true)
    setErrorMessage(null)

    try {
      const result = await commands.setupGpuCloudProvider({
        gpu_cloud_provider_id: RUNPOD_PROVIDER_ID,
        provider_api_key: providerApiKey,
      })

      if (result.status === 'ok') {
        setProviderSetup(result.data.gpu_cloud_provider_setup)
        setStatusMessage('RunPod provider access is configured.')
      }
      else {
        setErrorMessage(result.error.message)
        setStatusMessage('RunPod provider access is not configured.')
      }
    }
    catch (error) {
      setErrorMessage(error instanceof Error ? error.message : 'Provider setup failed.')
      setStatusMessage('RunPod provider access is not configured.')
    }
    finally {
      setProviderApiKey('')
      setIsSubmitting(false)
    }
  }

  async function handleSyncProviderSetup() {
    setIsSyncing(true)
    setErrorMessage(null)

    try {
      const result = await commands.syncGpuCloudProviderSetup({
        gpu_cloud_provider_id: RUNPOD_PROVIDER_ID,
      })

      if (result.status === 'ok') {
        setProviderSetup(result.data.gpu_cloud_provider_setup)
        setStatusMessage('RunPod provider access is configured.')
      }
      else {
        setErrorMessage(result.error.message)
        setStatusMessage(
          providerSetup
            ? 'RunPod provider access is configured from local state.'
            : 'RunPod provider access is not configured.',
        )
      }
    }
    catch (error) {
      setErrorMessage(error instanceof Error ? error.message : 'Provider setup sync failed.')
      setStatusMessage(
        providerSetup
          ? 'RunPod provider access is configured from local state.'
          : 'RunPod provider access is not configured.',
      )
    }
    finally {
      setIsSyncing(false)
    }
  }

  async function handleDeleteProviderSetup() {
    setIsDeleting(true)
    setErrorMessage(null)

    try {
      const result = await commands.deleteGpuCloudProviderSetup({
        gpu_cloud_provider_id: RUNPOD_PROVIDER_ID,
      })

      if (result.status === 'ok') {
        setProviderSetup(result.data.gpu_cloud_provider_setup)
        setStatusMessage('RunPod provider access is not configured.')
      }
      else {
        setErrorMessage(result.error.message)
      }
    }
    catch (error) {
      setErrorMessage(error instanceof Error ? error.message : 'Provider setup deletion failed.')
    }
    finally {
      setProviderApiKey('')
      setIsDeleting(false)
    }
  }

  return (
    <main className="min-h-svh bg-background text-foreground">
      <section className="mx-auto flex min-h-svh w-full max-w-5xl flex-col gap-8 px-6 py-10">
        <div className="flex flex-col gap-4 pt-8">
          <p className="text-sm font-medium text-muted-foreground">Luma Forge</p>
          <div className="flex flex-col gap-3">
            <h1 className="text-3xl font-semibold tracking-normal text-balance md:text-5xl">
              Workspace provisioning console
            </h1>
            <p className="max-w-2xl text-base leading-7 text-muted-foreground md:text-lg">
              Prepare provider access, endpoint profiles, and ComfyUI workspaces from one
              Tauri shell.
            </p>
          </div>
        </div>

        <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_minmax(320px,420px)]">
          <section className="flex flex-col gap-4 border-y border-border py-6">
            <div className="flex items-start justify-between gap-4">
              <div className="flex flex-col gap-1">
                <h2 className="text-lg font-semibold">RunPod setup</h2>
                <p className="text-sm leading-6 text-muted-foreground">{statusMessage}</p>
              </div>
              <span className="rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                {providerSetup ? 'Configured' : 'Required'}
              </span>
            </div>

            {providerSetup
              ? (
                  <div className="flex flex-col gap-4">
                    <dl className="grid gap-3 text-sm sm:grid-cols-2">
                      <div className="flex flex-col gap-1">
                        <dt className="text-muted-foreground">Provider</dt>
                        <dd className="font-medium">{providerSetup.gpu_cloud_provider_id}</dd>
                      </div>
                      <div className="flex flex-col gap-1">
                        <dt className="text-muted-foreground">User id</dt>
                        <dd className="break-all font-medium">{providerSetup.provider_user_id}</dd>
                      </div>
                      <div className="flex flex-col gap-1 sm:col-span-2">
                        <dt className="text-muted-foreground">Key fingerprint</dt>
                        <dd className="break-all font-medium">
                          {providerSetup.provider_api_key_fingerprint}
                        </dd>
                      </div>
                    </dl>
                    <div className="flex flex-wrap gap-3">
                      <Button
                        className="w-fit"
                        variant="outline"
                        type="button"
                        disabled={isSyncing || isDeleting}
                        onClick={handleSyncProviderSetup}
                      >
                        {isSyncing ? 'Syncing...' : 'Sync provider access'}
                      </Button>
                      <Button
                        className="w-fit"
                        variant="destructive"
                        type="button"
                        disabled={isSyncing || isDeleting}
                        onClick={handleDeleteProviderSetup}
                      >
                        {isDeleting ? 'Deleting...' : 'Delete provider access'}
                      </Button>
                    </div>
                  </div>
                )
              : (
                  <form className="flex flex-col gap-3" onSubmit={handleSubmit}>
                    <label className="flex flex-col gap-2 text-sm font-medium">
                      RunPod API key
                      <input
                        className="h-10 rounded-md border border-input bg-background px-3 text-sm outline-none transition-colors placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30 disabled:cursor-not-allowed disabled:opacity-50"
                        type="password"
                        autoComplete="off"
                        value={providerApiKey}
                        disabled={isStatusLoading || isSubmitting}
                        onChange={event => setProviderApiKey(event.target.value)}
                      />
                    </label>
                    <Button
                      className="w-fit"
                      type="submit"
                      disabled={isStatusLoading || isSubmitting || !providerApiKey.trim()}
                    >
                      {isSubmitting ? 'Validating...' : 'Save provider access'}
                    </Button>
                    <Button
                      className="w-fit"
                      variant="outline"
                      type="button"
                      disabled={isStatusLoading || isSubmitting || isSyncing || isDeleting}
                      onClick={handleSyncProviderSetup}
                    >
                      {isSyncing ? 'Checking...' : 'Check saved key'}
                    </Button>
                  </form>
                )}

            {errorMessage
              ? (
                  <p className="text-sm leading-6 text-destructive">{errorMessage}</p>
                )
              : null}
          </section>

          <aside className="flex flex-col gap-3 border-y border-border py-6 text-sm leading-6 text-muted-foreground">
            <div className="flex items-center justify-between gap-3 text-foreground">
              <span className="font-medium">Next workspace</span>
              <Button size="sm" disabled={!providerSetup}>New workspace</Button>
            </div>
            <p>
              Workspace creation stays locked until Native Layer reports complete provider
              setup.
            </p>
          </aside>
        </div>
      </section>
    </main>
  )
}
