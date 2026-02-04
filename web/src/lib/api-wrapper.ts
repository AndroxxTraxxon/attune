import axios from "axios";

/**
 * This module configures the generated API client to properly handle token refresh
 * and authentication errors.
 *
 * Key features:
 * - Automatic token refresh when access token expires (via axios interceptor)
 * - Proactive token refresh before expiration (every 60 seconds check)
 * - Proper redirect to login on authentication failure
 * - Distinguishes between 401 (expired/invalid token) and 403 (insufficient permissions)
 *
 * Strategy:
 * Since the generated API client creates its own axios instances, we configure
 * axios defaults globally and ensure the OpenAPI client uses our configured instance.
 */

// Helper to decode JWT and check if it's expired or about to expire
export function isTokenExpiringSoon(
  token: string,
  thresholdSeconds: number = 300,
): boolean {
  try {
    const parts = token.split(".");
    if (parts.length !== 3) return true;

    const payload = JSON.parse(atob(parts[1]));
    const exp = payload.exp;

    if (!exp) return true;

    const now = Math.floor(Date.now() / 1000);
    const timeUntilExpiry = exp - now;

    // Return true if token expires within threshold seconds (default 5 minutes)
    return timeUntilExpiry <= thresholdSeconds;
  } catch (error) {
    console.error("Failed to parse JWT:", error);
    return true;
  }
}

// Helper to check if token is completely expired (not just expiring soon)
export function isTokenExpired(token: string): boolean {
  try {
    const parts = token.split(".");
    if (parts.length !== 3) return true;

    const payload = JSON.parse(atob(parts[1]));
    const exp = payload.exp;

    if (!exp) return true;

    const now = Math.floor(Date.now() / 1000);
    return exp <= now;
  } catch (error) {
    console.error("Failed to parse JWT:", error);
    return true;
  }
}

// Helper to proactively refresh token if needed
export async function ensureValidToken(): Promise<void> {
  const token = localStorage.getItem("access_token");
  const refreshToken = localStorage.getItem("refresh_token");

  if (!token || !refreshToken) {
    return; // No tokens to refresh
  }

  // Check if token is expiring soon (within 5 minutes)
  if (isTokenExpiringSoon(token, 300)) {
    try {
      const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || "";
      const refreshUrl = API_BASE_URL
        ? `${API_BASE_URL}/auth/refresh`
        : "/auth/refresh";

      // Use base axios to avoid circular refresh attempts
      const response = await axios.post(refreshUrl, {
        refresh_token: refreshToken,
      });

      const { access_token, refresh_token: newRefreshToken } =
        response.data.data;

      localStorage.setItem("access_token", access_token);
      if (newRefreshToken) {
        localStorage.setItem("refresh_token", newRefreshToken);
      }

      // Token proactively refreshed
    } catch (error) {
      console.error("Proactive token refresh failed:", error);
      // Don't throw - let the interceptor handle it on the next request
    }
  }
}

// Set up automatic token refresh check
let tokenCheckInterval: ReturnType<typeof setInterval> | null = null;

export function startTokenRefreshMonitor(): void {
  if (tokenCheckInterval) {
    return; // Already running
  }

  // Starting token refresh monitor

  // Check token every 60 seconds
  tokenCheckInterval = setInterval(async () => {
    const token = localStorage.getItem("access_token");
    if (token && !isTokenExpired(token)) {
      await ensureValidToken();
    }
  }, 60000);

  // Also check immediately
  ensureValidToken();
}

export function stopTokenRefreshMonitor(): void {
  if (tokenCheckInterval) {
    // Stopping token refresh monitor
    clearInterval(tokenCheckInterval);
    tokenCheckInterval = null;
  }
}

// Configure axios defaults to apply to all instances
export function configureAxiosDefaults(): void {
  // Set default base URL
  const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || "";
  if (API_BASE_URL) {
    axios.defaults.baseURL = API_BASE_URL;
  }

  // Set default headers
  axios.defaults.headers.common["Content-Type"] = "application/json";

  // Copy our interceptors to the default axios instance
  // This ensures that even new axios instances inherit the behavior
  axios.interceptors.request.use(
    (config) => {
      const token = localStorage.getItem("access_token");
      if (token && config.headers) {
        config.headers.Authorization = `Bearer ${token}`;
      }
      return config;
    },
    (error) => {
      return Promise.reject(error);
    },
  );

  axios.interceptors.response.use(
    (response) => response,
    async (error) => {
      const originalRequest = error.config as any;

      // Handle 401 Unauthorized - token expired or invalid
      if (error.response?.status === 401 && !originalRequest._retry) {
        originalRequest._retry = true;

        try {
          const refreshToken = localStorage.getItem("refresh_token");
          if (!refreshToken) {
            console.warn("No refresh token available, redirecting to login");
            throw new Error("No refresh token available");
          }

          // Access token expired, attempting refresh

          const refreshUrl = API_BASE_URL
            ? `${API_BASE_URL}/auth/refresh`
            : "/auth/refresh";

          const response = await axios.post(refreshUrl, {
            refresh_token: refreshToken,
          });

          const { access_token, refresh_token: newRefreshToken } =
            response.data.data;

          localStorage.setItem("access_token", access_token);
          if (newRefreshToken) {
            localStorage.setItem("refresh_token", newRefreshToken);
          }

          // Token refreshed successfully

          // Retry original request with new token
          if (originalRequest.headers) {
            originalRequest.headers.Authorization = `Bearer ${access_token}`;
          }
          return axios(originalRequest);
        } catch (refreshError) {
          console.error(
            "Token refresh failed, clearing session and redirecting to login",
          );
          localStorage.removeItem("access_token");
          localStorage.removeItem("refresh_token");

          // Store the current path for redirect after login
          const currentPath = window.location.pathname;
          if (currentPath !== "/login") {
            sessionStorage.setItem("redirect_after_login", currentPath);
          }

          window.location.href = "/login";
          return Promise.reject(refreshError);
        }
      }

      // Handle 403 Forbidden - valid token but insufficient permissions
      if (error.response?.status === 403) {
        const enhancedError = error as any;
        enhancedError.isAuthorizationError = true;

        console.warn(
          "Access forbidden - insufficient permissions for this resource",
        );
        return Promise.reject(enhancedError);
      }

      return Promise.reject(error);
    },
  );

  // Axios defaults configured with interceptors
}

// Initialize the API wrapper
export function initializeApiWrapper(): void {
  // Initializing API wrapper

  // Configure axios defaults so all instances get the interceptors
  configureAxiosDefaults();

  // The generated API client will now inherit these interceptors

  // API wrapper initialized
}
