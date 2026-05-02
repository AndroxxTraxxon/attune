from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, BinaryIO, TextIO, TYPE_CHECKING, Generator

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

from ..types import UNSET, Unset
from typing import cast

if TYPE_CHECKING:
  from ..models.create_sensor_request_config_type_0 import CreateSensorRequestConfigType0
  from ..models.create_sensor_request_param_schema_type_0 import CreateSensorRequestParamSchemaType0





T = TypeVar("T", bound="CreateSensorRequest")



@_attrs_define
class CreateSensorRequest:
    """ Request DTO for creating a new sensor

        Attributes:
            entrypoint (str): Entry point for sensor execution (e.g., path to script, function name) Example:
                /sensors/monitoring/cpu_monitor.py.
            label (str): Human-readable label Example: CPU Monitoring Sensor.
            pack_ref (str): Pack reference this sensor belongs to Example: monitoring.
            ref (str): Unique reference identifier (e.g., "mypack.cpu_monitor") Example: monitoring.cpu_sensor.
            runtime_ref (str): Runtime reference for this sensor Example: python3.
            trigger_ref (str): Trigger reference this sensor monitors for Example: monitoring.cpu_threshold.
            config (CreateSensorRequestConfigType0 | None | Unset): Configuration values for this sensor instance (conforms
                to param_schema)
            description (None | str | Unset): Sensor description Example: Monitors CPU usage and generates events.
            enabled (bool | Unset): Whether the sensor is enabled Example: True.
            param_schema (CreateSensorRequestParamSchemaType0 | None | Unset): Parameter schema (flat format) for sensor
                configuration
     """

    entrypoint: str
    label: str
    pack_ref: str
    ref: str
    runtime_ref: str
    trigger_ref: str
    config: CreateSensorRequestConfigType0 | None | Unset = UNSET
    description: None | str | Unset = UNSET
    enabled: bool | Unset = UNSET
    param_schema: CreateSensorRequestParamSchemaType0 | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.create_sensor_request_config_type_0 import CreateSensorRequestConfigType0
        from ..models.create_sensor_request_param_schema_type_0 import CreateSensorRequestParamSchemaType0
        entrypoint = self.entrypoint

        label = self.label

        pack_ref = self.pack_ref

        ref = self.ref

        runtime_ref = self.runtime_ref

        trigger_ref = self.trigger_ref

        config: dict[str, Any] | None | Unset
        if isinstance(self.config, Unset):
            config = UNSET
        elif isinstance(self.config, CreateSensorRequestConfigType0):
            config = self.config.to_dict()
        else:
            config = self.config

        description: None | str | Unset
        if isinstance(self.description, Unset):
            description = UNSET
        else:
            description = self.description

        enabled = self.enabled

        param_schema: dict[str, Any] | None | Unset
        if isinstance(self.param_schema, Unset):
            param_schema = UNSET
        elif isinstance(self.param_schema, CreateSensorRequestParamSchemaType0):
            param_schema = self.param_schema.to_dict()
        else:
            param_schema = self.param_schema


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "entrypoint": entrypoint,
            "label": label,
            "pack_ref": pack_ref,
            "ref": ref,
            "runtime_ref": runtime_ref,
            "trigger_ref": trigger_ref,
        })
        if config is not UNSET:
            field_dict["config"] = config
        if description is not UNSET:
            field_dict["description"] = description
        if enabled is not UNSET:
            field_dict["enabled"] = enabled
        if param_schema is not UNSET:
            field_dict["param_schema"] = param_schema

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.create_sensor_request_config_type_0 import CreateSensorRequestConfigType0
        from ..models.create_sensor_request_param_schema_type_0 import CreateSensorRequestParamSchemaType0
        d = dict(src_dict)
        entrypoint = d.pop("entrypoint")

        label = d.pop("label")

        pack_ref = d.pop("pack_ref")

        ref = d.pop("ref")

        runtime_ref = d.pop("runtime_ref")

        trigger_ref = d.pop("trigger_ref")

        def _parse_config(data: object) -> CreateSensorRequestConfigType0 | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                config_type_0 = CreateSensorRequestConfigType0.from_dict(data)



                return config_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(CreateSensorRequestConfigType0 | None | Unset, data)

        config = _parse_config(d.pop("config", UNSET))


        def _parse_description(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description = _parse_description(d.pop("description", UNSET))


        enabled = d.pop("enabled", UNSET)

        def _parse_param_schema(data: object) -> CreateSensorRequestParamSchemaType0 | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                param_schema_type_0 = CreateSensorRequestParamSchemaType0.from_dict(data)



                return param_schema_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(CreateSensorRequestParamSchemaType0 | None | Unset, data)

        param_schema = _parse_param_schema(d.pop("param_schema", UNSET))


        create_sensor_request = cls(
            entrypoint=entrypoint,
            label=label,
            pack_ref=pack_ref,
            ref=ref,
            runtime_ref=runtime_ref,
            trigger_ref=trigger_ref,
            config=config,
            description=description,
            enabled=enabled,
            param_schema=param_schema,
        )


        create_sensor_request.additional_properties = d
        return create_sensor_request

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
