from __future__ import annotations

from collections.abc import Mapping
from typing import Any, TypeVar, BinaryIO, TextIO, TYPE_CHECKING, Generator

from attrs import define as _attrs_define
from attrs import field as _attrs_field

from ..types import UNSET, Unset







T = TypeVar("T", bound="PaginationMeta")



@_attrs_define
class PaginationMeta:
    """ Pagination metadata

        Attributes:
            page (int): Current page number (1-based) Example: 1.
            page_size (int): Number of items per page Example: 50.
            total_items (int): Total number of items Example: 150.
            total_pages (int): Total number of pages Example: 3.
     """

    page: int
    page_size: int
    total_items: int
    total_pages: int
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        page = self.page

        page_size = self.page_size

        total_items = self.total_items

        total_pages = self.total_pages


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "page": page,
            "page_size": page_size,
            "total_items": total_items,
            "total_pages": total_pages,
        })

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        d = dict(src_dict)
        page = d.pop("page")

        page_size = d.pop("page_size")

        total_items = d.pop("total_items")

        total_pages = d.pop("total_pages")

        pagination_meta = cls(
            page=page,
            page_size=page_size,
            total_items=total_items,
            total_pages=total_pages,
        )


        pagination_meta.additional_properties = d
        return pagination_meta

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
