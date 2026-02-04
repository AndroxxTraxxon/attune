from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, BinaryIO, TextIO, TYPE_CHECKING, Generator

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

from dateutil.parser import isoparse
from typing import cast
import datetime






T = TypeVar("T", bound="PaginatedResponseRuleSummaryDataItem")



@_attrs_define
class PaginatedResponseRuleSummaryDataItem:
    """ Simplified rule response (for list endpoints)

        Attributes:
            action_ref (str): Action reference Example: slack.post_message.
            created (datetime.datetime): Creation timestamp Example: 2024-01-13T10:30:00Z.
            description (str): Rule description Example: Send Slack notification when an error occurs.
            enabled (bool): Whether the rule is enabled Example: True.
            id (int): Rule ID Example: 1.
            label (str): Human-readable label Example: Notify on Error.
            pack_ref (str): Pack reference Example: slack.
            ref (str): Unique reference identifier Example: slack.notify_on_error.
            trigger_ref (str): Trigger reference Example: system.error_event.
            updated (datetime.datetime): Last update timestamp Example: 2024-01-13T10:30:00Z.
     """

    action_ref: str
    created: datetime.datetime
    description: str
    enabled: bool
    id: int
    label: str
    pack_ref: str
    ref: str
    trigger_ref: str
    updated: datetime.datetime
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        action_ref = self.action_ref

        created = self.created.isoformat()

        description = self.description

        enabled = self.enabled

        id = self.id

        label = self.label

        pack_ref = self.pack_ref

        ref = self.ref

        trigger_ref = self.trigger_ref

        updated = self.updated.isoformat()


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "action_ref": action_ref,
            "created": created,
            "description": description,
            "enabled": enabled,
            "id": id,
            "label": label,
            "pack_ref": pack_ref,
            "ref": ref,
            "trigger_ref": trigger_ref,
            "updated": updated,
        })

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        action_ref = d.pop("action_ref")

        created = isoparse(d.pop("created"))




        description = d.pop("description")

        enabled = d.pop("enabled")

        id = d.pop("id")

        label = d.pop("label")

        pack_ref = d.pop("pack_ref")

        ref = d.pop("ref")

        trigger_ref = d.pop("trigger_ref")

        updated = isoparse(d.pop("updated"))




        paginated_response_rule_summary_data_item = cls(
            action_ref=action_ref,
            created=created,
            description=description,
            enabled=enabled,
            id=id,
            label=label,
            pack_ref=pack_ref,
            ref=ref,
            trigger_ref=trigger_ref,
            updated=updated,
        )


        paginated_response_rule_summary_data_item.additional_properties = d
        return paginated_response_rule_summary_data_item

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
