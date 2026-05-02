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
  from ..models.sensor_response_param_schema_type_0 import SensorResponseParamSchemaType0





T = TypeVar("T", bound="SensorResponse")



@_attrs_define
class SensorResponse:
    """ Response DTO for sensor information

        Attributes:
            created (datetime.datetime): Creation timestamp Example: 2024-01-13T10:30:00Z.
            enabled (bool): Whether the sensor is enabled Example: True.
            entrypoint (str): Entry point Example: /sensors/monitoring/cpu_monitor.py.
            id (int): Sensor ID Example: 1.
            label (str): Human-readable label Example: CPU Monitoring Sensor.
            param_schema (None | SensorResponseParamSchemaType0): Parameter schema (StackStorm-style with inline
                required/secret)
            ref (str): Unique reference identifier Example: monitoring.cpu_sensor.
            runtime (int): Runtime ID Example: 1.
            runtime_ref (str): Runtime reference Example: python3.
            trigger (int): Trigger ID Example: 1.
            trigger_ref (str): Trigger reference Example: monitoring.cpu_threshold.
            updated (datetime.datetime): Last update timestamp Example: 2024-01-13T10:30:00Z.
            description (None | str | Unset): Sensor description Example: Monitors CPU usage and generates events.
            pack (int | None | Unset): Pack ID (optional) Example: 1.
            pack_ref (None | str | Unset): Pack reference (optional) Example: monitoring.
     """

    created: datetime.datetime
    enabled: bool
    entrypoint: str
    id: int
    label: str
    param_schema: None | SensorResponseParamSchemaType0
    ref: str
    runtime: int
    runtime_ref: str
    trigger: int
    trigger_ref: str
    updated: datetime.datetime
    description: None | str | Unset = UNSET
    pack: int | None | Unset = UNSET
    pack_ref: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.sensor_response_param_schema_type_0 import SensorResponseParamSchemaType0
        created = self.created.isoformat()

        enabled = self.enabled

        entrypoint = self.entrypoint

        id = self.id

        label = self.label

        param_schema: dict[str, Any] | None
        if isinstance(self.param_schema, SensorResponseParamSchemaType0):
            param_schema = self.param_schema.to_dict()
        else:
            param_schema = self.param_schema

        ref = self.ref

        runtime = self.runtime

        runtime_ref = self.runtime_ref

        trigger = self.trigger

        trigger_ref = self.trigger_ref

        updated = self.updated.isoformat()

        description: None | str | Unset
        if isinstance(self.description, Unset):
            description = UNSET
        else:
            description = self.description

        pack: int | None | Unset
        if isinstance(self.pack, Unset):
            pack = UNSET
        else:
            pack = self.pack

        pack_ref: None | str | Unset
        if isinstance(self.pack_ref, Unset):
            pack_ref = UNSET
        else:
            pack_ref = self.pack_ref


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "created": created,
            "enabled": enabled,
            "entrypoint": entrypoint,
            "id": id,
            "label": label,
            "param_schema": param_schema,
            "ref": ref,
            "runtime": runtime,
            "runtime_ref": runtime_ref,
            "trigger": trigger,
            "trigger_ref": trigger_ref,
            "updated": updated,
        })
        if description is not UNSET:
            field_dict["description"] = description
        if pack is not UNSET:
            field_dict["pack"] = pack
        if pack_ref is not UNSET:
            field_dict["pack_ref"] = pack_ref

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.sensor_response_param_schema_type_0 import SensorResponseParamSchemaType0
        d = dict(src_dict)
        created = isoparse(d.pop("created"))




        enabled = d.pop("enabled")

        entrypoint = d.pop("entrypoint")

        id = d.pop("id")

        label = d.pop("label")

        def _parse_param_schema(data: object) -> None | SensorResponseParamSchemaType0:
            if data is None:
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                param_schema_type_0 = SensorResponseParamSchemaType0.from_dict(data)



                return param_schema_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(None | SensorResponseParamSchemaType0, data)

        param_schema = _parse_param_schema(d.pop("param_schema"))


        ref = d.pop("ref")

        runtime = d.pop("runtime")

        runtime_ref = d.pop("runtime_ref")

        trigger = d.pop("trigger")

        trigger_ref = d.pop("trigger_ref")

        updated = isoparse(d.pop("updated"))




        def _parse_description(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        description = _parse_description(d.pop("description", UNSET))


        def _parse_pack(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        pack = _parse_pack(d.pop("pack", UNSET))


        def _parse_pack_ref(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        pack_ref = _parse_pack_ref(d.pop("pack_ref", UNSET))


        sensor_response = cls(
            created=created,
            enabled=enabled,
            entrypoint=entrypoint,
            id=id,
            label=label,
            param_schema=param_schema,
            ref=ref,
            runtime=runtime,
            runtime_ref=runtime_ref,
            trigger=trigger,
            trigger_ref=trigger_ref,
            updated=updated,
            description=description,
            pack=pack,
            pack_ref=pack_ref,
        )


        sensor_response.additional_properties = d
        return sensor_response

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
