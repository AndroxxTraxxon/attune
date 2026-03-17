import React, { useState, useEffect } from "react";
import { Link, Outlet, useNavigate, useLocation } from "react-router-dom";
import { useAuth } from "@/contexts/AuthContext";
import {
  Package,
  ChevronLeft,
  ChevronRight,
  User,
  LogOut,
  CirclePlay,
  CircleArrowRight,
  SquareArrowRight,
  SquarePlay,
  SquareDot,
  CircleDot,
  SquareAsterisk,
  KeyRound,
  Home,
  FolderArchive,
  TerminalSquare,
} from "lucide-react";

// Color mappings for navigation items — defined outside component for stable reference
const colorClasses = {
  gray: {
    inactive: "text-gray-300 hover:text-white hover:bg-gray-800",
    active: "bg-gray-800 text-white",
    icon: "text-gray-400",
  },
  cyan: {
    inactive: "text-cyan-300 hover:text-cyan-100 hover:bg-cyan-950/30",
    active: "bg-cyan-950/50 text-cyan-100 shadow-lg shadow-cyan-900/50",
    icon: "text-cyan-400",
  },
  blue: {
    inactive: "text-blue-300 hover:text-blue-100 hover:bg-blue-950/30",
    active: "bg-blue-950/50 text-blue-100 shadow-lg shadow-blue-900/50",
    icon: "text-blue-400",
  },
  violet: {
    inactive: "text-violet-300 hover:text-violet-100 hover:bg-violet-950/30",
    active: "bg-violet-950/50 text-violet-100 shadow-lg shadow-violet-900/50",
    icon: "text-violet-400",
  },
  purple: {
    inactive: "text-purple-300 hover:text-purple-100 hover:bg-purple-950/30",
    active: "bg-purple-950/50 text-purple-100 shadow-lg shadow-purple-900/50",
    icon: "text-purple-400",
  },
  fuchsia: {
    inactive: "text-fuchsia-300 hover:text-fuchsia-100 hover:bg-fuchsia-950/30",
    active:
      "bg-fuchsia-950/50 text-fuchsia-100 shadow-lg shadow-fuchsia-900/50",
    icon: "text-fuchsia-400",
  },
  rose: {
    inactive: "text-rose-300 hover:text-rose-100 hover:bg-rose-950/30",
    active: "bg-rose-950/50 text-rose-100 shadow-lg shadow-rose-900/50",
    icon: "text-rose-400",
  },
  orange: {
    inactive: "text-orange-300 hover:text-orange-100 hover:bg-orange-950/30",
    active: "bg-orange-950/50 text-orange-100 shadow-lg shadow-orange-900/50",
    icon: "text-orange-400",
  },
};

// Navigation sections with dividers and colors
const navSections = [
  {
    items: [{ to: "/", label: "Dashboard", icon: Home, color: "gray" }],
  },
  {
    // Component Management - Cool colors (cyan -> blue -> violet)
    items: [
      { to: "/actions", label: "Actions", icon: SquarePlay, color: "cyan" },
      {
        to: "/runtimes",
        label: "Runtimes",
        icon: TerminalSquare,
        color: "blue",
      },
      { to: "/rules", label: "Rules", icon: SquareArrowRight, color: "blue" },
      {
        to: "/triggers",
        label: "Triggers",
        icon: SquareDot,
        color: "violet",
      },
      {
        to: "/sensors",
        label: "Sensors",
        icon: SquareAsterisk,
        color: "purple",
      },
    ],
  },
  {
    // Runtime Logs - Warm colors (fuchsia -> rose -> orange)
    items: [
      {
        to: "/executions",
        label: "Execution History",
        icon: CirclePlay,
        color: "fuchsia",
      },
      {
        to: "/enforcements",
        label: "Enforcement History",
        icon: CircleArrowRight,
        color: "rose",
      },
      {
        to: "/events",
        label: "Event History",
        icon: CircleDot,
        color: "orange",
      },
    ],
  },
  {
    items: [
      { to: "/keys", label: "Keys & Secrets", icon: KeyRound, color: "gray" },
      {
        to: "/artifacts",
        label: "Artifacts",
        icon: FolderArchive,
        color: "gray",
      },
      {
        to: "/packs",
        label: "Pack Management",
        icon: Package,
        color: "gray",
      },
    ],
  },
];

// NavLink extracted outside MainLayout so React preserves DOM identity across
// re-renders, which is required for CSS transitions to work on collapse/expand.
function NavLink({
  to,
  label,
  icon: Icon,
  color = "gray",
  isCollapsed,
  isActive,
}: {
  to: string;
  label: string;
  icon: React.ElementType;
  color?: string;
  isCollapsed: boolean;
  isActive: boolean;
}) {
  const colors =
    colorClasses[color as keyof typeof colorClasses] || colorClasses.gray;

  return (
    <Link
      to={to}
      className={`flex items-center px-4 py-2 rounded-md transition-colors duration-200 whitespace-nowrap ${
        isActive ? colors.active : colors.inactive
      }`}
      title={isCollapsed ? label : undefined}
    >
      <Icon
        className={`w-5 h-5 flex-shrink-0 ${isActive ? "" : colors.icon}`}
      />
      <span
        className="ml-3 inline-block overflow-hidden transition-all duration-300"
        style={{ maxWidth: isCollapsed ? 0 : "10rem" }}
      >
        {label}
      </span>
    </Link>
  );
}

export default function MainLayout() {
  const { user, logout } = useAuth();
  const navigate = useNavigate();
  const location = useLocation();
  const [isCollapsed, setIsCollapsed] = useState(() => {
    // Initialize from localStorage
    const saved = localStorage.getItem("sidebar-collapsed");
    return saved === "true";
  });
  const [showUserMenu, setShowUserMenu] = useState(false);

  // Persist collapsed state to localStorage and close user menu when expanding
  useEffect(() => {
    localStorage.setItem("sidebar-collapsed", isCollapsed.toString());
  }, [isCollapsed]);

  const handleToggleCollapse = () => {
    setIsCollapsed((prev) => {
      const next = !prev;
      if (!next) {
        setShowUserMenu(false);
      }
      return next;
    });
  };

  const handleLogout = () => {
    logout();
    navigate("/login");
  };

  return (
    <div className="h-full bg-gray-100 flex overflow-hidden">
      {/* Sidebar */}
      <div
        className={`${
          isCollapsed ? "w-20" : "w-64"
        } bg-gray-900 text-white flex flex-col transition-all duration-300 relative flex-shrink-0 overflow-hidden`}
      >
        {/* Header */}
        <div className="flex items-center justify-center h-20 bg-gray-800 whitespace-nowrap">
          <Link to="/" className="flex items-center justify-center">
            <img
              src="/attune-logo-icon.svg"
              alt="Attune"
              className={`h-14 transition-opacity duration-300 ${isCollapsed ? "opacity-100" : "opacity-0 w-0"}`}
            />
            <img
              src="/attune-logo-navbar.svg"
              alt="Attune"
              className={`h-14 transition-opacity duration-300 ${isCollapsed ? "opacity-0 w-0" : "opacity-100"}`}
            />
          </Link>
        </div>

        {/* Navigation */}
        <nav className="flex-1 px-4 py-6 overflow-y-auto overflow-x-hidden">
          {navSections.map((section, sectionIndex) => (
            <div key={sectionIndex}>
              <div className="space-y-1 mb-3">
                {section.items.map((item) => {
                  const isActive =
                    location.pathname === item.to ||
                    (item.to !== "/" && location.pathname.startsWith(item.to));
                  return (
                    <NavLink
                      key={item.to}
                      to={item.to}
                      label={item.label}
                      icon={item.icon}
                      color={item.color}
                      isCollapsed={isCollapsed}
                      isActive={isActive}
                    />
                  );
                })}
              </div>
              {sectionIndex < navSections.length - 1 && (
                <div className="my-3 mx-2 border-t border-gray-700" />
              )}
            </div>
          ))}
        </nav>

        {/* Toggle Button */}
        <div className="px-4 py-3">
          <button
            onClick={handleToggleCollapse}
            className="flex items-center w-full px-3 py-2 text-gray-400 hover:text-white hover:bg-gray-800 rounded-md transition-colors whitespace-nowrap"
            title={isCollapsed ? "Expand sidebar" : "Collapse sidebar"}
          >
            <div className="w-5 h-5 flex-shrink-0 relative">
              <ChevronLeft
                className={`w-5 h-5 absolute inset-0 transition-opacity duration-300 ${
                  isCollapsed ? "opacity-0" : "opacity-100"
                }`}
              />
              <ChevronRight
                className={`w-5 h-5 absolute inset-0 transition-opacity duration-300 ${
                  isCollapsed ? "opacity-100" : "opacity-0"
                }`}
              />
            </div>
            <span
              className="ml-2 inline-block overflow-hidden text-sm transition-all duration-300"
              style={{ maxWidth: isCollapsed ? 0 : "10rem" }}
            >
              Collapse
            </span>
          </button>
        </div>

        {/* User Section */}
        <div className="p-4 bg-gray-800 border-t border-gray-700 overflow-hidden whitespace-nowrap">
          <div className="relative">
            <div className="flex items-center justify-between">
              <div className="flex items-center min-w-0">
                <button
                  onClick={() => isCollapsed && setShowUserMenu(!showUserMenu)}
                  className={`flex-shrink-0 ${isCollapsed ? "cursor-pointer" : "cursor-default"}`}
                  title={user?.login}
                >
                  <User className="w-5 h-5 text-gray-400" />
                </button>
                <span
                  className="ml-2 inline-block overflow-hidden transition-all duration-300 min-w-0"
                  style={{ maxWidth: isCollapsed ? 0 : "8rem" }}
                >
                  <p className="font-medium text-sm truncate">{user?.login}</p>
                </span>
              </div>
              <span
                className="ml-2 inline-block overflow-hidden transition-all duration-300"
                style={{ maxWidth: isCollapsed ? 0 : "2rem" }}
              >
                <button
                  onClick={handleLogout}
                  className="text-gray-400 hover:text-white p-1 flex-shrink-0"
                  title="Logout"
                >
                  <LogOut className="w-5 h-5" />
                </button>
              </span>
            </div>

            {/* User Menu Popup (collapsed mode only) */}
            {isCollapsed && showUserMenu && (
              <>
                <div
                  className="fixed inset-0 z-10"
                  onClick={() => setShowUserMenu(false)}
                />
                <div className="absolute bottom-full left-0 mb-2 w-48 bg-gray-800 border border-gray-700 rounded-md shadow-lg z-20">
                  <div className="px-4 py-3 border-b border-gray-700">
                    <p className="text-sm font-medium text-white">
                      {user?.login}
                    </p>
                  </div>
                  <button
                    onClick={handleLogout}
                    className="w-full flex items-center gap-2 px-4 py-2 text-left text-gray-300 hover:bg-gray-700 hover:text-white transition-colors"
                  >
                    <LogOut className="w-4 h-4" />
                    <span>Logout</span>
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      </div>

      {/* Main Content */}
      <div className="flex-1 overflow-y-auto">
        <Outlet />
      </div>
    </div>
  );
}
