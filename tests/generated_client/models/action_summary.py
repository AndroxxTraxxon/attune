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
  from ..models.action_summary_required_worker_runtimes import ActionSummaryRequiredWorkerRuntimes





T = TypeVar("T", bound="ActionSummary")



@_attrs_define
class ActionSummary:
    """ Simplified action response (for list endpoints)

        Attributes:
            accesses_mcp (bool): Hint that this action may invoke the Attune MCP server and spawn child executions. Default:
                False.
            created (datetime.datetime): Creation timestamp Example: 2024-01-13T10:30:00Z.
            entrypoint (str): Entry point Example: /actions/slack/post_message.py.
            id (int): Action ID Example: 1.
            label (str): Human-readable label Example: Post Message to Slack.
            pack_ref (str): Pack reference Example: slack.
            ref (str): Unique reference identifier Example: slack.post_message.
            updated (datetime.datetime): Last update timestamp Example: 2024-01-13T10:30:00Z.
            description (None | str | Unset): Action description Example: Posts a message to a Slack channel.
            required_worker_runtimes (ActionSummaryRequiredWorkerRuntimes | Unset): Additional worker runtime requirements
                keyed by runtime name/alias. Use "*" for any available version.
            runtime (int | None | Unset): Runtime ID Example: 1.
            runtime_ref (None | str | Unset): Runtime reference (stable identifier, e.g., "core.python") Example:
                core.python.
            runtime_version_constraint (None | str | Unset): Semver version constraint for the runtime Example: >=3.12.
            workflow_def (int | None | Unset): Workflow definition ID (non-null if this action is a workflow) Example: 42.
     """

    created: datetime.datetime
    entrypoint: str
    id: int
    label: str
    pack_ref: str
    ref: str
    updated: datetime.datetime
    accesses_mcp: bool = False
    description: None | str | Unset = UNSET
    required_worker_runtimes: ActionSummaryRequiredWorkerRuntimes | Unset = UNSET
    runtime: int | None | Unset = UNSET
    runtime_ref: None | str | Unset = UNSET
    runtime_version_constraint: None | str | Unset = UNSET
    workflow_def: int | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.action_summary_required_worker_runtimes import ActionSummaryRequiredWorkerRuntimes
        accesses_mcp = self.accesses_mcp

        created = self.created.isoformat()

        entrypoint = self.entrypoint

        id = self.id

        label = self.label

        pack_ref = self.pack_ref

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
            "label": label,
            "pack_ref": pack_ref,
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
        from ..models.action_summary_required_worker_runtimes import ActionSummaryRequiredWorkerRuntimes
        d = dict(src_dict)
        accesses_mcp = d.pop("accesses_mcp")

        created = isoparse(d.pop("created"))




        entrypoint = d.pop("entrypoint")

        id = d.pop("id")

        label = d.pop("label")

        pack_ref = d.pop("pack_ref")

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
        required_worker_runtimes: ActionSummaryRequiredWorkerRuntimes | Unset
        if isinstance(_required_worker_runtimes,  Unset):
            required_worker_runtimes = UNSET
        else:
            required_worker_runtimes = ActionSummaryRequiredWorkerRuntimes.from_dict(_required_worker_runtimes)




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


        action_summary = cls(
            accesses_mcp=accesses_mcp,
            created=created,
            entrypoint=entrypoint,
            id=id,
            label=label,
            pack_ref=pack_ref,
            ref=ref,
            updated=updated,
            description=description,
            required_worker_runtimes=required_worker_runtimes,
            runtime=runtime,
            runtime_ref=runtime_ref,
            runtime_version_constraint=runtime_version_constraint,
            workflow_def=workflow_def,
        )


        action_summary.additional_properties = d
        return action_summary

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
