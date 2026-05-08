import { Button } from "@shared/components/ui/button";

export function HomePage() {
  return (
    <main className="min-h-svh bg-background text-foreground">
      <section className="mx-auto flex min-h-svh w-full max-w-5xl flex-col justify-center gap-8 px-6 py-10">
        <div className="flex max-w-3xl flex-col gap-4">
          <p className="text-sm font-medium text-muted-foreground">Luma Forge</p>
          <h1 className="text-4xl font-semibold tracking-normal text-balance md:text-6xl">
            Workspace provisioning console
          </h1>
          <p className="max-w-2xl text-lg leading-8 text-muted-foreground">
            Prepare provider access, endpoint profiles, and ComfyUI workspaces from one
            Tauri shell.
          </p>
        </div>
        <div className="flex flex-wrap gap-3">
          <Button>New workspace</Button>
          <Button variant="outline">Provider setup</Button>
        </div>
      </section>
    </main>
  );
}
