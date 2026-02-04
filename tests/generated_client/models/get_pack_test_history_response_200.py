from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, BinaryIO, TextIO, TYPE_CHECKING, Generator

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset

from typing import cast

if TYPE_CHECKING:
  from ..models.get_pack_test_history_response_200_data_item import GetPackTestHistoryResponse200DataItem
  from ..models.pagination_meta import PaginationMeta





T = TypeVar("T", bound="GetPackTestHistoryResponse200")



@_attrs_define
class GetPackTestHistoryResponse200:
    """ Paginated response wrapper

        Attributes:
            data (list[GetPackTestHistoryResponse200DataItem]): The data items
            pagination (PaginationMeta): Pagination metadata
     """

    data: list[GetPackTestHistoryResponse200DataItem]
    pagination: PaginationMeta
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.get_pack_test_history_response_200_data_item import GetPackTestHistoryResponse200DataItem
        from ..models.pagination_meta import PaginationMeta
        data = []
        for data_item_data in self.data:
            data_item = data_item_data.to_dict()
            data.append(data_item)



        pagination = self.pagination.to_dict()


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "data": data,
            "pagination": pagination,
        })

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.get_pack_test_history_response_200_data_item import GetPackTestHistoryResponse200DataItem
        from ..models.pagination_meta import PaginationMeta
        d = dict(src_dict)
        data = []
        _data = d.pop("data")
        for data_item_data in (_data):
            data_item = GetPackTestHistoryResponse200DataItem.from_dict(data_item_data)



            data.append(data_item)


        pagination = PaginationMeta.from_dict(d.pop("pagination"))




        get_pack_test_history_response_200 = cls(
            data=data,
            pagination=pagination,
        )


        get_pack_test_history_response_200.additional_properties = d
        return get_pack_test_history_response_200

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
