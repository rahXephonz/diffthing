import clsx from "clsx";
import { ChevronDown, ChevronRight } from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import { iconUrlForPath, STATUS_CLASS, STATUS_LETTER } from "../libs/fileIcon";
import type { FileDiff } from "../libs/protocol";
import { basename } from "../libs/utils";
import { Counts, Highlight } from "./ReviewChrome";

type DirNode = { name: string; path: string; dirs: DirNode[]; files: FileDiff[] };

function buildTree(files: FileDiff[]): DirNode {
  const root: DirNode = { name: "", path: "", dirs: [], files: [] };
  for (const f of [...files].sort((a, b) => a.path.localeCompare(b.path))) {
    const parts = f.path.split("/");
    let node = root;
    let prefix = "";
    for (const part of parts.slice(0, -1)) {
      prefix = prefix ? `${prefix}/${part}` : part;
      let child = node.dirs.find((d) => d.name === part);
      if (!child) {
        child = { name: part, path: prefix, dirs: [], files: [] };
        node.dirs.push(child);
      }
      node = child;
    }
    node.files.push(f);
  }
  return root;
}

function visibleFiles(files: FileDiff[], query: string): FileDiff[] {
  if (!query) return files;
  return files.filter((file) => file.path.toLowerCase().includes(query));
}

function togglePath(paths: Set<string>, path: string): Set<string> {
  const next = new Set(paths);
  if (next.has(path)) next.delete(path);
  else next.add(path);
  return next;
}

function FileRow({
  file,
  depth,
  query,
  selected,
  onSelect,
}: {
  file: FileDiff;
  depth: number;
  query: string;
  selected: boolean;
  onSelect: (path: string) => void;
}) {
  const totals = file.hunks.reduce(
    (acc, hunk) => ({ added: acc.added + hunk.added, removed: acc.removed + hunk.removed }),
    { added: 0, removed: 0 },
  );
  const iconUrl = iconUrlForPath(file.path);

  return (
    <button
      className={clsx(
        "flex w-full items-center gap-1.5 rounded bg-transparent border-none cursor-pointer py-0.5 pr-1 text-xs",
        selected ? "text-text bg-panel" : "text-muted hover:text-text",
      )}
      style={{ paddingLeft: depth * 12 + 14 }}
      title={file.path}
      onClick={() => onSelect(file.path)}
    >
      {iconUrl && <img src={iconUrl} alt="" className="w-3.5 h-3.5 shrink-0" />}
      <span
        className={clsx(
          "text-[10px] font-bold w-3 shrink-0 text-center",
          STATUS_CLASS[file.status],
        )}
        title={file.status}
      >
        {STATUS_LETTER[file.status]}
      </span>
      <span className="min-w-0 flex-1 truncate text-left">
        <Highlight text={basename(file.path)} query={query} />
      </span>
      <Counts added={totals.added} removed={totals.removed} />
    </button>
  );
}

function DirectoryContents({
  node,
  depth,
  query,
  closedPaths,
  selectedPaths,
  onToggle,
  onSelectFile,
}: {
  node: DirNode;
  depth: number;
  query: string;
  closedPaths: Set<string>;
  selectedPaths: Set<string>;
  onToggle: (path: string) => void;
  onSelectFile: (path: string) => void;
}): ReactNode {
  return (
    <>
      {node.dirs.map((directory) => {
        const isClosed = !query && closedPaths.has(directory.path);
        return (
          <div key={directory.path}>
            <button
              className="flex w-full items-center gap-1 bg-transparent border-none text-muted cursor-pointer py-0.5 text-xs hover:text-text"
              style={{ paddingLeft: depth * 12 }}
              onClick={() => onToggle(directory.path)}
            >
              {isClosed ? <ChevronRight size={12} /> : <ChevronDown size={12} />}
              <span className="truncate">
                <Highlight text={directory.name} query={query} />
              </span>
            </button>
            {!isClosed && (
              <DirectoryContents
                node={directory}
                depth={depth + 1}
                query={query}
                closedPaths={closedPaths}
                selectedPaths={selectedPaths}
                onToggle={onToggle}
                onSelectFile={onSelectFile}
              />
            )}
          </div>
        );
      })}
      {node.files.map((file) => (
        <FileRow
          key={file.path}
          file={file}
          depth={depth}
          query={query}
          selected={selectedPaths.has(file.path)}
          onSelect={onSelectFile}
        />
      ))}
    </>
  );
}

export default function FileTreeSection({
  files,
  query,
  iconsVersion,
  selectedPaths,
  onSelectFile,
}: {
  files: FileDiff[];
  query: string;
  iconsVersion: number;
  selectedPaths: Set<string>;
  onSelectFile: (path: string) => void;
}) {
  const [closed, setClosed] = useState<Set<string>>(new Set());
  const tree = useMemo(() => buildTree(visibleFiles(files, query)), [files, query]);
  void iconsVersion;

  if (files.length === 0) return null;

  const toggle = (path: string) => setClosed((current) => togglePath(current, path));

  return (
    // The tree is fixed sidebar chrome (it never scrolls away), but a big
    // diff can't be allowed to crush the scope list below — so the tree
    // itself scrolls internally past ~a third of the viewport.
    <section className="shrink-0 flex flex-col min-h-0">
      <h2 className="text-xs uppercase tracking-wider text-muted mb-1">Files</h2>
      <div className="max-h-[32vh] overflow-y-auto">
        <DirectoryContents
          node={tree}
          depth={0}
          query={query}
          closedPaths={closed}
          selectedPaths={selectedPaths}
          onToggle={toggle}
          onSelectFile={onSelectFile}
        />
      </div>
    </section>
  );
}
