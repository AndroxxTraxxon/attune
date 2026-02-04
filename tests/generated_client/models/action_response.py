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
  from ..models.action_response_out_schema_type_0 import ActionResponseOutSchemaType0
  from ..models.action_response_param_schema_type_0 import ActionResponseParamSchemaType0





T = TypeVar("T", bound="ActionResponse")



@_attrs_define
class ActionResponse:
    """ Response DTO for action information

        Attributes:
            created (datetime.datetime): Creation timestamp Example: 2024-01-13T10:30:00Z.
            description (str): Action description Example: Posts a message to a Slack channel.
            entrypoint (str): Entry point Example: /actions/slack/post_message.py.
            id (int): Action ID Example: 1.
            label (str): Human-readable label Example: Post Message to Slack.
            out_schema (ActionResponseOutSchemaType0 | None): Output schema
            pack (int): Pack ID Example: 1.
            pack_ref (str): Pack reference Example: slack.
            param_schema (ActionResponseParamSchemaType0 | None): Parameter schema
            ref (str): Unique reference identifier Example: slack.post_message.
            updated (datetime.datetime): Last update timestamp Example: 2024-01-13T10:30:00Z.
            runtime (int | None | Unset): Runtime ID Example: 1.
     """

    created: datetime.datetime
    description: str
    entrypoint: str
    id: int
    label: str
    out_schema: ActionResponseOutSchemaType0 | None
    pack: int
    pack_ref: str
    param_schema: ActionResponseParamSchemaType0 | None
    ref: str
    updated: datetime.datetime
    runtime: int | None | Unset = UNSET
    additional_properties: dict[str, Any] = _attrs_field(init=False, factory=dict)





    def to_dict(self) -> dict[str, Any]:
        from ..models.action_response_param_schema_type_0 import ActionResponseParamSchemaType0
        from ..models.action_response_out_schema_type_0 import ActionResponseOutSchemaType0
        created = self.created.isoformat()

        description = self.description

        entrypoint = self.entrypoint

        id = self.id

        label = self.label

        out_schema: dict[str, Any] | None
        if isinstance(self.out_schema, ActionResponseOutSchemaType0):
            out_schema = self.out_schema.to_dict()
        else:
            out_schema = self.out_schema

        pack = self.pack

        pack_ref = self.pack_ref

        param_schema: dict[str, Any] | None
        if isinstance(self.param_schema, ActionResponseParamSchemaType0):
            param_schema = self.param_schema.to_dict()
        else:
            param_schema = self.param_schema

        ref = self.ref

        updated = self.updated.isoformat()

        runtime: int | None | Unset
        if isinstance(self.runtime, Unset):
            runtime = UNSET
        else:
            runtime = self.runtime


        field_dict: dict[str, Any] = {}
        field_dict.update(self.additional_properties)
        field_dict.update({
            "created": created,
            "description": description,
            "entrypoint": entrypoint,
            "id": id,
            "label": label,
            "out_schema": out_schema,
            "pack": pack,
            "pack_ref": pack_ref,
            "param_schema": param_schema,
            "ref": ref,
            "updated": updated,
        })
        if runtime is not UNSET:
            field_dict["runtime"] = runtime

        return field_dict



    @classmethod
    def from_dict(cls: type[T], src_dict: Mapping[str, Any]) -> T:
        from ..models.action_response_out_schema_type_0 import ActionResponseOutSchemaType0
        from ..models.action_response_param_schema_type_0 import ActionResponseParamSchemaType0
        d = dict(src_dict)
        created = isoparse(d.pop("created"))




        description = d.pop("description")

        entrypoint = d.pop("entrypoint")

        id = d.pop("id")

        label = d.pop("label")

        def _parse_out_schema(data: object) -> ActionResponseOutSchemaType0 | None:
            if data is None:
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                out_schema_type_0 = ActionResponseOutSchemaType0.from_dict(data)



                return out_schema_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(ActionResponseOutSchemaType0 | None, data)

        out_schema = _parse_out_schema(d.pop("out_schema"))


        pack = d.pop("pack")

        pack_ref = d.pop("pack_ref")

        def _parse_param_schema(data: object) -> ActionResponseParamSchemaType0 | None:
            if data is None:
                return data
            try:
                if not isinstance(data, dict):
                    raise TypeError()
                param_schema_type_0 = ActionResponseParamSchemaType0.from_dict(data)



                return param_schema_type_0
            except (TypeError, ValueError, AttributeError, KeyError):
                pass
            return cast(ActionResponseParamSchemaType0 | None, data)

        param_schema = _parse_param_schema(d.pop("param_schema"))


        ref = d.pop("ref")

        updated = isoparse(d.pop("updated"))




        def _parse_runtime(data: object) -> int | None | Unset:
            if data is None:
                return data
            if isinstance(data, Unset):
                return data
            return cast(int | None | Unset, data)

        runtime = _parse_runtime(d.pop("runtime", UNSET))


        action_response = cls(
            created=created,
            description=description,
            entrypoint=entrypoint,
            id=id,
            label=label,
            out_schema=out_schema,
            pack=pack,
            pack_ref=pack_ref,
            param_schema=param_schema,
            ref=ref,
            updated=updated,
            runtime=runtime,
        )


        action_response.additional_properties = d
        return action_response

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
