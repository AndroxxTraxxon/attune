from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, BinaryIO, TextIO, TYPE_CHECKING, Generator

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

from dateutil.parser import isoparse
from typing import cast
import datetime

if TYPE_CHECKING:
  from ..models.api_response_rule_response_data_action_params import ApiResponseRuleResponseDataActionParams
  from ..models.api_response_rule_response_data_conditions import ApiResponseRuleResponseDataConditions
  from ..models.api_response_rule_response_data_trigger_params import ApiResponseRuleResponseDataTriggerParams





T = TypeVar("T", bound="ApiResponseRuleResponseData")



@_attrs_define
class ApiResponseRuleResponseData:
    """ Response DTO for rule information

        Attributes:
            action (int): Action ID Example: 1.
            action_params (ApiResponseRuleResponseDataActionParams): Parameters to pass to the action when rule is triggered
            action_ref (str): Action reference Example: slack.post_message.
            conditions (ApiResponseRuleResponseDataConditions): Conditions for rule evaluation
            created (datetime.datetime): Creation timestamp Example: 2024-01-13T10:30:00Z.
            description (str): Rule description Example: Send Slack notification when an error occurs.
            enabled (bool): Whether the rule is enabled Example: True.
            id (int): Rule ID Example: 1.
            label (str): Human-readable label Example: Notify on Error.
            pack (int): Pack ID Example: 1.
            pack_ref (str): Pack reference Example: slack.
            ref (str): Unique reference identifier Example: slack.notify_on_error.
            trigger (int): Trigger ID Example: 1.
            trigger_params (ApiResponseRuleResponseDataTriggerParams): Parameters for trigger configuration and event
                filtering
            trigger_ref (str): Trigger reference Example: system.error_event.
            updated (datetime.datetime): Last update timestamp Example: 2024-01-13T10:30:00Z.
     """

    action: int
    action_params: ApiResponseRuleResponseDataActionParams
    action_ref: str
    conditions: ApiResponseRuleResponseDataConditions
    created: datetime.datetime
    description: str
    enabled: bool
    id: int
    label: str
    pack: int
    pack_ref: str
    ref: str
    trigger: int
    trigger_params: ApiResponseRuleResponseDataTriggerParams
    trigger_ref: str
    updated: datetime.datetime
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.api_response_rule_response_data_trigger_params import ApiResponseRuleResponseDataTriggerParams
        from ..models.api_response_rule_response_data_action_params import ApiResponseRuleResponseDataActionParams
        from ..models.api_response_rule_response_data_conditions import ApiResponseRuleResponseDataConditions
        action = self.action

        action_params = self.action_params.to_dict()

        action_ref = self.action_ref

        conditions = self.conditions.to_dict()

        created = self.created.isoformat()

        description = self.description

        enabled = self.enabled

        id = self.id

        label = self.label

        pack = self.pack

        pack_ref = self.pack_ref

        ref = self.ref

        trigger = self.trigger

        trigger_params = self.trigger_params.to_dict()

        trigger_ref = self.trigger_ref

        updated = self.updated.isoformat()


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "action": action,
            "action_params": action_params,
            "action_ref": action_ref,
            "conditions": conditions,
            "created": created,
            "description": description,
            "enabled": enabled,
            "id": id,
            "label": label,
            "pack": pack,
            "pack_ref": pack_ref,
            "ref": ref,
            "trigger": trigger,
            "trigger_params": trigger_params,
            "trigger_ref": trigger_ref,
            "updated": updated,
        })

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.api_response_rule_response_data_action_params import ApiResponseRuleResponseDataActionParams
        from ..models.api_response_rule_response_data_conditions import ApiResponseRuleResponseDataConditions
        from ..models.api_response_rule_response_data_trigger_params import ApiResponseRuleResponseDataTriggerParams
        d = dict(src_dict)
        action = d.pop("action")

        action_params = ApiResponseRuleResponseDataActionParams.from_dict(d.pop("action_params"))




        action_ref = d.pop("action_ref")

        conditions = ApiResponseRuleResponseDataConditions.from_dict(d.pop("conditions"))




        created = isoparse(d.pop("created"))




        description = d.pop("description")

        enabled = d.pop("enabled")

        id = d.pop("id")

        label = d.pop("label")

        pack = d.pop("pack")

        pack_ref = d.pop("pack_ref")

        ref = d.pop("ref")

        trigger = d.pop("trigger")

        trigger_params = ApiResponseRuleResponseDataTriggerParams.from_dict(d.pop("trigger_params"))




        trigger_ref = d.pop("trigger_ref")

        updated = isoparse(d.pop("updated"))




        api_response_rule_response_data = cls(
            action=action,
            action_params=action_params,
            action_ref=action_ref,
            conditions=conditions,
            created=created,
            description=description,
            enabled=enabled,
            id=id,
            label=label,
            pack=pack,
            pack_ref=pack_ref,
            ref=ref,
            trigger=trigger,
            trigger_params=trigger_params,
            trigger_ref=trigger_ref,
            updated=updated,
        )


        api_response_rule_response_data.additional_properties = d
        return api_response_rule_response_data

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
