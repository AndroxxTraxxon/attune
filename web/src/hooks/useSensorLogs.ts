import { useQuery } from "@tanstack/react-query";

import { SensorLogsService } from "@/api/sensorLogs";

export function useSensorLogs(sensorRef: string, enabled = true) {
  return useQuery({
    queryKey: ["sensor-logs", sensorRef],
    queryFn: () => SensorLogsService.listSensorLogs({ sensorRef }),
    enabled: enabled && Boolean(sensorRef),
    staleTime: 30000,
  });
}

export function useSensorLog(
  sensorRef: string,
  stream: "stdout" | "stderr",
  tail = 200,
  follow = false,
  enabled = true,
) {
  return useQuery({
    queryKey: ["sensor-log", sensorRef, stream, tail, follow],
    queryFn: () => SensorLogsService.getSensorLog({ sensorRef, stream, tail }),
    enabled: enabled && Boolean(sensorRef),
    refetchInterval: enabled && follow ? 3000 : false,
  });
}
