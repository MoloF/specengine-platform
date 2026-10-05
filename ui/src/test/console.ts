// Every console.error and console.warn a test causes is recorded; the setup fails the test if
// any are left at its end. A test that expects one takes it with takeConsoleCalls().

const recorded: string[] = [];

function format(values: unknown[]): string {
  return values.map((value) => (value instanceof Error ? value.message : String(value))).join(" ");
}

export function recordConsole(level: "error" | "warn", values: unknown[]): void {
  recorded.push(`console.${level}: ${format(values)}`);
}

/** The calls recorded so far, removed from the record. */
export function takeConsoleCalls(): string[] {
  return recorded.splice(0, recorded.length);
}
