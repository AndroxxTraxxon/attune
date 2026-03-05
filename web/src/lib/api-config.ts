import { OpenAPI } from "../api";
import { apiClient } from "./api-client";

declare global {
  interface Window {
    __ATTUNE_CONFIG__?: {
      API_BASE_URL: string;
      WITH_CREDENTIALS: boolean;
    };
  }
}

// Configure the OpenAPI client
// Use empty string to make requests relative to current origin (uses Vite proxy)
const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || "";

// API configuration (silent - check window.__ATTUNE_CONFIG__ for debug info)
if (import.meta.env.DEV) {
  window.__ATTUNE_CONFIG__ = {
    API_BASE_URL,
    WITH_CREDENTIALS: true,
  };
}

OpenAPI.BASE = API_BASE_URL;
OpenAPI.WITH_CREDENTIALS = true;
OpenAPI.CREDENTIALS = "include";

// Configure token resolution - this will be called for each authenticated request
OpenAPI.TOKEN = async (): Promise<string> => {
  const token = localStorage.getItem("access_token");
  if (!token) {
    return "";
  }
  return token;
};

// Optional: Configure custom headers
OpenAPI.HEADERS = {
  "Content-Type": "application/json",
};

// Export the configured axios client so the generated API can use it
export { OpenAPI, apiClient };
