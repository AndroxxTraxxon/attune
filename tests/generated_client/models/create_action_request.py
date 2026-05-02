from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, BinaryIO, TextIO, TYPE_CHECKING, Generator

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

from ..types import UNSET, Unset
from typing import cast

if TYPE_CHECKING:
  from ..models.create_action_request_out_schema_type_0 import CreateActionRequestOutSchemaType0
  from ..models.create_action_request_param_schema_type_0 import CreateActionRequestParamSchemaType0
  from ..models.create_action_request_required_worker_runtimes import CreateActionRequestRequiredWorkerRuntimes





T = TypeVar("T", bound="CreateActionRequest")



@_attrs_define
class CreateActionRequest:
    """ Request DTO for creating a new action

        Attributes:
            entrypoint (str): Entry point for action execution (e.g., path to script, function name) Example:
                /actions/slack/post_message.py.
            label (str): Human-readable label Example: Post Message to Slack.
            pack_ref (str): Pack reference this action belongs to Example: slack.
            ref (str): Unique reference identifier (e.g., "core.http", "aws.ec2.start_instance") Example:
                slack.post_message.
            accesses_mcp (bool | None | Unset): Hint that this action may invoke the Attune MCP server and spawn child
                executions.
                When true, consumers (UI, CLI, timeline charts) render subtask views eagerly. Default: False.
            description (None | str | Unset): Action description Example: Posts a message to a Slack channel.
            out_schema (CreateActionRequestOutSchemaType0 | None | Unset): Output schema (flat format) defining expected
                outputs with inline required/secret
            param_schema (CreateActionRequestParamSchemaType0 | None | Unset): Parameter schema (StackStorm-style) defining
                expected inputs with inline required/secret
            required_worker_runtimes (CreateActionRequestRequiredWorkerRuntimes | Unset): Additional worker runtime
                requirements keyed by runtime name/alias. Use "*" for any available version.
            runtime (int | None | Unset): Optional runtime ID for this action Example: 1.
            runtime_version_constraint (None | str | Unset): Optional semver version constraint for the runtime (e.g.,
                ">=3.12", ">=3.12,<4.0", "~18.0") Example: >=3.12.
     """

    entrypoint: str
    label: str
    pack_ref: str
    ref: str
    accesses_mcp: bool | None | Unset = False
    description: None | str | Unset = UNSET
    out_schema: CreateActionRequestOutSchemaType0 | None | Unset = UNSET
    param_schema: CreateActionRequestParamSchemaType0 | None | Unset = UNSET
    required_worker_runtimes: CreateActionRequestRequiredWorkerRuntimes | Unset = UNSET
    runtime: int | None | Unset = UNSET
    runtime_version_constraint: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.create_action_request_out_schema_type_0 import CreateActionRequestOutSchemaType0
        from ..models.create_action_request_param_schema_type_0 import CreateActionRequestParamSchemaType0
        from ..models.create_action_request_required_worker_runtimes import CreateActionRequestRequiredWorkerRuntimes
        entrypoint = self.entrypoint

        label = self.label

        pack_ref = self.pack_ref

        ref = self.ref

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

        out_schema: dict[str, Any] | None | Unset
        if isinstance(self.out_schema, Unset):
            out_schema = UNSET
        elif isinstance(self.out_schema, CreateActionRequestOutSchemaType0):
            out_schema = self.out_schema.to_dict()
        else:
            out_schema = self.out_schema

        param_schema: dict[str, Any] | None | Unset
        if isinstance(self.param_schema, Unset):
            param_schema = UNSET
        elif isinstance(self.param_schema, CreateActionRequestParamSchemaType0):
            param_schema = self.param_schema.to_dict()
        else:
            param_schema = self.param_schema

        required_worker_runtimes: dict[str, Any] | Unset = UNSET
        if not isinstance(self.required_worker_runtimes, Unset):
            required_worker_runtimes = self.required_worker_runtimes.to_dict()

        runtime: int | None | Unset
        if isinstance(self.runtime, Unset):
            runtime = UNSET
        else:
            runtime = self.runtime

        runtime_version_constraint: None | str | Unset
        if isinstance(self.runtime_version_constraint, Unset):
            runtime_version_constraint = UNSET
        else:
            runtime_version_constraint = self.runtime_version_constraint


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "entrypoint": entrypoint,
            "label": label,
            "pack_ref": pack_ref,
            "ref": ref,
        })
        if accesses_mcp is not UNSET:
            field_dict["accesses_mcp"] = accesses_mcp
        if description is not UNSET:
            field_dict["description"] = description
        if out_schema is not UNSET:
            field_dict["out_schema"] = out_schema
        if param_schema is not UNSET:
            field_dict["param_schema"] = param_schema
        if required_worker_runtimes is not UNSET:
            field_dict["required_worker_runtimes"] = required_worker_runtimes
        if runtime is not UNSET:
            field_dict["runtime"] = runtime
        if runtime_version_constraint is not UNSET:
            field_dict["runtime_version_constraint"] = runtime_version_constraint

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.create_action_request_out_schema_type_0 import CreateActionRequestOutSchemaType0
        from ..models.create_action_request_param_schema_type_0 import CreateActionRequestParamSchemaType0
        from ..models.create_action_request_required_worker_runtimes import CreateActionRequestRequiredWorkerRuntimes
        d = dict(src_dict)
        entrypoint = d.pop("entrypoint")

        label = d.pop("label")

        pack_ref = d.pop("pack_ref")

        ref = d.pop("ref")

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


        def _parse_out_schema(data: object) -> CreateActionRequestOutSchemaType0 | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                out_schema_type_0 = CreateActionRequestOutSchemaType0.from_dict(data)



                return out_schema_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(CreateActionRequestOutSchemaType0 | None | Unset, data)

        out_schema = _parse_out_schema(d.pop("out_schema", UNSET))


        def _parse_param_schema(data: object) -> CreateActionRequestParamSchemaType0 | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                param_schema_type_0 = CreateActionRequestParamSchemaType0.from_dict(data)



                return param_schema_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(CreateActionRequestParamSchemaType0 | None | Unset, data)

        param_schema = _parse_param_schema(d.pop("param_schema", UNSET))


        _required_worker_runtimes = d.pop("required_worker_runtimes", UNSET)
        required_worker_runtimes: CreateActionRequestRequiredWorkerRuntimes | Unset
        if isinstance(_required_worker_runtimes,  Unset):
            required_worker_runtimes = UNSET
        else:
            required_worker_runtimes = CreateActionRequestRequiredWorkerRuntimes.from_dict(_required_worker_runtimes)




        def _parse_runtime(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        runtime = _parse_runtime(d.pop("runtime", UNSET))


        def _parse_runtime_version_constraint(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        runtime_version_constraint = _parse_runtime_version_constraint(d.pop("runtime_version_constraint", UNSET))


        create_action_request = cls(
            entrypoint=entrypoint,
            label=label,
            pack_ref=pack_ref,
            ref=ref,
            accesses_mcp=accesses_mcp,
            description=description,
            out_schema=out_schema,
            param_schema=param_schema,
            required_worker_runtimes=required_worker_runtimes,
            runtime=runtime,
            runtime_version_constraint=runtime_version_constraint,
        )


        create_action_request.additional_properties = d
        return create_action_request

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
