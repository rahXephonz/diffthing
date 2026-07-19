import type { Hunk, Step } from "./protocol";

export function basename(path: string) {
  return path.split("/").pop() ?? path;
}

export function fileTotals(step: Step, hunksById: Map<string, Hunk>) {
  const byPath = new Map<string, { added: number; removed: number }>();
  for (const id of step.hunks) {
    const h = hunksById.get(id);
    if (!h) continue;
    const agg = byPath.get(h.path) ?? { added: 0, removed: 0 };
    agg.added += h.added;
    agg.removed += h.removed;
    byPath.set(h.path, agg);
  }
  const files = [...byPath.entries()].map(([path, agg]) => ({ path, ...agg }));
  const total = files.reduce(
    (acc, f) => ({ added: acc.added + f.added, removed: acc.removed + f.removed }),
    { added: 0, removed: 0 },
  );
  return { files, total };
}
