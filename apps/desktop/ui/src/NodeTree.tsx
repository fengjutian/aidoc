import {
  Copy,
  CornerDownLeft,
  ExternalLink,
  Link2,
  Tag,
  Trash2,
  type LucideIcon,
} from "lucide-react";

import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { cn } from "@/lib/utils";
import { buildTreeIndex } from "./nodeTreeIndex";

interface NodeRow {
  id: string;
  kind: string;
  parent: string | null;
  position: number;
  content: string;
  attributes?: Record<string, string>;
}

interface NodeTreeProps {
  nodes: NodeRow[];
  activeId: string | null;
  collapsed: Record<string, boolean>;
  setCollapsed: React.Dispatch<React.SetStateAction<Record<string, boolean>>>;
  setActiveId: (id: string | null) => void;
  setLinkSource: (id: string | null) => void;
  setAttrsNodeId: (id: string | null) => void;
  onCopyId: (id: string) => void;
  onReparent: (target: string, newParent: string | null) => void;
  onDeleteNode: (target: string) => void;
  dragOverId: string | null;
  setDragOverId: (id: string | null) => void;
}

const kindIcon = (kind: string): LucideIcon => {
  if (kind === "section" || kind === "heading") return ExternalLink;
  return ExternalLink;
};

export function NodeTree(props: NodeTreeProps) {
  const { byParent, roots } = buildTreeIndex(props.nodes);
  return (
    <ul className="space-y-0.5">
      {roots.map((n) => (
        <NodeTreeRow node={n} byParent={byParent} level={0} props={props} />
      ))}
    </ul>
  );
}

interface RowProps {
  node: NodeRow;
  byParent: Map<string | null, NodeRow[]>;
  level: number;
  props: NodeTreeProps;
}

function NodeTreeRow({ node, byParent, level, props }: RowProps) {
  const Icon = kindIcon(node.kind);
  const dropTarget = props.dragOverId === node.id;
  const isCollapsed = !!props.collapsed[node.id];
  const children = byParent.get(node.id) ?? [];
  const hasChildren = children.length > 0;

  return (
    <li>
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <div
            onDragOver={(e) => {
              if (e.dataTransfer.types.includes("application/x-aidoc-node")) {
                e.preventDefault();
                e.dataTransfer.dropEffect = "move";
                props.setDragOverId(node.id);
              }
            }}
            onDragLeave={(e) => {
              if (e.currentTarget === e.target) props.setDragOverId(null);
            }}
            onDrop={(e) => {
              const draggedId = e.dataTransfer.getData("application/x-aidoc-node");
              props.setDragOverId(null);
              if (!draggedId || draggedId === node.id) return;
              // Drop onto a node → nest it under that node as last child.
              void props.onReparent(draggedId, node.id);
            }}
            className={cn(
              "group flex items-center gap-1 rounded-md pr-2 transition-colors",
              dropTarget && "ring-2 ring-primary/60 ring-offset-1 rounded-md",
            )}
            style={{ paddingLeft: `${level * 14 + 4}px` }}
          >
            {hasChildren ? (
              <button
                type="button"
                aria-label={isCollapsed ? "Expand" : "Collapse"}
                className="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-accent"
                onClick={(e) => {
                  e.stopPropagation();
                  props.setCollapsed((c) => ({ ...c, [node.id]: !c[node.id] }));
                }}
              >
                <span className="font-mono text-[10px]">
                  {isCollapsed ? "▸" : "▾"}
                </span>
              </button>
            ) : (
              <span className="inline-block h-5 w-5 shrink-0" aria-hidden />
            )}
            <button
              type="button"
              draggable
              onDragStart={(e) => {
                e.dataTransfer.setData("application/x-aidoc-node", node.id);
                e.dataTransfer.effectAllowed = "move";
              }}
              onClick={() => props.setActiveId(node.id)}
              className={cn(
                "flex flex-1 cursor-grab items-center gap-2 rounded-md px-2 py-1 text-left text-sm transition-colors hover:bg-accent active:cursor-grabbing",
                node.id === props.activeId &&
                  "bg-accent font-medium text-accent-foreground",
              )}
            >
              <Icon className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
              <span className="truncate">{node.id}</span>
              <span className="ml-auto text-[10px] uppercase text-muted-foreground">
                {node.kind}
              </span>
            </button>
          </div>
        </ContextMenuTrigger>
        <ContextMenuContent className="w-48">
          <ContextMenuItem
            onSelect={() => {
              props.setActiveId(node.id);
            }}
          >
            <ExternalLink className="text-muted-foreground" />
            Open in editor
          </ContextMenuItem>
          <ContextMenuItem
            onSelect={() => {
              void props.onCopyId(node.id);
            }}
          >
            <Copy className="text-muted-foreground" />
            Copy node ID
          </ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuItem
            onSelect={() => {
              props.setLinkSource(node.id);
            }}
          >
            <Link2 className="text-muted-foreground" />
            Link to…
          </ContextMenuItem>
          <ContextMenuItem
            onSelect={() => {
              props.setAttrsNodeId(node.id);
            }}
          >
            <Tag className="text-muted-foreground" />
            Edit attributes…
          </ContextMenuItem>
          <ContextMenuItem
            disabled={node.parent === null}
            onSelect={() => {
              void props.onReparent(node.id, null);
            }}
          >
            <CornerDownLeft className="text-muted-foreground" />
            Move to root
          </ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuItem
            disabled={node.id === "root"}
            onSelect={() => {
              void props.onDeleteNode(node.id);
            }}
            className="text-destructive focus:text-destructive"
          >
            <Trash2 />
            Delete node
          </ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>
      {hasChildren && !isCollapsed && (
        <ul className="space-y-0.5">
          {children.map((c) => (
            <NodeTreeRow
              key={c.id}
              node={c}
              byParent={byParent}
              level={level + 1}
              props={props}
            />
          ))}
        </ul>
      )}
    </li>
  );
}