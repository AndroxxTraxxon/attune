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






T = TypeVar("T", bound="ActionSummary")



@_attrs_define
class ActionSummary:
    """ Simplified action response (for list endpoints)

        Attributes:
            created (datetime.datetime): Creation timestamp Example: 2024-01-13T10:30:00Z.
            description (str): Action description Example: Posts a message to a Slack channel.
            entrypoint (str): Entry point Example: /actions/slack/post_message.py.
            id (int): Action ID Example: 1.
            label (str): Human-readable label Example: Post Message to Slack.
            pack_ref (str): Pack reference Example: slack.
            ref (str): Unique reference identifier Example: slack.post_message.
            updated (datetime.datetime): Last update timestamp Example: 2024-01-13T10:30:00Z.
            runtime (int | None | Unset): Runtime ID Example: 1.
     """

    created: datetime.datetime
    description: str
    entrypoint: str
    id: int
    label: str
    pack_ref: str
    ref: str
    updated: datetime.datetime
    runtime: int | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        created = self.created.isoformat()

        description = self.description

        entrypoint = self.entrypoint

        id = self.id

        label = self.label

        pack_ref = self.pack_ref

        ref = self.ref

        updated = self.updated.isoformat()

        runtime: int | None | Unset
        if isinstance(self.runtime, Unset):
            runtime = UNSET
        else:
            runtime = self.runtime


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "created": created,
            "description": description,
            "entrypoint": entrypoint,
            "id": id,
            "label": label,
            "pack_ref": pack_ref,
            "ref": ref,
            "updated": updated,
        })
        if runtime is not UNSET:
            field_dict["runtime"] = runtime

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        created = isoparse(d.pop("created"))




        description = d.pop("description")

        entrypoint = d.pop("entrypoint")

        id = d.pop("id")

        label = d.pop("label")

        pack_ref = d.pop("pack_ref")

        ref = d.pop("ref")

        updated = isoparse(d.pop("updated"))




        def _parse_runtime(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        runtime = _parse_runtime(d.pop("runtime", UNSET))


        action_summary = cls(
            created=created,
            description=description,
            entrypoint=entrypoint,
            id=id,
            label=label,
            pack_ref=pack_ref,
            ref=ref,
            updated=updated,
            runtime=runtime,
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
