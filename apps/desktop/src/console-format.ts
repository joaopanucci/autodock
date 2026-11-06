export type ConsoleContainerRow = {
  id: string;
  name: string;
  image: string;
  state: string;
  status: string;
  ports: string;
};

function stringField(value: Record<string, unknown>, key: string): string {
  const field = value[key];
  return typeof field === "string" ? field : "";
}

export function parseConsoleContainers(
  output: string,
): ConsoleContainerRow[] | null {
  const rows: ConsoleContainerRow[] = [];
  const seen = new Set<string>();

  for (const line of output.split(/\r?\n/)) {
    if (!line.trim()) continue;

    let parsed: unknown;

    try {
      parsed = JSON.parse(line);
    } catch {
      return null;
    }

    if (
      parsed === null ||
      typeof parsed !== "object" ||
      Array.isArray(parsed)
    ) {
      return null;
    }

    const item = parsed as Record<string, unknown>;

    if (typeof item.ID !== "string" || typeof item.Names !== "string") {
      return null;
    }
    if (!item.ID.trim() || !item.Names.trim() || seen.has(item.ID)) return null;
    if (
      ["Image", "State", "Status", "Ports"].some(
        (key) => item[key] !== undefined && typeof item[key] !== "string",
      )
    )
      return null;
    seen.add(item.ID);

    rows.push({
      id: stringField(item, "ID"),
      name: stringField(item, "Names"),
      image: stringField(item, "Image"),
      state: stringField(item, "State"),
      status: stringField(item, "Status"),
      ports: stringField(item, "Ports"),
    });
  }

  return rows;
}
