/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Response DTO for sensor information
 */
export type SensorResponse = {
  /**
   * Creation timestamp
   */
  created: string;
  /**
   * Sensor description
   */
  description: string | null;
  /**
   * Whether the sensor is enabled
   */
  enabled: boolean;
  /**
   * Entry point
   */
  entrypoint: string;
  /**
   * Sensor ID
   */
  id: number;
  /**
   * Human-readable label
   */
  label: string;
  /**
   * Pack ID (optional)
   */
  pack?: number | null;
  /**
   * Pack reference (optional)
   */
  pack_ref?: string | null;
  /**
   * Parameter schema (StackStorm-style with inline required/secret)
   */
  param_schema: any | null;
  /**
   * Unique reference identifier
   */
  ref: string;
  /**
   * Runtime ID
   */
  runtime: number;
  /**
   * Runtime reference
   */
  runtime_ref: string;
  /**
   * Trigger ID
   */
  trigger: number;
  /**
   * Trigger reference
   */
  trigger_ref: string;
  /**
   * Last update timestamp
   */
  updated: string;
};
