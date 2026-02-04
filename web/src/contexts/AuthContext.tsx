import {
  createContext,
  useContext,
  useState,
  useEffect,
  ReactNode,
} from "react";
import { AuthService, ApiError } from "@/api";
import type { UserInfo, LoginRequest } from "@/api";
import {
  startTokenRefreshMonitor,
  stopTokenRefreshMonitor,
} from "@/lib/api-wrapper";

interface AuthContextType {
  user: UserInfo | null;
  isAuthenticated: boolean;
  isLoading: boolean;
  login: (credentials: LoginRequest) => Promise<void>;
  logout: () => void;
  refreshUser: () => Promise<void>;
  getToken: () => string | null;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

interface AuthProviderProps {
  children: ReactNode;
}

export function AuthProvider({ children }: AuthProviderProps) {
  const [user, setUser] = useState<UserInfo | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    loadUser();
  }, []);

  // Start/stop token refresh monitoring based on auth state
  useEffect(() => {
    if (user) {
      startTokenRefreshMonitor();
    } else {
      stopTokenRefreshMonitor();
    }

    return () => {
      stopTokenRefreshMonitor();
    };
  }, [user]);

  const loadUser = async () => {
    const token = localStorage.getItem("access_token");
    if (!token) {
      setIsLoading(false);
      return;
    }

    try {
      const response = await AuthService.getCurrentUser();
      setUser(response.data);
    } catch (error) {
      console.error("Failed to load user:", error);
      if (error instanceof ApiError) {
        console.error(`API Error ${error.status}: ${error.message}`);
      }
      localStorage.removeItem("access_token");
      localStorage.removeItem("refresh_token");
      setUser(null);
    } finally {
      setIsLoading(false);
    }
  };

  const login = async (credentials: LoginRequest) => {
    try {
      const response = await AuthService.login({
        requestBody: credentials,
      });

      const { access_token, refresh_token, user: userInfo } = response.data;
      localStorage.setItem("access_token", access_token);
      localStorage.setItem("refresh_token", refresh_token);

      // If user info is included in response, use it; otherwise load it
      if (userInfo) {
        setUser(userInfo);
      } else {
        await loadUser();
      }
    } catch (error) {
      console.error("Login failed:", error);
      if (error instanceof ApiError) {
        console.error(`API Error ${error.status}: ${error.message}`);
      }
      throw error;
    }
  };

  const logout = () => {
    localStorage.removeItem("access_token");
    localStorage.removeItem("refresh_token");
    stopTokenRefreshMonitor();
    setUser(null);
  };

  const refreshUser = async () => {
    await loadUser();
  };

  const getToken = () => {
    return localStorage.getItem("access_token");
  };

  const value: AuthContextType = {
    user,
    isAuthenticated: !!user,
    isLoading,
    login,
    logout,
    refreshUser,
    getToken,
  };

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error("useAuth must be used within an AuthProvider");
  }
  return context;
}
