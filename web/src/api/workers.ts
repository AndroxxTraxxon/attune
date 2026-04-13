import type { CancelablePromise } from "@/api";
import type { PaginationMeta } from "@/api";
import { OpenAPI } from "@/api";
import { request as __request } from "@/api/core/request";

export type WorkerType = "local" | "remote" | "container";
export type WorkerStatus = "active" | "inactive" | "busy" | "error";
export type WorkerRole = "action" | "sensor";

export interface WorkerRuntimeSupport {
  name: string;
  versions: string[];
}

export interface WorkerLoadSnapshot {
  requested: number;
  scheduling: number;
  scheduled: number;
  running: number;
  canceling: number;
  total_active: number;
  max_concurrent_executions?: number | null;
  utilization_percent?: number | null;
  queue_depth?: number | null;
  sensor_processes_monitored?: number | null;
  sensor_processes_running?: number | null;
  active_rules?: number | null;
  max_concurrent_sensors?: number | null;
}

export interface WorkerSummary {
  id: number;
  name: string;
  worker_type: WorkerType;
  worker_role: WorkerRole;
  host?: string | null;
  port?: number | null;
  status?: WorkerStatus | null;
  last_heartbeat?: string | null;
  supported_runtimes: WorkerRuntimeSupport[];
  load: WorkerLoadSnapshot;
  created: string;
  updated: string;
}

export interface PaginatedResponseWorkerSummary {
  data: WorkerSummary[];
  pagination: PaginationMeta;
}

export class WorkersService {
  public static listWorkers({
    page,
    pageSize,
  }: {
    page?: number;
    pageSize?: number;
  }): CancelablePromise<PaginatedResponseWorkerSummary> {
    return __request(OpenAPI, {
      method: "GET",
      url: "/api/v1/workers",
      query: {
        page,
        page_size: pageSize,
      },
    });
  }
}
