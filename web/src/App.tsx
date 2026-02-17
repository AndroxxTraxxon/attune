import { lazy, Suspense } from "react";
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { QueryClientProvider } from "@tanstack/react-query";
import { AuthProvider } from "@/contexts/AuthContext";
import { WebSocketProvider } from "@/contexts/WebSocketContext";
import { queryClient } from "@/lib/query-client";
import ProtectedRoute from "@/components/common/ProtectedRoute";
import MainLayout from "@/components/layout/MainLayout";

// Lazy-loaded page components for code splitting
const LoginPage = lazy(() => import("@/pages/auth/LoginPage"));
const DashboardPage = lazy(() => import("@/pages/dashboard/DashboardPage"));
const PacksPage = lazy(() => import("@/pages/packs/PacksPage"));
const PackCreatePage = lazy(() => import("@/pages/packs/PackCreatePage"));
const PackRegisterPage = lazy(() => import("@/pages/packs/PackRegisterPage"));
const PackInstallPage = lazy(() => import("@/pages/packs/PackInstallPage"));
const PackEditPage = lazy(() => import("@/pages/packs/PackEditPage"));
const ActionsPage = lazy(() => import("@/pages/actions/ActionsPage"));
const RulesPage = lazy(() => import("@/pages/rules/RulesPage"));
const RuleCreatePage = lazy(() => import("@/pages/rules/RuleCreatePage"));
const RuleEditPage = lazy(() => import("@/pages/rules/RuleEditPage"));
const ExecutionsPage = lazy(() => import("@/pages/executions/ExecutionsPage"));
const ExecutionDetailPage = lazy(
  () => import("@/pages/executions/ExecutionDetailPage"),
);
const EventsPage = lazy(() => import("@/pages/events/EventsPage"));
const EventDetailPage = lazy(() => import("@/pages/events/EventDetailPage"));
const EnforcementsPage = lazy(
  () => import("@/pages/enforcements/EnforcementsPage"),
);
const EnforcementDetailPage = lazy(
  () => import("@/pages/enforcements/EnforcementDetailPage"),
);
const KeysPage = lazy(() => import("@/pages/keys/KeysPage"));
const TriggersPage = lazy(() => import("@/pages/triggers/TriggersPage"));
const TriggerCreatePage = lazy(
  () => import("@/pages/triggers/TriggerCreatePage"),
);
const TriggerEditPage = lazy(() => import("@/pages/triggers/TriggerEditPage"));
const SensorsPage = lazy(() => import("@/pages/sensors/SensorsPage"));

function PageLoader() {
  return (
    <div className="flex items-center justify-center h-64">
      <div className="text-center">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600 mx-auto"></div>
        <p className="mt-3 text-sm text-gray-500">Loading…</p>
      </div>
    </div>
  );
}

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AuthProvider>
        <WebSocketProvider>
          <BrowserRouter>
            <Suspense fallback={<PageLoader />}>
              <Routes>
                {/* Public routes */}
                <Route path="/login" element={<LoginPage />} />

                {/* Protected routes */}
                <Route
                  path="/"
                  element={
                    <ProtectedRoute>
                      <MainLayout />
                    </ProtectedRoute>
                  }
                >
                  <Route index element={<DashboardPage />} />
                  <Route path="packs" element={<PacksPage />} />
                  <Route path="packs/new" element={<PackCreatePage />} />
                  <Route path="packs/register" element={<PackRegisterPage />} />
                  <Route path="packs/install" element={<PackInstallPage />} />
                  <Route path="packs/:ref" element={<PacksPage />} />
                  <Route path="packs/:ref/edit" element={<PackEditPage />} />
                  <Route path="actions" element={<ActionsPage />} />
                  <Route path="actions/:ref" element={<ActionsPage />} />
                  <Route path="rules" element={<RulesPage />} />
                  <Route path="rules/new" element={<RuleCreatePage />} />
                  <Route path="rules/:ref" element={<RulesPage />} />
                  <Route path="rules/:ref/edit" element={<RuleEditPage />} />
                  <Route path="executions" element={<ExecutionsPage />} />
                  <Route
                    path="executions/:id"
                    element={<ExecutionDetailPage />}
                  />
                  <Route path="events" element={<EventsPage />} />
                  <Route path="events/:id" element={<EventDetailPage />} />
                  <Route path="enforcements" element={<EnforcementsPage />} />
                  <Route
                    path="enforcements/:id"
                    element={<EnforcementDetailPage />}
                  />
                  <Route path="keys" element={<KeysPage />} />
                  <Route path="triggers" element={<TriggersPage />} />
                  <Route
                    path="triggers/create"
                    element={<TriggerCreatePage />}
                  />
                  <Route path="triggers/:ref" element={<TriggersPage />} />
                  <Route
                    path="triggers/:ref/edit"
                    element={<TriggerEditPage />}
                  />
                  <Route path="sensors" element={<SensorsPage />} />
                  <Route path="sensors/:ref" element={<SensorsPage />} />
                </Route>

                {/* Catch all - redirect to dashboard */}
                <Route path="*" element={<Navigate to="/" replace />} />
              </Routes>
            </Suspense>
          </BrowserRouter>
        </WebSocketProvider>
      </AuthProvider>
    </QueryClientProvider>
  );
}

export default App;
