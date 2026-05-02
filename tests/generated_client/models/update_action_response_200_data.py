from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, BinaryIO, TextIO, TYPE_CHECKING, Generator

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

from ..types import UNSET, Unset
from dateutil.parser import isoparse
from typing import cast
import datetime

if TYPE_CHECKING:
  from ..models.update_action_response_200_data_out_schema_type_0 import UpdateActionResponse200DataOutSchemaType0
  from ..models.update_action_response_200_data_param_schema_type_0 import UpdateActionResponse200DataParamSchemaType0
  from ..models.update_action_response_200_data_required_worker_runtimes import UpdateActionResponse200DataRequiredWorkerRuntimes





T = TypeVar("T", bound="UpdateActionResponse200Data")



@_attrs_define
class UpdateActionResponse200Data:
    """ Response DTO for action information

        Attributes:
            accesses_mcp (bool): Hint that this action may invoke the Attune MCP server and spawn child executions. Default:
                False.
            created (datetime.datetime): Creation timestamp Example: 2024-01-13T10:30:00Z.
            entrypoint (str): Entry point Example: /actions/slack/post_message.py.
            id (int): Action ID Example: 1.
            is_adhoc (bool): Whether this is an ad-hoc action (not from pack installation)
            label (str): Human-readable label Example: Post Message to Slack.
            out_schema (None | UpdateActionResponse200DataOutSchemaType0): Output schema
            pack (int): Pack ID Example: 1.
            pack_ref (str): Pack reference Example: slack.
            param_schema (None | UpdateActionResponse200DataParamSchemaType0): Parameter schema (StackStorm-style with
                inline required/secret)
            ref (str): Unique reference identifier Example: slack.post_message.
            updated (datetime.datetime): Last update timestamp Example: 2024-01-13T10:30:00Z.
            description (None | str | Unset): Action description Example: Posts a message to a Slack channel.
            required_worker_runtimes (UpdateActionResponse200DataRequiredWorkerRuntimes | Unset): Additional worker runtime
                requirements keyed by runtime name/alias. Use "*" for any available version.
            runtime (int | None | Unset): Runtime ID Example: 1.
            runtime_ref (None | str | Unset): Runtime reference (stable identifier, e.g., "core.python") Example:
                core.python.
            runtime_version_constraint (None | str | Unset): Semver version constraint for the runtime (e.g., ">=3.12",
                ">=3.12,<4.0", "~18.0") Example: >=3.12.
            workflow_def (int | None | Unset): Workflow definition ID (non-null if this action is a workflow) Example: 42.
     """

    created: datetime.datetime
    entrypoint: str
    id: int
    is_adhoc: bool
    label: str
    out_schema: None | UpdateActionResponse200DataOutSchemaType0
    pack: int
    pack_ref: str
    param_schema: None | UpdateActionResponse200DataParamSchemaType0
    ref: str
    updated: datetime.datetime
    accesses_mcp: bool = False
    description: None | str | Unset = UNSET
    required_worker_runtimes: UpdateActionResponse200DataRequiredWorkerRuntimes | Unset = UNSET
    runtime: int | None | Unset = UNSET
    runtime_ref: None | str | Unset = UNSET
    runtime_version_constraint: None | str | Unset = UNSET
    workflow_def: int | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.update_action_response_200_data_out_schema_type_0 import UpdateActionResponse200DataOutSchemaType0
        from ..models.update_action_response_200_data_param_schema_type_0 import UpdateActionResponse200DataParamSchemaType0
        from ..models.update_action_response_200_data_required_worker_runtimes import UpdateActionResponse200DataRequiredWorkerRuntimes
        accesses_mcp = self.accesses_mcp

        created = self.created.isoformat()

        entrypoint = self.entrypoint

        id = self.id

        is_adhoc = self.is_adhoc

        label = self.label

        out_schema: dict[str, Any] | None
        if isinstance(self.out_schema, UpdateActionResponse200DataOutSchemaType0):
            out_schema = self.out_schema.to_dict()
        else:
            out_schema = self.out_schema

        pack = self.pack

        pack_ref = self.pack_ref

        param_schema: dict[str, Any] | None
        if isinstance(self.param_schema, UpdateActionResponse200DataParamSchemaType0):
            param_schema = self.param_schema.to_dict()
        else:
            param_schema = self.param_schema

        ref = self.ref

        updated = self.updated.isoformat()

        description: None | str | Unset
        if isinstance(self.description, Unset):
            description = UNSET
        else:
            description = self.description

        required_worker_runtimes: dict[str, Any] | Unset = UNSET
        if not isinstance(self.required_worker_runtimes, Unset):
            required_worker_runtimes = self.required_worker_runtimes.to_dict()

        runtime: int | None | Unset
        if isinstance(self.runtime, Unset):
            runtime = UNSET
        else:
            runtime = self.runtime

        runtime_ref: None | str | Unset
        if isinstance(self.runtime_ref, Unset):
            runtime_ref = UNSET
        else:
            runtime_ref = self.runtime_ref

        runtime_version_constraint: None | str | Unset
        if isinstance(self.runtime_version_constraint, Unset):
            runtime_version_constraint = UNSET
        else:
            runtime_version_constraint = self.runtime_version_constraint

        workflow_def: int | None | Unset
        if isinstance(self.workflow_def, Unset):
            workflow_def = UNSET
        else:
            workflow_def = self.workflow_def


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "accesses_mcp": accesses_mcp,
            "created": created,
            "entrypoint": entrypoint,
            "id": id,
            "is_adhoc": is_adhoc,
            "label": label,
            "out_schema": out_schema,
            "pack": pack,
            "pack_ref": pack_ref,
            "param_schema": param_schema,
            "ref": ref,
            "updated": updated,
        })
        if description is not UNSET:
            field_dict["description"] = description
        if required_worker_runtimes is not UNSET:
            field_dict["required_worker_runtimes"] = required_worker_runtimes
        if runtime is not UNSET:
            field_dict["runtime"] = runtime
        if runtime_ref is not UNSET:
            field_dict["runtime_ref"] = runtime_ref
        if runtime_version_constraint is not UNSET:
            field_dict["runtime_version_constraint"] = runtime_version_constraint
        if workflow_def is not UNSET:
            field_dict["workflow_def"] = workflow_def

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.update_action_response_200_data_out_schema_type_0 import UpdateActionResponse200DataOutSchemaType0
        from ..models.update_action_response_200_data_param_schema_type_0 import UpdateActionResponse200DataParamSchemaType0
        from ..models.update_action_response_200_data_required_worker_runtimes import UpdateActionResponse200DataRequiredWorkerRuntimes
        d = dict(src_dict)
        accesses_mcp = d.pop("accesses_mcp")

        created = isoparse(d.pop("created"))




        entrypoint = d.pop("entrypoint")

        id = d.pop("id")

        is_adhoc = d.pop("is_adhoc")

        label = d.pop("label")

        def _parse_out_schema(data: object) -> None | UpdateActionResponse200DataOutSchemaType0:
            if data is None:
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                out_schema_type_0 = UpdateActionResponse200DataOutSchemaType0.from_dict(data)



                return out_schema_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | UpdateActionResponse200DataOutSchemaType0, data)

        out_schema = _parse_out_schema(d.pop("out_schema"))


        pack = d.pop("pack")

        pack_ref = d.pop("pack_ref")

        def _parse_param_schema(data: object) -> None | UpdateActionResponse200DataParamSchemaType0:
            if data is None:
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                param_schema_type_0 = UpdateActionResponse200DataParamSchemaType0.from_dict(data)



                return param_schema_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | UpdateActionResponse200DataParamSchemaType0, data)

        param_schema = _parse_param_schema(d.pop("param_schema"))


        ref = d.pop("ref")

        updated = isoparse(d.pop("updated"))




        def _parse_description(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description = _parse_description(d.pop("description", UNSET))


        _required_worker_runtimes = d.pop("required_worker_runtimes", UNSET)
        required_worker_runtimes: UpdateActionResponse200DataRequiredWorkerRuntimes | Unset
        if isinstance(_required_worker_runtimes,  Unset):
            required_worker_runtimes = UNSET
        else:
            required_worker_runtimes = UpdateActionResponse200DataRequiredWorkerRuntimes.from_dict(_required_worker_runtimes)




        def _parse_runtime(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        runtime = _parse_runtime(d.pop("runtime", UNSET))


        def _parse_runtime_ref(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        runtime_ref = _parse_runtime_ref(d.pop("runtime_ref", UNSET))


        def _parse_runtime_version_constraint(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        runtime_version_constraint = _parse_runtime_version_constraint(d.pop("runtime_version_constraint", UNSET))


        def _parse_workflow_def(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        workflow_def = _parse_workflow_def(d.pop("workflow_def", UNSET))


        update_action_response_200_data = cls(
            accesses_mcp=accesses_mcp,
            created=created,
            entrypoint=entrypoint,
            id=id,
            is_adhoc=is_adhoc,
            label=label,
            out_schema=out_schema,
            pack=pack,
            pack_ref=pack_ref,
            param_schema=param_schema,
            ref=ref,
            updated=updated,
            description=description,
            required_worker_runtimes=required_worker_runtimes,
            runtime=runtime,
            runtime_ref=runtime_ref,
            runtime_version_constraint=runtime_version_constraint,
            workflow_def=workflow_def,
        )


        update_action_response_200_data.additional_properties = d
        return update_action_response_200_data

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
