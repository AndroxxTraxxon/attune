import { FormEvent, useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { ApiError, AuthService } from "@/api";
import { useAuth } from "@/contexts/AuthContext";
import apiClient from "@/lib/api-client";

interface LocationState {
  from?: {
    pathname: string;
  };
}

interface AuthSettingsResponse {
  authentication_enabled: boolean;
  local_password_enabled: boolean;
  local_password_visible_by_default: boolean;
  oidc_enabled: boolean;
  oidc_visible_by_default: boolean;
  oidc_provider_name: string | null;
  oidc_provider_label: string | null;
  oidc_provider_icon_url: string | null;
  self_registration_enabled: boolean;
}

export default function LoginPage() {
  const navigate = useNavigate();
  const location = useLocation();
  const { login: startOidcLogin, completeLogin } = useAuth();
  const [settings, setSettings] = useState<AuthSettingsResponse | null>(null);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [overrideError, setOverrideError] = useState<string | null>(null);
  const [loginError, setLoginError] = useState<string | null>(null);
  const [isLoadingSettings, setIsLoadingSettings] = useState(true);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [credentials, setCredentials] = useState({ login: "", password: "" });

  const redirectPath = sessionStorage.getItem("redirect_after_login");
  const from =
    redirectPath || (location.state as LocationState)?.from?.pathname || "/";

  useEffect(() => {
    const loadAuthSettings = async () => {
      try {
        const response = await apiClient.get<{ data: AuthSettingsResponse }>(
          "/auth/settings",
        );
        setSettings(response.data.data);
      } catch (error) {
        console.error("Failed to load auth settings:", error);
        setSettingsError("Unable to load authentication options.");
      } finally {
        setIsLoadingSettings(false);
      }
    };

    void loadAuthSettings();
  }, []);

  const authOverride = new URLSearchParams(location.search)
    .get("auth")
    ?.trim()
    .toLowerCase();

  const localEnabled = settings?.local_password_enabled ?? false;
  const oidcEnabled = settings?.oidc_enabled ?? false;
  const authEnabled = settings?.authentication_enabled ?? true;
  const providerName = settings?.oidc_provider_name?.toLowerCase() ?? null;
  const providerLabel =
    settings?.oidc_provider_label ?? settings?.oidc_provider_name ?? "SSO";

  let showLocal = settings?.local_password_visible_by_default ?? false;
  let showOidc = settings?.oidc_visible_by_default ?? false;

  if (authOverride === "direct") {
    if (localEnabled) {
      showLocal = true;
      showOidc = false;
    }
  } else if (authOverride && providerName && authOverride === providerName) {
    if (oidcEnabled) {
      showLocal = false;
      showOidc = true;
    }
  }

  useEffect(() => {
    if (!authOverride || !settings) {
      setOverrideError(null);
      return;
    }

    if (authOverride === "direct") {
      setOverrideError(
        localEnabled
          ? null
          : "Local login was requested, but it is not available on this server.",
      );
      return;
    }

    if (providerName && authOverride === providerName) {
      setOverrideError(
        oidcEnabled
          ? null
          : `${providerLabel} was requested, but it is not available on this server.`,
      );
      return;
    }

    setOverrideError(
      `Unknown authentication override '${authOverride}'. Falling back to the server defaults.`,
    );
  }, [authOverride, localEnabled, oidcEnabled, providerLabel, providerName, settings]);

  const handleOidcLogin = () => {
    sessionStorage.setItem("redirect_after_login", from);
    startOidcLogin(from);
  };

  const handleLocalLogin = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setLoginError(null);
    setIsSubmitting(true);

    try {
      const response = await AuthService.login({
        requestBody: credentials,
      });
      await completeLogin({
        accessToken: response.data.access_token,
        refreshToken: response.data.refresh_token,
      });
      sessionStorage.removeItem("redirect_after_login");
      navigate(from, { replace: true });
    } catch (error) {
      if (error instanceof ApiError) {
        setLoginError(error.message);
      } else {
        setLoginError("Failed to sign in.");
      }
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-50 py-12 px-4 sm:px-6 lg:px-8">
      <div className="max-w-md w-full">
        <div>
          <h1 className="text-center text-4xl font-bold text-gray-900">
            Attune
          </h1>
          <h2 className="mt-6 text-center text-3xl font-extrabold text-gray-900">
            Sign in to your account
          </h2>
        </div>
        <div className="mt-8 rounded-2xl border border-gray-200 bg-white p-8 shadow-sm">
          {isLoadingSettings ? (
            <div className="flex items-center gap-3 text-sm text-gray-600">
              <div className="h-4 w-4 animate-spin rounded-full border-2 border-gray-300 border-t-gray-900" />
              Loading authentication options...
            </div>
          ) : (
            <>
              {settingsError ? (
                <div className="rounded-lg bg-red-50 p-4 text-sm text-red-700">
                  {settingsError}
                </div>
              ) : null}

              {overrideError ? (
                <div className="mb-4 rounded-lg bg-amber-50 p-4 text-sm text-amber-800">
                  {overrideError}
                </div>
              ) : null}

              {!authEnabled ? (
                <div className="rounded-lg bg-amber-50 p-4 text-sm text-amber-800">
                  Authentication is disabled in the current server
                  configuration.
                </div>
              ) : null}

              {authEnabled && showLocal ? (
                <form className="space-y-4" onSubmit={handleLocalLogin}>
                  <div>
                    <label
                      htmlFor="login"
                      className="block text-sm font-medium text-gray-700"
                    >
                      Login
                    </label>
                    <input
                      id="login"
                      type="text"
                      autoComplete="username"
                      value={credentials.login}
                      onChange={(event) =>
                        setCredentials((current) => ({
                          ...current,
                          login: event.target.value,
                        }))
                      }
                      className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm text-gray-900 shadow-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
                      required
                    />
                  </div>
                  <div>
                    <label
                      htmlFor="password"
                      className="block text-sm font-medium text-gray-700"
                    >
                      Password
                    </label>
                    <input
                      id="password"
                      type="password"
                      autoComplete="current-password"
                      value={credentials.password}
                      onChange={(event) =>
                        setCredentials((current) => ({
                          ...current,
                          password: event.target.value,
                        }))
                      }
                      className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2 text-sm text-gray-900 shadow-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
                      required
                    />
                  </div>
                  {loginError ? (
                    <div className="rounded-lg bg-red-50 p-4 text-sm text-red-700">
                      {loginError}
                    </div>
                  ) : null}
                  <button
                    type="submit"
                    disabled={isSubmitting}
                    className="w-full rounded-md bg-gray-900 px-4 py-2 text-sm font-medium text-white hover:bg-gray-800 focus:outline-none focus:ring-2 focus:ring-gray-900 focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    {isSubmitting ? "Signing in..." : "Sign in"}
                  </button>
                </form>
              ) : null}

              {authEnabled && showLocal && showOidc ? (
                <div className="my-6 flex items-center gap-3 text-xs uppercase tracking-[0.24em] text-gray-400">
                  <div className="h-px flex-1 bg-gray-200" />
                  or
                  <div className="h-px flex-1 bg-gray-200" />
                </div>
              ) : null}

              {authEnabled && showOidc ? (
                <>
                  <p className="mb-4 text-sm text-gray-600">
                    Continue with your configured single sign-on provider.
                  </p>
                  <button
                    type="button"
                    onClick={handleOidcLogin}
                    className="group relative flex w-full items-center justify-center gap-3 rounded-md border border-transparent bg-indigo-600 px-4 py-2 text-sm font-medium text-white hover:bg-indigo-700 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2"
                  >
                    {settings?.oidc_provider_icon_url ? (
                      <img
                        src={settings.oidc_provider_icon_url}
                        alt=""
                        className="h-5 w-5 rounded-sm bg-white/10 object-contain"
                      />
                    ) : null}
                    <span>Continue with {providerLabel}</span>
                  </button>
                </>
              ) : null}

              {!settingsError && authEnabled && !showLocal && !showOidc ? (
                <div className="rounded-lg bg-amber-50 p-4 text-sm text-amber-800">
                  No login method is shown by default for this server. Use
                  `?auth=direct`
                  {providerName ? ` or ?auth=${providerName}` : ""} to choose
                  a specific method.
                </div>
              ) : null}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
