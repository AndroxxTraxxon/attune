import { Pause, Play, Radio } from "lucide-react";

export const DEFAULT_LIVE_LIST_MAX_ITEMS = 100;

interface LiveStreamControlProps {
  /**
   * Whether live updates are currently paused.
   */
  paused: boolean;

  /**
   * Called when the user toggles pause/resume.
   */
  onTogglePaused: () => void;

  /**
   * Whether the underlying real-time connection is active.
   */
  connected?: boolean;

  /**
   * Maximum number of records retained in the live-updated list.
   */
  maxItems?: number;

  /**
   * Optional label for the type of records being streamed.
   */
  itemLabel?: string;

  /**
   * Whether to show the retained-record count hint.
   */
  showRetentionHint?: boolean;

  /**
   * Additional classes for the wrapper.
   */
  className?: string;
}

export default function LiveStreamControl({
  paused,
  onTogglePaused,
  connected = false,
  maxItems = DEFAULT_LIVE_LIST_MAX_ITEMS,
  itemLabel = "records",
  showRetentionHint = true,
  className = "",
}: LiveStreamControlProps) {
  const statusLabel = paused ? "Paused" : connected ? "Live" : "Connecting";

  const statusClass = paused
    ? "border-amber-200 bg-amber-50 text-amber-700"
    : connected
      ? "border-green-200 bg-green-50 text-green-700"
      : "border-gray-200 bg-gray-50 text-gray-600";

  const dotClass = paused
    ? "bg-amber-500"
    : connected
      ? "bg-green-500 animate-pulse"
      : "bg-gray-400";

  return (
    <div className={`flex flex-wrap items-center gap-2 ${className}`}>
      <div
        className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-medium ${statusClass}`}
        title={
          paused
            ? "Live updates are paused for this view"
            : connected
              ? "Live updates are enabled for this view"
              : "Waiting for the live update connection"
        }
      >
        <span className={`h-1.5 w-1.5 rounded-full ${dotClass}`} />
        <Radio className="h-3.5 w-3.5" />
        <span>{statusLabel}</span>
      </div>

      <button
        type="button"
        onClick={onTogglePaused}
        className={`inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1 text-xs font-medium transition-colors ${
          paused
            ? "border-green-200 bg-white text-green-700 hover:bg-green-50"
            : "border-gray-300 bg-white text-gray-700 hover:bg-gray-50"
        }`}
        aria-pressed={paused}
      >
        {paused ? (
          <>
            <Play className="h-3.5 w-3.5" />
            Resume
          </>
        ) : (
          <>
            <Pause className="h-3.5 w-3.5" />
            Pause
          </>
        )}
      </button>

      {showRetentionHint && (
        <span className="text-xs text-gray-500">
          Retains latest {maxItems.toLocaleString()} {itemLabel}
        </span>
      )}
    </div>
  );
}
