import { AlertCircle, ShieldAlert } from "lucide-react";

interface ErrorDisplayProps {
  error: Error | unknown;
  title?: string;
  showRetry?: boolean;
  onRetry?: () => void;
}

/**
 * ErrorDisplay component for consistent error messaging across the app.
 *
 * Distinguishes between:
 * - 403 Forbidden (insufficient permissions)
 * - 401 Unauthorized (handled by interceptor, but just in case)
 * - Other errors (network, server, etc.)
 */
export default function ErrorDisplay({
  error,
  title,
  showRetry = false,
  onRetry,
}: ErrorDisplayProps) {
  // Type guard for axios errors
  const isAxiosError = (err: any): boolean => {
    return err?.response?.status !== undefined;
  };

  // Check if this is a 403 (Forbidden) error
  const is403Error = (err: any): boolean => {
    return (
      err?.response?.status === 403 ||
      err?.isAuthorizationError === true
    );
  };

  // Check if this is a 401 (Unauthorized) error
  const is401Error = (err: any): boolean => {
    return err?.response?.status === 401;
  };

  // Extract error message
  const getErrorMessage = (err: any): string => {
    if (err?.response?.data?.message) {
      return err.response.data.message;
    }
    if (err?.message) {
      return err.message;
    }
    return "An unexpected error occurred";
  };

  // Determine error type and render appropriate UI
  if (is403Error(error)) {
    return (
      <div className="bg-amber-50 border border-amber-200 rounded-lg p-6">
        <div className="flex items-start">
          <div className="flex-shrink-0">
            <ShieldAlert className="h-6 w-6 text-amber-600" />
          </div>
          <div className="ml-3 flex-1">
            <h3 className="text-lg font-semibold text-amber-900">
              {title || "Access Denied"}
            </h3>
            <p className="mt-2 text-sm text-amber-800">
              You do not have permission to access this resource. Your current
              role or permissions do not allow this action.
            </p>
            <p className="mt-2 text-sm text-amber-700">
              If you believe you should have access, please contact your
              system administrator.
            </p>
          </div>
        </div>
      </div>
    );
  }

  if (is401Error(error)) {
    return (
      <div className="bg-red-50 border border-red-200 rounded-lg p-6">
        <div className="flex items-start">
          <div className="flex-shrink-0">
            <AlertCircle className="h-6 w-6 text-red-600" />
          </div>
          <div className="ml-3 flex-1">
            <h3 className="text-lg font-semibold text-red-900">
              {title || "Authentication Required"}
            </h3>
            <p className="mt-2 text-sm text-red-800">
              Your session has expired or is invalid. Please log in again.
            </p>
            <p className="mt-2 text-sm text-red-700">
              You will be redirected to the login page automatically.
            </p>
          </div>
        </div>
      </div>
    );
  }

  // Generic error display
  return (
    <div className="bg-red-50 border border-red-200 rounded-lg p-6">
      <div className="flex items-start">
        <div className="flex-shrink-0">
          <AlertCircle className="h-6 w-6 text-red-600" />
        </div>
        <div className="ml-3 flex-1">
          <h3 className="text-lg font-semibold text-red-900">
            {title || "Error"}
          </h3>
          <p className="mt-2 text-sm text-red-800">
            {getErrorMessage(error)}
          </p>
          {isAxiosError(error) && (error as any)?.response?.status && (
            <p className="mt-1 text-xs text-red-600">
              Status Code: {(error as any).response.status}
            </p>
          )}
          {showRetry && onRetry && (
            <button
              onClick={onRetry}
              className="mt-4 inline-flex items-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-red-700 bg-red-100 hover:bg-red-200 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-red-500"
            >
              Try Again
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
