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






T = TypeVar("T", bound="SensorSummary")



@_attrs_define
class SensorSummary:
    """ Simplified sensor response (for list endpoints)

        Attributes:
            created (datetime.datetime): Creation timestamp Example: 2024-01-13T10:30:00Z.
            description (str): Sensor description Example: Monitors CPU usage and generates events.
            enabled (bool): Whether the sensor is enabled Example: True.
            id (int): Sensor ID Example: 1.
            label (str): Human-readable label Example: CPU Monitoring Sensor.
            ref (str): Unique reference identifier Example: monitoring.cpu_sensor.
            trigger_ref (str): Trigger reference Example: monitoring.cpu_threshold.
            updated (datetime.datetime): Last update timestamp Example: 2024-01-13T10:30:00Z.
            pack_ref (None | str | Unset): Pack reference (optional) Example: monitoring.
     """

    created: datetime.datetime
    description: str
    enabled: bool
    id: int
    label: str
    ref: str
    trigger_ref: str
    updated: datetime.datetime
    pack_ref: None | str | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        created = self.created.isoformat()

        description = self.description

        enabled = self.enabled

        id = self.id

        label = self.label

        ref = self.ref

        trigger_ref = self.trigger_ref

        updated = self.updated.isoformat()

        pack_ref: None | str | Unset
        if isinstance(self.pack_ref, Unset):
            pack_ref = UNSET
        else:
            pack_ref = self.pack_ref


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "created": created,
            "description": description,
            "enabled": enabled,
            "id": id,
            "label": label,
            "ref": ref,
            "trigger_ref": trigger_ref,
            "updated": updated,
        })
        if pack_ref is not UNSET:
            field_dict["pack_ref"] = pack_ref

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        created = isoparse(d.pop("created"))




        description = d.pop("description")

        enabled = d.pop("enabled")

        id = d.pop("id")

        label = d.pop("label")

        ref = d.pop("ref")

        trigger_ref = d.pop("trigger_ref")

        updated = isoparse(d.pop("updated"))




        def _parse_pack_ref(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        pack_ref = _parse_pack_ref(d.pop("pack_ref", UNSET))


        sensor_summary = cls(
            created=created,
            description=description,
            enabled=enabled,
            id=id,
            label=label,
            ref=ref,
            trigger_ref=trigger_ref,
            updated=updated,
            pack_ref=pack_ref,
        )


        sensor_summary.additional_properties = d
        return sensor_summary

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
