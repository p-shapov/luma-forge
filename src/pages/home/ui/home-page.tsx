import { Badge } from "@shared/components/ui/badge";

export function HomePage() {
  return (
    <main className="min-h-svh bg-background text-foreground">
      <section className="mx-auto flex w-full max-w-4xl flex-col gap-6 px-6 py-10">
        <header className="flex flex-col gap-3">
          <Badge className="w-fit" variant="secondary">
            Refactor baseline
          </Badge>
          <div className="flex flex-col gap-2">
            <h1 className="text-3xl font-semibold tracking-normal md:text-5xl">
              Native backend refactor
            </h1>
            <p className="max-w-2xl text-base leading-7 text-muted-foreground">
              The previous Tauri backend has moved to `src-tauri-legacy`.
              The active `src-tauri` directory is now a minimal shell while
              the new native architecture is rebuilt.
            </p>
          </div>
        </header>

        <section className="grid gap-4 md:grid-cols-2">
          <StatusPanel
            label="Active backend"
            value="src-tauri"
            description="Minimal Tauri shell with temporary native command stubs."
          />
          <StatusPanel
            label="Legacy backend"
            value="src-tauri-legacy"
            description="Archived implementation kept as reference during the refactor."
          />
        </section>
      </section>
    </main>
  );
}

interface StatusPanelProps {
  label: string;
  value: string;
  description: string;
}

function StatusPanel({ label, value, description }: StatusPanelProps) {
  return (
    <article className="flex min-h-32 flex-col justify-between rounded-lg border bg-card p-5 text-card-foreground">
      <div className="flex flex-col gap-1">
        <p className="text-sm font-medium text-muted-foreground">{label}</p>
        <h2 className="text-xl font-semibold tracking-normal">{value}</h2>
      </div>
      <p className="text-sm leading-6 text-muted-foreground">{description}</p>
    </article>
  );
}
