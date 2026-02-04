from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, BinaryIO, TextIO, TYPE_CHECKING, Generator

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

from ..types import UNSET, Unset
from typing import cast

if TYPE_CHECKING:
  from ..models.update_sensor_request_param_schema_type_0 import UpdateSensorRequestParamSchemaType0





T = TypeVar("T", bound="UpdateSensorRequest")



@_attrs_define
class UpdateSensorRequest:
    """ Request DTO for updating a sensor

        Attributes:
            param_schema (None | UpdateSensorRequestParamSchemaType0): Parameter schema
            description (None | str | Unset): Sensor description Example: Enhanced CPU monitoring with alerts.
            enabled (bool | None | Unset): Whether the sensor is enabled
            entrypoint (None | str | Unset): Entry point for sensor execution Example:
                /sensors/monitoring/cpu_monitor_v2.py.
            label (None | str | Unset): Human-readable label Example: CPU Monitoring Sensor (Updated).
     """

    param_schema: None | UpdateSensorRequestParamSchemaType0
    description: None | str | Unset = UNSET
    enabled: bool | None | Unset = UNSET
    entrypoint: None | str | Unset = UNSET
    label: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.update_sensor_request_param_schema_type_0 import UpdateSensorRequestParamSchemaType0
        param_schema: dict[str, Any] | None
        if isinstance(self.param_schema, UpdateSensorRequestParamSchemaType0):
            param_schema = self.param_schema.to_dict()
        else:
            param_schema = self.param_schema

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


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "param_schema": param_schema,
        })
        if description is not UNSET:
            field_dict["description"] = description
        if enabled is not UNSET:
            field_dict["enabled"] = enabled
        if entrypoint is not UNSET:
            field_dict["entrypoint"] = entrypoint
        if label is not UNSET:
            field_dict["label"] = label

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.update_sensor_request_param_schema_type_0 import UpdateSensorRequestParamSchemaType0
        d = dict(src_dict)
        def _parse_param_schema(data: object) -> None | UpdateSensorRequestParamSchemaType0:
            if data is None:
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                param_schema_type_0 = UpdateSensorRequestParamSchemaType0.from_dict(data)



                return param_schema_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | UpdateSensorRequestParamSchemaType0, data)

        param_schema = _parse_param_schema(d.pop("param_schema"))


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


        update_sensor_request = cls(
            param_schema=param_schema,
            description=description,
            enabled=enabled,
            entrypoint=entrypoint,
            label=label,
        )


        update_sensor_request.additional_properties = d
        return update_sensor_request

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
