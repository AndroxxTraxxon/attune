import type { CancelablePromise } from "@/api";
import { OpenAPI } from "@/api";
import { request as __request } from "@/api/core/request";

export interface SensorLogEntry {
  stream: string;
  artifact_ref: string;
  artifact_id?: number | null;
}

export interface SensorLogSummary {
  sensor_ref: string;
  logs: SensorLogEntry[];
}

export class SensorLogsService {
  public static listSensorLogs({
    sensorRef,
  }: {
    sensorRef: string;
  }): CancelablePromise<SensorLogSummary> {
    return __request(OpenAPI, {
      method: "GET",
      url: "/api/v1/sensors/{sensor_ref}/logs",
      path: { sensor_ref: sensorRef },
    });
  }

  public static getSensorLog({
    sensorRef,
    stream,
    tail,
  }: {
    sensorRef: string;
    stream: "stdout" | "stderr";
    tail?: number;
  }): CancelablePromise<string> {
    return __request(OpenAPI, {
      method: "GET",
      url: "/api/v1/sensors/{sensor_ref}/logs/{stream}",
      path: { sensor_ref: sensorRef, stream },
      query: { tail },
    });
  }
}
