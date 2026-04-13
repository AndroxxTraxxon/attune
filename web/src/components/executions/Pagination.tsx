import { memo } from "react";

interface PaginationProps {
  page: number;
  setPage: (page: number) => void;
  pageSize: number;
  itemCount: number;
  total?: number;
  hasPrevious?: boolean;
  hasNext?: boolean;
  itemLabel?: string;
  floating?: boolean;
  floatingOffsetPx?: number;
}

function computeRange(page: number, pageSize: number, itemCount: number) {
  const start = (page - 1) * pageSize + 1;
  const end = start + Math.max(itemCount - 1, 0);
  return { start, end };
}

const Pagination = memo(function Pagination({
  page,
  setPage,
  pageSize,
  itemCount,
  total,
  hasPrevious,
  hasNext,
  itemLabel = "items",
  floating = false,
  floatingOffsetPx = 0,
}: PaginationProps) {
  const totalPages =
    typeof total === "number" ? Math.ceil(total / pageSize) : undefined;
  const canGoPrevious = hasPrevious ?? page > 1;
  const canGoNext =
    hasNext ?? (totalPages !== undefined ? page < totalPages : false);
  if (!canGoPrevious && !canGoNext) return null;

  const { start, end } = computeRange(page, pageSize, itemCount);
  const summary =
    typeof total === "number" ? (
      <p className="text-sm text-gray-700">
        Showing <span className="font-medium">{start}</span> to{" "}
        <span className="font-medium">{end}</span> of{" "}
        <span className="font-medium">{total}</span> {itemLabel}
      </p>
    ) : canGoNext ? (
      <p className="text-sm text-gray-700">
        Showing <span className="font-medium">{start}</span> to{" "}
        <span className="font-medium">{end}</span> {itemLabel}, more available
      </p>
    ) : (
      <p className="text-sm text-gray-700">
        Showing <span className="font-medium">{start}</span> to{" "}
        <span className="font-medium">{end}</span> {itemLabel}
      </p>
    );

  const containerClassName = floating
    ? "fixed bottom-6 left-1/2 z-30 w-[min(calc(100vw-2rem),48rem)] -translate-x-1/2 rounded-xl border-2 border-gray-300 bg-white/95 px-4 py-3 shadow-2xl shadow-black/20 ring-1 ring-black/10 backdrop-blur"
    : "bg-gray-50 px-6 py-4 flex items-center justify-between border-t border-gray-200";
  const containerStyle = floating
    ? {
        transform: `translateX(calc(-50% - ${floatingOffsetPx}px))`,
      }
    : undefined;

  return (
    <div className={containerClassName} style={containerStyle}>
      <div className="flex-1 flex justify-between sm:hidden">
        <button
          onClick={() => setPage(page - 1)}
          disabled={!canGoPrevious}
          className="relative inline-flex items-center px-4 py-2 border border-gray-300 text-sm font-medium rounded-md text-gray-700 bg-white hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          Previous
        </button>
        <button
          onClick={() => setPage(page + 1)}
          disabled={!canGoNext}
          className="ml-3 relative inline-flex items-center px-4 py-2 border border-gray-300 text-sm font-medium rounded-md text-gray-700 bg-white hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          Next
        </button>
      </div>
      <div className="hidden sm:flex sm:flex-1 sm:items-center sm:justify-between sm:gap-6">
        <div>{summary}</div>
        <div>
          <nav className="relative z-0 inline-flex rounded-md shadow-sm -space-x-px">
            <button
              onClick={() => setPage(page - 1)}
              disabled={!canGoPrevious}
              className="relative inline-flex items-center px-2 py-2 rounded-l-md border border-gray-300 bg-white text-sm font-medium text-gray-500 hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Previous
            </button>
            <button
              onClick={() => setPage(page + 1)}
              disabled={!canGoNext}
              className="relative inline-flex items-center px-2 py-2 rounded-r-md border border-gray-300 bg-white text-sm font-medium text-gray-500 hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Next
            </button>
          </nav>
        </div>
      </div>
    </div>
  );
});

Pagination.displayName = "Pagination";

export default Pagination;
