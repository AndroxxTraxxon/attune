import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { QueryClientProvider } from "@tanstack/react-query";
import { AuthProvider } from "@/contexts/AuthContext";
import { WebSocketProvider } from "@/contexts/WebSocketContext";
import { queryClient } from "@/lib/query-client";
import ProtectedRoute from "@/components/common/ProtectedRoute";
import MainLayout from "@/components/layout/MainLayout";
import LoginPage from "@/pages/auth/LoginPage";
import DashboardPage from "@/pages/dashboard/DashboardPage";
import PacksPage from "@/pages/packs/PacksPage";
import PackCreatePage from "@/pages/packs/PackCreatePage";
import PackRegisterPage from "@/pages/packs/PackRegisterPage";
import PackInstallPage from "@/pages/packs/PackInstallPage";
import PackEditPage from "@/pages/packs/PackEditPage";
import ActionsPage from "@/pages/actions/ActionsPage";
import RulesPage from "@/pages/rules/RulesPage";
import RuleCreatePage from "@/pages/rules/RuleCreatePage";
import RuleEditPage from "@/pages/rules/RuleEditPage";
import ExecutionsPage from "@/pages/executions/ExecutionsPage";
import ExecutionDetailPage from "@/pages/executions/ExecutionDetailPage";
import EventsPage from "@/pages/events/EventsPage";
import EventDetailPage from "@/pages/events/EventDetailPage";
import EnforcementsPage from "@/pages/enforcements/EnforcementsPage";
import EnforcementDetailPage from "@/pages/enforcements/EnforcementDetailPage";
import KeysPage from "@/pages/keys/KeysPage";
import TriggersPage from "@/pages/triggers/TriggersPage";
import TriggerCreatePage from "@/pages/triggers/TriggerCreatePage";
import TriggerEditPage from "@/pages/triggers/TriggerEditPage";
import SensorsPage from "@/pages/sensors/SensorsPage";

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AuthProvider>
        <WebSocketProvider>
          <BrowserRouter>
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
                <Route path="triggers/create" element={<TriggerCreatePage />} />
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
          </BrowserRouter>
        </WebSocketProvider>
      </AuthProvider>
    </QueryClientProvider>
  );
}

export default App;
