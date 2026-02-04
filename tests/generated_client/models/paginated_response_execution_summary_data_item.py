from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, BinaryIO, TextIO, TYPE_CHECKING, Generator

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

from ..models.execution_status import ExecutionStatus
from ..types import UNSET, Unset
from dateutil.parser import isoparse
from typing import cast
import datetime






T = TypeVar("T", bound="PaginatedResponseExecutionSummaryDataItem")



@_attrs_define
class PaginatedResponseExecutionSummaryDataItem:
    """ Simplified execution response (for list endpoints)

        Attributes:
            action_ref (str): Action reference Example: slack.post_message.
            created (datetime.datetime): Creation timestamp Example: 2024-01-13T10:30:00Z.
            id (int): Execution ID Example: 1.
            status (ExecutionStatus):
            updated (datetime.datetime): Last update timestamp Example: 2024-01-13T10:35:00Z.
            enforcement (int | None | Unset): Enforcement ID Example: 1.
            parent (int | None | Unset): Parent execution ID Example: 1.
     """

    action_ref: str
    created: datetime.datetime
    id: int
    status: ExecutionStatus
    updated: datetime.datetime
    enforcement: int | None | Unset = UNSET
    parent: int | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        action_ref = self.action_ref

        created = self.created.isoformat()

        id = self.id

        status = self.status.value

        updated = self.updated.isoformat()

        enforcement: int | None | Unset
        if isinstance(self.enforcement, Unset):
            enforcement = UNSET
        else:
            enforcement = self.enforcement

        parent: int | None | Unset
        if isinstance(self.parent, Unset):
            parent = UNSET
        else:
            parent = self.parent


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "action_ref": action_ref,
            "created": created,
            "id": id,
            "status": status,
            "updated": updated,
        })
        if enforcement is not UNSET:
            field_dict["enforcement"] = enforcement
        if parent is not UNSET:
            field_dict["parent"] = parent

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        action_ref = d.pop("action_ref")

        created = isoparse(d.pop("created"))




        id = d.pop("id")

        status = ExecutionStatus(d.pop("status"))




        updated = isoparse(d.pop("updated"))




        def _parse_enforcement(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        enforcement = _parse_enforcement(d.pop("enforcement", UNSET))


        def _parse_parent(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        parent = _parse_parent(d.pop("parent", UNSET))


        paginated_response_execution_summary_data_item = cls(
            action_ref=action_ref,
            created=created,
            id=id,
            status=status,
            updated=updated,
            enforcement=enforcement,
            parent=parent,
        )


        paginated_response_execution_summary_data_item.additional_properties = d
        return paginated_response_execution_summary_data_item

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
