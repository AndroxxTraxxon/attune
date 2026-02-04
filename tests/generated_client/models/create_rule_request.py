from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, BinaryIO, TextIO, TYPE_CHECKING, Generator

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

from ..types import UNSET, Unset
from typing import cast

if TYPE_CHECKING:
  from ..models.create_rule_request_action_params import CreateRuleRequestActionParams
  from ..models.create_rule_request_conditions import CreateRuleRequestConditions
  from ..models.create_rule_request_trigger_params import CreateRuleRequestTriggerParams





T = TypeVar("T", bound="CreateRuleRequest")



@_attrs_define
class CreateRuleRequest:
    """ Request DTO for creating a new rule

        Attributes:
            action_ref (str): Action reference to execute when rule matches Example: slack.post_message.
            description (str): Rule description Example: Send Slack notification when an error occurs.
            label (str): Human-readable label Example: Notify on Error.
            pack_ref (str): Pack reference this rule belongs to Example: slack.
            ref (str): Unique reference identifier (e.g., "mypack.notify_on_error") Example: slack.notify_on_error.
            trigger_ref (str): Trigger reference that activates this rule Example: system.error_event.
            action_params (CreateRuleRequestActionParams | Unset): Parameters to pass to the action when rule is triggered
            conditions (CreateRuleRequestConditions | Unset): Conditions for rule evaluation (JSON Logic or custom format)
            enabled (bool | Unset): Whether the rule is enabled Example: True.
            trigger_params (CreateRuleRequestTriggerParams | Unset): Parameters for trigger configuration and event
                filtering
     """

    action_ref: str
    description: str
    label: str
    pack_ref: str
    ref: str
    trigger_ref: str
    action_params: CreateRuleRequestActionParams | Unset = UNSET
    conditions: CreateRuleRequestConditions | Unset = UNSET
    enabled: bool | Unset = UNSET
    trigger_params: CreateRuleRequestTriggerParams | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.create_rule_request_action_params import CreateRuleRequestActionParams
        from ..models.create_rule_request_trigger_params import CreateRuleRequestTriggerParams
        from ..models.create_rule_request_conditions import CreateRuleRequestConditions
        action_ref = self.action_ref

        description = self.description

        label = self.label

        pack_ref = self.pack_ref

        ref = self.ref

        trigger_ref = self.trigger_ref

        action_params: dict[str, Any] | Unset = UNSET
        if not isinstance(self.action_params, Unset):
            action_params = self.action_params.to_dict()

        conditions: dict[str, Any] | Unset = UNSET
        if not isinstance(self.conditions, Unset):
            conditions = self.conditions.to_dict()

        enabled = self.enabled

        trigger_params: dict[str, Any] | Unset = UNSET
        if not isinstance(self.trigger_params, Unset):
            trigger_params = self.trigger_params.to_dict()


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "action_ref": action_ref,
            "description": description,
            "label": label,
            "pack_ref": pack_ref,
            "ref": ref,
            "trigger_ref": trigger_ref,
        })
        if action_params is not UNSET:
            field_dict["action_params"] = action_params
        if conditions is not UNSET:
            field_dict["conditions"] = conditions
        if enabled is not UNSET:
            field_dict["enabled"] = enabled
        if trigger_params is not UNSET:
            field_dict["trigger_params"] = trigger_params

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.create_rule_request_action_params import CreateRuleRequestActionParams
        from ..models.create_rule_request_conditions import CreateRuleRequestConditions
        from ..models.create_rule_request_trigger_params import CreateRuleRequestTriggerParams
        d = dict(src_dict)
        action_ref = d.pop("action_ref")

        description = d.pop("description")

        label = d.pop("label")

        pack_ref = d.pop("pack_ref")

        ref = d.pop("ref")

        trigger_ref = d.pop("trigger_ref")

        _action_params = d.pop("action_params", UNSET)
        action_params: CreateRuleRequestActionParams | Unset
        if isinstance(_action_params,  Unset):
            action_params = UNSET
        else:
            action_params = CreateRuleRequestActionParams.from_dict(_action_params)




        _conditions = d.pop("conditions", UNSET)
        conditions: CreateRuleRequestConditions | Unset
        if isinstance(_conditions,  Unset):
            conditions = UNSET
        else:
            conditions = CreateRuleRequestConditions.from_dict(_conditions)




        enabled = d.pop("enabled", UNSET)

        _trigger_params = d.pop("trigger_params", UNSET)
        trigger_params: CreateRuleRequestTriggerParams | Unset
        if isinstance(_trigger_params,  Unset):
            trigger_params = UNSET
        else:
            trigger_params = CreateRuleRequestTriggerParams.from_dict(_trigger_params)




        create_rule_request = cls(
            action_ref=action_ref,
            description=description,
            label=label,
            pack_ref=pack_ref,
            ref=ref,
            trigger_ref=trigger_ref,
            action_params=action_params,
            conditions=conditions,
            enabled=enabled,
            trigger_params=trigger_params,
        )


        create_rule_request.additional_properties = d
        return create_rule_request

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
