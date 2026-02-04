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
} from "lucide-react";

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

  // Persist collapsed state to localStorage
  useEffect(() => {
    localStorage.setItem("sidebar-collapsed", isCollapsed.toString());
  }, [isCollapsed]);

  const handleLogout = () => {
    logout();
    navigate("/login");
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
          to: "/packs",
          label: "Pack Management",
          icon: Package,
          color: "gray",
        },
      ],
    },
  ];

  // Color mappings for navigation items
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
      inactive:
        "text-fuchsia-300 hover:text-fuchsia-100 hover:bg-fuchsia-950/30",
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

  const NavLink = ({
    to,
    label,
    icon: Icon,
    color = "gray",
  }: {
    to: string;
    label: string;
    icon: React.ElementType;
    color?: string;
  }) => {
    const isActive =
      location.pathname === to ||
      (to !== "/" && location.pathname.startsWith(to));

    const colors =
      colorClasses[color as keyof typeof colorClasses] || colorClasses.gray;

    return (
      <Link
        to={to}
        className={`flex items-center gap-3 px-4 py-2 rounded-md transition-all duration-200 ${
          isActive ? colors.active : colors.inactive
        } ${isCollapsed ? "justify-center" : ""}`}
        title={isCollapsed ? label : undefined}
      >
        <Icon
          className={`w-5 h-5 flex-shrink-0 ${isActive ? "" : colors.icon}`}
        />
        {!isCollapsed && <span>{label}</span>}
      </Link>
    );
  };

  return (
    <div className="h-screen bg-gray-100 flex overflow-hidden">
      <div
        className={`${
          isCollapsed ? "w-20" : "w-64"
        } bg-gray-900 text-white flex flex-col transition-all duration-300 relative flex-shrink-0`}
      >
        {/* Header */}
        <div className="flex items-center justify-center h-16 bg-gray-800">
          <Link
            to="/"
            className={`font-bold transition-all ${
              isCollapsed ? "text-lg" : "text-xl"
            }`}
          >
            {isCollapsed ? "A" : "Attune"}
          </Link>
        </div>

        {/* Navigation */}
        <nav className="flex-1 px-4 py-6 overflow-y-auto">
          {navSections.map((section, sectionIndex) => (
            <div key={sectionIndex}>
              <div className="space-y-1 mb-3">
                {section.items.map((item) => (
                  <NavLink
                    key={item.to}
                    to={item.to}
                    label={item.label}
                    icon={item.icon}
                    color={item.color}
                  />
                ))}
              </div>
              {sectionIndex < navSections.length - 1 && (
                <div className="my-3 mx-2 border-t border-gray-700" />
              )}
            </div>
          ))}
        </nav>

        {/* Toggle Button */}
        <div
          className={`px-4 py-3 ${isCollapsed ? "flex justify-center" : ""}`}
        >
          <button
            onClick={() => setIsCollapsed(!isCollapsed)}
            className="flex items-center gap-2 w-full px-3 py-2 text-gray-400 hover:text-white hover:bg-gray-800 rounded-md transition-colors"
            title={isCollapsed ? "Expand sidebar" : "Collapse sidebar"}
          >
            {isCollapsed ? (
              <ChevronRight className="w-5 h-5" />
            ) : (
              <>
                <ChevronLeft className="w-5 h-5" />
                <span className="text-sm">Collapse</span>
              </>
            )}
          </button>
        </div>

        {/* User Section */}
        <div className="p-4 bg-gray-800 border-t border-gray-700">
          {isCollapsed ? (
            <div className="relative">
              <button
                onClick={() => setShowUserMenu(!showUserMenu)}
                className="w-full flex items-center justify-center p-2 rounded-md hover:bg-gray-700 transition-colors"
                title={user?.login}
              >
                <User className="w-6 h-6 text-gray-400" />
              </button>

              {/* User Menu Popup */}
              {showUserMenu && (
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
          ) : (
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2 min-w-0">
                <User className="w-5 h-5 text-gray-400 flex-shrink-0" />
                <p className="font-medium text-sm truncate">{user?.login}</p>
              </div>
              <button
                onClick={handleLogout}
                className="text-gray-400 hover:text-white p-1 flex-shrink-0"
                title="Logout"
              >
                <LogOut className="w-5 h-5" />
              </button>
            </div>
          )}
        </div>
      </div>
      <div className="flex-1 overflow-y-auto">
        <Outlet />
      </div>
    </div>
  );
}
