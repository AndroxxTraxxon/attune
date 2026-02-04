from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, BinaryIO, TextIO, TYPE_CHECKING, Generator

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

from ..types import UNSET, Unset
from typing import cast

if TYPE_CHECKING:
  from ..models.update_rule_request_action_params_type_0 import UpdateRuleRequestActionParamsType0
  from ..models.update_rule_request_conditions_type_0 import UpdateRuleRequestConditionsType0
  from ..models.update_rule_request_trigger_params_type_0 import UpdateRuleRequestTriggerParamsType0





T = TypeVar("T", bound="UpdateRuleRequest")



@_attrs_define
class UpdateRuleRequest:
    """ Request DTO for updating a rule

        Attributes:
            action_params (None | UpdateRuleRequestActionParamsType0): Parameters to pass to the action when rule is
                triggered
            conditions (None | UpdateRuleRequestConditionsType0): Conditions for rule evaluation
            trigger_params (None | UpdateRuleRequestTriggerParamsType0): Parameters for trigger configuration and event
                filtering
            description (None | str | Unset): Rule description Example: Enhanced error notification with filtering.
            enabled (bool | None | Unset): Whether the rule is enabled
            label (None | str | Unset): Human-readable label Example: Notify on Error (Updated).
     """

    action_params: None | UpdateRuleRequestActionParamsType0
    conditions: None | UpdateRuleRequestConditionsType0
    trigger_params: None | UpdateRuleRequestTriggerParamsType0
    description: None | str | Unset = UNSET
    enabled: bool | None | Unset = UNSET
    label: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.update_rule_request_action_params_type_0 import UpdateRuleRequestActionParamsType0
        from ..models.update_rule_request_trigger_params_type_0 import UpdateRuleRequestTriggerParamsType0
        from ..models.update_rule_request_conditions_type_0 import UpdateRuleRequestConditionsType0
        action_params: dict[str, Any] | None
        if isinstance(self.action_params, UpdateRuleRequestActionParamsType0):
            action_params = self.action_params.to_dict()
        else:
            action_params = self.action_params

        conditions: dict[str, Any] | None
        if isinstance(self.conditions, UpdateRuleRequestConditionsType0):
            conditions = self.conditions.to_dict()
        else:
            conditions = self.conditions

        trigger_params: dict[str, Any] | None
        if isinstance(self.trigger_params, UpdateRuleRequestTriggerParamsType0):
            trigger_params = self.trigger_params.to_dict()
        else:
            trigger_params = self.trigger_params

        description: None | str | Unset
        if isinstance(self.description, Unset):
            description = UNSET
        else:
            description = self.description

        enabled: bool | None | Unset
        if isinstance(self.enabled, Unset):
            enabled = UNSET
        else:
            enabled = self.enabled

        label: None | str | Unset
        if isinstance(self.label, Unset):
            label = UNSET
        else:
            label = self.label


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "action_params": action_params,
            "conditions": conditions,
            "trigger_params": trigger_params,
        })
        if description is not UNSET:
            field_dict["description"] = description
        if enabled is not UNSET:
            field_dict["enabled"] = enabled
        if label is not UNSET:
            field_dict["label"] = label

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.update_rule_request_action_params_type_0 import UpdateRuleRequestActionParamsType0
        from ..models.update_rule_request_conditions_type_0 import UpdateRuleRequestConditionsType0
        from ..models.update_rule_request_trigger_params_type_0 import UpdateRuleRequestTriggerParamsType0
        d = dict(src_dict)
        def _parse_action_params(data: object) -> None | UpdateRuleRequestActionParamsType0:
            if data is None:
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                action_params_type_0 = UpdateRuleRequestActionParamsType0.from_dict(data)



                return action_params_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | UpdateRuleRequestActionParamsType0, data)

        action_params = _parse_action_params(d.pop("action_params"))


        def _parse_conditions(data: object) -> None | UpdateRuleRequestConditionsType0:
            if data is None:
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                conditions_type_0 = UpdateRuleRequestConditionsType0.from_dict(data)



                return conditions_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | UpdateRuleRequestConditionsType0, data)

        conditions = _parse_conditions(d.pop("conditions"))


        def _parse_trigger_params(data: object) -> None | UpdateRuleRequestTriggerParamsType0:
            if data is None:
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                trigger_params_type_0 = UpdateRuleRequestTriggerParamsType0.from_dict(data)



                return trigger_params_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | UpdateRuleRequestTriggerParamsType0, data)

        trigger_params = _parse_trigger_params(d.pop("trigger_params"))


        def _parse_description(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description = _parse_description(d.pop("description", UNSET))


        def _parse_enabled(data: object) -> bool | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(bool | None | Unset, data)

        enabled = _parse_enabled(d.pop("enabled", UNSET))


        def _parse_label(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        label = _parse_label(d.pop("label", UNSET))


        update_rule_request = cls(
            action_params=action_params,
            conditions=conditions,
            trigger_params=trigger_params,
            description=description,
            enabled=enabled,
            label=label,
        )


        update_rule_request.additional_properties = d
        return update_rule_request

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
