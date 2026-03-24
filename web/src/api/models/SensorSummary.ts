/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Simplified sensor response (for list endpoints)
 */
export type SensorSummary = {
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
   * Sensor ID
   */
  id: number;
  /**
   * Human-readable label
   */
  label: string;
  /**
   * Pack reference (optional)
   */
  pack_ref?: string | null;
  /**
   * Unique reference identifier
   */
  ref: string;
  /**
   * Trigger reference
   */
  trigger_ref: string;
  /**
   * Last update timestamp
   */
  updated: string;
};
