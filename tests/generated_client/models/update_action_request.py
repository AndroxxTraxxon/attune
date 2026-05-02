from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, BinaryIO, TextIO, TYPE_CHECKING, Generator

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

from ..types import UNSET, Unset
from typing import cast

if TYPE_CHECKING:
  from ..models.runtime_version_constraint_patch_type_0 import RuntimeVersionConstraintPatchType0
  from ..models.runtime_version_constraint_patch_type_1 import RuntimeVersionConstraintPatchType1
  from ..models.update_action_request_out_schema_type_0 import UpdateActionRequestOutSchemaType0
  from ..models.update_action_request_param_schema_type_0 import UpdateActionRequestParamSchemaType0
  from ..models.update_action_request_required_worker_runtimes_type_0 import UpdateActionRequestRequiredWorkerRuntimesType0





T = TypeVar("T", bound="UpdateActionRequest")



@_attrs_define
class UpdateActionRequest:
    """ Request DTO for updating an action

        Attributes:
            out_schema (None | UpdateActionRequestOutSchemaType0): Output schema
            param_schema (None | UpdateActionRequestParamSchemaType0): Parameter schema (StackStorm-style with inline
                required/secret)
            required_worker_runtimes (None | UpdateActionRequestRequiredWorkerRuntimesType0): Additional worker runtime
                requirements keyed by runtime name/alias. Use "*" for any available version.
            accesses_mcp (bool | None | Unset): Hint that this action may invoke the Attune MCP server and spawn child
                executions.
            description (None | str | Unset): Action description Example: Posts a message to a Slack channel with enhanced
                features.
            entrypoint (None | str | Unset): Entry point for action execution Example: /actions/slack/post_message_v2.py.
            label (None | str | Unset): Human-readable label Example: Post Message to Slack (Updated).
            runtime (int | None | Unset): Runtime ID Example: 1.
            runtime_version_constraint (None | RuntimeVersionConstraintPatchType0 | RuntimeVersionConstraintPatchType1 |
                Unset):
     """

    out_schema: None | UpdateActionRequestOutSchemaType0
    param_schema: None | UpdateActionRequestParamSchemaType0
    required_worker_runtimes: None | UpdateActionRequestRequiredWorkerRuntimesType0
    accesses_mcp: bool | None | Unset = UNSET
    description: None | str | Unset = UNSET
    entrypoint: None | str | Unset = UNSET
    label: None | str | Unset = UNSET
    runtime: int | None | Unset = UNSET
    runtime_version_constraint: None | RuntimeVersionConstraintPatchType0 | RuntimeVersionConstraintPatchType1 | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.runtime_version_constraint_patch_type_0 import RuntimeVersionConstraintPatchType0
        from ..models.runtime_version_constraint_patch_type_1 import RuntimeVersionConstraintPatchType1
        from ..models.update_action_request_out_schema_type_0 import UpdateActionRequestOutSchemaType0
        from ..models.update_action_request_param_schema_type_0 import UpdateActionRequestParamSchemaType0
        from ..models.update_action_request_required_worker_runtimes_type_0 import UpdateActionRequestRequiredWorkerRuntimesType0
        out_schema: dict[str, Any] | None
        if isinstance(self.out_schema, UpdateActionRequestOutSchemaType0):
            out_schema = self.out_schema.to_dict()
        else:
            out_schema = self.out_schema

        param_schema: dict[str, Any] | None
        if isinstance(self.param_schema, UpdateActionRequestParamSchemaType0):
            param_schema = self.param_schema.to_dict()
        else:
            param_schema = self.param_schema

        required_worker_runtimes: dict[str, Any] | None
        if isinstance(self.required_worker_runtimes, UpdateActionRequestRequiredWorkerRuntimesType0):
            required_worker_runtimes = self.required_worker_runtimes.to_dict()
        else:
            required_worker_runtimes = self.required_worker_runtimes

        accesses_mcp: bool | None | Unset
        if isinstance(self.accesses_mcp, Unset):
            accesses_mcp = UNSET
        else:
            accesses_mcp = self.accesses_mcp

        description: None | str | Unset
        if isinstance(self.description, Unset):
            description = UNSET
        else:
            description = self.description

        entrypoint: None | str | Unset
        if isinstance(self.entrypoint, Unset):
            entrypoint = UNSET
        else:
            entrypoint = self.entrypoint

        label: None | str | Unset
        if isinstance(self.label, Unset):
            label = UNSET
        else:
            label = self.label

        runtime: int | None | Unset
        if isinstance(self.runtime, Unset):
            runtime = UNSET
        else:
            runtime = self.runtime

        runtime_version_constraint: dict[str, Any] | None | Unset
        if isinstance(self.runtime_version_constraint, Unset):
            runtime_version_constraint = UNSET
        elif isinstance(self.runtime_version_constraint, RuntimeVersionConstraintPatchType0):
            runtime_version_constraint = self.runtime_version_constraint.to_dict()
        elif isinstance(self.runtime_version_constraint, RuntimeVersionConstraintPatchType1):
            runtime_version_constraint = self.runtime_version_constraint.to_dict()
        else:
            runtime_version_constraint = self.runtime_version_constraint


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "out_schema": out_schema,
            "param_schema": param_schema,
            "required_worker_runtimes": required_worker_runtimes,
        })
        if accesses_mcp is not UNSET:
            field_dict["accesses_mcp"] = accesses_mcp
        if description is not UNSET:
            field_dict["description"] = description
        if entrypoint is not UNSET:
            field_dict["entrypoint"] = entrypoint
        if label is not UNSET:
            field_dict["label"] = label
        if runtime is not UNSET:
            field_dict["runtime"] = runtime
        if runtime_version_constraint is not UNSET:
            field_dict["runtime_version_constraint"] = runtime_version_constraint

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.runtime_version_constraint_patch_type_0 import RuntimeVersionConstraintPatchType0
        from ..models.runtime_version_constraint_patch_type_1 import RuntimeVersionConstraintPatchType1
        from ..models.update_action_request_out_schema_type_0 import UpdateActionRequestOutSchemaType0
        from ..models.update_action_request_param_schema_type_0 import UpdateActionRequestParamSchemaType0
        from ..models.update_action_request_required_worker_runtimes_type_0 import UpdateActionRequestRequiredWorkerRuntimesType0
        d = dict(src_dict)
        def _parse_out_schema(data: object) -> None | UpdateActionRequestOutSchemaType0:
            if data is None:
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                out_schema_type_0 = UpdateActionRequestOutSchemaType0.from_dict(data)



                return out_schema_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | UpdateActionRequestOutSchemaType0, data)

        out_schema = _parse_out_schema(d.pop("out_schema"))


        def _parse_param_schema(data: object) -> None | UpdateActionRequestParamSchemaType0:
            if data is None:
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                param_schema_type_0 = UpdateActionRequestParamSchemaType0.from_dict(data)



                return param_schema_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | UpdateActionRequestParamSchemaType0, data)

        param_schema = _parse_param_schema(d.pop("param_schema"))


        def _parse_required_worker_runtimes(data: object) -> None | UpdateActionRequestRequiredWorkerRuntimesType0:
            if data is None:
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                required_worker_runtimes_type_0 = UpdateActionRequestRequiredWorkerRuntimesType0.from_dict(data)



                return required_worker_runtimes_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | UpdateActionRequestRequiredWorkerRuntimesType0, data)

        required_worker_runtimes = _parse_required_worker_runtimes(d.pop("required_worker_runtimes"))


        def _parse_accesses_mcp(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        accesses_mcp = _parse_accesses_mcp(d.pop("accesses_mcp", UNSET))


        def _parse_description(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description = _parse_description(d.pop("description", UNSET))


        def _parse_entrypoint(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        entrypoint = _parse_entrypoint(d.pop("entrypoint", UNSET))


        def _parse_label(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        label = _parse_label(d.pop("label", UNSET))


        def _parse_runtime(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        runtime = _parse_runtime(d.pop("runtime", UNSET))


        def _parse_runtime_version_constraint(data: object) -> None | RuntimeVersionConstraintPatchType0 | RuntimeVersionConstraintPatchType1 | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                componentsschemas_runtime_version_constraint_patch_type_0 = RuntimeVersionConstraintPatchType0.from_dict(data)



                return componentsschemas_runtime_version_constraint_patch_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                componentsschemas_runtime_version_constraint_patch_type_1 = RuntimeVersionConstraintPatchType1.from_dict(data)



                return componentsschemas_runtime_version_constraint_patch_type_1
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | RuntimeVersionConstraintPatchType0 | RuntimeVersionConstraintPatchType1 | Unset, data)

        runtime_version_constraint = _parse_runtime_version_constraint(d.pop("runtime_version_constraint", UNSET))


        update_action_request = cls(
            out_schema=out_schema,
            param_schema=param_schema,
            required_worker_runtimes=required_worker_runtimes,
            accesses_mcp=accesses_mcp,
            description=description,
            entrypoint=entrypoint,
            label=label,
            runtime=runtime,
            runtime_version_constraint=runtime_version_constraint,
        )


        update_action_request.additional_properties = d
        return update_action_request

    @property
    def additional_keys(self) -> list[str]:
        return list(self.additional_properties.keys())

    def __getitem__(self, key: str) -> Any:
        return self.additional_properties[key]

    def __setitem__(self, key: str, value: Any) -> None:
        self.additional_properties[key] = value

    def __delitem__(self, key: str) -> None:
        del self.additional_properties[key]

    def __contains__(self, key: str) -> bool:
        return key in self.additional_properties
