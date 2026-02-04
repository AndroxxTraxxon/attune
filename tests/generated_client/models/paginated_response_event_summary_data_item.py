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






T = TypeVar("T", bound="PaginatedResponseEventSummaryDataItem")



@_attrs_define
class PaginatedResponseEventSummaryDataItem:
    """ Summary event response for list views

        Attributes:
            created (datetime.datetime): Creation timestamp Example: 2024-01-13T10:30:00Z.
            has_payload (bool): Whether event has payload data Example: True.
            id (int):
            trigger_ref (str): Trigger reference Example: core.webhook.
            source (int | None | Unset):
            source_ref (None | str | Unset): Source reference Example: monitoring.webhook_sensor.
            trigger (int | None | Unset):
     """

    created: datetime.datetime
    has_payload: bool
    id: int
    trigger_ref: str
    source: int | None | Unset = UNSET
    source_ref: None | str | Unset = UNSET
    trigger: int | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        created = self.created.isoformat()

        has_payload = self.has_payload

        id = self.id

        trigger_ref = self.trigger_ref

        source: int | None | Unset
        if isinstance(self.source, Unset):
            source = UNSET
        else:
            source = self.source

        source_ref: None | str | Unset
        if isinstance(self.source_ref, Unset):
            source_ref = UNSET
        else:
            source_ref = self.source_ref

        trigger: int | None | Unset
        if isinstance(self.trigger, Unset):
            trigger = UNSET
        else:
            trigger = self.trigger


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "created": created,
            "has_payload": has_payload,
            "id": id,
            "trigger_ref": trigger_ref,
        })
        if source is not UNSET:
            field_dict["source"] = source
        if source_ref is not UNSET:
            field_dict["source_ref"] = source_ref
        if trigger is not UNSET:
            field_dict["trigger"] = trigger

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        created = isoparse(d.pop("created"))




        has_payload = d.pop("has_payload")

        id = d.pop("id")

        trigger_ref = d.pop("trigger_ref")

        def _parse_source(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        source = _parse_source(d.pop("source", UNSET))


        def _parse_source_ref(data: object) -> None | str | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(None | str | Unset, data)

        source_ref = _parse_source_ref(d.pop("source_ref", UNSET))


        def _parse_trigger(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        trigger = _parse_trigger(d.pop("trigger", UNSET))


        paginated_response_event_summary_data_item = cls(
            created=created,
            has_payload=has_payload,
            id=id,
            trigger_ref=trigger_ref,
            source=source,
            source_ref=source_ref,
            trigger=trigger,
        )


        paginated_response_event_summary_data_item.additional_properties = d
        return paginated_response_event_summary_data_item

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
