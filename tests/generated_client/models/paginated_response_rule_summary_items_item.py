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
  from ..models.paginated_response_rule_summary_items_item_action_params import PaginatedResponseRuleSummaryItemsItemActionParams
  from ..models.paginated_response_rule_summary_items_item_trigger_params import PaginatedResponseRuleSummaryItemsItemTriggerParams





T = TypeVar("T", bound="PaginatedResponseRuleSummaryItemsItem")



@_attrs_define
class PaginatedResponseRuleSummaryItemsItem:
    """ Simplified rule response (for list endpoints)

        Attributes:
            action_params (PaginatedResponseRuleSummaryItemsItemActionParams): Parameters to pass to the action when rule is
                triggered
            action_ref (str): Action reference Example: slack.post_message.
            created (datetime.datetime): Creation timestamp Example: 2024-01-13T10:30:00Z.
            enabled (bool): Whether the rule is enabled Example: True.
            id (int): Rule ID Example: 1.
            label (str): Human-readable label Example: Notify on Error.
            pack_ref (str): Pack reference Example: slack.
            ref (str): Unique reference identifier Example: slack.notify_on_error.
            trigger_params (PaginatedResponseRuleSummaryItemsItemTriggerParams): Parameters for trigger configuration and
                event filtering
            trigger_ref (str): Trigger reference Example: system.error_event.
            updated (datetime.datetime): Last update timestamp Example: 2024-01-13T10:30:00Z.
            description (None | str | Unset): Rule description Example: Send Slack notification when an error occurs.
     """

    action_params: PaginatedResponseRuleSummaryItemsItemActionParams
    action_ref: str
    created: datetime.datetime
    enabled: bool
    id: int
    label: str
    pack_ref: str
    ref: str
    trigger_params: PaginatedResponseRuleSummaryItemsItemTriggerParams
    trigger_ref: str
    updated: datetime.datetime
    description: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.paginated_response_rule_summary_items_item_action_params import PaginatedResponseRuleSummaryItemsItemActionParams
        from ..models.paginated_response_rule_summary_items_item_trigger_params import PaginatedResponseRuleSummaryItemsItemTriggerParams
        action_params = self.action_params.to_dict()

        action_ref = self.action_ref

        created = self.created.isoformat()

        enabled = self.enabled

        id = self.id

        label = self.label

        pack_ref = self.pack_ref

        ref = self.ref

        trigger_params = self.trigger_params.to_dict()

        trigger_ref = self.trigger_ref

        updated = self.updated.isoformat()

        description: None | str | Unset
        if isinstance(self.description, Unset):
            description = UNSET
        else:
            description = self.description


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "action_params": action_params,
            "action_ref": action_ref,
            "created": created,
            "enabled": enabled,
            "id": id,
            "label": label,
            "pack_ref": pack_ref,
            "ref": ref,
            "trigger_params": trigger_params,
            "trigger_ref": trigger_ref,
            "updated": updated,
        })
        if description is not UNSET:
            field_dict["description"] = description

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.paginated_response_rule_summary_items_item_action_params import PaginatedResponseRuleSummaryItemsItemActionParams
        from ..models.paginated_response_rule_summary_items_item_trigger_params import PaginatedResponseRuleSummaryItemsItemTriggerParams
        d = dict(src_dict)
        action_params = PaginatedResponseRuleSummaryItemsItemActionParams.from_dict(d.pop("action_params"))




        action_ref = d.pop("action_ref")

        created = isoparse(d.pop("created"))




        enabled = d.pop("enabled")

        id = d.pop("id")

        label = d.pop("label")

        pack_ref = d.pop("pack_ref")

        ref = d.pop("ref")

        trigger_params = PaginatedResponseRuleSummaryItemsItemTriggerParams.from_dict(d.pop("trigger_params"))




        trigger_ref = d.pop("trigger_ref")

        updated = isoparse(d.pop("updated"))




        def _parse_description(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description = _parse_description(d.pop("description", UNSET))


        paginated_response_rule_summary_items_item = cls(
            action_params=action_params,
            action_ref=action_ref,
            created=created,
            enabled=enabled,
            id=id,
            label=label,
            pack_ref=pack_ref,
            ref=ref,
            trigger_params=trigger_params,
            trigger_ref=trigger_ref,
            updated=updated,
            description=description,
        )


        paginated_response_rule_summary_items_item.additional_properties = d
        return paginated_response_rule_summary_items_item

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
