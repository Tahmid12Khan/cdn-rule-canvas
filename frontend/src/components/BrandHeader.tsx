// Server component: static brand header for the landing page (Task 01/03).
export function BrandHeader() {
  return (
    <header className="flex flex-col gap-3">
      <div className="h-1.5 w-16 rounded-full bg-brand-500" aria-hidden />
      <h1 className="text-4xl font-bold tracking-tight text-nav">
        Response Rule Engine
      </h1>
      <p className="max-w-xl text-base text-status-prevFg">
        Author, version, and deploy response transformation rules with a visual
        rule builder.
      </p>
    </header>
  );
}
