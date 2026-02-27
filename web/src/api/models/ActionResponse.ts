/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Response DTO for action information
 */
export type ActionResponse = {
  /**
   * Creation timestamp
   */
  created: string;
  /**
   * Action description
   */
  description: string;
  /**
   * Entry point
   */
  entrypoint: string;
  /**
   * Action ID
   */
  id: number;
  /**
   * Whether this is an ad-hoc action (not from pack installation)
   */
  is_adhoc: boolean;
  /**
   * Human-readable label
   */
  label: string;
  /**
   * Output schema
   */
  out_schema: any | null;
  /**
   * Pack ID
   */
  pack: number;
  /**
   * Pack reference
   */
  pack_ref: string;
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
  runtime?: number | null;
  /**
   * Semver version constraint for the runtime (e.g., ">=3.12", ">=3.12,<4.0", "~18.0")
   */
  runtime_version_constraint?: string | null;
  /**
   * Last update timestamp
   */
  updated: string;
  /**
   * Workflow definition ID (non-null if this action is a workflow)
   */
  workflow_def?: number | null;
};
