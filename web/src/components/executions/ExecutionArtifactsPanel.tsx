import { useState, useMemo, useEffect, useCallback, useRef } from "react";
import { formatDistanceToNow } from "date-fns";
import {
  ChevronDown,
  ChevronRight,
  FileText,
  FileImage,
  File,
  BarChart3,
  Link as LinkIcon,
  Table2,
  Package,
  Loader2,
  Download,
  Eye,
  X,
  Radio,
} from "lucide-react";
import {
  useExecutionArtifacts,
  useArtifact,
  type ArtifactSummary,
  type ArtifactType,
} from "@/hooks/useArtifacts";
import { useArtifactStream } from "@/hooks/useArtifactStream";
import { OpenAPI } from "@/api/core/OpenAPI";

interface ExecutionArtifactsPanelProps {
  executionId: number;
  /** Whether the execution is still running (enables polling) */
  isRunning?: boolean;
  defaultCollapsed?: boolean;
}

function getArtifactTypeIcon(type: ArtifactType) {
  switch (type) {
    case "file_text":
      return <FileText className="h-4 w-4 text-blue-500" />;
    case "file_image":
      return <FileImage className="h-4 w-4 text-purple-500" />;
    case "file_binary":
      return <File className="h-4 w-4 text-gray-500" />;
    case "file_datatable":
      return <Table2 className="h-4 w-4 text-green-500" />;
    case "progress":
      return <BarChart3 className="h-4 w-4 text-amber-500" />;
    case "url":
      return <LinkIcon className="h-4 w-4 text-cyan-500" />;
    case "other":
    default:
      return <Package className="h-4 w-4 text-gray-400" />;
  }
}

function getArtifactTypeBadge(type: ArtifactType): {
  label: string;
  classes: string;
} {
  switch (type) {
    case "file_text":
      return { label: "Text File", classes: "bg-blue-100 text-blue-800" };
    case "file_image":
      return { label: "Image", classes: "bg-purple-100 text-purple-800" };
    case "file_binary":
      return { label: "Binary", classes: "bg-gray-100 text-gray-800" };
    case "file_datatable":
      return { label: "Data Table", classes: "bg-green-100 text-green-800" };
    case "progress":
      return { label: "Progress", classes: "bg-amber-100 text-amber-800" };
    case "url":
      return { label: "URL", classes: "bg-cyan-100 text-cyan-800" };
    case "other":
    default:
      return { label: "Other", classes: "bg-gray-100 text-gray-700" };
  }
}

function formatBytes(bytes: number | null): string {
  if (bytes == null || bytes === 0) return "—";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Download the latest version of an artifact using a fetch with auth token. */
async function downloadArtifact(artifactId: number, artifactRef: string) {
  const token = localStorage.getItem("access_token");
  const url = `${OpenAPI.BASE}/api/v1/artifacts/${artifactId}/download`;

  const response = await fetch(url, {
    headers: {
      Authorization: `Bearer ${token}`,
    },
  });

  if (!response.ok) {
    console.error(`Download failed: ${response.status} ${response.statusText}`);
    return;
  }

  // Extract filename from Content-Disposition header or fall back to ref
  const disposition = response.headers.get("Content-Disposition");
  let filename = artifactRef.replace(/\./g, "_") + ".bin";
  if (disposition) {
    const match = disposition.match(/filename="?([^"]+)"?/);
    if (match) filename = match[1];
  }

  const blob = await response.blob();
  const blobUrl = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = blobUrl;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(blobUrl);
}

// ============================================================================
// Text File Artifact Detail
// ============================================================================

interface TextFileDetailProps {
  artifactId: number;
  artifactName: string | null;
  isRunning?: boolean;
  onClose: () => void;
}

function TextFileDetail({
  artifactId,
  artifactName,
  isRunning = false,
  onClose,
}: TextFileDetailProps) {
  const [content, setContent] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [isLoadingContent, setIsLoadingContent] = useState(true);
  const [isStreaming, setIsStreaming] = useState(false);
  const [isWaiting, setIsWaiting] = useState(false);
  const [streamDone, setStreamDone] = useState(false);
  const preRef = useRef<HTMLPreElement>(null);
  const eventSourceRef = useRef<EventSource | null>(null);
  // Track whether the user has scrolled away from the bottom so we can
  // auto-scroll only when they're already at the end.
  const userScrolledAwayRef = useRef(false);

  // Auto-scroll the <pre> to the bottom when new content arrives,
  // unless the user has deliberately scrolled up.
  const scrollToBottom = useCallback(() => {
    const el = preRef.current;
    if (el && !userScrolledAwayRef.current) {
      el.scrollTop = el.scrollHeight;
    }
  }, []);

  // Detect whether the user has scrolled away from the bottom.
  const handleScroll = useCallback(() => {
    const el = preRef.current;
    if (!el) return;
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
    userScrolledAwayRef.current = !atBottom;
  }, []);

  // ---- SSE streaming path (used when execution is running) ----
  useEffect(() => {
    if (!isRunning) return;

    const token = localStorage.getItem("access_token");
    if (!token) {
      setLoadError("No authentication token available");
      setIsLoadingContent(false);
      return;
    }

    const url = `${OpenAPI.BASE}/api/v1/artifacts/${artifactId}/stream?token=${encodeURIComponent(token)}`;
    const es = new EventSource(url);
    eventSourceRef.current = es;
    setIsStreaming(true);
    setStreamDone(false);

    es.addEventListener("waiting", (e: MessageEvent) => {
      setIsWaiting(true);
      setIsLoadingContent(false);
      // If the message says "File found", the next event will be content
      if (e.data?.includes("File found")) {
        setIsWaiting(false);
      }
    });

    es.addEventListener("content", (e: MessageEvent) => {
      setContent(e.data);
      setLoadError(null);
      setIsLoadingContent(false);
      setIsWaiting(false);
      // Scroll after React renders the new content
      requestAnimationFrame(scrollToBottom);
    });

    es.addEventListener("append", (e: MessageEvent) => {
      setContent((prev) => (prev ?? "") + e.data);
      setLoadError(null);
      requestAnimationFrame(scrollToBottom);
    });

    es.addEventListener("done", () => {
      setStreamDone(true);
      setIsStreaming(false);
      es.close();
    });

    es.addEventListener("error", (e: MessageEvent) => {
      // SSE spec fires generic error events on connection close.
      // Only show user-facing errors if the server sent an explicit event.
      if (e.data) {
        setLoadError(e.data);
      }
    });

    es.onerror = () => {
      // Connection dropped — EventSource will auto-reconnect, but if it
      // reaches CLOSED state we fall back to the download endpoint.
      if (es.readyState === EventSource.CLOSED) {
        setIsStreaming(false);
        // If we never got any content via SSE, fall back to download
        setContent((prev) => {
          if (prev === null) {
            // Will be handled by the fetch fallback below
          }
          return prev;
        });
      }
    };

    return () => {
      es.close();
      eventSourceRef.current = null;
      setIsStreaming(false);
    };
  }, [artifactId, isRunning, scrollToBottom]);

  // ---- Fetch fallback (used when not running, or SSE never connected) ----
  const fetchContent = useCallback(async () => {
    const token = localStorage.getItem("access_token");
    const url = `${OpenAPI.BASE}/api/v1/artifacts/${artifactId}/download`;
    try {
      const response = await fetch(url, {
        headers: { Authorization: `Bearer ${token}` },
      });
      if (!response.ok) {
        setLoadError(`HTTP ${response.status}: ${response.statusText}`);
        setIsLoadingContent(false);
        return;
      }
      const text = await response.text();
      setContent(text);
      setLoadError(null);
    } catch (e) {
      setLoadError(e instanceof Error ? e.message : "Unknown error");
    } finally {
      setIsLoadingContent(false);
    }
  }, [artifactId]);

  // When NOT running (execution completed), use download endpoint once.
  useEffect(() => {
    if (isRunning) return;
    fetchContent();
  }, [isRunning, fetchContent]);

  return (
    <div className="border border-blue-200 bg-blue-50/50 rounded-lg p-4 mt-2">
      <div className="flex items-center justify-between mb-3">
        <h4 className="text-sm font-semibold text-blue-900 flex items-center gap-2">
          <FileText className="h-4 w-4" />
          {artifactName ?? "Text File"}
        </h4>
        <div className="flex items-center gap-2">
          {isStreaming && !streamDone && (
            <div className="flex items-center gap-1 text-xs text-green-600">
              <Radio className="h-3 w-3 animate-pulse" />
              <span>Streaming</span>
            </div>
          )}
          {streamDone && (
            <span className="text-xs text-gray-500">Stream complete</span>
          )}
          {isWaiting && (
            <div className="flex items-center gap-1 text-xs text-amber-600">
              <Loader2 className="h-3 w-3 animate-spin" />
              <span>Waiting for file…</span>
            </div>
          )}
          <button
            onClick={onClose}
            className="text-gray-400 hover:text-gray-600 p-1 rounded"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
      </div>

      {isLoadingContent && !isWaiting && (
        <div className="flex items-center gap-2 py-2 text-sm text-gray-500">
          <Loader2 className="h-4 w-4 animate-spin" />
          Loading content…
        </div>
      )}

      {loadError && (
        <p className="text-xs text-red-600 italic">Error: {loadError}</p>
      )}

      {!isLoadingContent && !loadError && content !== null && (
        <pre
          ref={preRef}
          onScroll={handleScroll}
          className="max-h-64 overflow-y-auto bg-gray-900 text-gray-100 rounded p-3 text-xs font-mono whitespace-pre-wrap break-all"
        >
          {content || <span className="text-gray-500 italic">(empty)</span>}
        </pre>
      )}

      {isWaiting && content === null && !loadError && (
        <div className="bg-gray-900 rounded p-3 text-xs text-gray-500 italic">
          Waiting for the worker to write the file…
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Progress Artifact Detail
// ============================================================================

interface ProgressDetailProps {
  artifactId: number;
  isRunning?: boolean;
  onClose: () => void;
}

function ProgressDetail({
  artifactId,
  isRunning = false,
  onClose,
}: ProgressDetailProps) {
  const { data: artifactData, isLoading } = useArtifact(artifactId, isRunning);
  const artifact = artifactData?.data;

  const progressEntries = useMemo(() => {
    if (!artifact?.data || !Array.isArray(artifact.data)) return [];
    return artifact.data as Array<Record<string, unknown>>;
  }, [artifact]);

  const latestEntry =
    progressEntries.length > 0
      ? progressEntries[progressEntries.length - 1]
      : null;
  const latestPercent =
    latestEntry && typeof latestEntry.percent === "number"
      ? latestEntry.percent
      : null;

  return (
    <div className="border border-amber-200 bg-amber-50/50 rounded-lg p-4 mt-2">
      <div className="flex items-center justify-between mb-3">
        <h4 className="text-sm font-semibold text-amber-900 flex items-center gap-2">
          <BarChart3 className="h-4 w-4" />
          {artifact?.name ?? "Progress"}
        </h4>
        <button
          onClick={onClose}
          className="text-gray-400 hover:text-gray-600 p-1 rounded"
        >
          <X className="h-4 w-4" />
        </button>
      </div>

      {isLoading && (
        <div className="flex items-center gap-2 py-2 text-sm text-gray-500">
          <Loader2 className="h-4 w-4 animate-spin" />
          Loading progress…
        </div>
      )}

      {!isLoading && latestPercent != null && (
        <div className="mb-3">
          <div className="flex items-center justify-between text-xs text-gray-600 mb-1">
            <span>
              {latestEntry?.message
                ? String(latestEntry.message)
                : `${latestPercent}%`}
            </span>
            <span className="font-mono">{latestPercent}%</span>
          </div>
          <div className="w-full bg-gray-200 rounded-full h-2.5">
            <div
              className="bg-amber-500 h-2.5 rounded-full transition-all duration-300"
              style={{ width: `${Math.min(latestPercent, 100)}%` }}
            />
          </div>
        </div>
      )}

      {!isLoading && progressEntries.length > 0 && (
        <div className="max-h-48 overflow-y-auto">
          <table className="w-full text-xs">
            <thead>
              <tr className="text-left text-gray-500 border-b border-amber-200">
                <th className="pb-1 pr-2">#</th>
                <th className="pb-1 pr-2">%</th>
                <th className="pb-1 pr-2">Message</th>
                <th className="pb-1">Time</th>
              </tr>
            </thead>
            <tbody>
              {progressEntries.map((entry, idx) => (
                <tr
                  key={idx}
                  className="border-b border-amber-100 last:border-0"
                >
                  <td className="py-1 pr-2 text-gray-400 font-mono">
                    {typeof entry.iteration === "number"
                      ? entry.iteration
                      : idx + 1}
                  </td>
                  <td className="py-1 pr-2 font-mono">
                    {typeof entry.percent === "number"
                      ? `${entry.percent}%`
                      : "—"}
                  </td>
                  <td className="py-1 pr-2 text-gray-700 truncate max-w-[200px]">
                    {entry.message ? String(entry.message) : "—"}
                  </td>
                  <td className="py-1 text-gray-400 whitespace-nowrap">
                    {entry.timestamp
                      ? formatDistanceToNow(new Date(String(entry.timestamp)), {
                          addSuffix: true,
                        })
                      : "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {!isLoading && progressEntries.length === 0 && (
        <p className="text-xs text-gray-500 italic">No progress entries yet.</p>
      )}
    </div>
  );
}

// ============================================================================
// Main Panel
// ============================================================================

export default function ExecutionArtifactsPanel({
  executionId,
  isRunning = false,
  defaultCollapsed = false,
}: ExecutionArtifactsPanelProps) {
  const [isCollapsed, setIsCollapsed] = useState(defaultCollapsed);
  const [expandedProgressId, setExpandedProgressId] = useState<number | null>(
    null,
  );
  const [expandedTextFileId, setExpandedTextFileId] = useState<number | null>(
    null,
  );

  // Subscribe to real-time artifact notifications for this execution.
  // WebSocket-driven cache invalidation replaces most of the polling need,
  // but we keep polling as a fallback (staleTime/refetchInterval in the hook).
  useArtifactStream({ executionId, enabled: isRunning });

  const { data, isLoading, error } = useExecutionArtifacts(
    executionId,
    isRunning,
  );

  const artifacts: ArtifactSummary[] = useMemo(() => {
    return data?.data ?? [];
  }, [data]);

  const summary = useMemo(() => {
    const total = artifacts.length;
    const files = artifacts.filter((a) =>
      ["file_text", "file_binary", "file_image", "file_datatable"].includes(
        a.type,
      ),
    ).length;
    const progress = artifacts.filter((a) => a.type === "progress").length;
    const other = total - files - progress;
    return { total, files, progress, other };
  }, [artifacts]);

  // Don't render anything if there are no artifacts and we're not loading
  if (!isLoading && artifacts.length === 0 && !error) {
    return null;
  }

  return (
    <div className="bg-white shadow rounded-lg">
      {/* Header */}
      <button
        onClick={() => setIsCollapsed(!isCollapsed)}
        className="w-full flex items-center justify-between p-6 text-left hover:bg-gray-50 rounded-lg transition-colors"
      >
        <div className="flex items-center gap-3">
          {isCollapsed ? (
            <ChevronRight className="h-5 w-5 text-gray-400" />
          ) : (
            <ChevronDown className="h-5 w-5 text-gray-400" />
          )}
          <Package className="h-5 w-5 text-indigo-500" />
          <h2 className="text-xl font-semibold">Artifacts</h2>
          {!isLoading && (
            <span className="text-sm text-gray-500">
              ({summary.total} artifact{summary.total !== 1 ? "s" : ""})
            </span>
          )}
          {isRunning && (
            <div className="flex items-center gap-1.5 text-xs text-blue-600">
              <Loader2 className="h-3 w-3 animate-spin" />
              <span>Live</span>
            </div>
          )}
        </div>

        {/* Summary badges */}
        <div className="flex items-center gap-2">
          {summary.files > 0 && (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-blue-100 text-blue-800">
              <FileText className="h-3 w-3" />
              {summary.files}
            </span>
          )}
          {summary.progress > 0 && (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-amber-100 text-amber-800">
              <BarChart3 className="h-3 w-3" />
              {summary.progress}
            </span>
          )}
          {summary.other > 0 && (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-700">
              {summary.other}
            </span>
          )}
        </div>
      </button>

      {/* Content */}
      {!isCollapsed && (
        <div className="px-6 pb-6">
          {isLoading && (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="h-5 w-5 animate-spin text-gray-400" />
              <span className="ml-2 text-sm text-gray-500">
                Loading artifacts…
              </span>
            </div>
          )}

          {error && (
            <div className="bg-red-50 border border-red-200 text-red-700 px-4 py-3 rounded text-sm">
              Error loading artifacts:{" "}
              {error instanceof Error ? error.message : "Unknown error"}
            </div>
          )}

          {!isLoading && !error && artifacts.length > 0 && (
            <div className="space-y-2">
              {/* Column headers */}
              <div className="grid grid-cols-12 gap-3 px-3 py-2 text-xs font-medium text-gray-500 uppercase tracking-wider border-b border-gray-100">
                <div className="col-span-1">Type</div>
                <div className="col-span-4">Name</div>
                <div className="col-span-3">Ref</div>
                <div className="col-span-1">Size</div>
                <div className="col-span-2">Created</div>
                <div className="col-span-1">Actions</div>
              </div>

              {/* Artifact rows */}
              {artifacts.map((artifact) => {
                const badge = getArtifactTypeBadge(artifact.type);
                const isProgress = artifact.type === "progress";
                const isTextFile = artifact.type === "file_text";
                const isFile = [
                  "file_text",
                  "file_binary",
                  "file_image",
                  "file_datatable",
                ].includes(artifact.type);
                const isProgressExpanded = expandedProgressId === artifact.id;
                const isTextExpanded = expandedTextFileId === artifact.id;

                return (
                  <div key={artifact.id}>
                    <div
                      className={`grid grid-cols-12 gap-3 px-3 py-3 rounded-lg hover:bg-gray-50 transition-colors items-center ${
                        isProgress || isTextFile ? "cursor-pointer" : ""
                      }`}
                      onClick={() => {
                        if (isProgress) {
                          setExpandedProgressId(
                            isProgressExpanded ? null : artifact.id,
                          );
                          setExpandedTextFileId(null);
                        } else if (isTextFile) {
                          setExpandedTextFileId(
                            isTextExpanded ? null : artifact.id,
                          );
                          setExpandedProgressId(null);
                        }
                      }}
                    >
                      {/* Type icon */}
                      <div className="col-span-1 flex items-center">
                        {getArtifactTypeIcon(artifact.type)}
                      </div>

                      {/* Name */}
                      <div className="col-span-4 flex items-center gap-2 min-w-0">
                        <span
                          className="text-sm font-medium text-gray-900 truncate"
                          title={artifact.name ?? artifact.ref}
                        >
                          {artifact.name ?? artifact.ref}
                        </span>
                        <span
                          className={`inline-flex px-1.5 py-0.5 rounded text-[10px] font-medium flex-shrink-0 ${badge.classes}`}
                        >
                          {badge.label}
                        </span>
                      </div>

                      {/* Ref */}
                      <div className="col-span-3 min-w-0">
                        <span
                          className="text-xs text-gray-500 truncate block font-mono"
                          title={artifact.ref}
                        >
                          {artifact.ref}
                        </span>
                      </div>

                      {/* Size */}
                      <div className="col-span-1 text-sm text-gray-500">
                        {formatBytes(artifact.size_bytes)}
                      </div>

                      {/* Created */}
                      <div className="col-span-2 text-xs text-gray-500">
                        {formatDistanceToNow(new Date(artifact.created), {
                          addSuffix: true,
                        })}
                      </div>

                      {/* Actions */}
                      <div
                        className="col-span-1 flex items-center gap-1"
                        onClick={(e) => e.stopPropagation()}
                      >
                        {isProgress && (
                          <button
                            onClick={() => {
                              setExpandedProgressId(
                                isProgressExpanded ? null : artifact.id,
                              );
                              setExpandedTextFileId(null);
                            }}
                            className="p-1 rounded hover:bg-gray-200 text-gray-500 hover:text-amber-600"
                            title="View progress"
                          >
                            <Eye className="h-4 w-4" />
                          </button>
                        )}
                        {isTextFile && (
                          <button
                            onClick={() => {
                              setExpandedTextFileId(
                                isTextExpanded ? null : artifact.id,
                              );
                              setExpandedProgressId(null);
                            }}
                            className="p-1 rounded hover:bg-gray-200 text-gray-500 hover:text-blue-600"
                            title="Preview text content"
                          >
                            <Eye className="h-4 w-4" />
                          </button>
                        )}
                        {isFile && (
                          <button
                            onClick={() =>
                              downloadArtifact(artifact.id, artifact.ref)
                            }
                            className="p-1 rounded hover:bg-gray-200 text-gray-500 hover:text-blue-600"
                            title="Download latest version"
                          >
                            <Download className="h-4 w-4" />
                          </button>
                        )}
                      </div>
                    </div>

                    {/* Expanded progress detail */}
                    {isProgress && isProgressExpanded && (
                      <div className="px-3">
                        <ProgressDetail
                          artifactId={artifact.id}
                          isRunning={isRunning}
                          onClose={() => setExpandedProgressId(null)}
                        />
                      </div>
                    )}

                    {/* Expanded text file preview */}
                    {isTextFile && isTextExpanded && (
                      <div className="px-3">
                        <TextFileDetail
                          artifactId={artifact.id}
                          artifactName={artifact.name}
                          isRunning={isRunning}
                          onClose={() => setExpandedTextFileId(null)}
                        />
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
