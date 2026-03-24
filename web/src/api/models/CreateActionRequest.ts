/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Request DTO for creating a new action
 */
export type CreateActionRequest = {
  /**
   * Action description
   */
  description?: string | null;
  /**
   * Entry point for action execution (e.g., path to script, function name)
   */
  entrypoint: string;
  /**
   * Human-readable label
   */
  label: string;
  /**
   * Output schema (flat format) defining expected outputs with inline required/secret
   */
  out_schema?: any | null;
  /**
   * Pack reference this action belongs to
   */
  pack_ref: string;
  /**
   * Parameter schema (StackStorm-style) defining expected inputs with inline required/secret
   */
  param_schema?: any | null;
  /**
   * Unique reference identifier (e.g., "core.http", "aws.ec2.start_instance")
   */
  ref: string;
  /**
   * Optional runtime ID for this action
   */
  runtime?: number | null;
  /**
   * Optional semver version constraint for the runtime (e.g., ">=3.12", ">=3.12,<4.0", "~18.0")
   */
  runtime_version_constraint?: string | null;
};
